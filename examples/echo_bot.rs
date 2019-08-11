#![feature(async_await)]

extern crate tweetust_pkg as tweetust;

use std::fs::File;
use std::path::PathBuf;

use futures::prelude::*;
use serde::de;
use serde::Deserialize;
use twitter_stream::{rt, Credentials, Token};

#[derive(Deserialize)]
#[serde(untagged)]
enum StreamMessage {
    Tweet(Tweet),
    Other(de::IgnoredAny),
}

#[derive(Deserialize)]
struct Tweet {
    created_at: String,
    entities: Entities,
    id: i64,
    text: String,
    user: User,
}

#[derive(Deserialize)]
struct Entities {
    user_mentions: Vec<UserMention>,
}

#[derive(Deserialize)]
struct UserMention {
    id: i64,
}

#[derive(Deserialize)]
struct User {
    id: i64,
    screen_name: String,
}

#[derive(Deserialize)]
#[serde(remote = "Token")]
struct TokenDef {
    #[serde(flatten)]
    #[serde(with = "Consumer")]
    client: Credentials,
    #[serde(flatten)]
    #[serde(with = "Access")]
    token: Credentials,
}

#[derive(Deserialize)]
#[serde(remote = "Credentials")]
struct Consumer {
    #[serde(rename = "consumer_key")]
    identifier: String,
    #[serde(rename = "consumer_secret")]
    secret: String,
}

#[derive(Deserialize)]
#[serde(remote = "Credentials")]
struct Access {
    #[serde(rename = "access_key")]
    identifier: String,
    #[serde(rename = "access_secret")]
    secret: String,
}

#[rt::main]
async fn main() {
    const TRACK: &str = "@NAME_OF_YOUR_ACCOUNT";

    let mut credential_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    credential_path.pop();
    credential_path.push("credential.json");

    let credential = File::open(credential_path).unwrap();
    let token = TokenDef::deserialize(&mut json::Deserializer::from_reader(credential)).unwrap();

    let stream = twitter_stream::Builder::filter(token.as_ref())
        .track(Some(TRACK))
        .listen()
        .unwrap()
        .try_flatten_stream();
    let rest = tweetust::TwitterClient::new(
        token,
        tweetust::DefaultHttpHandler::with_https_connector().unwrap(),
    );

    // Information of the authenticated user:
    let user = rest
        .account()
        .verify_credentials()
        .execute()
        .unwrap()
        .object;

    stream
        .try_for_each(move |json| {
            if let Ok(StreamMessage::Tweet(tweet)) = json::from_str(&json) {
                if tweet.user.id != user.id
                    && tweet
                        .entities
                        .user_mentions
                        .iter()
                        .any(|mention| mention.id == user.id)
                {
                    println!(
                        "On {}, @{} tweeted: {:?}",
                        tweet.created_at, tweet.user.screen_name, tweet.text
                    );

                    let response = format!("@{} {}", tweet.user.screen_name, tweet.text);
                    rest.statuses()
                        .update(response)
                        .in_reply_to_status_id(tweet.id)
                        .execute()
                        .unwrap();
                }
            }

            future::ok(())
        })
        .await
        .unwrap();
}
