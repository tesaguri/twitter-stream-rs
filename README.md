# Twitter Stream

[![Build Status](https://travis-ci.org/d12i/twitter-stream-rs.svg?branch=master)](https://travis-ci.org/d12i/twitter-stream-rs/)
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
extern crate twitter_stream;
use futures::{Future, Stream};
use twitter_stream::{StreamMessage, Token, TwitterStream};

fn main() {
    let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

    let stream = TwitterStream::user(&token).unwrap();

    stream
        .filter_map(|msg| {
            if let StreamMessage::Tweet(tweet) = msg {
                println!("{}", tweet.text);
            }
            Ok(())
        })
        .wait().unwrap();
}
```
