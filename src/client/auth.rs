// Implements HTTP authentication logic (Basic/Digest) for the client.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use digest_auth::{AuthContext, HttpMethod};
use http::{HeaderValue, Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_service::Service;

#[derive(Clone, Debug)]
pub struct DynamicAuthLayer {
    pub user: String,
    pub pass: String,
}

impl DynamicAuthLayer {
    pub fn new(user: String, pass: String) -> Self {
        Self { user, pass }
    }
}

impl<S> tower_layer::Layer<S> for DynamicAuthLayer {
    type Service = DynamicAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DynamicAuthService {
            inner,
            user: self.user.clone(),
            pass: self.pass.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DynamicAuthService<S> {
    inner: S,
    user: String,
    pass: String,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for DynamicAuthService<S>
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

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        // 1. Optimistic Basic Auth
        let basic_header_val = format!(
            "Basic {}",
            STANDARD.encode(format!("{}:{}", self.user, self.pass))
        );
        if let Ok(val) = HeaderValue::from_str(&basic_header_val) {
            req.headers_mut().insert(http::header::AUTHORIZATION, val);
        }

        let req_clone = req.clone();
        let mut inner = self.inner.clone();
        let user = self.user.clone();
        let pass = self.pass.clone();

        Box::pin(async move {
            let response = inner.call(req).await?;

            if response.status() != StatusCode::UNAUTHORIZED {
                return Ok(response);
            }

            let auth_header = match response.headers().get("www-authenticate") {
                Some(h) => h,
                None => return Ok(response),
            };

            let auth_str = match auth_header.to_str() {
                Ok(s) => s,
                Err(_) => return Ok(response),
            };

            if auth_str.to_lowercase().starts_with("digest") {
                let uri = req_clone.uri().path().to_string();

                let method = HttpMethod::from(req_clone.method().as_str());

                // Pass None for body (auth-int is rare for CalDAV)
                let body_bytes: Option<&[u8]> = None;

                let context = AuthContext::new_with_method(&user, &pass, &uri, body_bytes, method);

                if let Ok(mut prompt) = digest_auth::parse(auth_str)
                    && let Ok(answer) = prompt.respond(&context)
                {
                    let header_val = answer.to_string();
                    let mut new_req = req_clone;
                    if let Ok(val) = HeaderValue::from_str(&header_val) {
                        new_req
                            .headers_mut()
                            .insert(http::header::AUTHORIZATION, val);
                    }
                    return inner.call(new_req).await;
                }
            }

            Ok(response)
        })
    }
}
