// File: src/client/core.rs
use crate::cache::Cache;
use crate::client::auth::DynamicAuthLayer;
use crate::client::cert::NoVerifier;
use crate::config::Config;
use crate::journal::{Action, Journal};
use crate::model::{CalendarListEntry, Task, TaskStatus};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalStorage};

use libdav::caldav::{FindCalendarHomeSet, FindCalendars, GetCalendarResources};
use libdav::dav::{Delete, GetProperty, ListResources, PutResource};
use libdav::dav::{WebDavClient, WebDavError};
use libdav::{CalDavClient, PropertyName, names};

use futures::stream::{self, StreamExt};
use http::{Request, StatusCode, Uri};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_layer::Layer;
use uuid::Uuid;

#[cfg(not(target_os = "android"))]
use rustls_native_certs;

// FIX: No platform verifier import needed

pub const GET_CTAG: PropertyName = PropertyName::new("http://calendarserver.org/ns/", "getctag");
pub const APPLE_COLOR: PropertyName =
    PropertyName::new("http://apple.com/ns/ical/", "calendar-color");

use crate::client::auth::DynamicAuthService;

type HttpsClient = DynamicAuthService<
    Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        String,
    >,
>;

fn strip_host(href: &str) -> String {
    if let Ok(uri) = href.parse::<Uri>()
        && (uri.scheme().is_some() || uri.authority().is_some())
    {
        return uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| uri.path().to_string());
    }
    href.to_string()
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

#[derive(Clone, Debug)]
pub struct RustyClient {
    pub client: Option<CalDavClient<HttpsClient>>,
}

impl RustyClient {
    pub fn new(url: &str, user: &str, pass: &str, insecure: bool) -> Result<Self, String> {
        if url.is_empty() {
            return Ok(Self { client: None });
        }
        let uri: Uri = url
            .parse()
            .map_err(|e: http::uri::InvalidUri| e.to_string())?;

        let tls_config_builder = rustls::ClientConfig::builder();

        let tls_config = if insecure {
            tls_config_builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth()
        } else {
            #[cfg(not(target_os = "android"))]
            {
                let mut root_store = rustls::RootCertStore::empty();
                let result = rustls_native_certs::load_native_certs();
                root_store.add_parsable_certificates(result.certs);
                if root_store.is_empty() {
                    return Err("No valid system certificates found.".to_string());
                }
                tls_config_builder
                    .with_root_certificates(root_store)
                    .with_no_client_auth()
            }

            #[cfg(target_os = "android")]
            {
                // FIX: Use bundled Mozilla roots via webpki-roots.
                // This does not require JNI initialization or Context passing.
                let mut root_store = rustls::RootCertStore::empty();
                root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                tls_config_builder
                    .with_root_certificates(root_store)
                    .with_no_client_auth()
            }
        };

        let https_connector = HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let http_client = Client::builder(TokioExecutor::new()).build(https_connector);
        let auth_client =
            DynamicAuthLayer::new(user.to_string(), pass.to_string()).layer(http_client);
        let webdav = WebDavClient::new(uri, auth_client.clone());
        let caldav = CalDavClient::new(webdav);
        Ok(Self {
            client: Some(caldav),
        })
    }

    pub async fn discover_calendar(&self) -> Result<String, String> {
        if let Some(client) = &self.client {
            let base_path = client.base_url().path().to_string();
            if let Ok(response) = client.request(ListResources::new(&base_path)).await
                && response.resources.iter().any(|r| r.href.ends_with(".ics"))
            {
                return Ok(base_path);
            }
            if let Ok(Some(principal)) = client.find_current_user_principal().await
                && let Ok(response) = client.request(FindCalendarHomeSet::new(&principal)).await
                && let Some(home_url) = response.home_sets.first()
                && let Ok(cals_resp) = client.request(FindCalendars::new(home_url)).await
                && let Some(first) = cals_resp.calendars.first()
            {
                return Ok(first.href.clone());
            }
            Ok(base_path)
        } else {
            Err("Offline".to_string())
        }
    }

