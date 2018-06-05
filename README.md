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
twitter-stream = "0.6"
```

and this to your crate root:

```rust
extern crate twitter_stream;
```

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust
extern crate twitter_stream;

use twitter_stream::{Token, TwitterStreamBuilder};
use twitter_stream::rt::{self, Future, Stream};

fn main() {
    let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

    let future = TwitterStreamBuilder::filter(&token)
        .replies(true)
        .track(Some("@Twitter"))
        .listen()
        .flatten_stream()
        .for_each(|json| {
            println!("{}", json);
            Ok(())
        })
        .map_err(|e| println!("error: {}", e));

    rt::run(future);
}
```
