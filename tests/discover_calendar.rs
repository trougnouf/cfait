// File: tests/discover_calendar.rs
use cfait::client::core::RustyClient;
use cfait::context::TestContext;
use mockito::Server;
use std::sync::Arc;

#[tokio::test]
async fn test_discover_calendar_fast_heuristic_success() {
    let mut server = Server::new_async().await;
    let url = server.url();
    let base_path = "/dav/calendars/user/";

    // Mock the initial PROPFIND to list resources, including one ending in .ics.
    // This should trigger the fast heuristic and stop further discovery.
    let mock_list = server
        .mock("PROPFIND", base_path)
        .with_status(207)
        .with_body(
            r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>/dav/calendars/user/photos/</d:href>
                </d:response>
                <d:response>
                    <d:href>/dav/calendars/user/tasks.ics</d:href>
                </d:response>
            </d:multistatus>
        "#,
        )
        .create_async()
        .await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx, &format!("{}{}", url, base_path), "u", "p", false, None)
        .expect("Client creation failed");

    let result = client.discover_calendar().await;

    // The client should return its base path because it found an .ics file within it.
    assert_eq!(
        result,
        Ok(base_path.to_string()),
        "Should return base_path on fast heuristic success"
    );

    // Verify only the initial ListResources PROPFIND was made.
    mock_list.assert();
}

#[tokio::test]
async fn test_discover_calendar_full_fallback_success() {
    let mut server = Server::new_async().await;
    let url = server.url();
    let base_path = "/"; // Start discovery from the root

    // 1. Mock the initial PROPFIND to return no .ics files, forcing a fallback.
    let mock_list = server
        .mock("PROPFIND", base_path)
        .with_status(207)
        .with_body(
            r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>/some-other-folder/</d:href>
                </d:response>
            </d:multistatus>
        "#,
        )
        .create_async()
        .await;

    // 2. Mock the current-user-principal discovery.
    let principal_path = "/principals/users/testuser/";
    let mock_principal = server
        .mock("PROPFIND", base_path)
        .match_body(mockito::Matcher::Regex(
            "current-user-principal".to_string(),
        ))
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>/</d:href>
                    <d:propstat>
                        <d:prop>
                            <d:current-user-principal>
                                <d:href>{}</d:href>
                            </d:current-user-principal>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            principal_path
        ))
        .create_async()
        .await;

    // 3. Mock the calendar-home-set discovery.
    let home_set_path = "/calendars/testuser/";
    let mock_home_set = server
        .mock("PROPFIND", principal_path)
        .match_body(mockito::Matcher::Regex("calendar-home-set".to_string()))
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop>
                            <c:calendar-home-set>
                                <d:href>{}</d:href>
                            </c:calendar-home-set>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            principal_path, home_set_path
        ))
        .create_async()
        .await;

    // 4. Mock the final calendar discovery.
    let primary_calendar_path = "/calendars/testuser/primary/";
    let mock_calendars = server
        .mock("PROPFIND", home_set_path)
        .match_body(mockito::Matcher::Regex("resourcetype".to_string()))
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop>
                            <d:resourcetype><c:calendar/></d:resourcetype>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            primary_calendar_path
        ))
        .create_async()
        .await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx, &format!("{}{}", url, base_path), "u", "p", false, None)
        .expect("Client creation failed");

    let result = client.discover_calendar().await;

    // The client should return the path of the first calendar it found in the home-set.
    assert_eq!(
        result,
        Ok(primary_calendar_path.to_string()),
        "Should return the discovered calendar path on fallback success"
    );

    // Verify the full chain of discovery requests was made.
    mock_list.assert();
    mock_principal.assert();
    mock_home_set.assert();
    mock_calendars.assert();
}
