//! Builder type for `TwitterStream`.

pub use http::Method as RequestMethod;
pub use http::StatusCode;
pub use http::Uri;

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Formatter};

use http::header::{HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use http::Request;

use crate::service::HttpService;
use crate::token::Token;
use crate::util::fmt_join;
use crate::FutureTwitterStream;

/// A builder for `TwitterStream`.
///
/// ## Example
///
/// ```rust,no_run
/// use futures::prelude::*;
/// use twitter_stream::Token;
///
/// # #[tokio::main]
/// # async fn main() {
/// let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
///
/// twitter_stream::Builder::sample(token)
///     .listen()
///     .try_flatten_stream()
///     .try_for_each(|json| {
///         println!("{}", json);
///         future::ok(())
///     })
///     .await
///     .unwrap();
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Builder<'a, T = Token> {
    method: RequestMethod,
    endpoint: Uri,
    token: T,
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

/// A `BoundingBox` is a rectangular area on the globe specified by coordinates of
/// the southwest and northeast edges in decimal degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct BoundingBox {
    /// Longitude of the west side of the bounding box.
    pub west_longitude: f64,
    /// Latitude of the south side of the bounding box.
    pub south_latitude: f64,
    /// Longitude of the east side of the bounding box.
    pub east_longitude: f64,
    /// Latitude of the north side of the bounding box.
    pub north_latitude: f64,
}

str_enum! {
    /// Represents the `filter_level` parameter in API requests.
    #[derive(Clone, Debug, PartialEq, Hash, Eq)]
    pub enum FilterLevel {
        None = "none",
        Low = "low",
        Medium = "medium",
    }
}

impl<'a, C, A> Builder<'a, Token<C, A>>
where
    C: Borrow<str>,
    A: Borrow<str>,
{
    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    pub fn filter(token: Token<C, A>) -> Self {
        const URI: &str = "https://stream.twitter.com/1.1/statuses/filter.json";
        Self::custom(RequestMethod::POST, Uri::from_static(URI), token)
    }

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    pub fn sample(token: Token<C, A>) -> Self {
        const URI: &str = "https://stream.twitter.com/1.1/statuses/sample.json";
        Self::custom(RequestMethod::GET, Uri::from_static(URI), token)
    }

    /// Constructs a builder for a Stream at a custom endpoint.
    pub fn custom(method: RequestMethod, endpoint: Uri, token: Token<C, A>) -> Self {
        Self {
            method,
            endpoint,
            token,
            parameters: Parameters::default(),
        }
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

    /// Same as `listen` except that it uses `client` to make HTTP request to the endpoint.
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
        B: Default + From<Vec<u8>>,
    {
        let req = Request::builder()
            .method(self.method.clone())
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"));

        let mut oauth = oauth::Builder::new(self.token.client.as_ref(), oauth::HmacSha1);
        oauth.token(self.token.token.as_ref());
        let req = if RequestMethod::POST == self.method {
            let oauth::Request {
                authorization,
                data,
            } = oauth.post_form(&self.endpoint, &self.parameters);

            req.uri(self.endpoint.clone())
                .header(AUTHORIZATION, authorization)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded"),
                )
                .header(CONTENT_LENGTH, data.len())
                .body(data.into_bytes().into())
                .unwrap()
        } else {
            let oauth::Request {
                authorization,
                data: uri,
            } = oauth.build(self.method.as_ref(), &self.endpoint, &self.parameters);

            req.uri(uri)
                .header(AUTHORIZATION, authorization)
                .body(B::default())
                .unwrap()
        };

        let response = client.call(req);
        FutureTwitterStream { response }
    }
}

impl<'a, C, A> Builder<'a, Token<C, A>> {
    /// Reset the HTTP request method to be used when connecting
    /// to the server.
    pub fn method(&mut self, method: RequestMethod) -> &mut Self {
        self.method = method;
        self
    }

    /// Reset the API endpoint URI to be connected.
    pub fn endpoint(&mut self, endpoint: Uri) -> &mut Self {
        self.endpoint = endpoint;
        self
    }

    /// Reset the token to be used to log into Twitter.
    pub fn token(&mut self, token: Token<C, A>) -> &mut Self {
        self.token = token;
        self
    }

    /// Set whether to receive messages when in danger of
    /// being disconnected.
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
    /// See [`BoundingBox`](types/struct.BoundingBox.html) and
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

impl BoundingBox {
    /// Creates a `BoundingBox` with two `(longitude, latitude)` pairs.
    ///
    /// The first argument specifies the southwest edge and the second specifies the northeast edge.
    ///
    /// # Example
    ///
    /// ```rust
    /// use twitter_stream::types::BoundingBox;
    ///
    /// // Examples taken from Twitter's documentation.
    /// BoundingBox::new((-122.75, 36.8), (-121.75,37.8)); // San Francisco
    /// BoundingBox::new((-74.0, 40.0), (-73.0, 41.0)); // New York City
    /// ```
    pub fn new(
        (west_longitude, south_latitude): (f64, f64),
        (east_longitude, north_latitude): (f64, f64),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<(f64, f64, f64, f64)> for BoundingBox {
    fn from(
        (west_longitude, south_latitude, east_longitude, north_latitude): (f64, f64, f64, f64),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<((f64, f64), (f64, f64))> for BoundingBox {
    fn from(
        ((west_longitude, south_latitude), (east_longitude, north_latitude)): (
            (f64, f64),
            (f64, f64),
        ),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
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

const COMMA: &str = "%2C";

fn fmt_follow(ids: &[u64], f: &mut Formatter<'_>) -> fmt::Result {
    fmt_join(ids, COMMA, f)
}

fn fmt_locations(locs: &[BoundingBox], f: &mut Formatter<'_>) -> fmt::Result {
    use std::mem::size_of;
    use std::slice;

    use static_assertions::const_assert;

    let locs: &[f64] = unsafe {
        // Safety:
        // `BoundingBox` is defined with the `#[repr(C)]` attribute so it is sound to interpret
        // the struct as a `[f64; 4]` and also the fields are guaranteed to be placed in
        // `[west_longitude, south_latitude, east_longitude, north_latitude]` order.
        // We are checking the size of `BoundingBox` here just to be sure.
        const_assert!(size_of::<BoundingBox>() == 4 * size_of::<f64>());
        let ptr: *const BoundingBox = locs.as_ptr();
        let n = 4 * <[BoundingBox]>::len(locs);
        slice::from_raw_parts(ptr as *const f64, n)
    };

    fmt_join(locs, COMMA, f)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn not(p: &bool) -> bool {
    !p
}
