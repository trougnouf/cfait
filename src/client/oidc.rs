// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/client/oidc.rs
// OIDC / OAuth2 client support for CalDAV servers that only accept
// Bearer tokens (issue #118):
// - OIDC discovery (`/.well-known/openid-configuration`)
// - OAuth2 device flow (RFC 8628) for headless / terminal / mobile logins
// - PKCE authorization-code flow (RFC 7636) with a loopback redirect
//   for desktop logins
// - Token management with refresh-token rotation and OS keyring persistence

use base64::Engine;
use getrandom::fill as getrandom_fill;
use http_body_util::BodyExt;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

/// OS keyring service name (shared with the CalDAV password storage).
const KEYRING_SERVICE: &str = "cfait";
/// Refresh the access token this many seconds before it actually expires,
/// so in-flight requests never carry a token that dies mid-flight.
const REFRESH_SKEW_SECS: i64 = 30;
/// How long the PKCE loopback flow waits for the browser redirect.
const PKCE_WAIT_SECS: u64 = 300;
/// Cap for OIDC response bodies (1 MiB is far beyond any token response).
const MAX_BODY_BYTES: usize = 1024 * 1024;

// -----------------------------
// Errors
// -----------------------------

/// Errors produced by the OIDC client and token manager.
///
/// `Display` messages are user-presentable (shown in the GUI snackbar and
/// the TUI), so they avoid internal jargon.
#[derive(Debug)]
pub enum OidcError {
    /// Network / transport level failure.
    Transport(String),
    /// Discovery document missing or malformed.
    Discovery(String),
    /// A response body could not be parsed as the expected JSON.
    Parse(String),
    /// The identity provider returned an OAuth2 error object.
    Provider(String),
    /// Device flow: the user has not authorized yet (keep polling).
    DevicePending,
    /// Device flow: the provider asked us to slow down (RFC 8628 §3.5).
    DeviceSlowDown,
    /// Device flow: the user denied the request.
    DeviceAccessDenied,
    /// Device flow: the device code expired.
    DeviceExpired,
    /// No valid (non-expired) token is available and none could be obtained.
    NoToken,
    /// The PKCE loopback wait timed out.
    LoginTimeout,
    /// The `state` parameter of the loopback redirect did not match.
    StateMismatch,
    /// Local I/O or environment failure (e.g. could not bind the loopback
    /// listener, keyring unavailable).
    Io(String),
}

impl std::fmt::Display for OidcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OidcError::Transport(msg) => write!(
                f,
                "Network error while contacting the identity provider: {msg}"
            ),
            OidcError::Discovery(msg) => write!(f, "OIDC discovery failed: {msg}"),
            OidcError::Parse(msg) => write!(f, "Unexpected response from the identity provider: {msg}"),
            OidcError::Provider(msg) => write!(f, "{msg}"),
            OidcError::DevicePending => write!(f, "Waiting for authorization"),
            OidcError::DeviceSlowDown => write!(f, "Slowing down"),
            OidcError::DeviceAccessDenied => write!(f, "Authorization was denied by the identity provider"),
            OidcError::DeviceExpired => write!(f, "The authorization code expired, please try again"),
            OidcError::NoToken => write!(f, "No valid token available, please sign in again"),
            OidcError::LoginTimeout => write!(f, "Timed out waiting for the browser authorization"),
            OidcError::StateMismatch => write!(f, "Authorization state mismatch, please try again"),
            OidcError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for OidcError {}

// -----------------------------
// Metadata & token types
// -----------------------------

/// The subset of the OIDC discovery document we use.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub device_authorization_endpoint: Option<String>,
}

/// A persisted OIDC token: the access token plus everything needed to
/// refresh it later. Serialized as JSON into a dedicated OS keyring entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTokenBundle {
    pub access_token: String,
    #[serde(default = "default_bearer")]
    pub token_type: String,
    /// Unix seconds at which the access token expires.
    pub expires_at: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    /// The provider's token endpoint (kept so refreshes work even if the
    /// issuer metadata changes).
    pub token_endpoint: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}

fn default_bearer() -> String {
    "Bearer".to_string()
}

/// The raw JSON shape of an OAuth2 token response.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

/// Result of an RFC 8628 device authorization request.
#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

// -----------------------------
// Transport
// -----------------------------

