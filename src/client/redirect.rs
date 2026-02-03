// File: ./src/client/redirect.rs
use http::{Request, Response, Uri};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

#[derive(Clone, Debug)]
pub struct FollowRedirectLayer {
    max_redirects: usize,
}

impl FollowRedirectLayer {
    pub fn new(max_redirects: usize) -> Self {
        Self { max_redirects }
    }
}

impl<S> Layer<S> for FollowRedirectLayer {
    type Service = FollowRedirectService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        FollowRedirectService {
            inner,
            max_redirects: self.max_redirects,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FollowRedirectService<S> {
    inner: S,
    max_redirects: usize,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for FollowRedirectService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    ReqBody: Clone + Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        let max_redirects = self.max_redirects;

        Box::pin(async move {
            let mut current_req = req;
            let mut attempts = 0;

            loop {
                // Clone the request before consuming it, so we can retry if needed.
                // This relies on ReqBody (String) being Clone.
                let req_clone = current_req.clone();

                let response = inner.call(current_req).await?;

                if attempts >= max_redirects {
                    return Ok(response);
                }

                let status = response.status();
                if status.is_redirection()
                    && let Some(location) = response.headers().get(http::header::LOCATION)
                        && let Ok(loc_str) = location.to_str() {
                            // Resolve the new URI (handle relative paths)
                            let new_uri = if let Ok(parsed) = loc_str.parse::<Uri>() {
                                let parts = parsed.into_parts();
                                let mut builder = Uri::builder();

                                // Inherit scheme if missing
                                if let Some(scheme) = parts.scheme {
                                    builder = builder.scheme(scheme);
                                } else if let Some(s) = req_clone.uri().scheme() {
                                    builder = builder.scheme(s.clone());
                                }

                                // Inherit authority if missing
                                if let Some(authority) = parts.authority {
                                    builder = builder.authority(authority);
                                } else if let Some(a) = req_clone.uri().authority() {
                                    builder = builder.authority(a.clone());
                                }

                                if let Some(pq) = parts.path_and_query {
                                    builder = builder.path_and_query(pq);
                                }

                                builder.build().unwrap_or_else(|_| req_clone.uri().clone())
                            } else {
                                req_clone.uri().clone()
                            };

                            // Update request for retry
                            current_req = req_clone;
                            *current_req.uri_mut() = new_uri;
                            attempts += 1;
                            continue;
                        }

                return Ok(response);
            }
        })
    }
}
