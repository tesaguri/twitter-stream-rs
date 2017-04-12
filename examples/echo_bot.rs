#![cfg_attr(not(feature = "tweetust"), allow(unused_imports))]

extern crate futures;
extern crate serde_json as json;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use std::fs::File;
use tokio_core::reactor::Core;
use twitter_stream::{Error, StreamMessage, Token, TwitterStream};

#[cfg(feature = "tweetust")]
fn main() {
    extern crate tweetust;

    // `credential.json` must have the following form:
    // {"consumer_key": "...", "consumer_secret": "...", "access_key": "...", "access_secret": "..."}

    let credential = File::open("examples/credential.json").unwrap();
    let token: Token = json::from_reader(credential).unwrap();

    let mut core = Core::new().unwrap();

    let future_stream = TwitterStream::user(&token, &core.handle());
    let rest = tweetust::TwitterClient::new(token, tweetust::DefaultHttpHandler::with_https_connector().unwrap());

    // Information of the authenticated user:
    let user = rest.account().verify_credentials().execute().unwrap().object;

    let bot = future_stream.flatten_stream().for_each(|message| {
        if let StreamMessage::Tweet(tweet) = message {
            if tweet.user.id != user.id as u64
                && tweet.entities.user_mentions.iter().any(|mention| mention.id == user.id as u64)
            {
                println!("On {}, @{} tweeted: {:?}", tweet.created_at, tweet.user.screen_name, tweet.text);

                let response = format!("@{} {}", tweet.user.screen_name, tweet.text);
                rest.statuses()
                    .update(response)
                    .in_reply_to_status_id(tweet.id as _)
                    .execute()
                    .map_err(Error::custom)?;
            }
        }

        Ok(())
    });

    core.run(bot).unwrap();
}

#[cfg(not(feature = "tweetust"))]
fn main() {
    println!("This example needs `tweetust` feature");
}
