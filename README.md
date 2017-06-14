# Twitter Stream

[![Build Status](https://travis-ci.org/dmizuk/twitter-stream-rs.svg?branch=master)](https://travis-ci.org/dmizuk/twitter-stream-rs/)
[![Current Version](https://img.shields.io/crates/v/twitter-stream.svg)](https://crates.io/crates/twitter-stream)
[![Documentation](https://docs.rs/twitter-stream/badge.svg)](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Requirements

This library requires Rust 1.15 or later.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.2"
```

and this to your crate root:

```rust
extern crate twitter_stream;
```

Here is a basic example that prints each Tweet's text from User Stream:

```rust
extern crate futures;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use twitter_stream::{Token, TwitterStream};
use twitter_stream::message::StreamMessage;

fn main() {
    let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

    let mut core = Core::new().unwrap();

    let future = TwitterStream::user(&token, &core.handle()).flatten_stream().for_each(|json| {
        if let Ok(StreamMessage::Tweet(tweet)) = twitter_stream::message::parse(&json) {
            println!("{}", tweet.text);
        }
        Ok(())
    });

    core.run(future).unwrap();
}
```
