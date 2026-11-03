// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration tests for the OIDC / OAuth2 authentication support (issue
//! #118).
//!
//! The in-memory unit tests in `src/client/oidc.rs` cover the endpoint
//! logic with a scripted transport; these tests exercise the real HTTP
//! paths against a live mockito identity provider: discovery → device
//! authorization → token polling (including `slow_down`), refresh-token
//! rotation over HTTP, config plumbing, and a full CalDAV request carrying
//! the resolved bearer header.
//!
//! Keyring-dependent tests skip gracefully when no keyring backend is
//! available in the environment (e.g. a container without a Secret
//! Service provider).
use cfait::client::RustyClient;
use cfait::client::oidc::{
    HyperTransport, OidcClient, OidcTokenBundle, TokenManager, clear_bundle, finish_device_login,
    load_bundle, save_bundle, start_device_login, unix_now,
};
use cfait::config::{AuthMode, Config};
use cfait::context::TestContext;
use mockito::Server;
use std::sync::Arc;

fn discovery_json(issuer: &str) -> serde_json::Value {
    serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "device_authorization_endpoint": format!("{issuer}/device"),
    })
}

fn token_response(access: &str, expires_in: u64, refresh: Option<&str>) -> serde_json::Value {
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

fn json_body(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap()
}

fn test_bundle(access: &str, expires_at: i64, refresh: Option<&str>) -> OidcTokenBundle {
    OidcTokenBundle {
        access_token: access.to_string(),
        token_type: "Bearer".to_string(),
        expires_at,
        refresh_token: refresh.map(str::to_string),
        scope: None,
        token_endpoint: "http://127.0.0.1/token".to_string(),
        client_id: "cfait".to_string(),
        client_secret: None,
    }
}

// -----------------------------
// Config plumbing
// -----------------------------

#[test]
fn auth_mode_serde_round_trip() {
    let cases = [
        (AuthMode::Basic, "basic"),
        (AuthMode::Bearer, "bearer"),
        (AuthMode::Oidc, "oidc"),
    ];
    for (mode, name) in cases {
        assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{name}\""));
        assert_eq!(
            serde_json::from_value::<AuthMode>(serde_json::json!(name)).unwrap(),
            mode
        );
    }
}

#[test]
fn config_oidc_fields_serde_defaults() {
    // Old configs that predate OIDC support must still load.
    let cfg: Config = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(cfg.auth_mode, AuthMode::Basic);
    assert_eq!(cfg.oidc_issuer, "");
    assert_eq!(cfg.oidc_client_id, "");
    assert_eq!(cfg.oidc_scope, None);
    assert_eq!(cfg.oidc_client_secret_command, None);

    let mut cfg2 = Config::default();
    cfg2.auth_mode = AuthMode::Oidc;
    cfg2.oidc_issuer = "https://idp.example.com".into();
    cfg2.oidc_client_id = "cfait".into();
    cfg2.oidc_scope = Some("openid".into());
    let back: Config = serde_json::from_value(serde_json::to_value(&cfg2).unwrap()).unwrap();
    assert_eq!(back.auth_mode, AuthMode::Oidc);
    assert_eq!(back.oidc_issuer, "https://idp.example.com");
    assert_eq!(back.oidc_client_id, "cfait");
    assert_eq!(back.oidc_scope.as_deref(), Some("openid"));
}

#[test]
fn oidc_keyring_key_namespaced_by_host() {
    let mut cfg = Config::default();
    cfg.url = "https://cloud.example.com/remote.php/dav/".into();
    assert_eq!(cfg.oidc_keyring_key(), "oidc@cloud.example.com");

    // No URL → no host → unnamespaced key.
    assert_eq!(Config::default().oidc_keyring_key(), "oidc");
}

// -----------------------------
// Device flow over real HTTP (RFC 8628)
// -----------------------------

