# Twitter Stream

![Build Status](https://github.com/tesaguri/twitter-stream-rs/workflows/CI/badge.svg)
[![Current Version](https://img.shields.io/crates/v/twitter-stream.svg)](https://crates.io/crates/twitter-stream)
[![Documentation](https://docs.rs/twitter-stream/badge.svg)](https://docs.rs/twitter-stream/)

A Rust library for listening on Twitter Streaming API.

## Requirements

This library requires Rust 1.39.0 or later.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
tokio = { version = "0.2", features = ["macros"] }
twitter-stream = "=0.10.0-alpha.5"
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
        .try_flatten_stream()
        .try_for_each(|json| {
            println!("{}", json);
            future::ok(())
        })
        .await
        .unwrap();
}
```

## License

This project is licensed under the MIT license ([LICENSE](LICENSE) or https://opensource.org/licenses/MIT) unless explicitly stated otherwise.
