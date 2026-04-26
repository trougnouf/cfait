// SPDX-License-Identifier: GPL-3.0-or-later
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
use std::sync::OnceLock;
use tokio::sync::Mutex as AsyncMutex;

#[cfg(any(test, feature = "test_hooks"))]
use crate::client::core::test_hooks::TEST_FORCE_SYNC_ERROR;

static SYNC_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

// --- Internal Sync Outcome Types ---
enum StepOutcome {
    Success {
        etag: Option<String>,
        href: Option<String>,
        refresh_path: Option<String>,
    },
    RetryWith(Box<Action>),
    ReplaceWith(Vec<Action>),
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
/// Normalizes paths from the server. Fixes paths stripped by proxies and encodes spaces,
/// but crucially leaves '@' unencoded for Nextcloud compatibility.
fn fix_and_encode_path(
    client: &CalDavClient<HttpsClient>,
    raw_href: &str,
    filename: Option<&str>,
) -> String {
    let mut path = strip_host(raw_href);
    if let Some(idx) = path.find('?') {
        path.truncate(idx);
    }

    let base_path = client.base_url().path();

    // Proxy fixup: Reconstruct missing base paths (the Android bug fix)
    if !path.starts_with(base_path) && base_path != "/" {
        let mut fixed = base_path.to_string();
        if !fixed.ends_with('/') {
            fixed.push('/');
        }
        if !path.starts_with("/calendars/")
            && !path.starts_with("calendars/")
            && !fixed.ends_with("calendars/")
            && !fixed.contains("/calendars/")
        {
            fixed.push_str("calendars/");
        }
        fixed.push_str(path.trim_start_matches('/'));
        path = fixed;
    }

    if let Some(fname) = filename {
        if !path.ends_with('/') {
            path.push('/');
        }
        path.push_str(fname);
    }

    // Encode spaces and brackets, but leave '@' as-is.
    path.replace(" ", "%20")
        .replace("(", "%28")
        .replace(")", "%29")
}

impl RustyClient {
    async fn handle_create(
        &self,
        client: &CalDavClient<HttpsClient>,
        task: &Task,
    ) -> Result<StepResult, String> {
        let path = fix_and_encode_path(
            client,
            &task.calendar_href,
            Some(&format!("{}.ics", task.uid)),
        );
        let ics_string = IcsAdapter::to_ics(task);

        match client
            .request(PutResource::new(&path).create(ics_string, "text/calendar; charset=utf-8"))
            .await
        {
            Ok(resp) => {
                let href = if task.calendar_href.ends_with('/') {
                    format!("{}{}.ics", task.calendar_href, task.uid)
                } else {
                    format!("{}/{}.ics", task.calendar_href, task.uid)
                };

                let outcome = StepOutcome::Success {
                    etag: resp.etag,
                    href: Some(href),
                    refresh_path: Some(path),
                };
                Ok(StepResult::new(outcome))
            }
            Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
            | Err(WebDavError::PreconditionFailed(_)) => {
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
                if msg.contains("403")
                    || msg.contains("400")
                    || msg.contains("415")
                    || msg.contains("404")
                    || msg.contains("NotFound")
                    || msg.contains("409")
                    || msg.contains("Conflict")
                    || msg.contains("InvalidInput")
                    || msg.contains("invalid uri character")
                {
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
        let path = if task.href.is_empty() {
            fix_and_encode_path(
                client,
                &task.calendar_href,
                Some(&format!("{}.ics", task.uid)),
            )
        } else {
            fix_and_encode_path(client, &task.href, None)
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
                "text/calendar; charset=utf-8",
                etag_val,
            ))
            .await
        {
            Ok(resp) => {
                let new_href = if task.href.is_empty() {
                    if task.calendar_href.ends_with('/') {
                        Some(format!("{}{}.ics", task.calendar_href, task.uid))
                    } else {
                        Some(format!("{}/{}.ics", task.calendar_href, task.uid))
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
            Err(WebDavError::BadStatusCode(StatusCode::NOT_FOUND)) => Ok(StepResult::new(
                StepOutcome::RetryWith(Box::new(Action::Create(task.clone()))),
            )),
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
                } else if msg.contains("403")
                    || msg.contains("400")
                    || msg.contains("415")
                    || msg.contains("409")
                    || msg.contains("Conflict")
                    || msg.contains("InvalidInput")
                    || msg.contains("invalid uri character")
                {
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
        let path = fix_and_encode_path(client, &task.href, None);

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
            Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
            | Err(WebDavError::PreconditionFailed(_)) => {
                let mut retry_task = task.clone();
                retry_task.etag = String::new(); // clear etag to force delete on next attempt
                Ok(StepResult::new(StepOutcome::RetryWith(Box::new(Action::Delete(retry_task))))
                .with_warning(format!(
                    "Conflict on delete task '{}'. Forcing delete.",
                    task.summary
                )))
            }
            Err(e) => {
                let msg = format!("{:?}", e);
                if msg.contains("403")
                    || msg.contains("400")
                    || msg.contains("415")
                    || msg.contains("409")
                    || msg.contains("Conflict")
                    || msg.contains("InvalidInput")
                    || msg.contains("invalid uri character")
                {
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
                let mut moved = task.clone();
                moved.calendar_href = new_cal.to_string();
                moved.href = String::new();
                moved.etag = String::new();

                // CREATE first, then DELETE the original. This prevents data loss.
                Ok(StepResult::new(StepOutcome::ReplaceWith(vec![
                    Action::Create(moved),
                    Action::Delete(task.clone()),
                ]))
                .with_warning(format!(
                    "MOVE failed ({}), falling back to Create+Delete.",
                    e
                )))
            }
        }
    }

    pub async fn sync_journal(&self) -> Result<(Vec<String>, Vec<Task>), String> {
        // 1. Serialize sync loops process-wide to protect the physical journal file
        let lock = SYNC_LOCK.get_or_init(|| AsyncMutex::new(()));
        let _guard = lock.lock().await;

        let client = self.client.as_ref().ok_or("Offline")?;
        let mut warnings = Vec::new();
        let mut synced_tasks: Vec<Task> = Vec::new();

        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
        let events_enabled = config.create_events_for_tasks;
        let delete_on_completion = config.delete_events_on_completion;

        let mut recovery_cal_created_this_cycle = false;

        // 2. Transactional processing loop
        loop {
            let mut next_action_opt = None;

            // Peek the front of the queue
            Journal::modify(self.ctx.as_ref(), |queue| {
                let mut tmp_j = Journal {
                    queue: std::mem::take(queue),
                };
                tmp_j.compact();
                *queue = tmp_j.queue.clone();
                if !queue.is_empty() {
                    next_action_opt = Some(queue[0].clone());
                }
            })
            .map_err(|e| e.to_string())?;

            let next_action = match next_action_opt {
                Some(a) => a,
                None => break, // Queue is empty, we are done!
            };

            let mut conflict_resolved_action: Option<Action> = None;
            let mut replaced_actions: Option<Vec<Action>> = None;
            let mut new_etag_to_propagate: Option<String> = None;
            let mut new_href_to_propagate: Option<(String, String)> = None;
            let mut path_for_refresh: Option<String> = None;

            let test_forced_err: Option<anyhow::Error> = {
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

            let step_result = if let Some(err) = test_forced_err {
                let err_msg = err.to_string();
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
                            match &next_action {
                                Action::Move(t, new_cal) => {
                                    let _ = self
                                        .sync_companion_event(
                                            t,
                                            events_enabled,
                                            delete_on_completion,
                                            true,
                                        )
                                        .await;

                                    if (events_enabled || t.create_event.is_some())
                                        && let Some(new_h) = &href
                                    {
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
                                    Action::Create(t) => t.href.clone(),
                                    Action::Move(t, _) => t.href.clone(),
                                    Action::Update(t) => t.href.clone(),
                                    _ => String::new(),
                                };
                                new_href_to_propagate = Some((old, h));
                            }
                        }
                        StepOutcome::RetryWith(act) => {
                            conflict_resolved_action = Some(*act);
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
                        StepOutcome::ReplaceWith(acts) => {
                            replaced_actions = Some(acts);
                        }
                        StepOutcome::Discard => {
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
                                task.href = String::new();
                                task.etag = String::new();
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
                        }
                    }

                    if new_etag_to_propagate.is_none()
                        && let Some(path) = path_for_refresh
                        && let Some(fetched) = self.fetch_etag(&path).await
                    {
                        new_etag_to_propagate = Some(fetched);
                    }

                    let mut synced_task = match &next_action {
                        Action::Create(t) | Action::Update(t) | Action::Move(t, _) => {
                            Some(t.clone())
                        }
                        Action::Delete(_) => None,
                    };

                    if let Some(ref mut t) = synced_task {
                        if let Some(ref e) = new_etag_to_propagate {
                            t.etag = e.clone();
                        }
                        if let Some((_, ref new_href)) = new_href_to_propagate {
                            t.href = new_href.clone();
                        }
                        synced_tasks.push(t.clone());
                    }

                    // 3. Pop the item from the disk queue and propagate metadata
                    Journal::modify(self.ctx.as_ref(), |queue| {
                        let should_remove = if let Some(head) = queue.first() {
                            actions_match_identity(head, &next_action)
                        } else {
                            false
                        };

                        if should_remove {
                            queue.remove(0);

                            if let Some(act) = conflict_resolved_action {
                                queue.insert(0, act);
                            }
                            if let Some(acts) = replaced_actions {
                                for act in acts.into_iter().rev() {
                                    queue.insert(0, act);
                                }
                            }

                            if let Some(etag) = new_etag_to_propagate {
                                let (target_uid, target_cal_href) = match &next_action {
                                    Action::Create(t) | Action::Update(t) => {
                                        (t.uid.clone(), t.calendar_href.clone())
                                    }
                                    Action::Move(t, target) => (t.uid.clone(), target.clone()),
                                    _ => (String::new(), String::new()),
                                };
                                if !target_uid.is_empty() {
                                    for item in queue.iter_mut() {
                                        match item {
                                            Action::Update(t) | Action::Delete(t)
                                                if t.uid == target_uid
                                                    && t.calendar_href == target_cal_href =>
                                            {
                                                t.etag = etag.clone();
                                            }
                                            Action::Move(t, _)
                                                if t.uid == target_uid
                                                    && t.calendar_href == target_cal_href =>
                                            {
                                                t.etag = etag.clone();
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            if let Some((old_href, new_href)) = new_href_to_propagate {
                                let (target_uid, target_cal_href) = match &next_action {
                                    Action::Move(t, target) => (t.uid.clone(), target.clone()),
                                    Action::Create(t) => (t.uid.clone(), t.calendar_href.clone()),
                                    Action::Update(t) => (t.uid.clone(), t.calendar_href.clone()),
                                    _ => (String::new(), String::new()),
                                };
                                for item in queue.iter_mut() {
                                    match item {
                                        Action::Update(t) | Action::Delete(t)
                                            if (t.uid == target_uid
                                                && t.calendar_href == target_cal_href)
                                                || (!old_href.is_empty() && t.href == old_href) =>
                                        {
                                            t.href = new_href.clone();
                                            if let Some(last_slash) = new_href.rfind('/') {
                                                t.calendar_href =
                                                    new_href[..=last_slash].to_string();
                                            }
                                        }
                                        Action::Move(t, _)
                                            if t.uid == target_uid
                                                && t.calendar_href == target_cal_href =>
                                        {
                                            t.href = new_href.clone();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    })
                    .map_err(|e| e.to_string())?;
                }
                Err(msg) => {
                    // Stop processing on network error.
                    // The action safely remains at the front of the disk queue.
                    #[cfg(target_os = "android")]
                    log::error!("sync_journal step failed: {}", msg);
                    #[cfg(not(target_os = "android"))]
                    eprintln!("sync_journal step failed: {}", msg);

                    return Err(msg);
                }
            }
        }

        Ok((warnings, synced_tasks))
    }

    async fn attempt_conflict_resolution(&self, local_task: &Task) -> Option<(Action, String)> {
        let (cached_tasks, _) =
            crate::cache::Cache::load(self.ctx.as_ref(), &local_task.calendar_href).ok()?;
        let base_task = cached_tasks.iter().find(|t| t.uid == local_task.uid)?;

        let server_task = self.fetch_remote_task(&local_task.href).await?;

        if server_task.etag == local_task.etag {
            return None;
        }

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

        let source_path = fix_and_encode_path(client, &task.href, None);
        let source_uri = client
            .webdav_client
            .relative_uri(&source_path)
            .map_err(|e| format!("Invalid source URI: {}", e))?;

        let dest_path = fix_and_encode_path(
            client,
            new_calendar_href,
            Some(&format!("{}.ics", task.uid)),
        );

        let base = client.webdav_client.base_url();
        let scheme = base.scheme_str().unwrap_or("https");
        let authority = base.authority().map(|a| a.as_str()).unwrap_or("");

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
