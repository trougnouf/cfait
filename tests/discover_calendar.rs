// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for calendar discovery.
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
    assert!(
        result.is_ok() && result.as_ref().unwrap() == &base_path.to_string(),
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
    assert!(
        result.is_ok() && result.as_ref().unwrap() == &primary_calendar_path.to_string(),
        "Should return the discovered calendar path on fallback success"
    );

    // Verify the full chain of discovery requests was made.
    mock_list.assert();
    mock_principal.assert();
    mock_home_set.assert();
    mock_calendars.assert();
}

// Helper: mock a single-collection PROPFIND response carrying the given
// supported-calendar-component-set and current-user-privilege-set bodies,
// then return what get_supported_components() parsed out of it. Shared by the
// privilege tests below to avoid repeating the mock-and-parse setup three times.
async fn supported_components_for(privilege_set: &str, component_set: &str) -> (Vec<String>, bool) {
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/dav/calendars/user/tasks/";

    let body = format!(
        r#"
        <d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
            <d:response>
                <d:href>{}</d:href>
                <d:propstat>
                    <d:prop>
                        {}
                        {}
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>
    "#,
        cal_path, component_set, privilege_set
    );

    let mock = server
        .mock("PROPFIND", cal_path)
        .with_status(207)
        .with_body(body)
        .create_async()
        .await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx, &format!("{}{}", url, cal_path), "u", "p", false, None)
        .expect("Client creation failed");

    let result = client
        .get_supported_components(cal_path)
        .await
        .expect("get_supported_components failed");

    // Verify the PROPFIND request was actually made.
    mock.assert();

    result
}

const VTODO_COMPONENT_SET: &str = r#"
    <c:supported-calendar-component-set>
        <c:comp name="VEVENT"/>
        <c:comp name="VTODO"/>
    </c:supported-calendar-component-set>"#;

// Regression test: servers such as Xandikos advertise write access via the
// DAV:all aggregate privilege (RFC 3744 section 3.13) rather than an explicit
// <write/> element. Such collections must be treated as writable, otherwise
// they are dropped from discovery and no remote task lists appear.
#[tokio::test]
async fn test_supported_components_dav_all_is_writable() {
    let privilege_set = r#"
        <d:current-user-privilege-set>
            <d:privilege><d:all/></d:privilege>
        </d:current-user-privilege-set>"#;

    let (components, can_write) =
        supported_components_for(privilege_set, VTODO_COMPONENT_SET).await;

    assert!(
        can_write,
        "DAV:all aggregate privilege must be treated as writable"
    );
    assert!(
        components.iter().any(|c| c == "VTODO"),
        "VTODO component should be parsed from the component set"
    );
}

// A privilege set that grants only read access must keep can_write false so
// genuinely read-only collections are still filtered out of discovery.
#[tokio::test]
async fn test_supported_components_read_only_is_not_writable() {
    let privilege_set = r#"
        <d:current-user-privilege-set>
            <d:privilege><d:read/></d:privilege>
        </d:current-user-privilege-set>"#;

    let (_components, can_write) =
        supported_components_for(privilege_set, VTODO_COMPONENT_SET).await;

    assert!(
        !can_write,
        "a read-only privilege set must not be treated as writable"
    );
}

// When the server advertises no current-user-privilege-set at all, the client
// assumes the collection is writable rather than hiding it.
#[tokio::test]
async fn test_supported_components_absent_privilege_set_defaults_writable() {
    let (_components, can_write) = supported_components_for("", VTODO_COMPONENT_SET).await;

    assert!(
        can_write,
        "an absent privilege set should default to writable"
    );
}
