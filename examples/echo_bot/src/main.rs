extern crate futures;
extern crate serde_json as json;
extern crate tweetust;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use std::fs::File;
use std::path::PathBuf;
use tokio_core::reactor::Core;
use twitter_stream::{Error, Token, TwitterStream};
use twitter_stream::message::StreamMessage;

fn main() {
    // `credential.json` must have the following form:
    // {"consumer_key": "...", "consumer_secret": "...", "access_key": "...", "access_secret": "..."}

    let mut credential_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    credential_path.pop();
    credential_path.push("credential.json");

    let credential = File::open(credential_path).unwrap();
    let token: Token = json::from_reader(credential).unwrap();

    let mut core = Core::new().unwrap();

    let stream = TwitterStream::user(&token, &core.handle()).flatten_stream();
    let rest = tweetust::TwitterClient::new(token, tweetust::DefaultHttpHandler::with_https_connector().unwrap());

    // Information of the authenticated user:
    let user = rest.account().verify_credentials().execute().unwrap().object;

    let bot = stream.for_each(|json| {
        if let Ok(StreamMessage::Tweet(tweet)) = StreamMessage::from_str(&json) {
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