/// Minimal async HTTP transport for OIDC endpoint calls. Abstracted so unit
/// tests can substitute a scripted in-memory implementation.
#[allow(async_fn_in_trait)]
pub trait OidcTransport: Send + Sync {
    /// GET a URL and parse the JSON response body (2xx only).
    async fn get_json(&self, url: &str) -> Result<serde_json::Value, OidcError>;
    /// POST a urlencoded form. Returns `(status, parsed JSON body)`; non-2xx
    /// responses with a JSON body are returned as-is so callers can
    /// interpret OAuth2 error objects.
    async fn post_form(
        &self,
        url: &str,
        form: &[(String, String)],
    ) -> Result<(u16, serde_json::Value), OidcError>;
}

/// The production transport: a hyper client with the same trust anchors as
/// the CalDAV client (native roots, webpki-roots on Android).
pub struct HyperTransport {
    client: hyper_util::client::legacy::Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        String,
    >,
}

impl HyperTransport {
    pub fn new() -> anyhow::Result<Self> {
        let tls_config = super::core::RustyClient::root_tls_builder(false)?.with_no_client_auth();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();
        Ok(Self {
            client: hyper_util::client::legacy::Client::builder(
                hyper_util::rt::TokioExecutor::new(),
            )
            .build(https),
        })
    }
}

impl OidcTransport for HyperTransport {
    async fn get_json(&self, url: &str) -> Result<serde_json::Value, OidcError> {
        let req = http::Request::builder()
            .uri(url)
            .header(http::header::ACCEPT, "application/json")
            .body(String::new())
            .map_err(|e| OidcError::Io(e.to_string()))?;
        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| OidcError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| OidcError::Transport(e.to_string()))?
            .to_bytes();
        if body.len() > MAX_BODY_BYTES {
            return Err(OidcError::Parse("response too large".to_string()));
        }
        if !(200..300).contains(&status) {
            return Err(OidcError::Discovery(format!("HTTP {status}")));
        }
        serde_json::from_slice(&body).map_err(|e| OidcError::Parse(e.to_string()))
    }

    async fn post_form(
        &self,
        url: &str,
        form: &[(String, String)],
    ) -> Result<(u16, serde_json::Value), OidcError> {
        let body = form
            .iter()
            .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri(url)
            .header(http::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(http::header::ACCEPT, "application/json")
            .body(body)
            .map_err(|e| OidcError::Io(e.to_string()))?;
        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| OidcError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| OidcError::Transport(e.to_string()))?
            .to_bytes();
        if body.len() > MAX_BODY_BYTES {
            return Err(OidcError::Parse("response too large".to_string()));
        }
        let json: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| OidcError::Parse(format!("HTTP {status}: {e}")))?;
        Ok((status, json))
    }
}

// -----------------------------
// Client
// -----------------------------

/// Thin wrapper over a transport implementing the OIDC endpoint calls.
pub struct OidcClient<T: OidcTransport> {
    transport: T,
}

