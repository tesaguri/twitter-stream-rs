# Twitter Stream

[![Build Status](https://travis-ci.org/tesaguri/twitter-stream-rs.svg?branch=master)](https://travis-ci.org/tesaguri/twitter-stream-rs/)
[![Current Version](https://img.shields.io/crates/v/twitter-stream.svg)](https://crates.io/crates/twitter-stream)
[![Documentation](https://docs.rs/twitter-stream/badge.svg)](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Requirements

This library requires Rust 1.26 or later.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.10.0-alpha.1"
```

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust
use futures::prelude::*;
use twitter_stream::Token;

#[tokio::main]
async fn main() {
    let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

    twitter_stream::Builder::filter(token)
        .track(Some("@Twitter"))
        .listen()
        .unwrap()
        .try_flatten_stream()
        .try_for_each(|json| {
            println!("{}", json);
            future::ok(())
        })
        .await
        .unwrap();
}
```