    pub async fn connect_with_fallback(
        config: Config,
    ) -> Result<
        (
            Self,
            Vec<CalendarListEntry>,
            Vec<Task>,
            Option<String>,
            Option<String>,
        ),
        String,
    > {
        let client = Self::new(
            &config.url,
            &config.username,
            &config.password,
            config.allow_insecure_certs,
        )
        .map_err(|e| e.to_string())?;

        let _ = client.sync_journal().await;

        let (calendars, warning) = match client.get_calendars().await {
            Ok(c) => {
                let _ = Cache::save_calendars(&c);
                (c, None)
            }
            Err(e) => {
                if e.contains("InvalidCertificate") {
                    return Err(format!("Connection failed: {}", e));
                }
                (
                    Cache::load_calendars().unwrap_or_default(),
                    Some("Offline mode".to_string()),
                )
            }
        };

        let mut active_href = None;
        if let Some(def_cal) = &config.default_calendar
            && let Some(found) = calendars
                .iter()
                .find(|c| c.name == *def_cal || c.href == *def_cal)
        {
            active_href = Some(found.href.clone());
        }

        if active_href.is_none()
            && warning.is_none()
            && let Ok(href) = client.discover_calendar().await
        {
            active_href = Some(href);
        }

        let tasks = if warning.is_none() {
            if let Some(ref h) = active_href {
                client.get_tasks(h).await.unwrap_or_default()
            } else {
                vec![]
            }
        } else if let Some(ref h) = active_href {
            let (mut t, _) = Cache::load(h).unwrap_or((vec![], None));
            Journal::apply_to_tasks(&mut t, h);
            t
        } else {
            vec![]
        };

        Ok((client, calendars, tasks, active_href, warning))
    }

    pub async fn get_calendars(&self) -> Result<Vec<CalendarListEntry>, String> {
        if let Some(client) = &self.client {
            let principal = client
                .find_current_user_principal()
                .await
                .map_err(|e| format!("{:?}", e))?
                .ok_or("No principal")?;

            let home_set_resp = client
                .request(FindCalendarHomeSet::new(&principal))
                .await
                .map_err(|e| format!("{:?}", e))?;

            let home_url = home_set_resp.home_sets.first().ok_or("No home set")?;

            let cals_resp = client
                .request(FindCalendars::new(home_url))
                .await
                .map_err(|e| format!("{:?}", e))?;

            let mut calendars = Vec::new();
            for col in cals_resp.calendars {
                let name = client
                    .request(GetProperty::new(&col.href, &names::DISPLAY_NAME))
                    .await
                    .ok()
                    .and_then(|r| r.value)
                    .unwrap_or_else(|| col.href.clone());

                let color = client
                    .request(GetProperty::new(&col.href, &APPLE_COLOR))
                    .await
                    .ok()
                    .and_then(|r| r.value);

                calendars.push(CalendarListEntry {
                    name,
                    href: col.href,
                    color,
                });
            }
            Ok(calendars)
        } else {
            Ok(vec![])
        }
    }

