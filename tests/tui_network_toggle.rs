use cfait::model::{Task, TaskStatus};
use cfait::tui::action::{Action, AppEvent};
use cfait::tui::network::run_network_actor;
use mockito::Server;
use std::collections::HashMap;
use std::env;
use std::fs;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tui_toggle_task_does_not_revert_status() {
    // 1. Setup isolated environment
    let temp_dir = env::temp_dir().join(format!("cfait_tui_toggle_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    unsafe {
        env::set_var("CFAIT_TEST_DIR", &temp_dir);
    }

    // 2. Setup Mock Server
    let mut server = Server::new_async().await;
    let url = server.url();

    // Mock basic PROPFIND for calendar discovery to allow client to start without error
    let _m_propfind = server
        .mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    // The critical expectation: A PUT request updating the task to COMPLETED
    // If the bug exists, the body will contain "STATUS:NEEDS-ACTION" (reverted)
    let task_path = "/test.ics";
    let m_put = server
        .mock("PUT", task_path)
        .match_body(mockito::Matcher::Regex("STATUS:COMPLETED".to_string()))
        .with_status(201)
        .create_async()
        .await;

    // 3. Prepare the Action
    // The handler has already toggled this task to Completed in the UI
    let mut task = Task::new("Toggle Me", &HashMap::new(), None);
    task.uid = "test-uid".to_string();
    task.status = TaskStatus::Completed;
    task.calendar_href = url.clone();
    task.href = format!("{}{}", url, task_path);
    task.etag = "etag".to_string();

    // 4. Run the Network Actor
    let (action_tx, action_rx) = mpsc::channel(10);
    let (event_tx, mut event_rx) = mpsc::channel(10);

    let actor_handle = tokio::spawn(async move {
        run_network_actor(
            url,
            "user".into(),
            "pass".into(),
            true,
            None,
            action_rx,
            event_tx,
        )
        .await;
    });

    // Wait for initial events (connection status) to clear
    let _ = event_rx.recv().await;

    // Send the Toggle Action with the Completed task
    action_tx
        .send(Action::ToggleTask(task))
        .await
        .expect("Failed to send action");

    // Wait for completion (Status: Synced.)
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv()).await {
            Ok(Some(AppEvent::Status(s))) => {
                if s == "Synced." {
                    break;
                }
            }
            Ok(Some(AppEvent::Error(e))) => panic!("Network actor returned error: {}", e),
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed"),
            Err(_) => panic!("Timeout waiting for sync confirmation"),
        }
    }

    // 5. Verify Mock
    // This asserts the network actor sent COMPLETED, not NEEDS-ACTION
    m_put.assert();

    // Cleanup
    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
    unsafe {
        env::remove_var("CFAIT_TEST_DIR");
    }
    let _ = fs::remove_dir_all(&temp_dir);
}