#[tokio::test]
async fn device_flow_end_to_end_via_http() {
    let mut server = Server::new_async().await;
    let issuer = server.url();

    let mock_discovery = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_body(json_body(&discovery_json(&issuer)))
        .create_async()
        .await;

    let mock_device = server
        .mock("POST", "/device")
        .match_header("content-type", "application/x-www-form-urlencoded")
        .with_status(200)
        .with_body(json_body(&serde_json::json!({
            "device_code": "dc-e2e",
            "user_code": "WXYZ-9876",
            "verification_uri": format!("{issuer}/activate"),
            "verification_uri_complete": format!("{issuer}/activate?user_code=WXYZ-9876"),
            "expires_in": 120,
            "interval": 1,
        })))
        .create_async()
        .await;

    // mockito serves matching mocks in creation order while hits remain, so
    // create them in the order the poller should see them: pending, then ok.
    let mock_token_pending = server
        .mock("POST", "/token")
        .expect(1)
        .with_status(400)
        .with_body(json_body(
            &serde_json::json!({"error": "authorization_pending"}),
        ))
        .create_async()
        .await;
    let mock_token_ok = server
        .mock("POST", "/token")
        .expect(1)
        .with_status(200)
        .with_body(json_body(&token_response("at-e2e", 3600, Some("rt-e2e"))))
        .create_async()
        .await;

    let client = OidcClient::new(HyperTransport::new().unwrap());

    let (meta, authz) = start_device_login(&client, &issuer, "cfait", None, None)
        .await
        .expect("device flow should start");
    assert_eq!(meta.issuer, issuer);
    assert_eq!(authz.device_code, "dc-e2e");
    assert_eq!(authz.user_code, "WXYZ-9876");
    assert_eq!(
        authz.verification_uri_complete,
        Some(format!("{issuer}/activate?user_code=WXYZ-9876"))
    );

    let bundle = finish_device_login(&client, &meta, "cfait", None, &authz)
        .await
        .expect("device flow should complete");
    assert_eq!(bundle.access_token, "at-e2e");
    assert_eq!(bundle.refresh_token.as_deref(), Some("rt-e2e"));
    assert_eq!(bundle.client_id, "cfait");
    assert_eq!(bundle.token_endpoint, format!("{issuer}/token"));
    assert!(bundle.expires_at > unix_now());

    mock_discovery.assert();
    mock_device.assert();
    mock_token_pending.assert();
    mock_token_ok.assert();
}

#[tokio::test]
async fn device_flow_slow_down_then_success() {
    let mut server = Server::new_async().await;
    let issuer = server.url();

    let _mock_discovery = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_body(json_body(&discovery_json(&issuer)))
        .create_async()
        .await;
    let _mock_device = server
        .mock("POST", "/device")
        .with_status(200)
        .with_body(json_body(&serde_json::json!({
            "device_code": "dc-sd",
            "user_code": "AAAA-1111",
            "verification_uri": format!("{issuer}/activate"),
            "expires_in": 120,
            "interval": 1,
        })))
        .create_async()
        .await;

    // Creation order = response order: slow_down, then pending, then
    // success. The poller must double its interval after `slow_down` and
    // keep going.
    let mock_slow = server
        .mock("POST", "/token")
        .expect(1)
        .with_status(400)
        .with_body(json_body(&serde_json::json!({"error": "slow_down"})))
        .create_async()
        .await;
    let mock_pending = server
        .mock("POST", "/token")
        .expect(1)
        .with_status(400)
        .with_body(json_body(
            &serde_json::json!({"error": "authorization_pending"}),
        ))
        .create_async()
        .await;
    let mock_ok = server
        .mock("POST", "/token")
        .expect(1)
        .with_status(200)
        .with_body(json_body(&token_response("at-sd", 3600, None)))
        .create_async()
        .await;

    let client = OidcClient::new(HyperTransport::new().unwrap());
    let (meta, authz) = start_device_login(&client, &issuer, "cfait", None, None)
        .await
        .unwrap();
    let bundle = finish_device_login(&client, &meta, "cfait", None, &authz)
        .await
        .expect("polling should survive slow_down");
    assert_eq!(bundle.access_token, "at-sd");
    assert_eq!(bundle.refresh_token, None);

    mock_slow.assert();
    mock_pending.assert();
    mock_ok.assert();
}

// -----------------------------
// Token refresh over real HTTP
// -----------------------------

#[tokio::test]
async fn token_manager_refreshes_over_http_and_caches() {
    let mut server = Server::new_async().await;
    let issuer = server.url();

    let mock_refresh = server
        .mock("POST", "/token")
        .match_body(mockito::Matcher::Regex(
            r"grant_type=refresh_token&refresh_token=rt-old&client_id=cfait".to_string(),
        ))
        .expect(1)
        .with_status(200)
        .with_body(json_body(&token_response("at-new", 3600, Some("rt-new"))))
        .create_async()
        .await;

    let mut bundle = test_bundle("at-old", unix_now() - 10, Some("rt-old"));
    bundle.token_endpoint = format!("{issuer}/token");
    let manager = TokenManager::new(bundle, None);

    let token = manager
        .get_valid_token()
        .await
        .expect("expired token should be refreshed");
    assert_eq!(token, "at-new");

    // The refreshed bundle is now valid: served from memory, no HTTP call.
    let again = manager.get_valid_token().await.unwrap();
    assert_eq!(again, "at-new");

    mock_refresh.assert();
}

