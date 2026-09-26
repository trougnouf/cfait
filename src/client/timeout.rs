// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/client/timeout.rs
//
// A request-level timeout for the HTTP client stack.
//
// tower's `TimeoutLayer` cannot be used here directly: it reports failures as
// `Box<dyn Error + Send + Sync>`, which does not itself implement
// `std::error::Error`, while libdav's `HttpClient` bound requires
// `C::Error: Error + Send + Sync`. This module provides the same behavior with
// a concrete error type that satisfies the bound.

use http::{Request, Response};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tower_service::Service;

/// Error produced by [`TimeoutService`].
#[derive(Debug)]
pub enum ClientTimeoutError {
    /// The request did not complete within the configured timeout.
    TimedOut,
    /// The underlying client failed.
    Inner(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for ClientTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientTimeoutError::TimedOut => write!(f, "request timed out"),
            ClientTimeoutError::Inner(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientTimeoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ClientTimeoutError::TimedOut => None,
            ClientTimeoutError::Inner(e) => e.source(),
        }
    }
}

impl ClientTimeoutError {
    pub(crate) fn from_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        ClientTimeoutError::Inner(Box::new(e))
    }
}

/// Applies a timeout to a whole logical request (all redirect hops included,
/// since this wraps the redirect layer).
#[derive(Clone, Debug)]
pub struct TimeoutService<S> {
    inner: S,
    timeout: Duration,
}

impl<S> TimeoutService<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for TimeoutService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = ClientTimeoutError;
    type Future =
        Pin<Box<dyn Future<Output = Result<S::Response, ClientTimeoutError>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(ClientTimeoutError::from_error)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let fut = self.inner.call(req);
        let timeout = self.timeout;
        Box::pin(async move {
            match tokio::time::timeout(timeout, fut).await {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(e)) => Err(ClientTimeoutError::from_error(e)),
                Err(_) => Err(ClientTimeoutError::TimedOut),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server that answers a watering-can query instantly.
    struct QuickService;
    impl Service<Request<String>> for QuickService {
        type Response = Response<String>;
        type Error = std::io::Error;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: Request<String>) -> Self::Future {
            std::future::ready(Ok(Response::new("two cups of water".to_string())))
        }
    }

    /// A server that accepts the connection and never answers (hung garden hose).
    struct HangingService;
    impl Service<Request<String>> for HangingService {
        type Response = Response<String>;
        type Error = std::io::Error;
        type Future = std::future::Pending<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: Request<String>) -> Self::Future {
            std::future::pending()
        }
    }

    /// A server that burns the soup.
    struct FailingService;
    impl Service<Request<String>> for FailingService {
        type Response = Response<String>;
        type Error = std::io::Error;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: Request<String>) -> Self::Future {
            std::future::ready(Err(std::io::Error::other("the soup is burnt")))
        }
    }

    #[tokio::test]
    async fn quick_request_completes() {
        let mut svc = TimeoutService::new(QuickService, Duration::from_secs(5));
        let resp = svc
            .call(Request::new("water the roses".to_string()))
            .await
            .unwrap();
        assert_eq!(resp.body(), "two cups of water");
    }

    #[tokio::test]
    async fn hanging_request_times_out() {
        let mut svc = TimeoutService::new(HangingService, Duration::from_millis(50));
        let err = svc
            .call(Request::new("water the roses".to_string()))
            .await
            .unwrap_err();
        assert!(matches!(err, ClientTimeoutError::TimedOut));
        assert!(err.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn inner_error_is_preserved() {
        let mut svc = TimeoutService::new(FailingService, Duration::from_secs(5));
        let err = svc
            .call(Request::new("cook the soup".to_string()))
            .await
            .unwrap_err();
        assert!(matches!(err, ClientTimeoutError::Inner(_)));
        assert!(err.to_string().contains("the soup is burnt"));
    }
}
