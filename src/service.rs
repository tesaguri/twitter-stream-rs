//! A trait alias for [`Service`](tower_service::Service).

use http::{Request, Response};
use http_body::Body;
use tower_service::Service;

use private::Sealed;

/// An HTTP client (like [`hyper::Client`](hyper_pkg::client::Client)).
///
/// This is just an alias for [`tower_service::Service`](tower_service::Service)
/// introduced to reduce the number of type parameters in `Builder::listen_with_client`.
pub trait HttpService<B>: Service<Request<B>> + Sealed<B> {
    /// Body of the responses given by the service.
    type ResponseBody: Body;
}

impl<S, ReqB, ResB> HttpService<ReqB> for S
where
    S: Service<Request<ReqB>, Response = Response<ResB>> + ?Sized,
    ResB: Body,
{
    type ResponseBody = ResB;
}

mod private {
    use http::{Request, Response};
    use http_body::Body;
    use tower_service::Service;

    pub trait Sealed<B> {}

    impl<S, ReqB, ResB> Sealed<ReqB> for S
    where
        S: Service<Request<ReqB>, Response = Response<ResB>> + ?Sized,
        ResB: Body,
    {
    }
}
