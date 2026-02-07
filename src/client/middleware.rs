// File: ./src/client/middleware.rs
//! Tower middleware for adding a User-Agent header.
use http::Request;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

#[derive(Clone, Debug)]
pub struct UserAgentLayer {
    pub user_agent: String,
}

impl UserAgentLayer {
    pub fn new(user_agent: String) -> Self {
        Self { user_agent }
    }
}

impl<S> Layer<S> for UserAgentLayer {
    type Service = UserAgentService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        UserAgentService {
            inner,
            user_agent: self.user_agent.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserAgentService<S> {
    inner: S,
    user_agent: String,
}

impl<S, ReqBody> Service<Request<ReqBody>> for UserAgentService<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if let Ok(val) = http::HeaderValue::from_str(&self.user_agent) {
            req.headers_mut().insert(http::header::USER_AGENT, val);
        }
        self.inner.call(req)
    }
}
