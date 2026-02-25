// File: ./src/client/sync.rs
//! Journal synchronization logic for the CalDAV client.
//! This module contains the implementation for processing the offline action queue.
use crate::client::core::{HttpsClient, RustyClient, strip_host};
use crate::journal::{Action, Journal};
use crate::model::merge::three_way_merge;
use crate::model::{CalendarListEntry, IcsAdapter, Task};
use crate::storage::{LocalCalendarRegistry, LocalStorage};

use http::{Request, StatusCode};
use libdav::caldav::CalDavClient;
use libdav::dav::WebDavError;
use libdav::dav::{Delete, PutResource};
use std::mem;

#[cfg(any(test, feature = "test_hooks"))]
use crate::client::core::test_hooks::TEST_FORCE_SYNC_ERROR;

// --- Internal Sync Outcome Types ---
enum StepOutcome {
    Success {
        etag: Option<String>,
        href: Option<String>,
        refresh_path: Option<String>,
    },
    RetryWith(Box<Action>),
    Discard,
    RecoveryNeeded(String),
}

struct StepResult {
    outcome: StepOutcome,
    warnings: Vec<String>,
}

impl StepResult {
    fn new(outcome: StepOutcome) -> Self {
        Self {
            outcome,
            warnings: Vec::new(),
        }
    }

    fn with_warning(mut self, w: String) -> Self {
        self.warnings.push(w);
        self
    }
}

fn actions_match_identity(a: &Action, b: &Action) -> bool {
    match (a, b) {
        (Action::Create(t1), Action::Create(t2)) => t1.uid == t2.uid,
        (Action::Update(t1), Action::Update(t2)) => t1.uid == t2.uid && t1.sequence == t2.sequence,
        (Action::Delete(t1), Action::Delete(t2)) => t1.uid == t2.uid,
        (Action::Move(t1, d1), Action::Move(t2, d2)) => t1.uid == t2.uid && d1 == d2,
        _ => false,
    }
}

// --- Sync Logic as RustyClient methods ---
impl RustyClient {
    async fn handle_create(
        &self,
        client: &CalDavClient<HttpsClient>,
        task: &Task,
    ) -> Result<StepResult, String> {
        let filename = format!("{}.ics", task.uid);
        let full_href = if task.calendar_href.ends_with('/') {
            format!("{}{}", task.calendar_href, filename)
        } else {
            format!("{}/{}", task.calendar_href, filename)
        };
        let path = strip_host(&full_href);
        let ics_string = IcsAdapter::to_ics(task);

        match client
            .request(PutResource::new(&path).create(ics_string, "text/calendar"))
            .await
        {
            Ok(resp) => {
                let outcome = StepOutcome::Success {
                    etag: resp.etag,
                    href: Some(full_href),
                    refresh_path: Some(path),
                };
                Ok(StepResult::new(outcome))
            }
            Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
            | Err(WebDavError::PreconditionFailed(_)) => {
                // Conflict: already exists. Mark successful to remove from queue.
                Ok(StepResult::new(StepOutcome::Success {
                    etag: None,
                    href: None,
                    refresh_path: Some(path),
                })
                .with_warning(format!(
                    "Creation conflict: Task '{}' already exists on server. Mark as synced.",
                    task.summary
                )))
            }
            Err(e) => {
                let msg = format!("{:?}", e);
                if msg.contains("403") || msg.contains("400") || msg.contains("415") {
                    return Ok(StepResult::new(StepOutcome::RecoveryNeeded(msg)));
                }
                if msg.contains("413") {
                    return Ok(StepResult::new(StepOutcome::Discard).with_warning(msg));
                }
                Err(msg)
            }
        }
    }