    async fn fetch_calendar_tasks_internal(
        &self,
        calendar_href: &str,
        apply_journal: bool,
    ) -> Result<Vec<Task>, String> {
        if calendar_href == LOCAL_CALENDAR_HREF {
            let mut tasks = LocalStorage::load().map_err(|e| e.to_string())?;
            if apply_journal {
                Journal::apply_to_tasks(&mut tasks, calendar_href);
            }
            return Ok(tasks);
        }

        let (mut cached_tasks, cached_token) = Cache::load(calendar_href).unwrap_or((vec![], None));

        if let Some(client) = &self.client {
            let path_href = strip_host(calendar_href);

            // Optimization: Filter out pending deletes to avoid unnecessary fetches/404s
            let pending_deletions = if apply_journal {
                let journal = Journal::load();
                let mut dels = HashSet::new();
                for action in journal.queue {
                    match action {
                        Action::Delete(t) if t.calendar_href == calendar_href => {
                            dels.insert(t.uid);
                        }
                        Action::Move(t, _) if t.calendar_href == calendar_href => {
                            dels.insert(t.uid);
                        }
                        _ => {}
                    }
                }
                dels
            } else {
                HashSet::new()
            };

            let remote_token = if let Ok(resp) = client
                .request(GetProperty::new(&path_href, &GET_CTAG))
                .await
            {
                resp.value
            } else if let Ok(resp) = client
                .request(GetProperty::new(&path_href, &names::SYNC_TOKEN))
                .await
            {
                resp.value
            } else {
                None
            };

            let has_ghosts = cached_tasks
                .iter()
                .any(|t| t.etag.is_empty() && !t.href.is_empty());

            if !has_ghosts
                && let (Some(r_tok), Some(c_tok)) = (&remote_token, &cached_token)
                && r_tok == c_tok
            {
                if apply_journal {
                    Journal::apply_to_tasks(&mut cached_tasks, calendar_href);
                }
                return Ok(cached_tasks);
            }

            let list_resp = client
                .request(ListResources::new(&path_href))
                .await
                .map_err(|e| format!("PROPFIND: {:?}", e))?;

            let mut cache_map: HashMap<String, Task> = HashMap::new();
            for t in cached_tasks {
                cache_map.insert(strip_host(&t.href), t);
            }

            let mut final_tasks = Vec::new();
            let mut to_fetch = Vec::new();
            let mut server_hrefs = HashSet::new();

            for resource in list_resp.resources {
                if !resource.href.ends_with(".ics") {
                    continue;
                }

                let res_href_stripped = strip_host(&resource.href);

                // Optimization check using cache lookup
                let should_skip = if let Some(cached) = cache_map.get(&res_href_stripped) {
                    pending_deletions.contains(&cached.uid)
                } else {
                    false
                };

                if should_skip {
                    cache_map.remove(&res_href_stripped);
                    continue;
                }

                server_hrefs.insert(res_href_stripped.clone());
                let remote_etag = resource.etag;

                if let Some(local_task) = cache_map.remove(&res_href_stripped) {
                    if let Some(r_etag) = &remote_etag {
                        if !r_etag.is_empty() && *r_etag == local_task.etag {
                            final_tasks.push(local_task);
                        } else {
                            to_fetch.push(res_href_stripped);
                        }
                    } else {
                        to_fetch.push(res_href_stripped);
                    }
                } else {
                    to_fetch.push(res_href_stripped);
                }
            }

            // Tasks in cache but not on server (deleted on server)
            for (_href, task) in cache_map {
                let is_unsynced = task.etag.is_empty() || task.href.is_empty();
                // If it's unsynced (ghost), keep it. `apply_to_tasks` will prune it if invalid.
                if is_unsynced {
                    final_tasks.push(task);
                }
            }

            if !to_fetch.is_empty() {
                let fetched_resp = client
                    .request(GetCalendarResources::new(&path_href).with_hrefs(to_fetch))
                    .await
                    .map_err(|e| format!("MULTIGET: {:?}", e))?;

                for item in fetched_resp.resources {
                    if let Ok(content) = item.content
                        && let Ok(task) = Task::from_ics(
                            &content.data,
                            content.etag,
                            item.href,
                            calendar_href.to_string(),
                        )
                    {
                        if apply_journal && pending_deletions.contains(&task.uid) {
                            continue;
                        }
                        final_tasks.push(task);
                    }
                }
            }

            if apply_journal {
                Journal::apply_to_tasks(&mut final_tasks, calendar_href);
            }
            let _ = Cache::save(calendar_href, &final_tasks, remote_token);
            Ok(final_tasks)
        } else {
            if apply_journal {
                Journal::apply_to_tasks(&mut cached_tasks, calendar_href);
            }
            Ok(cached_tasks)
        }
    }

