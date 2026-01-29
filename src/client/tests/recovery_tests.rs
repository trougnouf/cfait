// File: ./src/client/tests/recovery_tests.rs
// Recovery-related unit tests moved out of the large `core.rs` test block.
//
// This file is included by `src/client/core.rs` inside the `#[cfg(test)]`
// `mod tests { ... mod recovery_tests; }` block, so the module hierarchy is:
//
// crate::client::core::tests::recovery_tests
//
// To access items declared in the core module we import via `super::super::*`.
// To access helpers declared in the tests module (like `TestGuard`) we import
// `super::TestGuard`.
// Rely on the including `tests` module to provide the appropriate imports and
// test helpers (avoid re-importing to prevent duplicate symbol definitions).

/// Test that a failing sync action is moved into the local recovery calendar.
///
/// This test relies on the test-only hook `TEST_FORCE_SYNC_ERROR` defined in
/// `core.rs` to simulate a fatal server response for a particular Action.
#[test]
#[serial]
fn test_sync_journal_moves_failed_task_to_recovery() {
    // Use an explicit TestContext for filesystem isolation (no global env var mutation)
    let ctx = std::sync::Arc::new(crate::context::TestContext::new());

    // Ensure registry does not contain recovery calendar initially
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    regs.retain(|c| c.href != "local://recovery");
    LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();

    // Create a task and push a Create action into the journal
    let mut task = Task::new("WillFail", &HashMap::new(), None);
    task.uid = "sync-err-1".to_string();
    task.calendar_href = "https://example.com/cal/".to_string();
    task.summary = "Should fail and be recovered".to_string();

    Journal::push(ctx.as_ref(), Action::Create(task.clone())).unwrap();

    // Install test force hook to cause sync_journal to treat this action as a 403 error
    let hook = TEST_FORCE_SYNC_ERROR.get_or_init(|| Mutex::new(None));
    {
        let mut g = hook.lock().unwrap();
        *g = Some(Box::new(move |action: &Action| match action {
            Action::Create(t) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
            Action::Update(t) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
            Action::Move(t, _) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
            _ => None,
        }));
    }

    let client = RustyClient::new(
        ctx.clone(),
        "http://dummy.test",
        "user",
        "pass",
        false,
        None,
    )
    .unwrap();
    // Run sync_journal; the test hook will simulate a fatal server error and the task should be moved
    let warnings = futures::executor::block_on(client.sync_journal()).unwrap();

    // Expect at least one warning about the recovery move
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("Recovery") || w.contains("Local (Recovery)")),
        "Expected a warning about recovery"
    );

    // Recovery storage should contain the task
    let recovered = LocalStorage::load_for_href(ctx.as_ref(), "local://recovery").unwrap();
    assert!(
        recovered.iter().any(|t| t.uid == "sync-err-1"),
        "Task should be moved to recovery"
    );

    // Cleanup hooks
    if let Some(h) = TEST_FETCH_REMOTE_HOOK.get() {
        *h.lock().unwrap() = None;
    }
    if let Some(h) = TEST_FORCE_SYNC_ERROR.get() {
        *h.lock().unwrap() = None;
    }
}

/// Test that the recovery calendar is only visible when it contains tasks.
#[tokio::test]
#[serial]
async fn test_recovery_calendar_visibility() {
    // Use an explicit TestContext for filesystem isolation (no global env var mutation)
    let ctx = std::sync::Arc::new(crate::context::TestContext::new());

    // Ensure registry does not contain recovery calendar initially
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    regs.retain(|c| c.href != "local://recovery");
    LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();

    // Use an offline client for this visibility test; attach the test ctx so client-side code
    // that expects a context can access it if needed.
    let client = RustyClient {
        client: None,
        ctx: ctx.clone(),
    };

    // When recovery calendar is not present and no tasks exist, it should not be visible
    let cals = client.get_calendars().await.unwrap();
    assert!(
        !cals.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should not be visible when absent and empty"
    );

    // Add recovery calendar entry to registry but no tasks -> still should not be visible
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    if !regs.iter().any(|c| c.href == "local://recovery") {
        regs.push(CalendarListEntry {
            name: "Local (Recovery)".to_string(),
            href: "local://recovery".to_string(),
            color: Some("#DB4437".to_string()),
        });
        LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();
    }

    let cals2 = client.get_calendars().await.unwrap();
    assert!(
        !cals2.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should not be visible when empty"
    );

    // Add a recovered task to the recovery storage -> calendar should become visible
    let mut task = Task::new("Recovered", &HashMap::new(), None);
    task.uid = "rec-1".to_string();
    task.calendar_href = "local://recovery".to_string();
    LocalStorage::save_for_href(
        ctx.as_ref(),
        "local://recovery",
        std::slice::from_ref(&task),
    )
    .unwrap();

    let cals3 = client.get_calendars().await.unwrap();
    assert!(
        cals3.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should be visible when it contains tasks"
    );
}
