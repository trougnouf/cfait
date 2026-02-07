// File: ./src/client/core.rs
/*
File: ./src/client/core.rs

Core networking and high-level CalDAV client APIs.

This file focuses on:
- Constructing the HTTP/CalDAV client
- High-level operations (connect_with_fallback, discover, fetch tasks,
  create/update/delete wrappers that journal + trigger sync)
- Utilities used across the client

Journal processing and low-level sync step implementations (handle_create,
handle_update, handle_delete, handle_move, sync_journal, conflict resolution,
etc.) have been moved into `src/client/sync.rs` to reduce duplication and keep
responsibilities clear.
*/

use crate::cache::Cache;

use crate::client::auth::DynamicAuthLayer;
use crate::client::cert::NoVerifier;
use crate::client::middleware::{UserAgentLayer, UserAgentService};
use crate::config::Config;
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::{CalendarListEntry, IcsAdapter, Task, TaskStatus};
use crate::storage::{LocalCalendarRegistry, LocalStorage};

use libdav::caldav::{FindCalendarHomeSet, FindCalendars, GetCalendarResources};
use libdav::dav::{Delete, GetProperty, ListResources, Propfind, PutResource};
use libdav::dav::{WebDavClient, WebDavError};
use libdav::{CalDavClient, PropertyName, names};
use roxmltree::Document;

use futures::stream::{self, StreamExt};
use http::Uri;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(not(target_os = "android"))]
use rustls_native_certs;

use tower_layer::Layer;

// Re-exports used elsewhere in the crate
pub const GET_CTAG: PropertyName = PropertyName::new("http://calendarserver.org/ns/", "getctag");
pub const APPLE_COLOR: PropertyName =
    PropertyName::new("http://apple.com/ns/ical/", "calendar-color");

use crate::client::FollowRedirectService;
use crate::client::auth::DynamicAuthService;

// Concrete HttpsClient type used throughout the crate. This is a FollowRedirect
// wrapper around the DynamicAuthService -> UserAgentService -> hyper Client.
pub(crate) type HttpsClient = FollowRedirectService<
    DynamicAuthService<
        UserAgentService<
            Client<
                hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
                String,
            >,
        >,
    >,
>;

// -----------------------------
// Test hooks (test-only)
// These are provided so unit/integration tests can inject deterministic
// behavior into network/fetch paths. Kept in this module so tests that refer
// to `cfait::client::core::test_hooks::...` keep working.
#[cfg(any(test, feature = "test_hooks"))]
pub mod test_hooks {
    use super::Task;
    use crate::journal::Action;
    use std::sync::{Mutex, OnceLock};

    pub type FetchRemoteHook = Box<dyn Fn(&str) -> Option<Task> + Send + Sync + 'static>;
    pub type ForceSyncErrorHook = Box<dyn Fn(&Action) -> Option<String> + Send + Sync + 'static>;

    /// Test hook to simulate fetch_remote_task responses in unit tests.
    pub static TEST_FETCH_REMOTE_HOOK: OnceLock<Mutex<Option<FetchRemoteHook>>> = OnceLock::new();

    /// Test hook to force a synthetic sync error for a given Action during tests.
    pub static TEST_FORCE_SYNC_ERROR: OnceLock<Mutex<Option<ForceSyncErrorHook>>> = OnceLock::new();
}

#[cfg(any(test, feature = "test_hooks"))]
pub use test_hooks::{TEST_FETCH_REMOTE_HOOK, TEST_FORCE_SYNC_ERROR};

// -----------------------------

