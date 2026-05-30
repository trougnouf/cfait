// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for TUI network create.
use cfait::cache::Cache;
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, Task};
use cfait::tui::action::{Action, AppEvent};
use cfait::tui::network::{NetworkActorConfig, run_network_actor};
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tui_create_preserves_existing_calendar_tasks() {
    let test_ctx = TestContext::new();
    let ctx = Arc::new(test_ctx);

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_href = format!("{}/cal/", url);

    Cache::save_calendars(
        ctx.as_ref(),
        &[CalendarListEntry {
            name: "Remote".to_string(),
            href: cal_href.clone(),
            color: None,
        }],
    )
    .unwrap();

    let mut existing = Task::new("Existing", &HashMap::new(), None);
    existing.uid = "existing-task".to_string();
    existing.calendar_href = cal_href.clone();
    existing.href = format!("{}existing-task.ics", cal_href);
    existing.etag = "\"existing-etag\"".to_string();
    Cache::save(
        ctx.as_ref(),
        &cal_href,
        &[existing],
        Some("cached-token".to_string()),
    )
    .unwrap();

    let _m_root = server
        .mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let new_uid = "new-task";
    let put_path = format!("/cal/{}.ics", new_uid);
    let m_put = server
        .mock("PUT", put_path.as_str())
        .match_header("If-None-Match", "*")
        .with_status(201)
        .with_header("etag", "\"new-etag\"")
        .create_async()
        .await;

    // Mocks for Refresh action
    let _m_root_refresh = server
        .mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/</d:href>
                <d:propstat>
                    <d:prop>
                        <d:current-user-principal><d:href>/principal/</d:href></d:current-user-principal>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>"#)
        .create_async()
        .await;

    let _m_home_refresh = server
        .mock("PROPFIND", "/principal/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/principal/</d:href>
                <d:propstat>
                    <d:prop>
                        <c:calendar-home-set xmlns:c="urn:ietf:params:xml:ns:caldav"><d:href>/cal/</d:href></c:calendar-home-set>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>"#)
        .create_async()
        .await;

    let _m_calendars_refresh = server
        .mock("PROPFIND", "/cal/")
        .with_status(207)
        .with_body(
            r#"<d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/cal/</d:href>
                <d:propstat>
                    <d:prop>
                        <d:resourcetype>
                            <d:collection/>
                            <c:calendar xmlns:c="urn:ietf:params:xml:ns:caldav"/>
                        </d:resourcetype>
                        <d:displayname>Remote</d:displayname>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>"#,
        )
        .create_async()
        .await;

    // Mock for get_supported_components during refresh
    let _m_components_refresh = server
        .mock("PROPFIND", "/cal/")
        .with_status(207)
        .with_body(
            r#"<d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/cal/</d:href>
                <d:propstat>
                    <d:prop>
                        <c:supported-calendar-component-set xmlns:c="urn:ietf:params:xml:ns:caldav">
                            <c:comp name="VTODO"/>
                        </c:supported-calendar-component-set>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>"#,
        )
        .create_async()
        .await;

    let _m_tasks_refresh = server
        .mock("PROPFIND", "/cal/")
        .with_status(207)
        .with_body(
            r#"<d:multistatus xmlns:d="DAV:">
            <d:response>
                <d:href>/cal/existing-task.ics</d:href>
                <d:propstat>
                    <d:prop>
                        <d:getetag>"existing-etag"</d:getetag>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
            <d:response>
                <d:href>/cal/new-task.ics</d:href>
                <d:propstat>
                    <d:prop>
                        <d:getetag>"new-etag"</d:getetag>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
                </d:response>
        </d:multistatus>"#,
        )
        .create_async()
        .await;

    let _m_report_refresh = server
        .mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(
            r#"<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
            <d:response>
                <d:href>/cal/new-task.ics</d:href>
                <d:propstat>
                    <d:prop>
                        <d:getetag>"new-etag"</d:getetag>
                        <cal:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:new-task
SUMMARY:New
END:VTODO
END:VCALENDAR</cal:calendar-data>
                    </d:prop>
                    <d:status>HTTP/1.1 200 OK</d:status>
                </d:propstat>
            </d:response>
        </d:multistatus>"#,
        )
        .create_async()
        .await;

    let (action_tx, action_rx) = mpsc::channel(10);
    let (event_tx, mut event_rx) = mpsc::channel(20);

    let mut app_config = cfait::config::Config::default();
    app_config.sync_settings = false;
    app_config.save(ctx.as_ref()).unwrap();

    let config = NetworkActorConfig {
        url: url.clone(),
        user: "user".into(),
        pass: "pass".into(),
        allow_insecure: true,
        enable_local_mode: true,
        default_cal: None,
    };

    let actor_handle = tokio::spawn(async move {
        run_network_actor(ctx.clone(), config, action_rx, event_tx).await;
    });

    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv()).await {
            Ok(Some(AppEvent::Status { key, .. })) if key == "ready" => break,
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed before startup completed"),
            Err(_) => panic!("Timeout waiting for actor startup"),
        }
    }

    let mut new_task = Task::new("New", &HashMap::new(), None);
    new_task.uid = new_uid.to_string();
    new_task.calendar_href = cal_href.clone();
    new_task.href = format!("{}{}.ics", cal_href, new_uid);
    new_task.etag = String::new();
    new_task.sequence = 0;

    action_tx
        .send(Action::PersistBatch(vec![cfait::journal::Action::Create(
            new_task,
        )]))
        .await
        .expect("Failed to send create action");

    let mut synced_new_task = false;

    // Wait for the status saved and task synced events
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(5), event_rx.recv()).await {
            Ok(Some(AppEvent::TaskSynced { uid, .. })) => {
                if uid == new_uid {
                    synced_new_task = true;
                }
            }
            Ok(Some(AppEvent::Status { key, .. })) if key == "status_saved" => {
                assert!(synced_new_task, "New task should have been synced!");
                break;
            }
            Ok(Some(AppEvent::Error(e))) => panic!("Actor returned error: {}", e),
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed"),
            Err(_) => panic!("Timeout"),
        }
    }

    m_put.assert();

    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
}