#[tokio::test]
async fn token_manager_refresh_error_propagates() {
    let mut server = Server::new_async().await;
    let issuer = server.url();

    let _mock_refresh = server
        .mock("POST", "/token")
        .with_status(400)
        .with_body(json_body(&serde_json::json!({
            "error": "invalid_grant",
            "error_description": "refresh token revoked"
        })))
        .create_async()
        .await;

    let mut bundle = test_bundle("at-old", unix_now() - 10, Some("rt-revoked"));
    bundle.token_endpoint = format!("{issuer}/token");
    let manager = TokenManager::new(bundle, None);

    let err = manager.get_valid_token().await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("invalid_grant") && msg.contains("refresh token revoked"),
        "error should carry the provider message, got: {msg}"
    );
}

// -----------------------------
// Keyring persistence
// -----------------------------

#[test]
fn keyring_bundle_round_trip() {
    cfait::system::init_keyring();
    let key = format!("oidc-test-{}", uuid::Uuid::new_v4());
    let bundle = test_bundle("at-k", unix_now() + 3600, Some("rt-k"));

    if save_bundle(&key, &bundle).is_err() {
        eprintln!("keyring unavailable in this environment; skipping keyring round-trip");
        return;
    }
    let loaded = load_bundle(&key).expect("saved bundle should load");
    assert_eq!(loaded.access_token, bundle.access_token);
    assert_eq!(loaded.refresh_token, bundle.refresh_token);
    assert_eq!(loaded.token_endpoint, bundle.token_endpoint);
    assert_eq!(loaded.client_id, bundle.client_id);
    assert_eq!(loaded.expires_at, bundle.expires_at);

    clear_bundle(&key);
    assert!(load_bundle(&key).is_none());
}

// -----------------------------
// End-to-end: CalDAV request with resolved auth header
// -----------------------------

const DAV_OK: &str = r#"<d:multistatus xmlns:d="DAV:">
    <d:response>
        <d:href>/calendar/test.ics</d:href>
        <d:propstat><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
    </d:response>
</d:multistatus>"#;

#[test]
fn rusty_client_oidc_sends_bearer_header() {
    cfait::system::init_keyring();
    // The keyring path calls `block_in_place`, which requires a
    // multi-threaded runtime.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let mut server = Server::new_async().await;
        let issuer = server.url();

        let mut config = Config::default();
        config.url = issuer.clone();
        config.auth_mode = AuthMode::Oidc;
        config.oidc_issuer = issuer.clone();
        config.oidc_client_id = "cfait".into();

        let bundle = test_bundle("at-caldav", unix_now() + 3600, Some("rt-caldav"));
        if save_bundle(&config.oidc_keyring_key(), &bundle).is_err() {
            eprintln!("keyring unavailable in this environment; skipping OIDC CalDAV e2e");
            return;
        }

        let mock_propfind = server
            .mock("PROPFIND", "/")
            .match_header("authorization", "Bearer at-caldav")
            .with_status(207)
            .with_body(DAV_OK)
            .create_async()
            .await;

        let ctx = Arc::new(TestContext::new());
        let client = RustyClient::from_config(ctx, &config, None).expect("client should build");
        client
            .discover_calendar()
            .await
            .expect("PROPFIND should succeed with the OIDC bearer token");

        mock_propfind.assert();
        clear_bundle(&config.oidc_keyring_key());
    });
}

#[test]
fn rusty_client_bearer_sends_static_token() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let mut server = Server::new_async().await;

        let mut config = Config::default();
        config.url = server.url();
        config.auth_mode = AuthMode::Bearer;
        config.password = "static-token-123".into();

        let mock_propfind = server
            .mock("PROPFIND", "/")
            .match_header("authorization", "Bearer static-token-123")
            .with_status(207)
            .with_body(DAV_OK)
            .create_async()
            .await;

        let ctx = Arc::new(TestContext::new());
        let client = RustyClient::from_config(ctx, &config, None).expect("client should build");
        client
            .discover_calendar()
            .await
            .expect("PROPFIND should succeed with the static bearer token");

        mock_propfind.assert();
    });
}
