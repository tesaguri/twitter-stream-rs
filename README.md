# Twitter Stream

[![Build Status](https://travis-ci.org/dmizuk/twitter-stream-rs.svg?branch=master)](https://travis-ci.org/dmizuk/twitter-stream-rs/)
[![Current Version](https://img.shields.io/crates/v/twitter-stream.svg)](https://crates.io/crates/twitter-stream)
[![Documentation](https://docs.rs/twitter-stream/badge.svg)](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Requirements

This library requires Rust 1.21 or later.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.5"
```

and this to your crate root:

```rust
extern crate twitter_stream;
```

Here is a basic example that prints public mentions @Twitter in JSON format:

```rust
extern crate futures;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use twitter_stream::{Token, TwitterStreamBuilder};

fn main() {
    let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

    let mut core = Core::new().unwrap();

    let future = TwitterStreamBuilder::filter(&token).handle(&core.handle())
        .replies(true)
        .track(Some("@Twitter"))
        .listen()
        .flatten_stream()
        .for_each(|json| {
            println!("{}", json);
            Ok(())
        });

    core.run(future).unwrap();
}
```
