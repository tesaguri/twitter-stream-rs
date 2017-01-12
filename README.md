# Twitter Stream

[![Build Status](https://travis-ci.org/d12i/twitter-stream.svg?branch=master)](https://travis-ci.org/d12i/twitter-stream)
[![Current Version](http://meritbadge.herokuapp.com/twitter-stream)](https://crates.io/crates/twitter-stream)

[Documentation](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.1"
```

and this to your crate root:

```rust
extern crate twitter_stream;
```

Here is a basic example that prints each message from the User Stream:

```rust
extern crate futures;
extern crate twitter_stream;

use futures::{Future, Stream}:
use twitter_stream::TwitterStreamBuidler;

fn main() {
    let consumer = "...";
    let consumer_secret = "...";
    let token = "...";
    let token_secret = "...";

    let stream = TwitterStreamBuilder::user(consumer, consumer_secret, token, token_secret)
        .follow(&[12, 783214])
        .track("twitter,facebook")
        .login().unwrap();

    stream
        .for_each(|msg| println!("{}", msg))
        .wait().unwrap();
}
```

The crate also defines a parser for messages on the Stream API.
Rewrite the above example like the following to print each text of the Tweets from the Stream:

```rust
use twitter_stream::StreamMessage;

stream
    .then(|msg| msg.parse::<StreamMessage>())
    .for_each(|msg| {
        if let StreamMessage::Tweet(tweet) = msg {
            println!("{}", tweet.text);
        }
    })
    .wait().unwrap();
```
