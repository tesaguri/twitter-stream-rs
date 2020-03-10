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

use std::fs::File;
use std::path::PathBuf;

use futures::prelude::*;
use serde::de;
use serde::Deserialize;
use tokio01::runtime::current_thread::block_on_all as block_on_all01;
use twitter_stream::TwitterStream;

#[derive(Deserialize)]
#[serde(untagged)]
enum StreamMessage {
    Tweet(Tweet),
    Other(de::IgnoredAny),
}

struct Tweet {
    created_at: String,
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

#[derive(Deserialize)]
#[serde(remote = "twitter_stream::Token")]
struct TokenDef {
    // The `getter` attribute is required to make the `Deserialize` impl use the `From` conversion,
    // even if we are not deriving `Serialize` here.
    #[serde(getter = "__")]
    consumer_key: String,
    consumer_secret: String,
    access_key: String,
    access_secret: String,
}

impl From<TokenDef> for twitter_stream::Token {
    fn from(def: TokenDef) -> twitter_stream::Token {
        twitter_stream::Token::new(
            def.consumer_key,
            def.consumer_secret,
            def.access_key,
            def.access_secret,
        )
    }
}

#[tokio::main]
async fn main() {
    const TRACK: &str = "@NAME_OF_YOUR_ACCOUNT";

    let mut credential_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    credential_path.pop();
    credential_path.push("credential.json");

    let credential = File::open(credential_path).unwrap();
    let token = TokenDef::deserialize(&mut json::Deserializer::from_reader(credential)).unwrap();

    let stream = TwitterStream::track(TRACK, token.as_ref()).try_flatten_stream();

    let twitter_stream::Token { client, token } = token;
    let token = egg_mode::Token::Access {
        consumer: egg_mode::KeyPair::new(client.identifier, client.secret),
        access: egg_mode::KeyPair::new(token.identifier, token.secret),
    };

    // Information of the authenticated user:
    let user = block_on_all01(egg_mode::verify_tokens(&token)).unwrap();

    stream
        .try_for_each(move |json| {
            if let Ok(StreamMessage::Tweet(tweet)) = json::from_str(&json) {
                if !tweet.is_retweet
                    && tweet.user.id != user.id
                    && tweet.entities.map_or(false, |e| {
                        e.user_mentions.iter().any(|mention| mention.id == user.id)
                    })
                {
                    println!(
                        "On {}, @{} tweeted: {:?}",
                        tweet.created_at, tweet.user.screen_name, tweet.text
                    );

                    let response = format!("@{} {}", tweet.user.screen_name, tweet.text);
                    let response = egg_mode::tweet::DraftTweet::new(response).in_reply_to(tweet.id);
                    block_on_all01(response.send(&token)).unwrap();
                }
            }

            future::ok(())
        })
        .await
        .unwrap();
}

// The custom `Deserialize` impl is needed to handle Tweets with >140 characters.
// Once the Labs API is available, this impl should be unnecessary.
impl<'de> Deserialize<'de> for Tweet {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Prototype {
            created_at: String,
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

        Prototype::deserialize(d).map(|p| {
            let (text, entities) = p
                .extended_tweet
                .map_or((p.text, p.entities), |e| (e.full_text, e.entities));
            Ok(Tweet {
                created_at: p.created_at,
                entities: entities,
                id: p.id,
                text,
                user: p.user,
                is_retweet: p.retweeted_status.is_some(),
            })
        })?
    }
}
