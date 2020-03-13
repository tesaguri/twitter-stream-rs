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

use std::error::Error;
use std::io;
use std::time::Duration;

use futures::prelude::*;
use twitter_stream::Token;

const TIMEOUT: Duration = Duration::from_secs(90);

#[tokio::main]
async fn main() {
    let token = Token::new(
        "consumer_key",
        "consumer_secret",
        "access_key",
        "access_secret",
    );

    let mut conn = hyper::client::HttpConnector::new();
    conn.set_connect_timeout(Some(TIMEOUT));
    let tls = native_tls::TlsConnector::new().unwrap();
    let conn = hyper_tls::HttpsConnector::from((conn, tls.into()));
    let mut conn = hyper_timeout::TimeoutConnector::new(conn);
    conn.set_read_timeout(Some(TIMEOUT));

    let client = hyper::Client::builder().build::<_, hyper::Body>(conn);

    let result = twitter_stream::Builder::new(token)
        .track("@Twitter")
        .listen_with_client(client)
        .try_flatten_stream()
        .try_for_each(|json| {
            println!("{}", json);
            future::ok(())
        })
        .await;

    match result {
        Ok(()) => {}
        Err(twitter_stream::Error::Service(e)) => {
            if let Some(e) = e.source() {
                if let Some(e) = e.downcast_ref::<io::Error>() {
                    if e.kind() == io::ErrorKind::TimedOut {
                        eprintln!("timed out");
                        return;
                    }
                }
            }
            eprintln!("error: {:?}", e);
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
        }
    }
}
