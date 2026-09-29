// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration tests for the settings-synchronization feature (FR #91).
//!
//! These drive `TaskController::sync_settings()` end-to-end against a mocked
//! CalDAV server (mockito) and assert on the observable side effects: the
//! `SettingsMeta` cache, the in-memory store, and the write journal.
//!
//! Two storage strategies are covered:
//!   * **Property mode** — the settings payload is PROPPATCHed onto the default
//!     calendar collection as custom properties in the Cfait namespace. This is
//!     invisible to standard CalDAV clients.
//!   * **Object mode (fallback)** — for servers without custom-property support,
//!     the settings live in a 1970 CANCELLED VEVENT carrier that most clients
//!     filter out by time range.
//!
//! The carrier object is only ever *journaled* here; the actual PUT happens
//! later in `sync_journal`, so no PUT mocks are needed.

use cfait::cache::Cache;
use cfait::client::core::RustyClient;
use cfait::config::{
    Config, SettingsPayload, CFAIT_NS, SETTINGS_CATEGORY, SETTINGS_SUMMARY, SETTINGS_UID,
};
use cfait::context::{AppContext, TestContext};
use cfait::controller::TaskController;
use cfait::journal::{Action, Journal};
use cfait::model::{Task, TaskStatus};
use cfait::store::TaskStore;
use mockito::{Matcher, Server};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

/// Calendar collection that hosts the settings in every scenario.
const TARGET: &str = "/calendars/u/cal1";
/// A different collection, used by the "collection was deleted" scenario.
const OLD_TARGET: &str = "/calendars/u/old-cal";

/// Build an online client rooted at the mock server. The calendar `href`s used
/// in the tests are plain paths, so all requests (PROPPATCH / PROPFIND /
/// REPORT) land on those exact paths.
async fn build_client(ctx: &Arc<dyn AppContext>, url: &str) -> RustyClient {
    RustyClient::new(ctx.clone(), url, "u", "p", false, None).expect("client construction")
}

fn save_config(ctx: &Arc<dyn AppContext>, default_calendar: Option<&str>, updated_at: i64) {
    let mut config = Config::default();
    config.sync_settings = true;
    config.default_calendar = default_calendar.map(|s| s.to_string());
    config.settings_updated_at = updated_at;
    config.save(ctx.as_ref()).expect("save config");
}

/// Wire up a store, client slot and controller around a context.
struct Harness {
    ctx: Arc<dyn AppContext>,
    store: Arc<TokioMutex<TaskStore>>,
    controller: TaskController,
}

async fn harness(ctx: Arc<dyn AppContext>, client: Option<RustyClient>) -> Harness {
    let store = Arc::new(TokioMutex::new(TaskStore::new(ctx.clone())));
    let slot = Arc::new(TokioMutex::new(client));
    let controller = TaskController::new(store.clone(), slot, ctx.clone());
    Harness { ctx, store, controller }
}

/// Register the three mocks that make a custom-property probe succeed on `path`:
/// a PROPPATCH that sets the probe property (capturing its UUID), a PROPFIND
/// that reads the probe back (echoing the captured UUID), and a PROPPATCH that
/// removes it.
async fn mock_probe_supported(server: &mut Server, path: &str) {
    let probe_uuid: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let pu = probe_uuid.clone();
    let _ = server
        .mock("PROPPATCH", path)
        .match_body(Matcher::Regex("<CF:probe>".to_string()))
        .with_status(200)
        .with_body_from_request(move |req: &mockito::Request| {
            if let Ok(body) = req.utf8_lossy_body() {
                let open = "<CF:probe>";
                let close = "</CF:probe>";
                if let Some(s) = body.find(open) {
                    let start = s + open.len();
                    if let Some(e) = body[start..].find(close) {
                        let end = start + e;
                        if end > start {
                            *pu.lock().unwrap() = Some(body[start..end].to_string());
                        }
                    }
                }
            }
            Vec::new()
        })
        .create_async()
        .await;

    let pu2 = probe_uuid.clone();
    let _ = server
        .mock("PROPFIND", path)
        .match_body(Matcher::Regex("CF:probe".to_string()))
        .with_status(207)
        .with_body_from_request(move |_req: &mockito::Request| {
            let uuid = pu2.lock().unwrap().clone().unwrap_or_default();
            format!(
                r#"<D:multistatus xmlns:D="DAV:" xmlns:CF="{CFAIT_NS}"><D:response><D:propstat><D:prop><CF:probe>{uuid}</CF:probe></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#
            )
            .into_bytes()
        })
        .create_async()
        .await;

    let _ = server
        .mock("PROPPATCH", path)
        .match_body(Matcher::Regex("D:remove".to_string()))
        .with_status(200)
        .create_async()
        .await;
}

