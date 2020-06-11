//! A [`Builder`] type for [`TwitterStream`](crate::TwitterStream).
//!
//! The Streaming API has two different endpoints: [`POST statuses/filter`] and
//! [`GET statuses/sample`]. `Builder` automatically determines which endpoint to use based on the
//! specified parameters. Specifically, when any of [`follow`][Builder::follow],
//! [`track`][Builder::track] and [`locations`][Builder::locations] parameters is specified,
//! `filter` will be used, and when none is specified, `sample` will be used.
//!
//! [`POST statuses/filter`]: https://developer.twitter.com/en/docs/tweets/filter-realtime/api-reference/post-statuses-filter
//! [`GET statuses/sample`]: https://developer.twitter.com/en/docs/tweets/filter-realtime/api-reference/post-statuses-filter
//!
//! `filter` yields public Tweets that match the filter predicates specified by the parameters,
//! and `sample` yields "a small random sample" of all public Tweets.
//!
//! ## Example
//!
//! ```rust,no_run
//! use futures::prelude::*;
//! use twitter_stream::{builder::BoundingBox, Token};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
//!
//! const TOKYO: &'static [BoundingBox] = &[BoundingBox::new(139.56, 35.53, 139.92, 35.82)];
//!
//! // Prints geolocated English Tweets associated with Tokyo (the 23 special wards).
//! twitter_stream::Builder::new(token)
//!     .locations(TOKYO)
//!     .language("en")
//!     .listen()
//!     .try_flatten_stream()
//!     .try_for_each(|json| {
//!         println!("{}", json);
//!         future::ok(())
//!     })
//!     .await
//!     .unwrap();
//! # }
//! ```

mod bounding_box;

pub use http::Method as RequestMethod;
pub use http::Uri;

pub use bounding_box::BoundingBox;

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Formatter};

use http::header::{HeaderValue, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use http::Request;
use slice_of_array::SliceFlatExt;

use crate::service::HttpService;
use crate::token::Token;
use crate::util::fmt_join;
use crate::FutureTwitterStream;

/// A builder for [`TwitterStream`](crate::TwitterStream).
///
/// See the [`builder`][crate::builder] module documentation for details.
#[derive(Clone, Debug)]
pub struct Builder<'a, T = Token> {
    token: T,
    endpoint: Option<(RequestMethod, Uri)>,
    parameters: Parameters<'a>,
}

/// Parameters to the Streaming API.
#[derive(Clone, Debug, Default, oauth::Authorize)]
struct Parameters<'a> {
    #[oauth1(skip_if = "not")]
    stall_warnings: bool,
    filter_level: Option<FilterLevel>,
    #[oauth1(skip_if = "str::is_empty")]
    language: Cow<'a, str>,
    #[oauth1(encoded, fmt = "fmt_follow", skip_if = "<[_]>::is_empty")]
    follow: Cow<'a, [u64]>,
    #[oauth1(skip_if = "str::is_empty")]
    track: Cow<'a, str>,
    #[oauth1(encoded, fmt = "fmt_locations", skip_if = "<[_]>::is_empty")]
    #[allow(clippy::type_complexity)]
    locations: Cow<'a, [BoundingBox]>,
    #[oauth1(encoded)]
    count: Option<i32>,
}

str_enum! {
    /// Represents the [`filter_level`] parameter in API requests.
    ///
    /// [`filter_level`]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#filter-level
    #[derive(Clone, Debug, PartialEq, Hash, Eq)]
    pub enum FilterLevel {
        /// `"none"`
        None = "none",
        /// `"low"`
        Low = "low",
        /// `"medium"`
        Medium = "medium",
    }
}

const FILTER: &str = "https://stream.twitter.com/1.1/statuses/filter.json";
const SAMPLE: &str = "https://stream.twitter.com/1.1/statuses/sample.json";

impl<'a, C, A> Builder<'a, Token<C, A>>
where
    C: Borrow<str>,
    A: Borrow<str>,
{
    /// Creates a builder.
    pub fn new(token: Token<C, A>) -> Self {
        Builder {
            token,
            endpoint: None,
            parameters: Parameters::default(),
        }
    }

    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    #[deprecated(since = "0.10.0", note = "Use `Builder::new` instead")]
    pub fn filter(token: Token<C, A>) -> Self {
        let mut ret = Self::new(token);
        ret.endpoint((RequestMethod::POST, Uri::from_static(FILTER)));
        ret
    }

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    #[deprecated(since = "0.10.0", note = "Use `Builder::new` instead")]
    pub fn sample(token: Token<C, A>) -> Self {
        let mut ret = Self::new(token);
        ret.endpoint((RequestMethod::GET, Uri::from_static(SAMPLE)));
        ret
    }

    /// Start listening on the Streaming API endpoint, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    #[cfg(feature = "hyper")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hyper")))]
    pub fn listen(&self) -> crate::hyper::FutureTwitterStream {
        let conn = hyper_tls::HttpsConnector::new();
        self.listen_with_client(hyper_pkg::Client::builder().build::<_, hyper_pkg::Body>(conn))
    }

    /// Same as [`listen`](Builder::listen) except that it uses `client` to make HTTP request
    /// to the endpoint.
    ///
    /// `client` must be able to handle the `https` scheme.
    ///
    /// # Panics
    ///
    /// This will call `<S as Service>::call` without checking for `<S as Service>::poll_ready`
    /// and may cause a panic if `client` is not ready to send an HTTP request yet.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use tower::ServiceExt;
    ///
    /// # async fn doc() {
    /// # let mut client = hyper_pkg::Client::new();
    /// # let token = twitter_stream::Token::new("", "", "", "");
    /// client.ready().await; // Ensure that the client is ready to send a request.
    /// let stream = twitter_stream::Builder::sample(token)
    ///     .listen_with_client(&mut client)
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    pub fn listen_with_client<S, B>(&self, mut client: S) -> FutureTwitterStream<S::Future>
    where
        S: HttpService<B>,
        B: From<Vec<u8>>,
    {
        let req = prepare_request(
            self.endpoint.as_ref(),
            self.token.as_ref(),
            &self.parameters,
        );
        let response = client.call(req.map(Into::into));

        FutureTwitterStream { response }
    }
}

