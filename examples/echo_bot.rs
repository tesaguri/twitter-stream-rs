// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org/>

// This line shouldn't be necessary in a real project.
extern crate hyper_pkg as hyper;

use std::fs::File;
use std::path::PathBuf;

use futures::prelude::*;
use http::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::de;
use serde::Deserialize;
use twitter_stream::Token;

#[derive(Deserialize)]
#[serde(untagged)]
enum StreamMessage {
    Tweet(Tweet),
    Other(de::IgnoredAny),
}

struct Tweet {
    entities: Option<Entities>,
    id: u64,
    text: String,
    user: User,
    is_retweet: bool,
}

#[derive(Deserialize)]
struct Entities {
    user_mentions: Vec<UserMention>,
}

#[derive(Deserialize)]
struct UserMention {
    id: u64,
}

#[derive(Deserialize)]
struct User {
    id: u64,
    screen_name: String,
}

/// Represents a GET statuses/update request.
#[derive(oauth::Request)]
struct StatusUpdate<'a> {
    status: &'a str,
    in_reply_to_status_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(remote = "Token")]
struct TokenDef {
    // The `getter` attribute is required to make the `Deserialize` impl use the `From` conversion,
    // even if we are not deriving `Serialize` here.
    #[serde(getter = "__")]
    consumer_key: String,
    consumer_secret: String,
    access_key: String,
    access_secret: String,
}

impl From<TokenDef> for Token {
    fn from(def: TokenDef) -> Token {
        Token::from_parts(
            def.consumer_key,
            def.consumer_secret,
            def.access_key,
            def.access_secret,
        )
    }
}

type HttpsConnector = hyper_tls::HttpsConnector<hyper::client::HttpConnector>;

#[tokio::main]
async fn main() {
    let mut credential_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    credential_path.push("credential.json");

    let conn = HttpsConnector::new();
    let mut client = hyper::Client::builder().build::<_, hyper::Body>(conn);

    let credential = File::open(credential_path).unwrap();
    let token =
        TokenDef::deserialize(&mut serde_json::Deserializer::from_reader(credential)).unwrap();

    // Information of the authenticated user:
    let user = verify_credentials(&token, &client).await;

    let mut stream = twitter_stream::Builder::new(token.as_ref())
        .track(format!("@{}", user.screen_name))
        .listen_with_client(&mut client)
        .try_flatten_stream();

    while let Some(json) = stream.next().await {
        if let Ok(StreamMessage::Tweet(tweet)) = serde_json::from_str(&json.unwrap()) {
            let mentions_me = |entities: Entities| {
                entities
                    .user_mentions
                    .iter()
                    .any(|mention| mention.id == user.id)
            };
            if !tweet.is_retweet
                && tweet.user.id != user.id
                && tweet.entities.map_or(false, mentions_me)
            {
                // Send a reply
                let tweeting = StatusUpdate {
                    status: &format!("@{} {}", tweet.user.screen_name, tweet.text),
                    in_reply_to_status_id: Some(tweet.id),
                }
                .send(&token, &client);
                tokio::spawn(tweeting);
            }
        }
    }
}

// The custom `Deserialize` impl is needed to handle Tweets with >140 characters.
// Once the Labs API is available, this impl should be unnecessary.
impl<'de> Deserialize<'de> for Tweet {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Tweet {
            entities: Option<Entities>,
            id: u64,
            // Add the following attribute if you want to deserialize REST API responses too.
            // #[serde(alias = "full_text")]
            text: String,
            user: User,
            extended_tweet: Option<ExtendedTweet>,
            retweeted_status: Option<de::IgnoredAny>,
        }

        #[derive(Deserialize)]
        struct ExtendedTweet {
            full_text: String,
            entities: Option<Entities>,
        }

        Tweet::deserialize(d).map(|t| {
            let (text, entities) = t
                .extended_tweet
                .map_or((t.text, t.entities), |e| (e.full_text, e.entities));
            Self {
                entities,
                id: t.id,
                text,
                user: t.user,
                is_retweet: t.retweeted_status.is_some(),
            }
        })
    }
}

impl<'a> StatusUpdate<'a> {
    /// Performs the GET statuses/update request.
    fn send(
        &self,
        token: &Token,
        client: &hyper::Client<HttpsConnector>,
    ) -> impl Future<Output = ()> {
        const URI: &str = "https://api.twitter.com/1.1/statuses/update.json";

        let authorization = oauth::post(URI, self, token, oauth::HmacSha1);
        let form = oauth::to_form_urlencoded(self);
        let req = http::Request::post(http::Uri::from_static(URI))
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            )
            .header(AUTHORIZATION, authorization)
            .body(form.into())
            .unwrap();

        parse_response::<de::IgnoredAny>(client.request(req)).map(|_| ())
    }
}

/// Performs a GET account/verify_credentials request.
async fn verify_credentials(token: &Token, client: &hyper::Client<HttpsConnector>) -> User {
    const URI: &str = "https://api.twitter.com/1.1/account/verify_credentials.json";

    let authorization = oauth::get(URI, &(), token, oauth::HmacSha1);
    let req = http::Request::get(http::Uri::from_static(URI))
        .header(AUTHORIZATION, authorization)
        .body(Default::default())
        .unwrap();

    parse_response(client.request(req)).await
}

fn parse_response<T: de::DeserializeOwned>(
    res: hyper::client::ResponseFuture,
) -> impl Future<Output = T> {
    res.then(|res| {
        let res = res.unwrap();
        if !res.status().is_success() {
            panic!("HTTP error: {}", res.status());
        }
        hyper::body::to_bytes(res).map(|body| serde_json::from_slice(&body.unwrap()).unwrap())
    })
}
