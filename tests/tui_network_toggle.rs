// File: ./tests/tui_network_toggle.rs
// Integration tests for TUI network task toggling.
use cfait::context::TestContext;
use cfait::model::{Task, TaskStatus};
use cfait::tui::action::{Action, AppEvent};
use cfait::tui::network::{NetworkActorConfig, run_network_actor};
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tui_toggle_task_does_not_revert_status() {
    // 1. Setup isolated test context
    let test_ctx = TestContext::new();
    let ctx = Arc::new(test_ctx);

    // 2. Setup Mock Server
    let mut server = Server::new_async().await;
    let url = server.url();

    let _m_propfind = server
        .mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let task_path = "/test.ics";
    let m_put = server
        .mock("PUT", task_path)
        .match_body(mockito::Matcher::Regex("STATUS:COMPLETED".to_string()))
        .with_status(201)
        .create_async()
        .await;

    // 3. Prepare the Action
    let mut task = Task::new("Toggle Me", &HashMap::new(), None);
    task.uid = "test-uid".to_string();
    task.status = TaskStatus::Completed;
    task.calendar_href = url.clone();
    task.href = format!("{}{}", url, task_path);
    task.etag = "etag".to_string();

    // 4. Run the Network Actor
    let (action_tx, action_rx) = mpsc::channel(10);
    let (event_tx, mut event_rx) = mpsc::channel(10);

    // --- FIX: Wrap connection details in NetworkActorConfig ---
    let config = NetworkActorConfig {
        url: url.clone(),
        user: "user".into(),
        pass: "pass".into(),
        allow_insecure: true,
        default_cal: None,
    };

    let actor_handle = tokio::spawn(async move {
        run_network_actor(ctx.clone(), config, action_rx, event_tx).await;
    });

    // Wait for initial events
    let _ = event_rx.recv().await;

    action_tx
        .send(Action::UpdateTask(task))
        .await
        .expect("Failed to send action");

    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv()).await {
            Ok(Some(AppEvent::Status { key, human })) => {
                // Prefer asserting on the stable key; accept the canonical English human string as fallback.
                if key == "status_saved" || human == "Saved." {
                    break;
                }
            }
            Ok(Some(AppEvent::Error(e))) => panic!("Network actor returned error: {}", e),
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed"),
            Err(_) => panic!("Timeout waiting for sync confirmation"),
        }
    }

    m_put.assert();

    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
}
