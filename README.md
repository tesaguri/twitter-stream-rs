# Twitter Stream

[![Current Version](https://img.shields.io/crates/v/twitter-stream.svg)](https://crates.io/crates/twitter-stream)
[![Documentation](https://docs.rs/twitter-stream/badge.svg)](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Requirements

This library requires Rust 1.15 or later.

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

Here is a basic example that prints each Tweet's text from User Stream:

```rust
extern crate futures;
extern crate twitter_stream;
use futures::{Future, Stream};
use twitter_stream::{StreamMessage, TwitterStream};

fn main() {
    let consumer_key = "...";
    let consumer_secret = "...";
    let token = "...";
    let token_secret = "...";

    let stream = TwitterStream::user(consumer_key, consumer_secret, token, token_secret).unwrap();

    stream
        .filter_map(|msg| {
            if let StreamMessage::Tweet(tweet) = msg {
                Some(tweet.text)
            } else {
                None
            }
        })
        .for_each(|tweet| {
            println!("{}", tweet);
            Ok(())
        })
        .wait().unwrap();
}
```