/// Mock a REPORT (calendar-query) that finds no settings carrier object.
async fn mock_no_carrier_object(server: &mut Server, path: &str) {
    let _ = server
        .mock("REPORT", path)
        .with_status(207)
        .with_body(r#"<D:multistatus xmlns:D="DAV:"></D:multistatus>"#)
        .expect(2) // VEVENT and VTODO are both queried
        .create_async()
        .await;
}

/// Extract and XML-unescape the value of a `<CF:name>…</CF:name>` property from
/// a PROPPATCH body.
fn extract_prop(body: &str, name: &str) -> Option<String> {
    let open = format!("<CF:{name}>");
    let close = format!("</CF:{name}>");
    let s = body.find(&open)? + open.len();
    let e = body[s..].find(&close)? + s;
    let raw = &body[s..e];
    Some(
        raw.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// First sync on a property-capable server: the settings are pushed as a
/// PROPPATCH onto the collection and NO carrier object is created or journaled.
#[tokio::test]
async fn property_mode_first_sync_writes_property_not_carrier() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());
    save_config(&ctx, Some(TARGET), 0);

    mock_probe_supported(&mut server, TARGET).await;
    mock_no_carrier_object(&mut server, TARGET).await;

    // Settings read: empty (first sync, nothing on the server yet).
    let _m_settings_read = server
        .mock("PROPFIND", TARGET)
        .match_body(Matcher::Regex("CF:settings".to_string()))
        .with_status(207)
        .with_body(r#"<D:multistatus xmlns:D="DAV:"></D:multistatus>"#)
        .create_async()
        .await;

    // Settings write: capture the JSON payload for inspection.
    let settings_body: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let sb = settings_body.clone();
    let m_settings_write = server
        .mock("PROPPATCH", TARGET)
        .match_body(Matcher::Regex("CF:settings".to_string()))
        .with_status(200)
        .with_body_from_request(move |req: &mockito::Request| {
            if let Ok(b) = req.utf8_lossy_body() {
                *sb.lock().unwrap() = Some(b.into_owned());
            }
            Vec::new()
        })
        .create_async()
        .await;

    let client = build_client(&ctx, &url).await;
    let h = harness(ctx.clone(), Some(client)).await;

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(changed, "first sync should report a change");

    // Property mode engaged and recorded.
    let meta = Cache::load_settings_meta(h.ctx.as_ref());
    assert_eq!(meta.property_href.as_deref(), Some(TARGET));
    assert_eq!(meta.probed.get(TARGET), Some(&true));

    // No carrier object in the store and nothing journaled.
    {
        let s = h.store.lock().await;
        assert!(s.get_task_ref(SETTINGS_UID).is_none());
    }
    assert!(
        Journal::load(h.ctx.as_ref()).queue.is_empty(),
        "property mode must not journal a carrier"
    );

    // The settings PROPPATCH carried a well-formed payload matching the config.
    m_settings_write.assert();
    let body = settings_body.lock().unwrap().clone().expect("settings PROPPATCH captured");
    let json = extract_prop(&body, "settings").expect("settings property present");
    let payload: SettingsPayload = serde_json::from_str(&json).expect("payload is valid JSON");
    assert!(payload.updated_at > 0, "payload must carry a stamped revision");
    assert_eq!(payload.config.default_calendar.as_deref(), Some(TARGET));

    _m_settings_read.assert();
}

/// When the probe is rejected (4xx), the server is treated as not supporting
/// custom properties and the settings fall back to a VEVENT carrier that is
/// journaled for upload.
#[tokio::test]
async fn probe_rejected_falls_back_to_vevent_carrier() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());
    save_config(&ctx, Some(TARGET), 0);

    // The probe PROPPATCH is rejected → custom properties unsupported.
    let m_probe = server
        .mock("PROPPATCH", TARGET)
        .match_body(Matcher::Regex("<CF:probe>".to_string()))
        .with_status(403)
        .with_body("custom properties not supported")
        .create_async()
        .await;
    mock_no_carrier_object(&mut server, TARGET).await;

    let client = build_client(&ctx, &url).await;
    let h = harness(ctx.clone(), Some(client)).await;

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(changed, "fallback sync should report a change");

    // Demoted to object mode.
    let meta = Cache::load_settings_meta(h.ctx.as_ref());
    assert_eq!(meta.property_href, None);
    assert_eq!(meta.probed.get(TARGET), Some(&false));

    // A VEVENT carrier was created in the store…
    let task = {
        let s = h.store.lock().await;
        s.get_task_ref(SETTINGS_UID).cloned().expect("carrier in store")
    };
    assert!(task.is_event, "carrier must be a VEVENT");
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert!(task
        .categories
        .iter()
        .any(|c| c == SETTINGS_CATEGORY));
    let payload: SettingsPayload =
        serde_json::from_str(&task.description).expect("carrier description is a payload");
    assert!(payload.updated_at > 0);

    // …and journaled as a Create.
    assert!(Journal::load(h.ctx.as_ref())
        .queue
        .iter()
        .any(|a| matches!(a, Action::Create(t) if t.uid == SETTINGS_UID)));

    m_probe.assert();
}

