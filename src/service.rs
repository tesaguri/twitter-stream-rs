//! A trait alias for [`Service`](tower_service::Service).

pub(crate) use private::IntoService;

use std::future::Future;
use std::task::{Context, Poll};

use http::{Request, Response};
use http_body::Body;
use tower_service::Service;

use private::Sealed;

/// An HTTP client (like [`hyper::Client`](hyper_pkg::client::Client)).
///
/// This is just an alias for [`tower_service::Service`](tower_service::Service)
/// introduced to reduce the number of type parameters in `Builder::listen_with_client`.
pub trait HttpService<B>: Sealed<B> {
    /// Body of the responses given by the service.
    type ResponseBody: Body;
    /// Alias for `Service::Error`.
    type Error;
    #[doc(hidden)]
    type Future: Future<Output = Result<Response<Self::ResponseBody>, Self::Error>>;

    #[doc(hidden)]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    #[doc(hidden)]
    fn call(&mut self, req: Request<B>) -> Self::Future;
}

impl<S, ReqB, ResB> HttpService<ReqB> for S
where
    S: Service<Request<ReqB>, Response = Response<ResB>> + ?Sized,
    ResB: Body,
{
    type ResponseBody = ResB;
    type Error = <Self as Service<Request<ReqB>>>::Error;
    type Future = <Self as Service<Request<ReqB>>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn call(&mut self, req: Request<ReqB>) -> Self::Future {
        Service::call(self, req)
    }
}

mod private {
    use std::task::{Context, Poll};

    use http::{Request, Response};
    use http_body::Body;
    use tower_service::Service;

    use super::HttpService;

    pub trait Sealed<B> {
        fn into_service(self) -> IntoService<Self>
        where
            Self: Sized;
    }

    pub struct IntoService<S>(S);

    impl<S, ReqB, ResB> Sealed<ReqB> for S
    where
        S: Service<Request<ReqB>, Response = Response<ResB>> + ?Sized,
        ResB: Body,
    {
        fn into_service(self) -> IntoService<Self>
        where
            Self: Sized,
        {
            IntoService(self)
        }
    }

    impl<S, B> Service<Request<B>> for IntoService<S>
    where
        S: HttpService<B>,
    {
        type Response = Response<S::ResponseBody>;
        type Error = <S as HttpService<B>>::Error;
        type Future = <S as HttpService<B>>::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.poll_ready(cx)
        }

        fn call(&mut self, req: Request<B>) -> Self::Future {
            self.0.call(req)
        }
    }
}