pub(crate) fn strip_host(href: &str) -> String {
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

// -----------------------------
// High-level RustyClient - network construction and high-level APIs.
// Lower-level sync steps are implemented in src/client/sync.rs (impl RustyClient there).
#[derive(Clone, Debug)]
pub struct RustyClient {
    pub client: Option<CalDavClient<HttpsClient>>,
    pub ctx: Arc<dyn AppContext>,
}

impl RustyClient {
    /// Construct a new client. If `url` is empty, this returns an "offline" client
    /// (client == None) which is used for local-only operations.
    pub fn new(
        ctx: Arc<dyn AppContext>,
        url: &str,
        user: &str,
        pass: &str,
        insecure: bool,
        client_type: Option<&str>,
    ) -> Result<Self, String> {
        if url.is_empty() {
            return Ok(Self {
                client: None,
                ctx: ctx.clone(),
            });
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

        // Build a deterministic User-Agent string
        let version = env!("CARGO_PKG_VERSION");
        let ua_string = if let Some(ctype) = client_type {
            format!("Cfait/{} ({})", version, ctype)
        } else {
            format!("Cfait/{}", version)
        };

        let ua_client = UserAgentLayer::new(ua_string).layer(http_client);
        let auth_client =
            DynamicAuthLayer::new(user.to_string(), pass.to_string()).layer(ua_client);
        let redirect_client = FollowRedirectService::new(auth_client);

        let webdav = WebDavClient::new(uri, redirect_client.clone());
        let caldav = CalDavClient::new(webdav);

        Ok(Self {
            client: Some(caldav),
            ctx,
        })
    }

    /// Attempts to automatically discover the primary calendar for the user.
    /// Returns a path string on success.
    pub async fn discover_calendar(&self) -> Result<String, String> {
        if let Some(client) = &self.client {
            let base_path = client.base_url().path().to_string();
            // Fast heuristic: if any resource in base path ends with .ics treat it as calendar root
            if let Ok(response) = client.request(ListResources::new(&base_path)).await
                && response.resources.iter().any(|r| r.href.ends_with(".ics"))
            {
                return Ok(base_path);
            }
            // Fallback to principal/home-set discovery
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

    /// The primary entry point for UIs to connect.
    /// This function handles connection, discovery, fallback to cache on error,
    /// and initial data loading.
    pub async fn connect_with_fallback(
        ctx: Arc<dyn AppContext>,
        config: Config,
        client_type: Option<&str>,
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
        // Clone config so we can update/save if we detect an auto-corrected root.
        let mut config_for_saving = config.clone();

        let client = Self::new(
            ctx.clone(),
            &config.url,
            &config.username,
            &config.password,
            config.allow_insecure_certs,
            client_type,
        )
        .map_err(|e| e.to_string())?;

        // Ensure any queued actions are attempted as we connect.
        // The sync implementation lives in src/client/sync.rs as `impl RustyClient`.
        // Call it if available; ignore errors here so connect remains resilient.
        let _ = client.sync_journal().await;

        // Attempt to fetch calendars and optionally auto-correct URL/prefixes
        let ((calendars, corrected_url_opt), warning) = match client.get_calendars().await {
            Ok((c, corrected_url)) => {
                if c.is_empty() {
                    let helpful_msg =
                        "Connection successful, but no Task calendars found. Check your URL."
                            .to_string();
                    ((c, corrected_url), Some(helpful_msg))
                } else {
                    let _ = Cache::save_calendars(client.ctx.as_ref(), &c);
                    ((c, corrected_url), None)
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                let mut specific_warning = None;
                if error_msg.contains("InvalidCertificate") {
                    return Err(format!(
                        "Connection failed: Invalid TLS Certificate. {}",
                        error_msg
                    ));
                }
                if error_msg.contains("Unauthorized")
                    || error_msg.contains("Forbidden")
                    || error_msg.contains("401")
                    || error_msg.contains("403")
                {
                    specific_warning =
                        Some("Authentication failed. Check username and password.".to_string());
                } else if error_msg.contains("NotFound") || error_msg.contains("404") {
                    specific_warning = Some(
                        "The CalDAV resource or user principal was not found (404). Check the URL."
                            .to_string(),
                    );
                } else if error_msg.contains("Timeout") {
                    specific_warning =
                        Some("Connection timed out. Check network or server address.".to_string());
                }

                let cals = Cache::load_calendars(client.ctx.as_ref()).unwrap_or_default();

                let final_warning = specific_warning.unwrap_or_else(|| {
                    format!("Offline mode (Network or server error: {}).", error_msg)
                });

                ((cals, None), Some(final_warning))
            }
        };

        // If discovery produced a corrected root URL, persist it asynchronously.
        if let Some(corrected_url) = corrected_url_opt {
            config_for_saving.url = corrected_url;
            let ctx_clone = client.ctx.clone();
            tokio::spawn(async move {
                if let Err(e) = config_for_saving.save(ctx_clone.as_ref()) {
                    #[cfg(not(target_os = "android"))]
                    eprintln!("[Warning] Failed to auto-save corrected URL: {}", e);
                    #[cfg(target_os = "android")]
                    log::warn!("Failed to auto-save corrected URL: {}", e);
                }
            });
        }

        // Determine active/default calendar href (if configured)
        let mut active_href: Option<String> = None;
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

        // If no warning, fetch tasks for the active calendar (best-effort)
        let tasks = if warning.is_none() {
            if let Some(ref h) = active_href {
                client.get_tasks(h).await.unwrap_or_default()
            } else {
                vec![]
            }
        } else if let Some(ref h) = active_href {
            // Fallback: load tasks from cache + apply journal if present
            let (mut t, _) = Cache::load(client.ctx.as_ref(), h).unwrap_or((vec![], None));
            Journal::apply_to_tasks(client.ctx.as_ref(), &mut t, h);
            t
        } else {
            vec![]
        };

        Ok((client, calendars, tasks, active_href, warning))
    }

    // Helper to encapsulate the core discovery logic (used by get_calendars)
    async fn perform_calendar_discovery(
        &self,
        _discovery_path: &str,
    ) -> Result<Vec<CalendarListEntry>, String> {
        let client = self.client.as_ref().ok_or("Offline")?;

        let principal_res = client
            .find_current_user_principal()
            .await
            .map_err(|e| format!("{:?}", e))?;
        let Some(principal) = principal_res else {
            return Err("No current user principal found.".to_string());
        };

        let home_set_resp = client
            .request(FindCalendarHomeSet::new(&principal))
            .await
            .map_err(|e| format!("{:?}", e))?;

        let home_url = home_set_resp
            .home_sets
            .first()
            .ok_or("No calendar home set found.")?;

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

            let comps = self
                .get_supported_components(&col.href)
                .await
                .unwrap_or_default();

            if comps.iter().any(|c| c.eq_ignore_ascii_case("VTODO")) {
                calendars.push(CalendarListEntry {
                    name,
                    href: col.href,
                    color,
                });
            }
        }
        Ok(calendars)
    }

    /// Get calendars (remote + local), with optional auto-corrected URL returned.
    pub async fn get_calendars(&self) -> Result<(Vec<CalendarListEntry>, Option<String>), String> {
        if let Some(_client) = &self.client {
            // attempt discovery at configured path
            let user_configured_path = self.client.as_ref().unwrap().base_url().path();
            let mut corrected_url = None;

            let mut calendars = self
                .perform_calendar_discovery(user_configured_path)
                .await
                .map_err(|e| format!("{:?}", e))?;

            // Fallback: if nothing found, try server root and offer corrected root URL
            if calendars.is_empty()
                && user_configured_path != "/"
                && let Ok(fallback) = self.perform_calendar_discovery("/").await
                && !fallback.is_empty()
            {
                calendars = fallback;
                let base_uri = self.client.as_ref().unwrap().base_url();
                if let (Some(scheme), Some(authority)) = (base_uri.scheme(), base_uri.authority()) {
                    corrected_url = Some(format!("{}://{}", scheme, authority));
                }
            }

            // Include local calendars; but only show recovery if it contains tasks
            if let Ok(local_cals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
                for local_cal in local_cals {
                    if local_cal.href == "local://recovery" {
                        if let Ok(tasks) =
                            LocalStorage::load_for_href(self.ctx.as_ref(), &local_cal.href)
                            && !tasks.is_empty()
                        {
                            calendars.push(local_cal);
                        }
                    } else {
                        calendars.push(local_cal);
                    }
                }
            }

            Ok((calendars, corrected_url))
        } else {
            // Offline mode: return cached + local calendars
            let mut calendars = Cache::load_calendars(self.ctx.as_ref()).unwrap_or_default();
            if let Ok(local_cals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
                for local_cal in local_cals {
                    if calendars.iter().any(|c| c.href == local_cal.href) {
                        continue;
                    }
                    if local_cal.href == "local://recovery" {
                        if let Ok(tasks) =
                            LocalStorage::load_for_href(self.ctx.as_ref(), &local_cal.href)
                            && !tasks.is_empty()
                        {
                            calendars.push(local_cal);
                        }
                    } else {
                        calendars.push(local_cal);
                    }
                }
            }
            Ok((calendars, None))
        }
    }

    pub async fn get_supported_components(
        &self,
        calendar_href: &str,
    ) -> Result<Vec<String>, String> {
        if let Some(_client) = &self.client {
            let req = Propfind::new(calendar_href)
                .with_properties(&[&names::SUPPORTED_CALENDAR_COMPONENT_SET])
                .with_depth(libdav::Depth::Zero);
            let response = self
                .client
                .as_ref()
                .unwrap()
                .request(req)
                .await
                .map_err(|e| e.to_string())?;
            let xml_str = std::str::from_utf8(&response.body).map_err(|e| e.to_string())?;
            let doc = Document::parse(xml_str).map_err(|e| e.to_string())?;
            let mut components = Vec::new();
            for node in doc.descendants() {
                if node.tag_name().name().eq_ignore_ascii_case("comp")
                    && let Some(name) = node.attribute("name")
                {
                    components.push(name.to_uppercase());
                }
            }
            Ok(components)
        } else {
            Err("Offline".to_string())
        }
    }

    /// Sync companion event for a single task (creates/updates/deletes calendar event
    /// corresponding to the task). This is high-level convenience; underlying network
    /// errors are mapped to boolean success/failure.
    pub(crate) async fn sync_companion_event(
        &self,
        task: &Task,
        config_enabled: bool,
        delete_on_completion: bool,
        is_delete_intent: bool,
    ) -> bool {
        // Local calendars don't have server-side events
        if task.calendar_href.starts_with("local://") {
            return false;
        }

        let should_create_events = task.create_event.unwrap_or(config_enabled);
        let event_uid = format!("evt-{}", task.uid);
        let filename = format!("{}.ics", event_uid);

        let cal_path = if task.calendar_href.ends_with('/') {
            task.calendar_href.clone()
        } else {
            let p = strip_host(&task.href);
            if let Some(idx) = p.rfind('/') {
                p[..=idx].to_string()
            } else {
                task.calendar_href.clone()
            }
        };
        let event_path = format!("{}{}", strip_host(&cal_path), filename);

        let client = match &self.client {
            Some(c) => c,
            None => return false,
        };

        let has_dates = task.due.is_some() || task.dtstart.is_some();
        let keep_completed = !delete_on_completion && task.status.is_done();

        let should_delete = is_delete_intent
            || (delete_on_completion && task.status.is_done())
            || (!has_dates && !keep_completed)
            || !should_create_events;

        if should_delete {
            return match client.request(Delete::new(&event_path).force()).await {
                Ok(_) => true,
                Err(WebDavError::BadStatusCode(http::StatusCode::NOT_FOUND)) => false,
                Err(_) => false,
            };
        } else if let Some((_, ics_body)) = IcsAdapter::to_event_ics(task) {
            let create_req =
                PutResource::new(&event_path).create(ics_body.clone(), "text/calendar");
            match client.request(create_req).await {
                Ok(_) => return true,
                Err(WebDavError::BadStatusCode(http::StatusCode::PRECONDITION_FAILED))
                | Err(WebDavError::PreconditionFailed(_)) => {
                    let update_req =
                        PutResource::new(&event_path).update(ics_body, "text/calendar", "");
                    return client.request(update_req).await.is_ok();
                }
                Err(_) => return false,
            }
        }
        false
    }

    /// Public wrapper for convenience
    pub async fn sync_task_companion_event(
        &self,
        task: &Task,
        config_enabled: bool,
    ) -> Result<bool, String> {
        let cfg = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let delete_on_completion = cfg.delete_events_on_completion;
        let res = self
            .sync_companion_event(task, config_enabled, delete_on_completion, false)
            .await;
        Ok(res)
    }

    pub(crate) async fn fetch_remote_task(&self, task_href: &str) -> Option<Task> {
        // Test hook injection (tests can override)
        #[cfg(any(test, feature = "test_hooks"))]
        {
            if let Some(h) = TEST_FETCH_REMOTE_HOOK.get()
                && let Some(cb) = &*h.lock().unwrap()
            {
                return cb(task_href);
            }
        }

        if let Some(client) = &self.client {
            let path_href = strip_host(task_href);
            let parent_path = if let Some(idx) = path_href.rfind('/') {
                &path_href[..=idx]
            } else {
                "/"
            };

            let req = GetCalendarResources::new(parent_path).with_hrefs(vec![path_href.clone()]);

            if let Ok(resp) = client.request(req).await
                && let Some(item) = resp.resources.into_iter().next()
                && let Ok(content) = item.content
            {
                return IcsAdapter::from_ics(
                    &content.data,
                    content.etag,
                    item.href,
                    parent_path.to_string(),
                )
                .ok();
            }
        }
        None
    }

    async fn fetch_calendar_tasks_internal(
        &self,
        calendar_href: &str,
        apply_journal: bool,
    ) -> Result<Vec<Task>, String> {
        // Local calendar short-circuit
        if calendar_href.starts_with("local://") {
            let mut tasks = LocalStorage::load_for_href(self.ctx.as_ref(), calendar_href)
                .map_err(|e| e.to_string())?;
            if apply_journal {
                Journal::apply_to_tasks(self.ctx.as_ref(), &mut tasks, calendar_href);
            }
            return Ok(tasks);
        }

        // Attempt to load cache and compare tokens
        let (mut cached_tasks, cached_token) =
            Cache::load(self.ctx.as_ref(), calendar_href).unwrap_or((vec![], None));

        if let Some(client) = &self.client {
            let path_href = strip_host(calendar_href);

            // Build a pending-deletions set from the in-disk journal (if requested)
            let pending_deletions = if apply_journal {
                let journal = Journal::load(self.ctx.as_ref());
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

            // Fetch remote sync token
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

            // Fast-path: if tokens match and there are no unsynced "ghosts"
            let has_ghosts = cached_tasks
                .iter()
                .any(|t| t.etag.is_empty() && !t.href.is_empty());
            if !has_ghosts
                && let (Some(r_tok), Some(c_tok)) = (&remote_token, &cached_token)
                && r_tok == c_tok
            {
                if apply_journal {
                    Journal::apply_to_tasks(self.ctx.as_ref(), &mut cached_tasks, calendar_href);
                }
                return Ok(cached_tasks);
            }

            // Otherwise, enumerate & multiget as needed
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

            for (_href, task) in cache_map {
                let is_unsynced = task.etag.is_empty() || task.href.is_empty();
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
                        && let Ok(task) = IcsAdapter::from_ics(
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
                Journal::apply_to_tasks(self.ctx.as_ref(), &mut final_tasks, calendar_href);
            }
            let _ = Cache::save(self.ctx.as_ref(), calendar_href, &final_tasks, remote_token);
            Ok(final_tasks)
        } else {
            if apply_journal {
                Journal::apply_to_tasks(self.ctx.as_ref(), &mut cached_tasks, calendar_href);
            }
            Ok(cached_tasks)
        }
    }

    // --- High-level public APIs that UIs call ---
    // These functions push to the Journal for remote calendars (or write local storage
    // for local calendars), then trigger `sync_journal()` which is implemented in the
    // dedicated sync module. Keeping the optimistic in-memory store updates should
    // happen at the Store/Controller layer (higher up) — these functions perform the
    // persistence/journaling/network steps.

    pub async fn get_tasks(&self, calendar_href: &str) -> Result<Vec<Task>, String> {
        // Best-effort: ensure pending journal processed first
        let _ = self.sync_journal().await;
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
        if task.calendar_href.starts_with("local://") {
            let mut all = LocalStorage::load_for_href(self.ctx.as_ref(), &task.calendar_href)
                .map_err(|e| e.to_string())?;
            all.push(task.clone());
            LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &all)
                .map_err(|e| e.to_string())?;
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

        Journal::push(self.ctx.as_ref(), Action::Create(task.clone()))
            .map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn update_task(&self, task: &mut Task) -> Result<Vec<String>, String> {
        task.sequence += 1;

        if task.calendar_href.starts_with("local://") {
            let mut all = LocalStorage::load_for_href(self.ctx.as_ref(), &task.calendar_href)
                .map_err(|e| e.to_string())?;
            if let Some(idx) = all.iter().position(|t| t.uid == task.uid) {
                all[idx] = task.clone();
                LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &all)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(vec![]);
        }

        Journal::push(self.ctx.as_ref(), Action::Update(task.clone()))
            .map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn delete_task(&self, task: &Task) -> Result<Vec<String>, String> {
        if task.calendar_href.starts_with("local://") {
            let mut all = LocalStorage::load_for_href(self.ctx.as_ref(), &task.calendar_href)
                .map_err(|e| e.to_string())?;
            all.retain(|t| t.uid != task.uid);
            LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &all)
                .map_err(|e| e.to_string())?;
            return Ok(vec![]);
        }

        Journal::push(self.ctx.as_ref(), Action::Delete(task.clone()))
            .map_err(|e| e.to_string())?;
        self.sync_journal().await
    }

    pub async fn toggle_task(
        &self,
        task: &mut Task,
    ) -> Result<(Task, Option<Task>, Vec<String>), String> {
        // This method implements the higher-level recurring/termination logic and then
        // delegates persistence/sync to the usual create/update paths.
        let mut logs = Vec::new();
        let mut history_snapshot = None;

        if task.status == TaskStatus::Completed && task.rrule.is_some() {
            let mut snapshot = task.clone();
            snapshot.uid = Uuid::new_v4().to_string();
            snapshot.href = String::new();
            snapshot.etag = String::new();
            snapshot.status = TaskStatus::Completed;
            snapshot.percent_complete = Some(100);
            snapshot.rrule = None;
            snapshot.alarms.clear();
            snapshot.create_event = None;
            snapshot.related_to.push(task.uid.clone());

            if !snapshot
                .unmapped_properties
                .iter()
                .any(|p| p.key == "COMPLETED")
            {
                let now_str = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
                snapshot
                    .unmapped_properties
                    .push(crate::model::RawProperty {
                        key: "COMPLETED".to_string(),
                        value: now_str,
                        params: vec![],
                    });
            }

            if crate::model::RecurrenceEngine::advance(task) {
                history_snapshot = Some(snapshot);
            }
        }

        // Local path short-circuit
        if task.calendar_href.starts_with("local://") {
            let mut all = LocalStorage::load_for_href(self.ctx.as_ref(), &task.calendar_href)
                .map_err(|e| e.to_string())?;

            if let Some(ref mut snap) = history_snapshot {
                all.push(snap.clone());
            }

            if let Some(idx) = all.iter().position(|t| t.uid == task.uid) {
                all[idx] = task.clone();
            }

            LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &all)
                .map_err(|e| e.to_string())?;
            return Ok((task.clone(), history_snapshot, vec![]));
        }

        // Remote path: create history snapshot then update main task
        if let Some(ref mut snap) = history_snapshot {
            let create_logs = self.create_task(snap).await?;
            logs.extend(create_logs);
        }

        let update_logs = self.update_task(task).await?;
        logs.extend(update_logs);

        Ok((task.clone(), history_snapshot, logs))
    }

    pub async fn terminate_task(
        &self,
        task: &mut Task,
        status: TaskStatus,
    ) -> Result<(Task, Option<Task>, Vec<String>), String> {
        let (primary, secondary) = task.recycle(status);
        let mut logs: Vec<String> = Vec::new();

        if primary.uid != task.uid {
            let mut p = primary.clone();
            let l = self.create_task(&mut p).await?;
            logs.extend(l);
        } else {
            let mut p = primary.clone();
            let l = self.update_task(&mut p).await?;
            logs.extend(l);
        }

        if let Some(sec) = &secondary {
            let mut s = sec.clone();
            let l = self.update_task(&mut s).await?;
            logs.extend(l);
        }

        Ok((primary, secondary, logs))
    }

    pub async fn move_task(
        &self,
        task: &Task,
        new_calendar_href: &str,
    ) -> Result<(Task, Vec<String>), String> {
        // Local->remote migration: attempt create+verify, otherwise journal a move.
        if task.calendar_href.starts_with("local://") {
            let mut new_task = task.clone();
            new_task.calendar_href = new_calendar_href.to_string();
            let cal_path = new_calendar_href.trim_end_matches('/');
            let filename = format!("{}.ics", task.uid);
            let expected_remote_href = format!("{}/{}", cal_path, filename);

            new_task.href = String::new();
            new_task.etag = String::new();

            let logs = self.create_task(&mut new_task).await?;

            let verify_target = if !new_task.href.is_empty() {
                new_task.href.clone()
            } else {
                expected_remote_href.clone()
            };

            match self.fetch_remote_task(&verify_target).await {
                Some(remote_task) => {
                    if remote_task.uid != task.uid {
                        return Err(format!(
                            "Migration Verification Failed: Server returned UID '{}' but uploaded '{}'. Local preserved.",
                            remote_task.uid, task.uid
                        ));
                    }
                    let _ = self.delete_task(task).await?;
                    return Ok((remote_task, logs));
                }
                None => {
                    return Err(format!(
                        "Migration Verification Failed: Task '{}' uploaded but not retrievable immediately. Local preserved.",
                        task.summary
                    ));
                }
            }
        }

        // Otherwise, queue a Move action in the journal and trigger sync.
        Journal::push(
            self.ctx.as_ref(),
            Action::Move(task.clone(), new_calendar_href.to_string()),
        )
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

        let mut stream = stream::iter(futures).buffer_unordered(1);
        let mut count = 0;
        while let Some(res) = stream.next().await {
            if res.is_some() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub(crate) async fn fetch_etag(&self, path: &str) -> Option<String> {
        if let Some(client) = &self.client
            && let Ok(resp) = client
                .request(GetProperty::new(path, &names::GETETAG))
                .await
        {
            return resp.value;
        }
        None
    }

    // Note: The following methods are implemented in src/client/sync.rs:
    // - handle_create, handle_update, handle_delete, handle_move
    // - sync_journal
    // - attempt_conflict_resolution
    // - execute_move
    //
    // They remain part of the RustyClient impl but are defined in the dedicated
    // sync module to avoid duplication and to keep the synchronization logic
    // consolidated.
}