    pub async fn get_tasks(&self, calendar_href: &str) -> Result<Vec<Task>, String> {
        let _ = self.sync_journal().await.ok();
        self.fetch_calendar_tasks_internal(calendar_href, true)
            .await
    }

    pub async fn get_all_tasks(
        &self,
        calendars: &[CalendarListEntry],
    ) -> Result<Vec<(String, Vec<Task>)>, String> {
        let _ = self.sync_journal().await;

        let hrefs: Vec<String> = calendars.iter().map(|c| c.href.clone()).collect();
        let futures = hrefs.into_iter().map(|href| {
            let client = self.clone();
            async move {
                (
                    href.clone(),
                    client.fetch_calendar_tasks_internal(&href, true).await,
                )
            }
        });

        let mut stream = stream::iter(futures).buffer_unordered(4);
        let mut final_results = Vec::new();

        while let Some((href, res)) = stream.next().await {
            if let Ok(tasks) = res {
                final_results.push((href, tasks));
            }
        }

        Ok(final_results)
    }

    pub async fn create_task(&self, task: &mut Task) -> Result<Vec<String>, String> {
        if task.calendar_href == LOCAL_CALENDAR_HREF {
            let mut all = LocalStorage::load().map_err(|e| e.to_string())?;
            all.push(task.clone());
            LocalStorage::save(&all).map_err(|e| e.to_string())?;
            return Ok(vec![]);
        }

        let cal_path = task.calendar_href.clone();
        let filename = format!("{}.ics", task.uid);
        let full_href = if cal_path.ends_with('/') {
            format!("{}{}", cal_path, filename)
        } else {
            format!("{}/{}", cal_path, filename)
        };
        task.href = full_href;

        Journal::push(Action::Create(task.clone())).map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn update_task(&self, task: &mut Task) -> Result<Vec<String>, String> {
        task.sequence += 1;

        if task.calendar_href == LOCAL_CALENDAR_HREF {
            let mut all = LocalStorage::load().map_err(|e| e.to_string())?;
            if let Some(idx) = all.iter().position(|t| t.uid == task.uid) {
                all[idx] = task.clone();
                LocalStorage::save(&all).map_err(|e| e.to_string())?;
            }
            return Ok(vec![]);
        }

        Journal::push(Action::Update(task.clone())).map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn delete_task(&self, task: &Task) -> Result<Vec<String>, String> {
        if task.calendar_href == LOCAL_CALENDAR_HREF {
            let mut all = LocalStorage::load().map_err(|e| e.to_string())?;
            all.retain(|t| t.uid != task.uid);
            LocalStorage::save(&all).map_err(|e| e.to_string())?;
            return Ok(vec![]);
        }

        Journal::push(Action::Delete(task.clone())).map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn toggle_task(
        &self,
        task: &mut Task,
    ) -> Result<(Task, Option<Task>, Vec<String>), String> {
        // --- START MERGE ---
        if task.status == TaskStatus::Completed && task.rrule.is_some() && task.advance_recurrence()
        {
            // Task Recycled
        }
        // --- END MERGE ---

        if task.calendar_href == LOCAL_CALENDAR_HREF {
            let mut all = LocalStorage::load().map_err(|e| e.to_string())?;
            if let Some(idx) = all.iter().position(|t| t.uid == task.uid) {
                all[idx] = task.clone();
            }
            LocalStorage::save(&all).map_err(|e| e.to_string())?;
            return Ok((task.clone(), None, vec![]));
        }

        let logs = self.update_task(task).await?;
        Ok((task.clone(), None, logs))
    }

    pub async fn move_task(
        &self,
        task: &Task,
        new_calendar_href: &str,
    ) -> Result<(Task, Vec<String>), String> {
        if task.calendar_href == LOCAL_CALENDAR_HREF {
            let mut new_task = task.clone();
            new_task.calendar_href = new_calendar_href.to_string();
            new_task.href = String::new();
            new_task.etag = String::new();
            self.create_task(&mut new_task).await?;
            self.delete_task(task).await?;
            return Ok((new_task, vec![]));
        }

        Journal::push(Action::Move(task.clone(), new_calendar_href.to_string()))
            .map_err(|e| e.to_string())?;

        let mut t = task.clone();
        t.calendar_href = new_calendar_href.to_string();
        let logs = self.sync_journal().await?;
        Ok((t, logs))
    }

    pub async fn migrate_tasks(
        &self,
        tasks: Vec<Task>,
        target_calendar_href: &str,
    ) -> Result<usize, String> {
        let futures = tasks.into_iter().map(|task| {
            let client = self.clone();
            let target = target_calendar_href.to_string();
            async move { client.move_task(&task, &target).await.ok() }
        });

        let mut stream = stream::iter(futures).buffer_unordered(4);
        let mut count = 0;
        while let Some(res) = stream.next().await {
            if res.is_some() {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn fetch_etag(&self, path: &str) -> Option<String> {
        if let Some(client) = &self.client
            && let Ok(resp) = client
                .request(GetProperty::new(path, &names::GETETAG))
                .await
        {
            return resp.value;
        }
        None
    }

    pub async fn sync_journal(&self) -> Result<Vec<String>, String> {
        // Compact journal to merge redundant updates before syncing ---
        let _ = Journal::modify(|queue| {
            let mut tmp_j = Journal {
                queue: queue.drain(..).collect(),
            };
            tmp_j.compact();
            *queue = tmp_j.queue;
        });
        let client = self.client.as_ref().ok_or("Offline")?;
        let mut warnings = Vec::new();

        loop {
            let next_action = {
                let j = Journal::load();
                if j.queue.is_empty() {
                    return Ok(warnings);
                }
                j.queue[0].clone()
            };

            let mut conflict_resolved_action = None;
            let mut new_etag_to_propagate: Option<String> = None;
            let mut new_href_to_propagate: Option<(String, String)> = None;
            let mut path_for_refresh: Option<String> = None;

            let result = match &next_action {
                Action::Create(task) => {
                    let filename = format!("{}.ics", task.uid);
                    let full_href = if task.calendar_href.ends_with('/') {
                        format!("{}{}", task.calendar_href, filename)
                    } else {
                        format!("{}/{}", task.calendar_href, filename)
                    };
                    let path = strip_host(&full_href);
                    let ics_string = task.to_ics();
                    match client
                        .request(PutResource::new(&path).create(ics_string, "text/calendar"))
                        .await
                    {
                        Ok(resp) => {
                            if let Some(etag) = resp.etag {
                                new_etag_to_propagate = Some(etag);
                            } else {
                                path_for_refresh = Some(path.clone());
                            }
                            Ok(())
                        }
                        Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
                        | Err(WebDavError::PreconditionFailed(_)) => {
                            // The resource already exists on the server.
                            // This usually happens if a previous sync attempt succeeded
                            // but the client timed out before receiving confirmation.
                            // We treat this as success to unblock the journal queue.
                            warnings.push(format!(
                                "Creation conflict: Task '{}' already exists on server. Mark as synced.",
                                task.summary
                            ));
                            path_for_refresh = Some(path.clone());
                            Ok(())
                        }
                        Err(e) => Err(format!("{:?}", e)),
                    }
                }
                Action::Update(task) => {
                    let path = strip_host(&task.href);
                    let ics_string = task.to_ics();
                    match client
                        .request(PutResource::new(&path).update(
                            ics_string,
                            "text/calendar; charset=utf-8; component=VTODO",
                            &task.etag,
                        ))
                        .await
                    {
                        Ok(resp) => {
                            if let Some(etag) = resp.etag {
                                new_etag_to_propagate = Some(etag);
                            } else {
                                path_for_refresh = Some(path.clone());
                            }
                            Ok(())
                        }
                        Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED))
                        | Err(WebDavError::PreconditionFailed(_)) => {
                            if let Some((resolution, msg)) =
                                self.attempt_conflict_resolution(task).await
                            {
                                warnings.push(msg);
                                conflict_resolved_action = Some(resolution);
                                Ok(())
                            } else {
                                let msg = format!(
                                    "Conflict (412) on task '{}'. Merge failed. Creating copy.",
                                    task.summary
                                );
                                warnings.push(msg);

                                let mut conflict_copy = task.clone();
                                conflict_copy.uid = Uuid::new_v4().to_string();
                                conflict_copy.summary = format!("{} (Conflict Copy)", task.summary);
                                conflict_copy.href = String::new();
                                conflict_copy.etag = String::new();
                                conflict_resolved_action = Some(Action::Create(conflict_copy));
                                Ok(())
                            }
                        }
                        Err(WebDavError::BadStatusCode(StatusCode::NOT_FOUND)) => {
                            conflict_resolved_action = Some(Action::Create(task.clone()));
                            Ok(())
                        }
                        Err(e) => {
                            let msg = format!("{:?}", e);
                            if msg.contains("412") || msg.contains("PreconditionFailed") {
                                let w = format!(
                                    "Conflict (412-Fallback) on task '{}'. Creating copy.",
                                    task.summary
                                );
                                warnings.push(w);

                                let mut conflict_copy = task.clone();
                                conflict_copy.uid = Uuid::new_v4().to_string();
                                conflict_copy.summary = format!("{} (Conflict Copy)", task.summary);
                                conflict_copy.href = String::new();
                                conflict_copy.etag = String::new();
                                conflict_resolved_action = Some(Action::Create(conflict_copy));
                                Ok(())
                            } else {
                                Err(msg)
                            }
                        }
                    }
                }
                Action::Delete(task) => {
                    let path = strip_host(&task.href);
                    match client
                        .request(Delete::new(&path).with_etag(&task.etag))
                        .await
                    {
                        Ok(_) => Ok(()),
                        Err(WebDavError::BadStatusCode(StatusCode::NOT_FOUND)) => Ok(()),
                        Err(WebDavError::BadStatusCode(StatusCode::PRECONDITION_FAILED)) => {
                            warnings.push(format!(
                                "Conflict on delete task '{}'. Already modified/deleted.",
                                task.summary
                            ));
                            Ok(())
                        }
                        Err(e) => Err(format!("{:?}", e)),
                    }
                }
                Action::Move(task, new_cal) => match self.execute_move(task, new_cal).await {
                    Ok(_) => {
                        let filename = format!("{}.ics", task.uid);
                        let new_href = if new_cal.ends_with('/') {
                            format!("{}{}", new_cal, filename)
                        } else {
                            format!("{}/{}", new_cal, filename)
                        };
                        new_href_to_propagate = Some((task.href.clone(), new_href.clone()));
                        path_for_refresh = Some(strip_host(&new_href));
                        Ok(())
                    }
                    Err(e) => {
                        if e.contains("404") || e.contains("NotFound") || e.contains("403") {
                            warnings.push(format!(
                                "Move source missing for '{}', assuming success.",
                                task.summary
                            ));
                            Ok(())
                        } else {
                            Err(e)
                        }
                    }
                },
            };

            match result {
                Ok(_) => {
                    if new_etag_to_propagate.is_none()
                        && let Some(path) = path_for_refresh
                        && let Some(fetched) = self.fetch_etag(&path).await
                    {
                        new_etag_to_propagate = Some(fetched);
                    }

                    let commit_res = Journal::modify(|queue| {
                        let should_remove = if let Some(head) = queue.first() {
                            actions_match_identity(head, &next_action)
                        } else {
                            false
                        };

                        if !should_remove {
                            return;
                        }

                        queue.remove(0);

                        if let Some(act) = conflict_resolved_action {
                            queue.insert(0, act);
                        }

                        if let Some(etag) = new_etag_to_propagate {
                            let target_uid = match &next_action {
                                Action::Create(t) | Action::Update(t) => t.uid.clone(),
                                Action::Move(t, _) => t.uid.clone(),
                                _ => String::new(),
                            };
                            if !target_uid.is_empty() {
                                for item in queue.iter_mut() {
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
                                _ => String::new(),
                            };
                            for item in queue.iter_mut() {
                                match item {
                                    Action::Update(t) | Action::Delete(t) => {
                                        if t.uid == target_uid || t.href == old_href {
                                            t.href = new_href.clone();
                                            if let Some(last_slash) = new_href.rfind('/') {
                                                t.calendar_href =
                                                    new_href[..=last_slash].to_string();
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
                    });

                    if let Err(e) = commit_res {
                        return Err(e.to_string());
                    }
                }
                Err(msg) => {
                    if msg.contains("400")
                        || msg.contains("403")
                        || msg.contains("413")
                        || msg.contains("415")
                    {
                        warnings.push(format!("Fatal error on action: {}. Dropping.", msg));
                        let _ = Journal::modify(|queue| {
                            if !queue.is_empty() {
                                queue.remove(0);
                            }
                        });
                        continue;
                    }
                    return Err(msg);
                }
            }
        }
    }

    async fn attempt_conflict_resolution(&self, local_task: &Task) -> Option<(Action, String)> {
        let (cached_tasks, _) = Cache::load(&local_task.calendar_href).ok()?;
        let base_task = cached_tasks.iter().find(|t| t.uid == local_task.uid)?;

        let server_tasks = self
            .fetch_calendar_tasks_internal(&local_task.calendar_href, false)
            .await
            .ok()?;
        let server_task = server_tasks.iter().find(|t| t.uid == local_task.uid)?;

        if let Some(merged) = three_way_merge(base_task, local_task, server_task) {
            let msg = format!(
                "Conflict (412) on '{}' resolved via 3-way merge.",
                local_task.summary
            );
            return Some((Action::Update(merged), msg));
        }

        None
    }

    async fn execute_move(&self, task: &Task, new_calendar_href: &str) -> Result<(), String> {
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
            .header("Overwrite", "F")
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

fn three_way_merge(base: &Task, local: &Task, server: &Task) -> Option<Task> {
    let mut merged = server.clone();

    macro_rules! merge_field {
        ($field:ident) => {
            if local.$field != base.$field {
                if server.$field == base.$field {
                    merged.$field = local.$field.clone();
                } else if local.$field != server.$field {
                    return None;
                }
            }
        };
    }

    merge_field!(summary);
    merge_field!(description);
    merge_field!(status);
    merge_field!(priority);
    merge_field!(due);
    merge_field!(dtstart);
    merge_field!(estimated_duration);
    merge_field!(rrule);
    merge_field!(percent_complete);

    if local.categories != base.categories {
        let mut new_cats = server.categories.clone();
        for cat in &local.categories {
            if !new_cats.contains(cat) {
                new_cats.push(cat.clone());
            }
        }
        new_cats.sort();
        new_cats.dedup();
        merged.categories = new_cats;
    }

    if local.unmapped_properties != base.unmapped_properties {
        for prop in &local.unmapped_properties {
            if !merged.unmapped_properties.iter().any(|p| p.key == prop.key) {
                merged.unmapped_properties.push(prop.clone());
            }
        }
    }

    merge_field!(parent_uid);
    if local.dependencies != base.dependencies {
        let mut new_deps = server.dependencies.clone();
        for dep in &local.dependencies {
            if !new_deps.contains(dep) {
                new_deps.push(dep.clone());
            }
        }
        merged.dependencies = new_deps;
    }

    Some(merged)
}