/// If the collection that currently hosts the settings property is no longer
/// the target (deleted or default changed), the stale property is removed from
/// the old collection and the settings are re-provisioned on the new one.
#[tokio::test]
async fn collection_deleted_reprovisions_on_new_target() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());
    // Default moved to a new collection.
    save_config(&ctx, Some(TARGET), 0);
    // The settings property currently lives on the (now gone) old collection.
    let mut meta = cfait::cache::SettingsMeta::default();
    meta.property_href = Some(OLD_TARGET.to_string());
    Cache::save_settings_meta(ctx.as_ref(), &meta).expect("save meta");

    // Best-effort removal of the two stale properties from the old collection.
    let m_old_remove = server
        .mock("PROPPATCH", OLD_TARGET)
        .match_body(Matcher::Regex("D:remove".to_string()))
        .with_status(200)
        .expect(2)
        .create_async()
        .await;

    // Probe + write on the new target.
    mock_probe_supported(&mut server, TARGET).await;
    mock_no_carrier_object(&mut server, TARGET).await;
    let _m_settings_read = server
        .mock("PROPFIND", TARGET)
        .match_body(Matcher::Regex("CF:settings".to_string()))
        .with_status(207)
        .with_body(r#"<D:multistatus xmlns:D="DAV:"></D:multistatus>"#)
        .create_async()
        .await;
    let m_settings_write = server
        .mock("PROPPATCH", TARGET)
        .match_body(Matcher::Regex("CF:settings".to_string()))
        .with_status(200)
        .create_async()
        .await;

    let client = build_client(&ctx, &url).await;
    let h = harness(ctx.clone(), Some(client)).await;

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(changed, "re-provisioning should report a change");

    // The property now lives on the new target.
    let meta = Cache::load_settings_meta(h.ctx.as_ref());
    assert_eq!(meta.property_href.as_deref(), Some(TARGET));
    assert_eq!(meta.probed.get(TARGET), Some(&true));

    // The stale property was removed from the old collection…
    m_old_remove.assert();
    // …and written to the new one.
    m_settings_write.assert();

    // No carrier involved.
    {
        let s = h.store.lock().await;
        assert!(s.get_task_ref(SETTINGS_UID).is_none());
    }
    assert!(Journal::load(h.ctx.as_ref()).queue.is_empty());
}

