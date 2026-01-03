// Tests for HTTP Digest authentication.
use cfait::client::RustyClient;
use mockito::Server;

#[tokio::test]
async fn test_client_handles_digest_auth_challenge() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let digest_header = r#"Digest realm="Test Realm", qop="auth", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", opaque="5ccc069c403ebaf9f0171e9517f40e41""#;

    // Mock 1: Initial request (Basic Auth) -> 401 with Digest Challenge
    let mock_unauthorized = server
        .mock("PROPFIND", "/")
        .with_status(401)
        .with_header("WWW-Authenticate", digest_header)
        .create_async()
        .await;

    // Mock 2: Retry -> Success (207 Multi-Status)
    // IMPORTANT: We provide a body containing a ".ics" resource.
    // This tells `discover_calendar` that it found a calendar, so it returns immediately
    // instead of trying subsequent fallback discovery requests (which causes the test to fail).
    let success_body = r#"
        <d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/calendar/test.ics</d:href>
                <d:propstat>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>
    "#;

    let mock_authorized = server
        .mock("PROPFIND", "/")
        .match_header(
            "Authorization",
            mockito::Matcher::Regex(r#"Digest.*response="[0-9a-f]{32}""#.to_string()),
        )
        .with_status(207)
        .with_body(success_body)
        .create_async()
        .await;

    let client = RustyClient::new(&url, "user", "pass", false).unwrap();
    let _ = client.discover_calendar().await;

    mock_unauthorized.assert();
    mock_authorized.assert();
}
