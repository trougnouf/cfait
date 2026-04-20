// SPDX-License-Identifier: GPL-3.0-or-later
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

    let (action_tx, action_rx) = mpsc::channel(10);
    let (event_tx, mut event_rx) = mpsc::channel(20);

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

    action_tx
        .send(Action::CreateTask(new_task))
        .await
        .expect("Failed to send create action");

    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(3), event_rx.recv()).await {
            Ok(Some(AppEvent::TasksLoaded(results))) => {
                if let Some((_, tasks)) = results.iter().find(|(href, _)| href == &cal_href) {
                    let uids: std::collections::HashSet<_> =
                        tasks.iter().map(|t| t.uid.as_str()).collect();
                    if uids.contains("existing-task") && uids.contains("new-task") {
                        break;
                    }
                }
            }
            Ok(Some(AppEvent::Error(e))) => panic!("Actor returned error: {}", e),
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed during create flow"),
            Err(_) => panic!("Timeout waiting for post-create task snapshot"),
        }
    }

    m_put.assert();

    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
}