impl<'a, C, A> Builder<'a, Token<C, A>> {
    /// Set the API endpoint URI to be connected.
    ///
    /// This overrides the default behavior of automatically determining the endpoint to use.
    pub fn endpoint(&mut self, endpoint: impl Into<Option<(RequestMethod, Uri)>>) -> &mut Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Reset the token to be used to log into Twitter.
    pub fn token(&mut self, token: Token<C, A>) -> &mut Self {
        self.token = token;
        self
    }

    /// Set whether to receive messages when in danger of being disconnected.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#stall-warnings
    pub fn stall_warnings(&mut self, stall_warnings: bool) -> &mut Self {
        self.parameters.stall_warnings = stall_warnings;
        self
    }

    /// Set the minimum `filter_level` Tweet attribute to receive.
    /// The default is `FilterLevel::None`.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#filter-level
    pub fn filter_level(&mut self, filter_level: impl Into<Option<FilterLevel>>) -> &mut Self {
        self.parameters.filter_level = filter_level.into();
        self
    }

    /// Set a comma-separated language identifiers to receive Tweets
    /// written in the specified languages only.
    ///
    /// Setting an empty string will unset this parameter.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#language
    pub fn language(&mut self, language: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parameters.language = language.into();
        self
    }

    /// Set a list of user IDs to receive Tweets from the specified users.
    ///
    /// Setting an empty slice will unset this parameter.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#follow
    pub fn follow(&mut self, follow: impl Into<Cow<'a, [u64]>>) -> &mut Self {
        self.parameters.follow = follow.into();
        self
    }

    /// A comma separated list of phrases to filter Tweets by.
    ///
    /// Setting an empty string will unset this parameter.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#track
    pub fn track(&mut self, track: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parameters.track = track.into();
        self
    }

    /// Set a list of bounding boxes to filter Tweets by.
    ///
    /// Setting an empty slice will unset this parameter.
    ///
    /// See [`BoundingBox`](struct.BoundingBox.html) and
    /// the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#locations
    pub fn locations(&mut self, locations: impl Into<Cow<'a, [BoundingBox]>>) -> &mut Self {
        self.parameters.locations = locations.into();
        self
    }

    /// The `count` parameter.
    /// This parameter requires elevated access to use.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#count
    pub fn count(&mut self, count: impl Into<Option<i32>>) -> &mut Self {
        self.parameters.count = count.into();
        self
    }
}

impl std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}

impl std::fmt::Display for FilterLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        AsRef::<str>::as_ref(self).fmt(f)
    }
}

fn prepare_request(
    endpoint: Option<&(RequestMethod, Uri)>,
    token: Token<&str, &str>,
    parameters: &Parameters<'_>,
) -> http::Request<Vec<u8>> {
    let uri;
    let (method, endpoint) = if let Some(&(ref method, ref endpoint)) = endpoint {
        (method, endpoint)
    } else if parameters.follow.is_empty()
        && parameters.track.is_empty()
        && parameters.locations.is_empty()
    {
        uri = Uri::from_static(SAMPLE);
        (&RequestMethod::GET, &uri)
    } else {
        uri = Uri::from_static(FILTER);
        (&RequestMethod::POST, &uri)
    };

    let req = Request::builder().method(method.clone());

    #[cfg(feature = "gzip")]
    let req = req.header(
        http::header::ACCEPT_ENCODING,
        HeaderValue::from_static("gzip"),
    );

    let mut oauth = oauth::Builder::new(token.client.as_ref(), oauth::HmacSha1);
    oauth.token(token.token.as_ref());

    if RequestMethod::POST == method {
        let oauth::Request {
            authorization,
            data,
        } = oauth.post_form(endpoint, parameters);

        req.uri(endpoint.clone())
            .header(AUTHORIZATION, authorization)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            )
            .header(CONTENT_LENGTH, data.len())
            .body(data.into_bytes())
            .unwrap()
    } else {
        let oauth::Request {
            authorization,
            data: uri,
        } = oauth.build(method.as_ref(), endpoint, parameters);

        req.uri(uri)
            .header(AUTHORIZATION, authorization)
            .body(Vec::default())
            .unwrap()
    }
}

const COMMA: &str = "%2C";

fn fmt_follow(ids: &[u64], f: &mut Formatter<'_>) -> fmt::Result {
    fmt_join(ids, COMMA, f)
}

fn fmt_locations(locs: &[BoundingBox], f: &mut Formatter<'_>) -> fmt::Result {
    fmt_join(BoundingBox::flatten_slice(locs).flat(), COMMA, f)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn not(p: &bool) -> bool {
    !p
}