    async fn handle_update(
        &self,
        client: &CalDavClient<HttpsClient>,
        task: &Task,
    ) -> Result<StepResult, String> {
        // Handle "Ghost Update" where href is missing.
        // Reconstruct the correct path from the calendar_href and UID.
        let path = if task.href.is_empty() {
            let filename = format!("{}.ics", task.uid);
            let cal_path = strip_host(&task.calendar_href);
            if cal_path.ends_with('/') {
                format!("{}{}", cal_path, filename)
            } else {
                format!("{}/{}", cal_path, filename)
            }
        } else {
            strip_host(&task.href)
        };

        let ics_string = IcsAdapter::to_ics(task);
        let etag_val = if task.etag == "pending_refresh" {
            ""
        } else {
            &task.etag
        };

        match client
            .request(PutResource::new(&path).update(
                ics_string,
                "text/calendar; charset=utf-8; component=VTODO",
                etag_val,
            ))
            .await
        {
            Ok(resp) => {
                // If we reconstructed the path, we must ensure we update the HREF in the success outcome
                // so subsequent actions use the correct path.
                let new_href = if task.href.is_empty() {
                    let filename = format!("{}.ics", task.uid);
                    if task.calendar_href.ends_with('/') {
                        Some(format!("{}{}", task.calendar_href, filename))
                    } else {
                        Some(format!("{}/{}", task.calendar_href, filename))
                    }
                } else {
                    None
                };

                Ok(StepResult::new(StepOutcome::Success {
                    etag: resp.etag,
                    href: new_href,
                    refresh_path: Some(path),
                }))
            }
            Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
            | Err(WebDavError::PreconditionFailed(_)) => {
                if let Some((resolution, msg)) = self.attempt_conflict_resolution(task).await {
                    Ok(
                        StepResult::new(StepOutcome::RetryWith(Box::new(resolution)))
                            .with_warning(msg),
                    )
                } else {
                    // Force copy logic
                    let mut conflict_copy = task.clone();
                    conflict_copy.uid = uuid::Uuid::new_v4().to_string();
                    conflict_copy.summary = format!("{} (Conflict Copy)", task.summary);
                    conflict_copy.href = String::new();
                    conflict_copy.etag = String::new();
                    Ok(
                        StepResult::new(StepOutcome::RetryWith(Box::new(Action::Create(
                            conflict_copy,
                        ))))
                        .with_warning(format!(
                            "Conflict (412) on task '{}'. Merge failed. Creating copy.",
                            task.summary
                        )),
                    )
                }
            }
            Err(WebDavError::BadStatusCode(StatusCode::NOT_FOUND)) => {
                // Resurrect if missing
                Ok(StepResult::new(StepOutcome::RetryWith(Box::new(
                    Action::Create(task.clone()),
                ))))
            }
            Err(e) => {
                let msg = format!("{:?}", e);
                if msg.contains("412") || msg.contains("PreconditionFailed") {
                    let mut conflict_copy = task.clone();
                    conflict_copy.uid = uuid::Uuid::new_v4().to_string();
                    conflict_copy.summary = format!("{} (Conflict Copy)", task.summary);
                    conflict_copy.href = String::new();
                    conflict_copy.etag = String::new();
                    Ok(
                        StepResult::new(StepOutcome::RetryWith(Box::new(Action::Create(
                            conflict_copy,
                        ))))
                        .with_warning(format!(
                            "Conflict (412-Fallback) on task '{}'. Creating copy.",
                            task.summary
                        )),
                    )
                } else if msg.contains("403") || msg.contains("400") || msg.contains("415") {
                    Ok(StepResult::new(StepOutcome::RecoveryNeeded(msg)))
                } else if msg.contains("413") {
                    Ok(StepResult::new(StepOutcome::Discard).with_warning(msg))
                } else {
                    Err(msg)
                }
            }
        }
    }

