// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org/>

// This line shouldn't be necessary in a real project.
extern crate hyper_pkg as hyper;

use futures::prelude::*;
use tower_http::decompression::Decompression;
use twitter_stream::Token;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = Token::from_parts(
        "consumer_key",
        "consumer_secret",
        "access_key",
        "access_secret",
    );

    let client = hyper::Client::builder().build::<_, hyper::Body>(hyper_tls::HttpsConnector::new());
    // Make the HTTP client request the `gzip` encoding and transparently decode the response body.
    let client = Decompression::new(client);

    twitter_stream::Builder::new(token)
        .track("@Twitter")
        .listen_with_client(client)
        .await?
        .try_for_each(|json| {
            println!("{}", json);
            future::ok(())
        })
        .await?;

    Ok(())
}