/// When the local and remote payloads agree and we are already in property
/// mode, a sync is a no-op: no probe, no write, no carrier.
#[tokio::test]
async fn in_sync_property_mode_is_a_noop() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());
    let mut config = Config::default();
    config.sync_settings = true;
    config.default_calendar = Some(TARGET.to_string());
    config.settings_updated_at = 1000;
    let local_syncable = config.get_syncable();
    config.save(ctx.as_ref()).expect("save config");

    // Already in property mode for this target → no probe is issued.
    let mut meta = cfait::cache::SettingsMeta::default();
    meta.property_href = Some(TARGET.to_string());
    Cache::save_settings_meta(ctx.as_ref(), &meta).expect("save meta");

    // The server holds a payload identical to the local one.
    let payload = SettingsPayload {
        updated_at: 1000,
        config: local_syncable,
    };
    let json = serde_json::to_string(&payload).expect("serialize payload");
    let _m_settings_read = server
        .mock("PROPFIND", TARGET)
        .match_body(Matcher::Regex("CF:settings".to_string()))
        .with_status(207)
        .with_body(format!(
            r#"<D:multistatus xmlns:D="DAV:" xmlns:CF="{CFAIT_NS}"><D:response><D:propstat><D:prop><CF:settings>{json}</CF:settings><CF:settings-rev>1000</CF:settings-rev></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#
        ))
        .create_async()
        .await;
    mock_no_carrier_object(&mut server, TARGET).await;

    // Deliberately NO settings-write PROPPATCH mock: any spurious write would
    // 404 and demote to object mode, which the assertions below would catch.

    let client = build_client(&ctx, &url).await;
    let h = harness(ctx.clone(), Some(client)).await;

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(!changed, "in-sync must not report a change");

    let meta = Cache::load_settings_meta(h.ctx.as_ref());
    assert_eq!(meta.property_href.as_deref(), Some(TARGET));

    {
        let s = h.store.lock().await;
        assert!(s.get_task_ref(SETTINGS_UID).is_none());
    }
    assert!(
        Journal::load(h.ctx.as_ref()).queue.is_empty(),
        "in-sync must not journal anything"
    );
    _m_settings_read.assert();
}

/// Offline in property mode is a no-op: the settings already live on the
/// server, so nothing is journaled.
#[tokio::test]
async fn offline_property_mode_is_a_noop() {
    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());
    save_config(&ctx, Some(TARGET), 0);

    let mut meta = cfait::cache::SettingsMeta::default();
    meta.property_href = Some(TARGET.to_string());
    Cache::save_settings_meta(ctx.as_ref(), &meta).expect("save meta");

    // No client → offline.
    let h = harness(ctx.clone(), None).await;

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(!changed, "offline property mode must be a no-op");
    assert!(Journal::load(h.ctx.as_ref()).queue.is_empty());
}

/// Offline in object (fallback) mode keeps the carrier object up to date so it
/// is journaled for upload on reconnect.
#[tokio::test]
async fn offline_object_mode_journals_carrier_update() {
    let ctx: Arc<dyn AppContext> = Arc::new(TestContext::new());

    let mut config = Config::default();
    config.sync_settings = true;
    config.default_calendar = Some(TARGET.to_string());
    config.settings_updated_at = 1000;
    let local_syncable = config.get_syncable();
    config.save(ctx.as_ref()).expect("save config");

    // Object mode (no property_href).
    let meta = cfait::cache::SettingsMeta::default();
    Cache::save_settings_meta(ctx.as_ref(), &meta).expect("save meta");

    let h = harness(ctx.clone(), None).await;

    // Seed a carrier that is older than the local config (→ SyncUp).
    let mut existing = Task::new(SETTINGS_SUMMARY, &std::collections::HashMap::new(), None);
    existing.uid = SETTINGS_UID.to_string();
    existing.status = TaskStatus::Cancelled;
    existing.is_event = true;
    existing.categories = vec![SETTINGS_CATEGORY.to_string()];
    existing.calendar_href = TARGET.to_string();
    existing.href = format!("{TARGET}/cfait-global-settings-v1.ics");
    existing.etag = "old-etag".to_string();
    existing.sequence = 3;
    existing.description = serde_json::to_string(&SettingsPayload {
        updated_at: 500,
        config: local_syncable,
    })
    .expect("serialize carrier payload");
    {
        let mut s = h.store.lock().await;
        s.update_or_add_task(existing);
    }

    let changed = h.controller.sync_settings().await.expect("sync_settings");
    assert!(changed, "offline object-mode update should report a change");

    // The carrier was journaled as an Update (not a fresh Create).
    assert!(Journal::load(h.ctx.as_ref())
        .queue
        .iter()
        .any(|a| matches!(a, Action::Update(t) if t.uid == SETTINGS_UID)));

    // The store copy was re-anchored with the newer revision.
    let task = {
        let s = h.store.lock().await;
        s.get_task_ref(SETTINGS_UID).cloned().expect("carrier in store")
    };
    let payload: SettingsPayload =
        serde_json::from_str(&task.description).expect("carrier description is a payload");
    assert_eq!(payload.updated_at, 1000);
}