impl<T: OidcTransport> OidcClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Fetch and parse the OIDC discovery document for `issuer`.
    pub async fn discover(&self, issuer: &str) -> Result<OidcMetadata, OidcError> {
        let issuer = issuer.trim().trim_end_matches('/');
        if issuer.is_empty() {
            return Err(OidcError::Discovery("empty issuer".to_string()));
        }
        let url = format!("{issuer}/.well-known/openid-configuration");
        let value = self.transport.get_json(&url).await?;
        serde_json::from_value(value).map_err(|e| OidcError::Discovery(e.to_string()))
    }

    /// Request a device authorization (RFC 8628 §3.1).
    pub async fn device_authorization(
        &self,
        meta: &OidcMetadata,
        client_id: &str,
        client_secret: Option<&str>,
        scope: Option<&str>,
    ) -> Result<DeviceAuthorization, OidcError> {
        let endpoint = meta.device_authorization_endpoint.clone().ok_or_else(|| {
            OidcError::Discovery(
                "the provider does not advertise a device_authorization_endpoint (device flow unsupported)"
                    .to_string(),
            )
        })?;
        let mut form = vec![
            (
                "grant_type".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ),
            ("client_id".to_string(), client_id.to_string()),
        ];
        if let Some(secret) = client_secret {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        if let Some(scope) = scope.filter(|s| !s.trim().is_empty()) {
            form.push(("scope".to_string(), scope.to_string()));
        }
        let (status, value) = self.transport.post_form(&endpoint, &form).await?;
        if !(200..300).contains(&status) {
            return Err(provider_error(&value));
        }
        Ok(DeviceAuthorization {
            device_code: required_str(&value, "device_code")?,
            user_code: required_str(&value, "user_code")?,
            verification_uri: required_str(&value, "verification_uri")?,
            verification_uri_complete: value
                .get("verification_uri_complete")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            expires_in: value
                .get("expires_in")
                .and_then(|v| v.as_u64())
                .unwrap_or(600),
            interval: value
                .get("interval")
                .and_then(|v| v.as_u64())
                .unwrap_or(5)
                .max(1),
        })
    }

    /// Poll the token endpoint for a device code (RFC 8628 §3.4).
    ///
    /// Returns `Err(OidcError::DevicePending)` until the user authorizes, so
    /// callers can simply retry.
    pub async fn device_token(
        &self,
        meta: &OidcMetadata,
        client_id: &str,
        client_secret: Option<&str>,
        device_code: &str,
    ) -> Result<OidcTokenBundle, OidcError> {
        let mut form = vec![
            (
                "grant_type".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ),
            ("device_code".to_string(), device_code.to_string()),
            ("client_id".to_string(), client_id.to_string()),
        ];
        if let Some(secret) = client_secret {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        let (status, value) = self.transport.post_form(&meta.token_endpoint, &form).await?;
        if (200..300).contains(&status) {
            return token_bundle_from_response(
                &value,
                &meta.token_endpoint,
                client_id,
                client_secret.map(str::to_string),
                None,
            );
        }
        match value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
        {
            "authorization_pending" => Err(OidcError::DevicePending),
            "slow_down" => Err(OidcError::DeviceSlowDown),
            "access_denied" => Err(OidcError::DeviceAccessDenied),
            "expired_token" => Err(OidcError::DeviceExpired),
            _other => Err(OidcError::Provider(format!(
                "token endpoint error: {}",
                provider_error(&value)
            ))),
        }
    }
}

// -----------------------------
// Keyring persistence
// -----------------------------

/// Load a stored OIDC token bundle from the OS keyring, if any.
pub fn load_bundle(keyring_key: &str) -> Option<OidcTokenBundle> {
    let entry = keyring_core::Entry::new(KEYRING_SERVICE, keyring_key).ok()?;
    let json = entry.get_password().ok()?;
    serde_json::from_str(&json).ok()
}

/// Store a token bundle in the OS keyring, replacing any previous one.
pub fn save_bundle(keyring_key: &str, bundle: &OidcTokenBundle) -> Result<(), OidcError> {
    let json = serde_json::to_string(bundle).map_err(|e| OidcError::Parse(e.to_string()))?;
    let entry = keyring_core::Entry::new(KEYRING_SERVICE, keyring_key)
        .map_err(|e| OidcError::Io(e.to_string()))?;
    entry
        .set_password(&json)
        .map_err(|e| OidcError::Io(e.to_string()))
}

/// Delete a stored token bundle (sign-out), ignoring a missing entry.
pub fn clear_bundle(keyring_key: &str) {
    if let Ok(entry) = keyring_core::Entry::new(KEYRING_SERVICE, keyring_key) {
        let _ = entry.delete_credential();
    }
}

// -----------------------------
// Token manager
// -----------------------------

/// Long-lived holder of an OIDC token bundle. Hands out valid access tokens,
/// transparently refreshing them via the `refresh_token` grant (serialized
/// so concurrent callers share a single refresh), and persists the updated
/// bundle to the OS keyring.
pub struct TokenManager {
    state: tokio::sync::Mutex<TokenState>,
    /// Serializes refresh attempts (thundering-herd protection).
    refresh_lock: tokio::sync::Mutex<()>,
    keyring_key: Option<String>,
}

struct TokenState {
    bundle: OidcTokenBundle,
}

impl TokenManager {
    pub fn new(bundle: OidcTokenBundle, keyring_key: Option<String>) -> Self {
        Self {
            state: tokio::sync::Mutex::new(TokenState { bundle }),
            refresh_lock: tokio::sync::Mutex::new(()),
            keyring_key,
        }
    }

    /// Return a non-expired access token, refreshing if necessary.
    pub async fn get_valid_token(&self) -> Result<String, OidcError> {
        {
            let state = self.state.lock().await;
            if state.bundle.expires_at > unix_now() + REFRESH_SKEW_SECS {
                return Ok(state.bundle.access_token.clone());
            }
        }
        self.refresh().await
    }

    /// Force a refresh even if the current token is still valid.
    pub async fn force_refresh_token(&self) -> Result<String, OidcError> {
        self.refresh().await
    }

    async fn refresh(&self) -> Result<String, OidcError> {
        let _guard = self.refresh_lock.lock().await;

        // Re-check under the lock: another caller may have refreshed while
        // we were waiting for `refresh_lock`.
        let (refresh_token, endpoint, client_id, secret, scope) = {
            let state = self.state.lock().await;
            if state.bundle.expires_at > unix_now() + REFRESH_SKEW_SECS {
                return Ok(state.bundle.access_token.clone());
            }
            (
                state
                    .bundle
                    .refresh_token
                    .clone()
                    .filter(|t| !t.trim().is_empty()),
                state.bundle.token_endpoint.clone(),
                state.bundle.client_id.clone(),
                state.bundle.client_secret.clone(),
                state.bundle.scope.clone(),
            )
        };

        let refresh_token = refresh_token.ok_or(OidcError::NoToken)?;

        let transport =
            HyperTransport::new().map_err(|e| OidcError::Io(e.to_string()))?;
        let mut form = vec![
            ("grant_type".to_string(), "refresh_token".to_string()),
            ("refresh_token".to_string(), refresh_token.clone()),
            ("client_id".to_string(), client_id.clone()),
        ];
        if let Some(scope) = &scope {
            form.push(("scope".to_string(), scope.clone()));
        }
        if let Some(secret) = &secret {
            form.push(("client_secret".to_string(), secret.clone()));
        }
        let (status, value) = transport.post_form(&endpoint, &form).await?;
        if !(200..300).contains(&status) {
            // The refresh token is likely revoked; the user must sign in again.
            return Err(provider_error(&value));
        }
        let new_bundle = token_bundle_from_response(
            &value,
            &endpoint,
            &client_id,
            secret,
            Some(refresh_token),
        )?;
        let token = new_bundle.access_token.clone();
        if let Some(key) = &self.keyring_key {
            if let Err(e) = save_bundle(key, &new_bundle) {
                log::warn!("Failed to persist refreshed OIDC token: {e}");
            }
        }
        {
            let mut state = self.state.lock().await;
            state.bundle = new_bundle;
        }
        Ok(token)
    }
}

// -----------------------------
// Device flow (RFC 8628)
// -----------------------------

/// Start an RFC 8628 device flow: discover the provider and request a device
/// authorization. Returns the metadata (needed for polling) and the
/// authorization the user must complete on any device.
pub async fn start_device_login(
    client: &OidcClient<HyperTransport>,
    issuer: &str,
    client_id: &str,
    client_secret: Option<&str>,
    scope: Option<&str>,
) -> Result<(OidcMetadata, DeviceAuthorization), OidcError> {
    let meta = client.discover(issuer).await?;
    let authz = client
        .device_authorization(&meta, client_id, client_secret, scope)
        .await?;
    Ok((meta, authz))
}

/// Poll the token endpoint until the user authorizes (or the code expires).
pub async fn finish_device_login(
    client: &OidcClient<HyperTransport>,
    meta: &OidcMetadata,
    client_id: &str,
    client_secret: Option<&str>,
    authz: &DeviceAuthorization,
) -> Result<OidcTokenBundle, OidcError> {
    let mut interval = authz.interval.max(1);
    let deadline = unix_now() + authz.expires_in.max(1) as i64;
    loop {
        if unix_now() >= deadline {
            return Err(OidcError::DeviceExpired);
        }
        sleep(Duration::from_secs(interval)).await;
        match client
            .device_token(meta, client_id, client_secret, &authz.device_code)
            .await
        {
            Ok(bundle) => return Ok(bundle),
            Err(OidcError::DevicePending) => continue,
            Err(OidcError::DeviceSlowDown) => {
                interval *= 2;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

// -----------------------------
// PKCE authorization-code flow (RFC 7636 / RFC 8252 §7.3)
// -----------------------------

/// Generate a cryptographically random PKCE code verifier (RFC 7636 §4.1).
pub fn pkce_verifier() -> Result<String, OidcError> {
    let mut buf = [0u8; 32];
    getrandom_fill(&mut buf)
        .map_err(|e| OidcError::Io(format!("could not read random bytes: {e}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// The S256 code challenge for a verifier (RFC 7636 §4.2).
pub fn pkce_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// A random `state` parameter for CSRF protection.
pub fn random_state() -> Result<String, OidcError> {
    let mut buf = [0u8; 16];
    getrandom_fill(&mut buf)
        .map_err(|e| OidcError::Io(format!("could not read random bytes: {e}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// Full PKCE authorization-code login against a local loopback redirect.
///
/// Opens the user's default browser, waits (up to 5 minutes) for the
/// redirect on a per-invocation `127.0.0.1:0` listener, verifies `state`,
/// and exchanges the code for a token bundle.
pub async fn pkce_login(
    issuer: &str,
    client_id: &str,
    client_secret: Option<String>,
    scope: Option<&str>,
) -> Result<OidcTokenBundle, OidcError> {
    let transport =
        HyperTransport::new().map_err(|e| OidcError::Io(e.to_string()))?;
    let client = OidcClient::new(transport);
    let meta = client.discover(issuer).await?;

    let verifier = pkce_verifier()?;
    let challenge = pkce_challenge_s256(&verifier);
    let state = random_state()?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| OidcError::Io(format!("could not bind loopback listener: {e}")))?;
    let addr = listener
        .local_addr()
        .map_err(|e| OidcError::Io(e.to_string()))?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", addr.port());

    let mut auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        meta.authorization_endpoint,
        url_encode(client_id),
        url_encode(&redirect_uri),
        url_encode(&state),
        url_encode(&challenge),
    );
    if let Some(scope) = scope.filter(|s| !s.trim().is_empty()) {
        auth_url.push_str(&format!("&scope={}", url_encode(scope)));
    }

    // Best-effort: open the default browser. If that fails the user can
    // still complete the login manually (the wait runs for 5 minutes).
    let _ = open_url(&auth_url);

    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(PKCE_WAIT_SECS), listener.accept())
        .await
        .map_err(|_| OidcError::LoginTimeout)?
        .map_err(|e| OidcError::Io(e.to_string()))?;

    // Read the request head (up to the blank line), bounded.
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let head = loop {
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break String::from_utf8_lossy(&buf[..pos]).to_string();
        }
        if buf.len() > 64 * 1024 {
            break String::new();
        }
        match socket.read(&mut chunk).await {
            Ok(0) => break String::new(),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(OidcError::Io(format!("reading redirect request: {e}"))),
        }
    };

    // Parse "GET /callback?code=...&state=... HTTP/1.1".
    let request_line = head.lines().next().unwrap_or("");
    let target = request_line.split_whitespace().nth(1).unwrap_or("");
    let query = target.split('?').nth(1).unwrap_or("");
    let params = parse_query(query);

    if params.get("state").map(|s| s.as_str()) != Some(state.as_str()) {
        let _ = socket
            .write_all(
                browser_page(
                    rust_i18n::t!("oidc_browser_failed_title").to_string().as_str(),
                    rust_i18n::t!("oidc_browser_failed_body").to_string().as_str(),
                )
                .as_bytes(),
            )
            .await;
        let _ = socket.shutdown().await;
        return Err(OidcError::StateMismatch);
    }
    let code = match params.get("code") {
        Some(c) if !c.is_empty() => c.clone(),
        _ => {
            let err = params
                .get("error")
                .cloned()
                .unwrap_or_else(|| "missing code".to_string());
            let page = browser_page(
                rust_i18n::t!("oidc_browser_failed_title").to_string().as_str(),
                rust_i18n::t!("oidc_browser_failed_body").to_string().as_str(),
            );
            let _ = socket.write_all(page.as_bytes()).await;
            let _ = socket.shutdown().await;
            return Err(OidcError::Provider(format!(
                "authorization endpoint error: {err}"
            )));
        }
    };

    let page = browser_page(
        rust_i18n::t!("oidc_browser_done_title").to_string().as_str(),
        rust_i18n::t!("oidc_browser_done_body").to_string().as_str(),
    );
    let _ = socket.write_all(page.as_bytes()).await;
    let _ = socket.shutdown().await;

    // Exchange the code for tokens.
    let mut form = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("code".to_string(), code),
        ("redirect_uri".to_string(), redirect_uri),
        ("client_id".to_string(), client_id.to_string()),
        ("code_verifier".to_string(), verifier),
    ];
    if let Some(secret) = &client_secret {
        form.push(("client_secret".to_string(), secret.clone()));
    }
    let (status, value) = client
        .transport
        .post_form(&meta.token_endpoint, &form)
        .await?;
    if !(200..300).contains(&status) {
        return Err(provider_error(&value));
    }
    token_bundle_from_response(&value, &meta.token_endpoint, client_id, client_secret, None)
}

/// Best-effort open of a URL in the user's default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else if cfg!(target_os = "android") {
        let mut c = std::process::Command::new("am");
        c.args(["start", "-a", "android.intent.action.VIEW", "-d", url]);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command.spawn().map(|_| ())
}

/// The small HTML page sent to the browser on the loopback redirect.
fn browser_page(title: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
         <!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body style=\"font-family:sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;\">\
         <div style=\"text-align:center\"><h1>{title}</h1><p>{body}</p></div></body></html>"
    )
}

// -----------------------------
// Helpers
// -----------------------------

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Everything except RFC 3986 unreserved characters (`A-Za-z0-9 - . _ ~`)
/// is percent-encoded. Conservative but universally decodable by any
/// `application/x-www-form-urlencoded` consumer.
const FORM_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

fn url_encode(s: &str) -> String {
    utf8_percent_encode(s, FORM_ENCODE).to_string()
}

/// Parse a `k=v&k=v` query string (percent-decoding keys and values).
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.find('=') {
            Some(i) => (&pair[..i], &pair[i + 1..]),
            None => (pair, ""),
        };
        let k = percent_decode_str(k).decode_utf8_lossy().to_string();
        let v = percent_decode_str(v).decode_utf8_lossy().to_string();
        map.insert(k, v);
    }
    map
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

fn required_str<'a>(value: &'a serde_json::Value, field: &str) -> Result<String, OidcError> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| OidcError::Parse(format!("missing '{field}' in response")))
}

/// Build a user-presentable error from an OAuth2 error object.
fn provider_error(value: &serde_json::Value) -> OidcError {
    let code = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error");
    let description = value
        .get("error_description")
        .and_then(|v| v.as_str());
    match description {
        Some(d) if !d.trim().is_empty() => OidcError::Provider(format!("{code}: {d}")),
        _ => OidcError::Provider(code.to_string()),
    }
}

/// Convert a token response JSON into a persistable bundle.
fn token_bundle_from_response(
    value: &serde_json::Value,
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<String>,
    previous_refresh: Option<String>,
) -> Result<OidcTokenBundle, OidcError> {
    let resp: TokenResponse = serde_json::from_value(value.clone())
        .map_err(|e| OidcError::Parse(format!("token response: {e}")))?;
    let expires_at = unix_now() + resp.expires_in.unwrap_or(3600) as i64;
    // If the provider rotated the refresh token, the response carries the
    // new one; otherwise keep the previous one.
    let refresh_token = resp.refresh_token.or(previous_refresh);
    Ok(OidcTokenBundle {
        access_token: resp.access_token,
        token_type: resp
            .token_type
            .unwrap_or_else(|| "Bearer".to_string()),
        expires_at,
        refresh_token,
        scope: resp.scope,
        token_endpoint: token_endpoint.to_string(),
        client_id: client_id.to_string(),
        client_secret,
    })
}

// -----------------------------
// Unit tests (scripted in-memory transport)
// -----------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A scripted in-memory transport: responses are consumed in order.
    #[derive(Default)]
    struct MockTransport {
        get_responses: Mutex<Vec<serde_json::Value>>,
        post_responses: Mutex<Vec<(u16, serde_json::Value)>>,
        calls: Mutex<Vec<String>>,
    }

    impl OidcTransport for MockTransport {
        async fn get_json(&self, url: &str) -> Result<serde_json::Value, OidcError> {
            self.calls.lock().unwrap().push(format!("GET {url}"));
            let mut responses = self.get_responses.lock().unwrap();
            match responses.first().cloned() {
                Some(resp) => {
                    responses.remove(0);
                    Ok(resp)
                }
                None => Err(OidcError::Parse("no mock GET response".to_string())),
            }
        }

        async fn post_form(
            &self,
            url: &str,
            form: &[(String, String)],
        ) -> Result<(u16, serde_json::Value), OidcError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("POST {url} form={form:?}"));
            let mut responses = self.post_responses.lock().unwrap();
            match responses.first().cloned() {
                Some(resp) => {
                    responses.remove(0);
                    Ok(resp)
                }
                None => Err(OidcError::Parse("no mock POST response".to_string())),
            }
        }
    }

    fn token_json(access: &str, expires_in: u64, refresh: Option<&str>) -> serde_json::Value {
        let mut v = serde_json::json!({
            "access_token": access,
            "token_type": "Bearer",
            "expires_in": expires_in,
        });
        if let Some(r) = refresh {
            v["refresh_token"] = serde_json::json!(r);
        }
        v
    }

    #[tokio::test]
    async fn discover_parses_metadata() {
        let transport = MockTransport {
            get_responses: Mutex::new(vec![serde_json::json!({
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "device_authorization_endpoint": "https://idp.example.com/device",
            })]),
            ..Default::default()
        };
        let client = OidcClient::new(transport);
        let meta = client
            .discover("https://idp.example.com/")
            .await
            .unwrap();
        assert_eq!(meta.authorization_endpoint, "https://idp.example.com/authorize");
        assert_eq!(
            meta.device_authorization_endpoint.as_deref(),
            Some("https://idp.example.com/device")
        );
    }

    #[tokio::test]
    async fn discover_rejects_empty_issuer() {
        let client = OidcClient::new(MockTransport::default());
        let err = client.discover("   ").await.unwrap_err();
        assert!(matches!(err, OidcError::Discovery(_)));
    }

    #[tokio::test]
    async fn device_authorization_parses_fields() {
        let transport = MockTransport {
            post_responses: Mutex::new(vec![(
                200,
                serde_json::json!({
                    "device_code": "dc-1",
                    "user_code": "ABCD-1234",
                    "verification_uri": "https://idp.example.com/activate",
                    "verification_uri_complete": "https://idp.example.com/activate?user_code=ABCD-1234",
                    "expires_in": 900,
                    "interval": 5,
                }),
            )]),
            ..Default::default()
        };
        let client = OidcClient::new(transport);
        let meta = OidcMetadata {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: "https://idp.example.com/authorize".into(),
            token_endpoint: "https://idp.example.com/token".into(),
            device_authorization_endpoint: Some("https://idp.example.com/device".into()),
        };
        let authz = client
            .device_authorization(&meta, "client-1", None, Some("openid"))
            .await
            .unwrap();
        assert_eq!(authz.device_code, "dc-1");
        assert_eq!(authz.user_code, "ABCD-1234");
        assert_eq!(authz.expires_in, 900);
        assert_eq!(authz.interval, 5);
    }

    #[tokio::test]
    async fn device_authorization_fails_without_endpoint() {
        let client = OidcClient::new(MockTransport::default());
        let meta = OidcMetadata {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: "https://idp.example.com/authorize".into(),
            token_endpoint: "https://idp.example.com/token".into(),
            device_authorization_endpoint: None,
        };
        let err = client
            .device_authorization(&meta, "client-1", None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, OidcError::Discovery(_)));
    }

    #[tokio::test]
    async fn device_token_maps_pending_and_success() {
        let transport = MockTransport {
            post_responses: Mutex::new(vec![
                (400, serde_json::json!({"error": "authorization_pending"})),
                (
                    200,
                    token_json("at-1", 3600, Some("rt-1")),
                ),
            ]),
            ..Default::default()
        };
        let client = OidcClient::new(transport);
        let meta = OidcMetadata {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: "https://idp.example.com/authorize".into(),
            token_endpoint: "https://idp.example.com/token".into(),
            device_authorization_endpoint: None,
        };
        let err = client
            .device_token(&meta, "client-1", None, "dc-1")
            .await
            .unwrap_err();
        assert!(matches!(err, OidcError::DevicePending));

        let bundle = client
            .device_token(&meta, "client-1", None, "dc-1")
            .await
            .unwrap();
        assert_eq!(bundle.access_token, "at-1");
        assert_eq!(bundle.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(bundle.client_id, "client-1");
        assert!(bundle.expires_at > unix_now());
    }

    #[tokio::test]
    async fn device_token_maps_denied_and_expired() {
        let transport = MockTransport {
            post_responses: Mutex::new(vec![
                (400, serde_json::json!({"error": "access_denied"})),
                (400, serde_json::json!({"error": "expired_token"})),
                (400, serde_json::json!({"error": "slow_down"})),
            ]),
            ..Default::default()
        };
        let client = OidcClient::new(transport);
        let meta = OidcMetadata {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: "https://idp.example.com/authorize".into(),
            token_endpoint: "https://idp.example.com/token".into(),
            device_authorization_endpoint: None,
        };
        assert!(matches!(
            client.device_token(&meta, "c", None, "dc").await.unwrap_err(),
            OidcError::DeviceAccessDenied
        ));
        assert!(matches!(
            client.device_token(&meta, "c", None, "dc").await.unwrap_err(),
            OidcError::DeviceExpired
        ));
        assert!(matches!(
            client.device_token(&meta, "c", None, "dc").await.unwrap_err(),
            OidcError::DeviceSlowDown
        ));
    }

    #[test]
    fn pkce_challenge_is_s256() {
        // RFC 7636 §4.1 appendix test vector (truncated verifier).
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_challenge_s256(verifier);
        assert_eq!(
            challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn pkce_verifier_is_url_safe_base64() {
        let v1 = pkce_verifier().unwrap();
        let v2 = pkce_verifier().unwrap();
        assert_eq!(v1.len(), 43);
        assert_ne!(v1, v2);
        assert!(v1
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn parse_query_decodes_pairs() {
        let q = "code=abc%20123&state=s%2Ft&extra=x";
        let map = parse_query(q);
        assert_eq!(map.get("code").unwrap(), "abc 123");
        assert_eq!(map.get("state").unwrap(), "s/t");
        assert_eq!(map.get("extra").unwrap(), "x");
    }

    #[test]
    fn url_encode_round_trips() {
        let s = "http://127.0.0.1:54321/callback";
        let enc = url_encode(s);
        assert!(!enc.contains(' '));
        assert_eq!(
            percent_decode_str(&enc).decode_utf8_lossy().as_ref(),
            s
        );
    }

    #[test]
    fn bundle_json_round_trip() {
        let bundle = OidcTokenBundle {
            access_token: "at".into(),
            token_type: "Bearer".into(),
            expires_at: 123,
            refresh_token: Some("rt".into()),
            scope: Some("openid".into()),
            token_endpoint: "https://idp/token".into(),
            client_id: "cid".into(),
            client_secret: None,
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let back: OidcTokenBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.access_token, "at");
        assert_eq!(back.refresh_token.as_deref(), Some("rt"));
    }

    #[tokio::test]
    async fn token_manager_returns_valid_token_without_refresh() {
        let bundle = OidcTokenBundle {
            access_token: "at-valid".into(),
            token_type: "Bearer".into(),
            expires_at: unix_now() + 3600,
            refresh_token: None,
            scope: None,
            token_endpoint: "https://idp/token".into(),
            client_id: "cid".into(),
            client_secret: None,
        };
        let manager = TokenManager::new(bundle, None);
        let token = manager.get_valid_token().await.unwrap();
        assert_eq!(token, "at-valid");
    }

    #[tokio::test]
    async fn token_manager_expired_without_refresh_token_fails() {
        let bundle = OidcTokenBundle {
            access_token: "at-expired".into(),
            token_type: "Bearer".into(),
            expires_at: unix_now() - 10,
            refresh_token: None,
            scope: None,
            token_endpoint: "https://idp/token".into(),
            client_id: "cid".into(),
            client_secret: None,
        };
        let manager = TokenManager::new(bundle, None);
        let err = manager.get_valid_token().await.unwrap_err();
        assert!(matches!(err, OidcError::NoToken));
    }

    #[test]
    fn find_subsequence_locates_needle() {
        let hay = b"GET /callback?code=a\r\nHost: x\r\n\r\n";
        assert_eq!(find_subsequence(hay, b"\r\n\r\n"), Some(29));
        assert_eq!(find_subsequence(hay, b"zzz"), None);
    }

    #[test]
    fn provider_error_includes_description() {
        let v = serde_json::json!({"error": "invalid_grant", "error_description": "token revoked"});
        assert_eq!(
            provider_error(&v).to_string(),
            "invalid_grant: token revoked"
        );
    }
}