    async fn handle_delete(
        &self,
        client: &CalDavClient<HttpsClient>,
        task: &Task,
    ) -> Result<StepResult, String> {
        if task.href.is_empty() {
            return Ok(StepResult::new(StepOutcome::Discard));
        }
        let path = strip_host(&task.href);

        let resp = if !task.etag.is_empty() && task.etag != "pending_refresh" {
            client
                .request(Delete::new(&path).with_etag(&task.etag))
                .await
        } else {
            client.request(Delete::new(&path).force()).await
        };

        match resp {
            Ok(_) => Ok(StepResult::new(StepOutcome::Success {
                etag: None,
                href: None,
                refresh_path: None,
            })),
            Err(WebDavError::BadStatusCode(StatusCode::NOT_FOUND)) => {
                Ok(StepResult::new(StepOutcome::Discard))
            }
            Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED)) => {
                Ok(StepResult::new(StepOutcome::Success {
                    etag: None,
                    href: None,
                    refresh_path: None,
                })
                .with_warning(format!(
                    "Conflict on delete task '{}'. Already modified/deleted.",
                    task.summary
                )))
            }
            Err(e) => {
                let msg = format!("{:?}", e);
                if msg.contains("403") || msg.contains("400") || msg.contains("415") {
                    // For delete, if we can't delete due to permissions, just discard from queue
                    // to prevent blocking. We can't easily recover a deleted task state.
                    Ok(StepResult::new(StepOutcome::Discard).with_warning(msg))
                } else {
                    Err(msg)
                }
            }
        }
    }

    async fn handle_move(&self, task: &Task, new_cal: &str) -> Result<StepResult, String> {
        let mut move_res = self.execute_move(task, new_cal, false).await;

        if let Err(ref e) = move_res
            && (e.contains("412") || e.contains("PreconditionFailed"))
        {
            move_res = self.execute_move(task, new_cal, true).await;
        }

        match move_res {
            Ok(_) => {
                let filename = format!("{}.ics", task.uid);
                let new_href = if new_cal.ends_with('/') {
                    format!("{}{}", new_cal, filename)
                } else {
                    format!("{}/{}", new_cal, filename)
                };
                Ok(StepResult::new(StepOutcome::Success {
                    etag: None,
                    href: Some(new_href.clone()),
                    refresh_path: Some(strip_host(&new_href)),
                }))
            }
            Err(e) => {
                if e.contains("404") || e.contains("NotFound") || e.contains("403") {
                    Ok(StepResult::new(StepOutcome::Discard).with_warning(format!(
                        "Move source missing for '{}', assuming success.",
                        task.summary
                    )))
                } else if e.contains("400") || e.contains("415") {
                    Ok(StepResult::new(StepOutcome::RecoveryNeeded(e)))
                } else {
                    Err(e)
                }
            }
        }
    }

    pub async fn sync_journal(&self) -> Result<Vec<String>, String> {
        // Load and compact journal once at the start
        let mut journal = Journal::load(self.ctx.as_ref());
        let mut tmp_j = Journal {
            queue: mem::take(&mut journal.queue),
        };
        tmp_j.compact();
        journal.queue = tmp_j.queue;

        let client = self.client.as_ref().ok_or("Offline")?;
        let mut warnings = Vec::new();

        // Load config for the create_events_for_tasks flag
        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
        let events_enabled = config.create_events_for_tasks;
        let delete_on_completion = config.delete_events_on_completion;

        // ADD THIS FLAG:
        // Ensures we only check/create the recovery calendar once per sync cycle.
        let mut recovery_cal_created_this_cycle = false;

        // Process actions in-memory
        while !journal.queue.is_empty() {
            let next_action = journal.queue[0].clone();
            let mut conflict_resolved_action: Option<Action> = None;
            let mut new_etag_to_propagate: Option<String> = None;
            let mut new_href_to_propagate: Option<(String, String)> = None;
            let mut path_for_refresh: Option<String> = None;

            // Allow tests to force a sync error for this action without performing network requests.
            // If the test hook returns Some(error_string) we skip the normal network path and
            // treat the action as having failed with that message.
            let test_forced_err: Option<String> = {
                #[cfg(any(test, feature = "test_hooks"))]
                {
                    if let Some(h) = TEST_FORCE_SYNC_ERROR.get() {
                        if let Some(cb) = &*h.lock().unwrap() {
                            cb(&next_action)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                #[cfg(not(any(test, feature = "test_hooks")))]
                {
                    None
                }
            };

            let step_result = if let Some(err_msg) = test_forced_err {
                if err_msg.contains("400") || err_msg.contains("403") || err_msg.contains("415") {
                    Ok(StepResult::new(StepOutcome::RecoveryNeeded(err_msg)))
                } else if err_msg.contains("413") {
                    Ok(StepResult::new(StepOutcome::Discard).with_warning(err_msg))
                } else {
                    Err(err_msg)
                }
            } else {
                match &next_action {
                    Action::Create(t) => self.handle_create(client, t).await,
                    Action::Update(t) => self.handle_update(client, t).await,
                    Action::Delete(t) => self.handle_delete(client, t).await,
                    Action::Move(t, new_cal) => self.handle_move(t, new_cal).await,
                }
            };

            match step_result {
                Ok(res) => {
                    warnings.extend(res.warnings);

                    match res.outcome {
                        StepOutcome::Success {
                            etag,
                            href,
                            refresh_path,
                        } => {
                            // Sync companion event if successful
                            match &next_action {
                                Action::Move(t, new_cal) => {
                                    // Move companion event too
                                    if events_enabled || t.create_event.is_some() {
                                        // 1. Delete ALL variants from the OLD location by invoking
                                        // sync_companion_event with is_delete_intent = true so it
                                        // cleans up static and session variants.
                                        let _ = self
                                            .sync_companion_event(
                                                t,
                                                events_enabled,
                                                delete_on_completion,
                                                true, // is_delete_intent
                                            )
                                            .await;

                                        // 2. Create updated variants in the NEW location
                                        if let Some(new_h) = &href {
                                            let mut moved_task = t.clone();
                                            moved_task.calendar_href = new_cal.clone();
                                            moved_task.href = new_h.clone();
                                            let _ = self
                                                .sync_companion_event(
                                                    &moved_task,
                                                    events_enabled,
                                                    delete_on_completion,
                                                    false,
                                                )
                                                .await;
                                        }
                                    }
                                }
                                Action::Create(t) | Action::Update(t) => {
                                    self.sync_companion_event(
                                        t,
                                        events_enabled,
                                        delete_on_completion,
                                        false,
                                    )
                                    .await;
                                }
                                Action::Delete(t) => {
                                    self.sync_companion_event(
                                        t,
                                        events_enabled,
                                        delete_on_completion,
                                        true,
                                    )
                                    .await;
                                }
                            }

                            new_etag_to_propagate = etag;
                            path_for_refresh = refresh_path;
                            if let Some(h) = href {
                                let old = match &next_action {
                                    Action::Create(t) => t.href.clone(), // Doesn't matter for create
                                    Action::Move(t, _) => t.href.clone(),
                                    // For Update, propagate if we fixed a missing href
                                    Action::Update(t) => t.href.clone(),
                                    _ => String::new(),
                                };
                                new_href_to_propagate = Some((old, h));
                            }
                        }
                        StepOutcome::RetryWith(act) => {
                            // Dereference the box to get the action
                            conflict_resolved_action = Some(*act);
                            // If we resolved a conflict or are creating a copy, try to sync event for the new state
                            if let Action::Create(t) | Action::Update(t) =
                                &conflict_resolved_action.as_ref().unwrap()
                            {
                                self.sync_companion_event(
                                    t,
                                    events_enabled,
                                    delete_on_completion,
                                    false,
                                )
                                .await;
                            }
                        }
                        StepOutcome::Discard => {
                            // Sync event anyway (e.g. ensure delete on 404)
                            if let Action::Delete(t) = &next_action {
                                self.sync_companion_event(
                                    t,
                                    events_enabled,
                                    delete_on_completion,
                                    true,
                                )
                                .await;
                            }
                        }
                        StepOutcome::RecoveryNeeded(msg) => {
                            // Move to recovery calendar
                            let recovered_task = match &next_action {
                                Action::Create(t) => Some(t.clone()),
                                Action::Update(t) => Some(t.clone()),
                                Action::Move(t, _) => Some(t.clone()),
                                Action::Delete(_) => None,
                            };

                            if let Some(mut task) = recovered_task {
                                let recovery_href = "local://recovery";

                                if !recovery_cal_created_this_cycle {
                                    if let Ok(mut locals) =
                                        LocalCalendarRegistry::load(self.ctx.as_ref())
                                        && !locals.iter().any(|c| c.href == recovery_href)
                                    {
                                        locals.push(CalendarListEntry {
                                            name: "Local (Recovery)".to_string(),
                                            href: recovery_href.to_string(),
                                            color: Some("#DB4437".to_string()),
                                        });
                                        let _ =
                                            LocalCalendarRegistry::save(self.ctx.as_ref(), &locals);
                                    }
                                    recovery_cal_created_this_cycle = true;
                                }

                                task.calendar_href = recovery_href.to_string();
                                task.description
                                    .push_str(&format!("\n\n[Sync Error]: {}", msg));

                                let task_clone = task.clone();
                                let _ = LocalStorage::modify_for_href(
                                    self.ctx.as_ref(),
                                    recovery_href,
                                    |existing| {
                                        existing.push(task_clone);
                                    },
                                );
                                warnings.push(
                                    "Fatal sync error. Task moved to 'Local (Recovery)'."
                                        .to_string(),
                                );
                            }
                            // Fall through to remove action from queue
                        }
                    }

                    // --- Propagate & Cleanup ---
                    if new_etag_to_propagate.is_none()
                        && let Some(path) = path_for_refresh
                        && let Some(fetched) = self.fetch_etag(&path).await
                    {
                        new_etag_to_propagate = Some(fetched);
                    }

                    // Update in-memory queue instead of writing to disk each time
                    let should_remove = if let Some(head) = journal.queue.first() {
                        actions_match_identity(head, &next_action)
                    } else {
                        false
                    };

                    if !should_remove {
                        continue;
                    }

                    journal.queue.remove(0);

                    if let Some(act) = conflict_resolved_action {
                        journal.queue.insert(0, act);
                    }

                    if let Some(etag) = new_etag_to_propagate {
                        let target_uid = match &next_action {
                            Action::Create(t) | Action::Update(t) => t.uid.clone(),
                            Action::Move(t, _) => t.uid.clone(),
                            _ => String::new(),
                        };
                        if !target_uid.is_empty() {
                            for item in journal.queue.iter_mut() {
                                match item {
                                    Action::Update(t) | Action::Delete(t) => {
                                        if t.uid == target_uid {
                                            t.etag = etag.clone();
                                        }
                                    }
                                    Action::Move(t, _) => {
                                        if t.uid == target_uid {
                                            t.etag = etag.clone();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    if let Some((old_href, new_href)) = new_href_to_propagate {
                        let target_uid = match &next_action {
                            Action::Move(t, _) => t.uid.clone(),
                            Action::Create(t) => t.uid.clone(),
                            Action::Update(t) => t.uid.clone(),
                            _ => String::new(),
                        };
                        for item in journal.queue.iter_mut() {
                            match item {
                                Action::Update(t) | Action::Delete(t) => {
                                    // Match by UID or old HREF
                                    if t.uid == target_uid
                                        || (!old_href.is_empty() && t.href == old_href)
                                    {
                                        t.href = new_href.clone();
                                        if let Some(last_slash) = new_href.rfind('/') {
                                            t.calendar_href = new_href[..=last_slash].to_string();
                                        }
                                    }
                                }
                                Action::Move(t, _) => {
                                    if t.uid == target_uid {
                                        t.href = new_href.clone();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(msg) => {
                    // Check for fatal errors that might have slipped through without a test hook
                    // (Though the new logic catches them inside handle_*)
                    // If we are here, it's a real hard network error or retryable failure.
                    let _ = Journal::modify(self.ctx.as_ref(), |queue| {
                        *queue = journal.queue.clone();
                    });
                    return Err(msg);
                }
            }
        }

        // Write final state to disk once at the end
        Journal::modify(self.ctx.as_ref(), |queue| {
            *queue = journal.queue;
        })
        .map_err(|e| e.to_string())?;

        Ok(warnings)
    }

    async fn attempt_conflict_resolution(&self, local_task: &Task) -> Option<(Action, String)> {
        let (cached_tasks, _) =
            crate::cache::Cache::load(self.ctx.as_ref(), &local_task.calendar_href).ok()?;
        let base_task = cached_tasks.iter().find(|t| t.uid == local_task.uid)?;

        let server_task = self.fetch_remote_task(&local_task.href).await?;

        if let Some(merged) = three_way_merge(base_task, local_task, &server_task) {
            let msg = format!(
                "Conflict (412) on '{}' resolved via 3-way merge.",
                local_task.summary
            );
            return Some((Action::Update(merged), msg));
        }

        None
    }

    async fn execute_move(
        &self,
        task: &Task,
        new_calendar_href: &str,
        overwrite: bool,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Offline")?;
        let destination = if new_calendar_href.ends_with('/') {
            format!("{}{}.ics", new_calendar_href, task.uid)
        } else {
            format!("{}/{}.ics", new_calendar_href, task.uid)
        };
        let source_path = strip_host(&task.href);
        let source_uri = client
            .webdav_client
            .relative_uri(&source_path)
            .map_err(|e| format!("Invalid source URI: {}", e))?;

        let base = client.webdav_client.base_url();
        let scheme = base.scheme_str().unwrap_or("https");
        let authority = base.authority().map(|a| a.as_str()).unwrap_or("");
        let dest_path = strip_host(&destination);
        let clean_dest_path = if dest_path.starts_with('/') {
            dest_path
        } else {
            format!("/{}", dest_path)
        };
        let absolute_destination = format!("{}://{}{}", scheme, authority, clean_dest_path);

        let req = Request::builder()
            .method("MOVE")
            .uri(source_uri)
            .header("Destination", absolute_destination)
            .header("Overwrite", if overwrite { "T" } else { "F" })
            .body(String::new())
            .map_err(|e| e.to_string())?;
        let (parts, _) = client
            .webdav_client
            .request_raw(req)
            .await
            .map_err(|e| format!("{:?}", e))?;
        if parts.status.is_success() {
            Ok(())
        } else {
            Err(format!("MOVE failed: {}", parts.status))
        }
    }
}
