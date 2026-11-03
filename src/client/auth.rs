// SPDX-License-Identifier: GPL-3.0-or-later
// Implements HTTP authentication for the client: username/password
// (Basic/Digest), static bearer tokens, and OIDC-managed tokens.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use digest_auth::{AuthContext, HttpMethod};
use http::{HeaderValue, Request, Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_service::Service;

use crate::client::oidc::TokenManager;

/// The authentication scheme used against a CalDAV server.
///
/// - [`Auth::Basic`]: username/password, sent as an optimistic HTTP Basic
///   header and upgraded to HTTP Digest when the server answers 401 with a
///   `Digest` challenge (the CalDAV norm).
/// - [`Auth::Bearer`]: a static token, or the output of a shell command,
///   sent as `Authorization: Bearer <token>`. On 401 the command (if any) is
///   re-run once and the request retried.
/// - [`Auth::Oidc`]: a token issued by an OpenID Connect provider and kept
///   alive by a [`TokenManager`] (transparent refresh, keyring persistence).
#[derive(Clone)]
pub enum Auth {
    Basic {
        user: String,
        pass: String,
    },
    Bearer {
        token: String,
        /// Optional shell command that prints a fresh token; re-run once on 401.
        token_command: Option<String>,
    },
    Oidc {
        manager: Arc<TokenManager>,
    },
}

impl Auth {
    pub fn basic(user: &str, pass: &str) -> Self {
        Auth::Basic {
            user: user.to_string(),
            pass: pass.to_string(),
        }
    }

    pub fn bearer(token: String, token_command: Option<String>) -> Self {
        Auth::Bearer {
            token,
            token_command,
        }
    }

    pub fn oidc(manager: Arc<TokenManager>) -> Self {
        Auth::Oidc { manager }
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::Basic { user, .. } => f
                .debug_struct("Basic")
                .field("user", user)
                .field("pass", &"<redacted>")
                .finish(),
            Auth::Bearer { token_command, .. } => f
                .debug_struct("Bearer")
                .field("token", &"<redacted>")
                .field("token_command", token_command)
                .finish(),
            Auth::Oidc { .. } => f.debug_struct("Oidc").finish(),
        }
    }
}

#[derive(Clone)]
pub struct DynamicAuthLayer {
    pub auth: Auth,
}

impl DynamicAuthLayer {
    pub fn new(auth: Auth) -> Self {
        Self { auth }
    }
}

impl std::fmt::Debug for DynamicAuthLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicAuthLayer")
            .field("auth", &self.auth)
            .finish()
    }
}

impl<S> tower_layer::Layer<S> for DynamicAuthLayer {
    type Service = DynamicAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DynamicAuthService {
            inner,
            auth: self.auth.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DynamicAuthService<S> {
    inner: S,
    auth: Auth,
}

impl<S> std::fmt::Debug for DynamicAuthService<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicAuthService")
            .field("inner", &"<service>")
            .field("auth", &self.auth)
            .finish()
    }
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
        let mut req_clone = req.clone();
        let mut inner = self.inner.clone();
        let auth = self.auth.clone();

        Box::pin(async move {
            // Attach the initial Authorization header for the scheme.
            match &auth {
                Auth::Basic { user, pass } => {
                    let basic_header_val =
                        format!("Basic {}", STANDARD.encode(format!("{}:{}", user, pass)));
                    if let Ok(val) = HeaderValue::from_str(&basic_header_val) {
                        req.headers_mut().insert(http::header::AUTHORIZATION, val);
                    }
                }
                Auth::Bearer { token, .. } if !token.trim().is_empty() => {
                    if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                        req.headers_mut().insert(http::header::AUTHORIZATION, val);
                    }
                }
                // An empty bearer token is sent without a header so the 401
                // surfaces to the caller; the retry below may re-run the
                // token command.
                Auth::Bearer { .. } => {}
                // For OIDC the token may have to be refreshed first, which
                // needs to happen inside the async block.
                Auth::Oidc { manager } => {
                    match manager.get_valid_token().await {
                        Ok(token) => {
                            let val = HeaderValue::from_str(&format!("Bearer {}", token));
                            if let Ok(val) = val {
                                let retry_val = val.clone();
                                req.headers_mut().insert(http::header::AUTHORIZATION, val);
                                // Keep the retry clone in sync.
                                req_clone
                                    .headers_mut()
                                    .insert(http::header::AUTHORIZATION, retry_val);
                            }
                        }
                        Err(e) => {
                            // Proceed without a header so the 401 surfaces to
                            // the caller instead of failing before the request.
                            log::warn!("Could not obtain a valid OIDC token: {e}");
                        }
                    }
                }
            }

            let response = inner.call(req).await?;

            if response.status() != StatusCode::UNAUTHORIZED {
                return Ok(response);
            }

            match &auth {
                // Basic: negotiate HTTP Digest from the 401 challenge.
                Auth::Basic { user, pass } => {
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
                        let context =
                            AuthContext::new_with_method(user, pass, &uri, body_bytes, method);

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
                }
                // Bearer: re-run the token command once and retry.
                Auth::Bearer { token_command, .. } => {
                    let Some(cmd) = token_command else {
                        return Ok(response);
                    };
                    match crate::config::Config::run_password_command(cmd) {
                        Ok(new_token) if !new_token.trim().is_empty() => {
                            let mut new_req = req_clone;
                            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", new_token))
                            {
                                new_req
                                    .headers_mut()
                                    .insert(http::header::AUTHORIZATION, val);
                            }
                            inner.call(new_req).await
                        }
                        _ => Ok(response),
                    }
                }
                // Oidc: force a refresh once and retry.
                Auth::Oidc { manager } => match manager.force_refresh_token().await {
                    Ok(new_token) => {
                        let mut new_req = req_clone;
                        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", new_token)) {
                            new_req
                                .headers_mut()
                                .insert(http::header::AUTHORIZATION, val);
                        }
                        inner.call(new_req).await
                    }
                    Err(e) => {
                        log::warn!("OIDC token refresh failed: {e}");
                        Ok(response)
                    }
                },
            }
        })
    }
}
