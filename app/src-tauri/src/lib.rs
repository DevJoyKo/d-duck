//! Tauri shell: window/tray lifecycle, background workers and IPC commands.
//!
//! The webview is untrusted (D-015, D-030): every security decision — dangerous permission modes, CLI path and
//! config dir selection, notifications — happens here in Rust. Nothing in this file logs transcript content.

mod add_items;
mod db_pool;
mod ext_items_cmds;
mod limit_cmds;
mod login_cmds;
mod memory_cmds;
mod projects_cmds;
mod report_save_cmds;
mod delivery_cmds;
mod lifecycle_cmds;
mod timing;
mod workflow_check_cmds;

use dduck_core::i18n::Fail;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use dduck_core::analytics::{self, Bucket, Dimension, Query};
use dduck_core::claude_cli::{AgentSession, ClaudeCli, CliError, DangerApproval, PermissionMode, SessionKind, TaskSpec};
use dduck_core::cli_path::{self, CliInfo, ConfigDir, ShellProbe};
use dduck_core::pricing::PriceTable;
use dduck_core::routines::{self, RoutineInput};
use dduck_core::agents_catalog::{self, AgentDef};
use dduck_core::i18n::{Code as C, Params as P, ReasonCode, tr, trp};
use dduck_core::{artifacts, db, discover, i18n, ingest, live_agents, settings, task_meta, tracker};
use rusqlite::Connection;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;

const POLL_VISIBLE: Duration = Duration::from_secs(3);
const POLL_HIDDEN: Duration = Duration::from_secs(10);
/// After a start the new task shows up within about a second (D-159).
const POLL_FAST: Duration = Duration::from_secs(1);
const FAST_POLL_FOR_MS: i64 = 30_000;

/// The poller's next wait: every second for a while after a start (visible or not), else the usual pace.
fn poll_interval(visible: bool, now_ms: i64, fast_until_ms: i64) -> Duration {
    if now_ms < fast_until_ms {
        POLL_FAST
    } else if visible {
        POLL_VISIBLE
    } else {
        POLL_HIDDEN
    }
}

/// The poller thread, so a start can wake it at once instead of waiting out its sleep.
static POLLER: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();
static FAST_POLL_UNTIL_MS: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// A task was just started: poll now and then every second for [`FAST_POLL_FOR_MS`].
fn poll_soon() {
    FAST_POLL_UNTIL_MS.store(chrono_now_ms() + FAST_POLL_FOR_MS, Ordering::Relaxed);
    if let Some(t) = POLLER.get() {
        t.unpark();
    }
}
const RESCAN_EVERY: Duration = Duration::from_secs(30);
const TRAY_ID: &str = "main";

/// Messages for the ingestion worker.
enum IngestMsg {
    /// Files changed (from the watcher) — ingest what is new.
    Changed,
    /// Drop all usage data and re-read every transcript.
    Rebuild,
    /// Config dir changed: re-point the watcher and rebuild.
    Reconfigure(PathBuf),
    /// Title storage switched on/off. Done here so the scrub (checkpoint + VACUUM) runs on the writer connection,
    /// off the main thread (D-023, D-039).
    SetTitles(bool),
    /// Something the user typed was deleted or replaced (routine prompt): drop freed pages from DB and WAL (D-050).
    Scrub,
}

struct Cli {
    info: Option<CliInfo>,
    client: Option<ClaudeCli>,
}

pub struct AppState {
    /// One connection per caller (D-161): a nested call gets its own instead of waiting on itself.
    db: db_pool::DbPool,
    prices: PriceTable,
    config: RwLock<ConfigDir>,
    probe: ShellProbe,
    cli: RwLock<Cli>,
    agents: RwLock<Vec<AgentSession>>,
    agents_error: RwLock<Option<CliError>>,
    window_visible: AtomicBool,
    ingest_tx: Mutex<Sender<IngestMsg>>,
    tray_signature: Mutex<Option<Vec<(String, String)>>>,
    /// Last agent catalog sent to the webview (ids → files). Saves re-discover and compare (D-054).
    catalog: RwLock<Vec<AgentDef>>,
    /// One cloud-routine fetch at a time (D-066).
    cloud_lock: Mutex<()>,
    /// One usage-insights Claude run at a time, and its cancel flag (D-078).
    insights_busy: AtomicBool,
    insights_cancel: AtomicBool,
    /// "요구사항 다듬기" (brief-grilling, D-105): one run at a time, its cancel flag, and the consent the last native
    /// confirmation granted (rounds, money, expiry — enforced here, never by the webview).
    refine_busy: AtomicBool,
    refine_cancel: dduck_core::brief_refine::CancelEpoch,
    /// The deny-all hook self-test passed for this binary (D-159).
    refine_guard: dduck_core::brief_refine::SelfTestCache,
    refine_consent: Mutex<Option<dduck_core::brief_refine::Consent>>,
    /// One transcript timeline read at a time (security review MEDIUM).
    timeline_lock: Mutex<()>,
    /// `claude mcp list` launches every server: one at a time (held for the whole run), 15 s after the last one.
    mcp_busy: std::sync::atomic::AtomicBool,
    mcp_last: Mutex<Option<Instant>>,
    /// One plugin enable/disable/update at a time (D-075).
    plugin_busy: AtomicBool,
    /// One routine approval dialog at a time (D-049).
    confirm_lock: Mutex<()>,
    /// External links open at most once per second (D-053).
    last_open: Mutex<Option<Instant>>,
    /// Artifact card previews: in-memory scan and preview caches only (D-102).
    previews: dduck_core::artifact_preview::PreviewCache,
    /// Starts being prepared, by the webview's start id → cancel pressed (D-161). Only ids registered here can be
    /// cancelled, and each entry leaves when its start ends, so the map stays as small as the starts in flight.
    start_pending: Mutex<std::collections::HashMap<String, bool>>,
}

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl AppState {
    /// A connection from the pool. Never waits for another caller, the same thread included (D-161).
    fn conn(&self) -> Res<db_pool::DbConn<'_>> {
        self.db.get().map_err(|_| "database unavailable".to_string())
    }

    fn client(&self) -> Res<ClaudeCli> {
        self.cli
            .read()
            .ok()
            .and_then(|c| c.client.clone())
            .ok_or_else(|| Fail::CliNotFound.now())
    }

    fn resolve_cli(&self) {
        let home = home_dir();
        let setting = self.conn().ok().and_then(|c| settings::cli_path(&c).ok().flatten());
        let info = cli_path::resolve(setting.as_deref(), &home, &self.probe);
        let env = self.config.read().ok().and_then(|c| c.env_value().map(PathBuf::from));
        let client = info.as_ref().map(|i| ClaudeCli::new(i.path.clone(), env).with_path_env(self.probe.path.clone()));
        if let Ok(mut cli) = self.cli.write() {
            *cli = Cli { info, client };
        }
    }

    fn send_ingest(&self, msg: IngestMsg) {
        if let Ok(tx) = self.ingest_tx.lock() {
            let _ = tx.send(msg);
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

// ---------- ingestion worker ----------

fn spawn_ingest_worker(app: AppHandle, db_path: PathBuf, config: PathBuf, rx: Receiver<IngestMsg>, tx: Sender<IngestMsg>) {
    std::thread::spawn(move || {
        let mut root = config;
        let mut backoff = Duration::from_secs(5);
        loop {
            // Restart after a panic or a DB open failure, with exponential backoff; only a category is reported.
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| ingest_loop(&app, &db_path, &mut root, &rx, &tx)));
            match result {
                Ok(Ok(())) => return, // channel closed: app exiting
                Ok(Err(())) | Err(_) => {
                    let _ = app.emit("ingest-status", IngestStatus::error("worker_restarted"));
                    std::thread::sleep(backoff);
                    backoff = (backoff * 2).min(Duration::from_secs(300));
                }
            }
        }
    });
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestStatus {
    phase: &'static str,
    files_done: usize,
    files_total: usize,
    error: Option<&'static str>,
}

impl IngestStatus {
    fn error(category: &'static str) -> Self {
        IngestStatus { phase: "error", files_done: 0, files_total: 0, error: Some(category) }
    }
}

/// `Err(())` asks the supervisor to restart after a backoff.
fn ingest_loop(
    app: &AppHandle,
    db_path: &std::path::Path,
    config: &mut PathBuf,
    rx: &Receiver<IngestMsg>,
    tx: &Sender<IngestMsg>,
) -> Result<(), ()> {
    let Ok(mut conn) = db::open(db_path) else {
        let _ = app.emit("ingest-status", IngestStatus::error("db_open_failed"));
        return Err(());
    };
    let mut watcher = watch(config, tx.clone());
    if settings::get(&conn, db::SETTING_ARTIFACTS_BACKFILL).ok().flatten().as_deref() == Some("pending") {
        // Re-read every file once so artifacts in already-ingested transcripts are found; upserts keep usage intact.
        let _ = conn.execute("DELETE FROM ingest_files", []);
        run_pass(app, &mut conn, config, true);
        let _ = conn.execute("UPDATE settings SET value = 'done' WHERE key = ?1", [db::SETTING_ARTIFACTS_BACKFILL]);
    } else {
        run_pass(app, &mut conn, config, true);
    }
    let mut last_scan = Instant::now();
    // A non-Changed message pulled out while debouncing is kept here, never dropped.
    let mut pending: Option<IngestMsg> = None;
    loop {
        let msg = match pending.take() {
            Some(m) => Ok(m),
            None => rx.recv_timeout(RESCAN_EVERY.saturating_sub(last_scan.elapsed())),
        };
        match msg {
            Ok(IngestMsg::Changed) => {
                // Debounce bursts of file events.
                std::thread::sleep(Duration::from_secs(1));
                loop {
                    match rx.try_recv() {
                        Ok(IngestMsg::Changed) => continue,
                        Ok(other) => {
                            pending = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                run_pass(app, &mut conn, config, false);
            }
            Ok(IngestMsg::Scrub) => {
                let _ = ingest::scrub(&conn);
            }
            Ok(IngestMsg::SetTitles(enabled)) => {
                if ingest::set_store_titles(&conn, enabled).is_err() {
                    let _ = app.emit("ingest-status", IngestStatus::error("title_scrub_failed"));
                }
                let _ = app.emit("usage-updated", ());
            }
            Ok(IngestMsg::Rebuild) => {
                if clear_usage(&conn).is_ok() {
                    run_pass(app, &mut conn, config, true);
                }
            }
            Ok(IngestMsg::Reconfigure(dir)) => {
                *config = dir;
                watcher = watch(config, tx.clone());
                if clear_usage(&conn).is_ok() {
                    run_pass(app, &mut conn, config, true);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                run_pass(app, &mut conn, config, false);
                last_scan = Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => {
                drop(watcher);
                return Ok(());
            }
        }
    }
}

fn clear_usage(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         DELETE FROM api_calls; DELETE FROM tool_calls; DELETE FROM command_invocations; DELETE FROM subagents;
         DELETE FROM cost_segments; DELETE FROM sessions; DELETE FROM ingest_files; DELETE FROM artifacts;
         DELETE FROM tool_errors; DELETE FROM compactions;
         COMMIT;",
    )
}

fn watch(config: &std::path::Path, tx: Sender<IngestMsg>) -> Option<notify::RecommendedWatcher> {
    use notify::{RecursiveMode, Watcher};
    let root = discover::projects_root(config)?;
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        use notify::EventKind;
        // Only content changes count; access/open events (Linux inotify) would otherwise loop on our own reads.
        let Ok(ev) = res else { return };
        let relevant = matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_))
            && ev.paths.iter().any(|p| p.extension().is_some_and(|e| e == "jsonl" || e == "json"));
        if relevant {
            let _ = tx.send(IngestMsg::Changed);
        }
    })
    .ok()?;
    w.watch(&root, RecursiveMode::Recursive).ok()?;
    Some(w)
}

fn run_pass(app: &AppHandle, conn: &mut Connection, config: &std::path::Path, report_progress: bool) {
    let Some(root) = discover::projects_root(config) else {
        let _ = app.emit("ingest-status", IngestStatus::error("projects_dir_missing"));
        return;
    };
    let files = discover::discover(&root);
    let total = files.len();
    let mut changed = false;
    let mut ing = ingest::Ingestor::new(conn, ingest::IngestOptions::default());
    for (i, f) in files.iter().enumerate() {
        // An unreadable file is retried on the next pass; no content is reported.
        if let Ok(s) = ing.ingest_file(f, |_| {}) {
            changed |= s.lines > 0 || s.restarted;
        }
        if report_progress && (i % 10 == 0 || i + 1 == total) {
            let _ = app.emit("ingest-status", IngestStatus { phase: "backfill", files_done: i + 1, files_total: total, error: None });
        }
    }
    let _ = app.emit("ingest-status", IngestStatus { phase: "idle", files_done: total, files_total: total, error: None });
    if report_progress {
        // Limit stops found by the first full pass are history, not news (usage-limit-resume).
        limit_cmds::on_backfill_done(conn);
    }
    if changed || report_progress {
        let _ = app.emit("usage-updated", ());
    }
}

// ---------- agents poller ----------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentsPayload {
    sessions: Vec<AgentSession>,
    error: Option<CliError>,
    /// Persona and task type per session key.
    tags: std::collections::HashMap<String, task_meta::Tag>,
    /// Titles the user gave sessions, by session key.
    labels: std::collections::HashMap<String, String>,
    /// 리뷰 탭 badge (review-inbox, D-140). Left out when the listing failed, so the badge hides instead of guessing.
    #[serde(skip_serializing_if = "Option::is_none")]
    review_pending: Option<u32>,
}

/// Board lanes (task-lifecycle) on the tags, so the webview places ended tasks by the backend's rule.
fn put_lanes(lanes: &std::collections::HashMap<String, dduck_core::task_lifecycle::Lane>, tags: &mut std::collections::HashMap<String, task_meta::Tag>) {
    for (key, lane) in lanes {
        tags.entry(key.clone()).or_default().lane = Some(*lane);
    }
}

fn add_lanes(conn: &Connection, sessions: &[AgentSession], tags: &mut std::collections::HashMap<String, task_meta::Tag>) {
    if let Ok(since) = settings::review_since(conn, chrono_now_ms()) {
        if let Ok(lanes) = dduck_core::task_lifecycle::lanes(conn, sessions, since) {
            put_lanes(&lanes, tags);
        }
    }
}

/// "리뷰 전" count for the sidebar: DB only, no report reads.
fn review_pending(conn: &Connection, sessions: &[AgentSession], error: &Option<CliError>) -> Option<u32> {
    if error.is_some() || !dduck_core::review_inbox::poll_known() {
        return None;
    }
    let since = settings::review_since(conn, chrono_now_ms()).ok()?;
    dduck_core::review_inbox::pending_count(conn, &dduck_core::review_inbox::running_map(sessions), since).ok()
}

fn spawn_poller(app: AppHandle, db_path: PathBuf) {
    let handle = std::thread::spawn(move || {
        let mut poll = tracker::PollState::new();
        let conn = db::open(&db_path).ok();
        loop {
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| poll_once(&app, conn.as_ref(), &mut poll)));
            let state = app.state::<AppState>();
            let visible = state.window_visible.load(Ordering::Relaxed);
            // `poll_soon` unparks this early; a spurious wake-up only means one extra poll.
            std::thread::park_timeout(poll_interval(visible, chrono_now_ms(), FAST_POLL_UNTIL_MS.load(Ordering::Relaxed)));
        }
    });
    let _ = POLLER.set(handle.thread().clone());
}

fn poll_once(app: &AppHandle, conn: Option<&Connection>, poll: &mut tracker::PollState) {
    let state = app.state::<AppState>();
    let result = state.client().map_err(|_| CliError::Spawn).and_then(|c| c.list_agents()).and_then(|list| match conn {
        // A DB error here is treated like a failed poll, never as "no sessions".
        Some(c) => tracker::visible_sessions(c, list, chrono_now_ms()).map_err(|_| CliError::UnsupportedOutput),
        None => Ok(list),
    });
    let ok = result.is_ok();
    dduck_core::review_inbox::note_poll(ok);
    let tracker::PollView { sessions, error, transitions } = poll.apply(result);
    if ok {
        if let Some(c) = conn {
            let _ = routines::track(c, &sessions, chrono_now_ms());
            // How each task ended, kept for its record card once the CLI forgets it (task-lifecycle 5.2).
            let _ = dduck_core::task_lifecycle::note_ended(c, &sessions);
        }
        // Usage-limit stops: group, announce once per reset time (usage-limit-resume).
        limit_cmds::on_poll(app, &sessions);
    }
    // Board lanes of ended tasks (task-lifecycle): the 확인 필요 number and the tray use the same rule as the board.
    let lanes = conn
        .and_then(|c| settings::review_since(c, chrono_now_ms()).ok().and_then(|since| dduck_core::task_lifecycle::lanes(c, &sessions, since).ok()))
        .unwrap_or_default();
    let finished = conn.and_then(|c| settings::notify_finished(c).ok()).unwrap_or(true);
    {
        if finished {
            // Name and state only; nothing the model wrote (D-067).
            for t in transitions.iter().filter(|t| t.became_finished()) {
                let body = if t.to == dduck_core::claude_cli::SessionStatus::Failed {
                    dduck_core::task_lifecycle::failed_body(&t.name, i18n::current())
                } else {
                    // task-lifecycle: never "완료" before a review, and "리뷰 대기" only for a task that lands there (a
                    // report task from before `review_since` goes to 정리).
                    let waits = lanes.get(&t.key) == Some(&dduck_core::task_lifecycle::Lane::ReviewWait);
                    dduck_core::task_lifecycle::finished_body(&t.name, waits, i18n::current())
                };
                let _ = app.notification().builder().title("D.Duck").body(body).show();
            }
        }
    }
    let notify = conn.and_then(|c| settings::notify_waiting(c).ok()).unwrap_or(true);
    if notify {
        for t in transitions.iter().filter(|t| t.became_waiting()) {
            let _ = app.notification().builder().title("D.Duck").body(tracker::waiting_notification_body(&t.name, i18n::current())).show();
        }
    }
    let review_pending = conn.and_then(|c| review_pending(c, &sessions, &error));
    update_tray(app, &state, &sessions, &lanes, review_pending.unwrap_or(0));
    if let Ok(mut a) = state.agents.write() {
        *a = sessions.clone();
    }
    if let Ok(mut e) = state.agents_error.write() {
        *e = error.clone();
    }
    let mut tags = conn.and_then(|c| task_meta::tags(c, &sessions).ok()).unwrap_or_default();
    if let Some(c) = conn {
        add_report_presence(app, c, &sessions, &mut tags);
    }
    put_lanes(&lanes, &mut tags);
    if ok {
        let config_dir = state.config.read().map(|c| c.path.clone()).ok();
        if let Some(dir) = config_dir {
            let persona = |s: &AgentSession| tags.get(&s.key).and_then(|t| t.agent.clone());
            let activity = live_agents::collect(&dir, &sessions, &persona, chrono_now_ms());
            let _ = app.emit("agent-activity", activity);
        }
    }
    let labels = conn.and_then(|c| session_labels(c, &sessions).ok()).unwrap_or_default();
    let _ = app.emit("agents-updated", AgentsPayload { sessions, error, tags, labels, review_pending });
}

/// SQLite extended result code of a core error (`-1` when it is not an SQLite error). Numbers only, for timing lines.
fn sqlite_code(e: &dduck_core::Error) -> i64 {
    match e {
        dduck_core::Error::Db(rusqlite::Error::SqliteFailure(f, _)) => i64::from(f.extended_code),
        dduck_core::Error::Db(_) => -2,
        _ => -1,
    }
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- tray ----------

/// The saved app language with `system` resolved; Korean if the DB cannot be read (app-language, D-171). Also
/// keeps `i18n::current()` (errors' fallback sentences) in step: it runs at start and after every change.
fn app_lang(app: &AppHandle) -> i18n::Lang {
    let lang = app.try_state::<AppState>().and_then(|s| s.conn().ok().and_then(|c| settings::resolved_language(&c).ok())).unwrap_or(i18n::Lang::Ko);
    i18n::set_current(lang);
    lang
}

fn build_tray_menu(app: &AppHandle, waiting: &[&AgentSession]) -> tauri::Result<Menu<tauri::Wry>> {
    let lang = app_lang(app);
    let m = |code| i18n::msg(lang, code, &i18n::Params::new());
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "open", m(i18n::Code::TrayOpen), true, None::<&str>)?)?;
    if !waiting.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(app, "waiting-header", m(i18n::Code::TrayWaitingHeader), false, None::<&str>)?)?;
        for s in waiting.iter().take(10) {
            let label = if s.name.trim().is_empty() { m(i18n::Code::TraySessionFallback) } else { s.name.clone() };
            menu.append(&MenuItem::with_id(app, format!("session:{}", s.key), label, true, None::<&str>)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", m(i18n::Code::TrayQuit), true, None::<&str>)?)?;
    Ok(menu)
}

fn update_tray(app: &AppHandle, state: &AppState, sessions: &[AgentSession], lanes: &std::collections::HashMap<String, dduck_core::task_lifecycle::Lane>, pending: u32) {
    let waiting: Vec<&AgentSession> =
        sessions.iter().filter(|s| s.status == dduck_core::claude_cli::SessionStatus::WaitingInput).collect();
    // Rebuild the menu only when the waiting list changes, so an open menu is not replaced under the user.
    let mut signature: Vec<(String, String)> = waiting.iter().map(|s| (s.key.clone(), s.name.clone())).collect();
    // Same number as the sidebar's task badge: waiting for an answer, or failed without a report to review.
    let review = dduck_core::task_lifecycle::needs_check(sessions, lanes);
    signature.push(("check".into(), review.to_string()));
    // 리뷰 대기 (the review badge): a change of that number alone refreshes the tooltip.
    signature.push(("pending".into(), pending.to_string()));
    if let Ok(mut last) = state.tray_signature.lock() {
        if last.as_ref() == Some(&signature) {
            return;
        }
        *last = Some(signature);
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    if let Ok(menu) = build_tray_menu(app, &waiting) {
        let _ = tray.set_menu(Some(menu));
    }
    let count = (review > 0).then(|| review.to_string());
    #[cfg(target_os = "macos")]
    let _ = tray.set_title(count.as_deref());
    let _ = tray.set_tooltip(Some(dduck_core::task_lifecycle::tooltip(review, pending, i18n::current())));
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_badge_count((review > 0).then_some(review as i64));
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        if let Some(state) = app.try_state::<AppState>() {
            state.window_visible.store(true, Ordering::Relaxed);
        }
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = build_tray_menu(app, &[])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID).menu(&menu).tooltip("D.Duck").on_menu_event(|app, event| {
        let id = event.id().as_ref();
        match id {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {
                if let Some(key) = id.strip_prefix("session:") {
                    show_main(app);
                    let _ = app.emit("select-session", key.to_string());
                }
            }
        }
    });
    // Monochrome template image: macOS tints it for light/dark menu bars and it stays legible at 22pt.
    match tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template@2x.png")) {
        Ok(img) => builder = builder.icon(img).icon_as_template(true),
        Err(_) => {
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
        }
    }
    builder.build(app)
}

// ---------- commands: status & settings ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusDto {
    cli: Option<CliInfo>,
    config_dir: String,
    config_dir_explicit: bool,
    notify_waiting: bool,
    notify_finished: bool,
    remote_alerts: bool,
    store_task_prompts: bool,
    task_report: bool,
    limit_notify: bool,
    limit_auto_resume: bool,
    /// Auto resume is built but off until spike 1.1 (usage-limit-resume, design 13).
    limit_auto_shipped: bool,
    review_capacity: settings::ReviewCapacity,
    store_titles: bool,
    permission_modes: Vec<&'static str>,
    default_cwd: Option<String>,
}

#[tauri::command(async)]
fn get_status(state: State<'_, AppState>) -> Res<StatusDto> {
    let cfg = state.config.read().map_err(err)?.clone();
    let conn = state.conn()?;
    Ok(StatusDto {
        cli: state.cli.read().map_err(err)?.info.clone(),
        config_dir: cfg.path.to_string_lossy().into_owned(),
        config_dir_explicit: cfg.explicit,
        notify_waiting: settings::notify_waiting(&conn).map_err(err)?,
        notify_finished: settings::notify_finished(&conn).map_err(err)?,
        remote_alerts: settings::remote_alerts(&conn).map_err(err)?,
        store_task_prompts: settings::store_task_prompts(&conn).map_err(err)?,
        task_report: settings::task_report(&conn).map_err(err)?,
        limit_notify: settings::limit_notify(&conn).map_err(err)?,
        limit_auto_resume: settings::limit_auto_resume(&conn).map_err(err)?,
        limit_auto_shipped: dduck_core::limit_resume::AUTO_RESUME_SHIPPED,
        review_capacity: settings::review_capacity(&conn).map_err(err)?,
        store_titles: settings::get(&conn, ingest::SETTING_STORE_TITLES).map_err(err)?.as_deref() != Some("0"),
        permission_modes: PermissionMode::ALL.iter().map(|m| m.as_cli()).collect(),
        default_cwd: settings::default_cwd(&conn).map_err(err)?.map(|p| p.to_string_lossy().into_owned()),
    })
}

/// Native folder picker for the default task folder (pre-filled in the new-task form, D-048).
#[tauri::command]
async fn pick_default_cwd(app: AppHandle) -> Res<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let path = app.dialog().file().set_title(tr(C::PickDefaultCwd)).blocking_pick_folder()?.into_path().ok()?;
        Some((app, path))
    })
    .await
    .map_err(err)?;
    let Some((app, path)) = picked else { return Ok(None) };
    settings::set_default_cwd(&*app.state::<AppState>().conn()?, &path).map_err(err)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command(async)]
fn reset_default_cwd(state: State<'_, AppState>) -> Res<()> {
    settings::clear_default_cwd(&*state.conn()?).map_err(err)
}

/// First instruction of a listed session, read from its transcript for display only (never stored, D-048).
#[tauri::command(async)]
fn session_prompt(state: State<'_, AppState>, key: String) -> Result<Option<String>, CliError> {
    let s = listed_session(&state, &key)?;
    // What the user typed into D.Duck comes first; the transcript is the fallback (D-069).
    if let Some(id) = s.id.as_deref().filter(|i| task_meta::valid_bg_id(i)) {
        if let Ok(conn) = state.conn() {
            if let Ok(Some(d)) = task_meta::record_detail(&conn, id) {
                if let Some(p) = d.prompt {
                    return Ok(Some(p));
                }
            }
        }
    }
    let Some(session_id) = s.session_id else { return Ok(None) };
    let config = state.config.read().map_err(|_| CliError::Spawn)?.path.clone();
    let Some(root) = discover::projects_root(&config) else { return Ok(None) };
    Ok(dduck_core::prompt::first_prompt(&root, &session_id))
}

#[tauri::command(async)]
fn set_pref(state: State<'_, AppState>, key: String, value: String) -> Res<()> {
    settings::set_user_pref(&*state.conn()?, &key, &value).map_err(err)
}

/// App language (app-language, D-171): the stored choice and the language it resolves to (`system` via sys-locale).
#[derive(serde::Serialize)]
struct LanguageInfo {
    choice: &'static str,
    resolved: i18n::Lang,
}

#[tauri::command(async)]
fn get_language(state: State<'_, AppState>) -> Res<LanguageInfo> {
    let conn = state.conn()?;
    Ok(LanguageInfo { choice: settings::ui_language(&conn).map_err(err)?, resolved: settings::resolved_language(&conn).map_err(err)? })
}

/// Saves through the generic allowlist (three values only), then rebuilds the tray menu in the new language.
#[tauri::command(async)]
fn set_language(app: AppHandle, state: State<'_, AppState>, choice: String) -> Res<LanguageInfo> {
    let info = {
        let conn = state.conn()?;
        settings::set_user_pref(&conn, settings::UI_LANGUAGE, &choice).map_err(err)?;
        LanguageInfo { choice: settings::ui_language(&conn).map_err(err)?, resolved: settings::resolved_language(&conn).map_err(err)? }
    };
    i18n::set_current(info.resolved);
    // Forget the last menu so the next poll puts waiting tasks back; show the new labels right away.
    if let Ok(mut last) = state.tray_signature.lock() {
        *last = None;
    }
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_tray_menu(&app, &[])) {
        let _ = tray.set_menu(Some(menu));
    }
    Ok(info)
}

#[tauri::command(async)]
fn get_onboarding_state(state: State<'_, AppState>) -> Res<Option<settings::OnboardingState>> {
    settings::onboarding_state(&*state.conn()?).map_err(err)
}

/// The tour's own setter: size, shape and card ids are checked in core (onboarding-tour).
#[tauri::command(async)]
fn set_onboarding_state(state: State<'_, AppState>, json: String) -> Res<()> {
    settings::set_onboarding_state(&*state.conn()?, &json).map_err(err)
}

/// App data folder. Debug builds may point it elsewhere with `DDUCK_DATA_DIR` to try a clean first run without
/// touching real data; release builds compile the override out.
fn app_data_dir<R: tauri::Runtime, M: Manager<R>>(app: &M) -> tauri::Result<std::path::PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("DDUCK_DATA_DIR").map(std::path::PathBuf::from).filter(|d| d.is_absolute()) {
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    app.path().app_data_dir()
}

#[tauri::command(async)]
fn set_store_titles(state: State<'_, AppState>, enabled: bool) {
    state.send_ingest(IngestMsg::SetTitles(enabled));
}

/// Opens a native file picker (Rust side) and stores the CLI path only if it validates.
#[tauri::command]
async fn pick_cli_path(app: AppHandle) -> Res<Option<CliInfo>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let path = app.dialog().file().set_title(tr(C::PickCliFile)).blocking_pick_file()?.into_path().ok()?;
        Some((app, path))
    })
    .await
    .map_err(err)?;
    let Some((app, path)) = picked else { return Ok(None) };
    let info = cli_path::validate(&path).map_err(|r| r.message().to_string())?;
    let state = app.state::<AppState>();
    settings::set_cli_path(&*state.conn()?, &info.path).map_err(err)?;
    state.resolve_cli();
    Ok(Some(info))
}

#[tauri::command(async)]
fn reset_cli_path(state: State<'_, AppState>) -> Res<()> {
    settings::clear_cli_path(&*state.conn()?).map_err(err)?;
    state.resolve_cli();
    Ok(())
}

/// Opens a native folder picker and switches the Claude config dir.
#[tauri::command]
async fn pick_config_dir(app: AppHandle) -> Res<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let path = app.dialog().file().set_title(tr(C::PickConfigDir)).blocking_pick_folder()?.into_path().ok()?;
        Some((app, path))
    })
    .await
    .map_err(err)?;
    let Some((app, path)) = picked else { return Ok(None) };
    let state = app.state::<AppState>();
    settings::set_config_dir(&*state.conn()?, &path).map_err(err)?;
    apply_config_dir(&state);
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command(async)]
fn reset_config_dir(state: State<'_, AppState>) -> Res<()> {
    settings::clear_config_dir(&*state.conn()?).map_err(err)?;
    apply_config_dir(&state);
    Ok(())
}

fn apply_config_dir(state: &AppState) {
    let setting = state.conn().ok().and_then(|c| settings::config_dir(&c).ok().flatten());
    let cfg = cli_path::effective_config_dir(setting.as_deref(), &state.probe, &home_dir());
    let path = cfg.path.clone();
    if let Ok(mut c) = state.config.write() {
        *c = cfg;
    }
    state.resolve_cli();
    state.send_ingest(IngestMsg::Reconfigure(path));
}

/// Native folder picker for the task working directory (no JS dialog plugin, D-030).
#[tauri::command]
async fn pick_directory(app: AppHandle) -> Res<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title(tr(C::PickTaskDir)).blocking_pick_folder().and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(err)?;
    Ok(picked.map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command(async)]
fn rebuild_usage(state: State<'_, AppState>) {
    state.send_ingest(IngestMsg::Rebuild);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestFileDto {
    path: String,
    skipped_lines: i64,
    lines: i64,
}

#[tauri::command(async)]
fn ingest_files(state: State<'_, AppState>) -> Res<Vec<IngestFileDto>> {
    let conn = state.conn()?;
    let mut stmt = conn.prepare("SELECT path, skipped_lines, line_no FROM ingest_files ORDER BY skipped_lines DESC, path").map_err(err)?;
    let rows = stmt
        .query_map([], |r| Ok(IngestFileDto { path: r.get(0)?, skipped_lines: r.get(1)?, lines: r.get(2)? }))
        .map_err(err)?;
    rows.collect::<Result<_, _>>().map_err(err)
}

// ---------- commands: usage ----------

#[tauri::command(async)]
fn usage_summary(state: State<'_, AppState>, q: Query) -> Res<analytics::Summary> {
    analytics::summary(&*state.conn()?, &q, &state.prices).map_err(err)
}

#[tauri::command(async)]
fn usage_timeseries(state: State<'_, AppState>, q: Query) -> Res<Vec<analytics::Point>> {
    let bucket = Bucket::for_range(q.from_ms, q.to_ms);
    analytics::timeseries(&*state.conn()?, &q, &state.prices, bucket).map_err(err)
}

#[tauri::command(async)]
fn usage_distribution(state: State<'_, AppState>, q: Query, dim: Dimension, top: usize) -> Res<Vec<analytics::Item>> {
    analytics::distribution(&*state.conn()?, &q, &state.prices, dim, top.min(100)).map_err(err)
}

#[tauri::command(async)]
fn usage_sessions(state: State<'_, AppState>, q: Query) -> Res<Vec<analytics::SessionRow>> {
    analytics::sessions(&*state.conn()?, &q, &state.prices, 200).map_err(err)
}

#[tauri::command(async)]
fn usage_session_detail(state: State<'_, AppState>, session_id: String) -> Res<analytics::SessionDetail> {
    analytics::session_detail(&*state.conn()?, &state.prices, &session_id).map_err(err)
}

#[tauri::command(async)]
fn usage_filter_options(state: State<'_, AppState>) -> Res<(Vec<String>, Vec<String>)> {
    analytics::filter_options(&*state.conn()?).map_err(err)
}

// ---------- commands: tasks ----------

#[tauri::command(async)]
fn list_agents(app: AppHandle, state: State<'_, AppState>) -> Res<AgentsPayload> {
    let sessions = state.agents.read().map_err(err)?.clone();
    let conn = state.conn()?;
    let mut tags = task_meta::tags(&conn, &sessions).map_err(err)?;
    add_report_presence(&app, &conn, &sessions, &mut tags);
    add_lanes(&conn, &sessions, &mut tags);
    let labels = session_labels(&conn, &sessions).map_err(err)?;
    let error = state.agents_error.read().map_err(err)?.clone();
    let review_pending = review_pending(&conn, &sessions, &error);
    Ok(AgentsPayload { sessions, error, tags, labels, review_pending })
}

// ---------- sessions, skills, connectors ----------

/// Session key → user-given title.
fn session_labels(conn: &Connection, sessions: &[AgentSession]) -> Result<std::collections::HashMap<String, String>, dduck_core::Error> {
    let ids: Vec<String> = sessions.iter().filter_map(|s| s.session_id.clone()).collect();
    let by_id = dduck_core::labels::for_sessions(conn, &ids)?;
    Ok(sessions.iter().filter_map(|s| s.session_id.as_ref().and_then(|id| by_id.get(id)).map(|l| (s.key.clone(), l.clone()))).collect())
}

#[tauri::command(async)]
fn usage_sessions_page(state: State<'_, AppState>, q: Query, offset: usize) -> Res<analytics::SessionPage> {
    analytics::sessions_page(&*state.conn()?, &q, &state.prices, offset, 10).map_err(err)
}

// ---------- usage insights (D-078) ----------

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn insights_inputs(state: &AppState, q: &Query) -> Res<(dduck_core::insights::Stats, dduck_core::insights::LocalReport)> {
    use dduck_core::{insights, skills_catalog};
    let roots = catalog_roots(state)?;
    let defs = skills_catalog::discover(&roots.config_dir, &roots.project_dirs);
    let (st, used) = {
        let conn = state.conn()?;
        (insights::stats(&conn, q).map_err(err)?, skills_catalog::usage(&conn).map_err(err)?)
    };
    let report = insights::local(&st, &state.prices, &skills_catalog::catalog(defs, used), now_ms());
    Ok((st, report))
}

/// Rule-based checks over the selected period. No network, no CLI.
#[tauri::command(async)]
fn usage_insights_local(state: State<'_, AppState>, q: Query) -> Res<dduck_core::insights::LocalReport> {
    Ok(insights_inputs(&state, &q)?.1)
}

/// The exact JSON a Claude review would send, for the in-page preview.
#[tauri::command(async)]
fn usage_insights_preview(state: State<'_, AppState>, q: Query) -> Res<String> {
    let (st, report) = insights_inputs(&state, &q)?;
    Ok(dduck_core::insights::summary(&st, &report, &state.prices))
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InsightRun {
    Done { advice: dduck_core::insights::Advice },
    Declined,
    Failed { error: dduck_core::insights::RunError, message: String },
}

struct InsightsBusy<'a>(&'a AtomicBool);

impl Drop for InsightsBusy<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Native confirmation with the exact summary and the estimated cost, then one guarded headless run (D-078).
#[tauri::command]
async fn usage_insights_claude(app: AppHandle, q: Query) -> Result<InsightRun, login_cmds::CmdError> {
    // Login gate first (claude-login): before any claim, confirmation or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    tauri::async_runtime::spawn_blocking(move || {
        use dduck_core::insights as ins;
        use std::sync::atomic::Ordering as O;
        let state = app.state::<AppState>();
        if state.insights_busy.swap(true, O::SeqCst) {
            return Err(Fail::AlreadyAnalyzing.now());
        }
        let _busy = InsightsBusy(&state.insights_busy);
        state.insights_cancel.store(false, O::SeqCst);
        let (st, report) = insights_inputs(&state, &q)?;
        let summary = ins::summary(&st, &report, &state.prices);
        let ok = {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            app.dialog()
                .message(trp(C::DlgInsightsBody, &P::new().text("brief", &ins::summary_brief(&summary, i18n::current()), 400).text("cost", &tr(C::InsightsCost), 80)))
                .title(tr(C::DlgInsightsTitle))
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSendAnalyze), tr(C::BtnCancel)))
                .blocking_show()
        };
        if !ok {
            return Ok(InsightRun::Declined);
        }
        // App-owned empty working folder and hook counters (0700): no project CLAUDE.md, hooks or settings apply.
        let base = app_data_dir(&app).map_err(err)?.join("usage-insights");
        let cwd = base.join("cwd");
        let counters = base.join("guard");
        std::fs::create_dir_all(&cwd).map_err(err)?;
        std::fs::create_dir_all(&counters).map_err(err)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
        }
        let exe = std::env::current_exe().map_err(err)?;
        let hook = ins::guard_command(&exe, &counters);
        // A hook that cannot run would not block the web tools' allow path: refuse to start instead (fail closed).
        if !dduck_core::cloud_routines::guard_self_test(&hook, ins::SELF_TEST_DENY, ins::SELF_TEST_ALLOW) {
            return Err(Fail::GuardUnchecked.now());
        }
        ins::reset_counters(&counters);
        let client = state.client()?;
        let projects = state.config.read().map_err(err)?.path.join("projects");
        let raw = client.insights_raw(&cwd, &ins::args(&hook, &summary), Duration::from_secs(120), &|| {
            state.insights_cancel.load(O::SeqCst)
        });
        // Parse first, then clean (D-072 order); cleanup runs whatever happened.
        let parsed = raw.and_then(|out| ins::parse_output(&out, now_ms()));
        dduck_core::cloud_routines::clean_owned_cwd(&projects, &cwd, "usage-insights");
        ins::reset_counters(&counters);
        match parsed {
            Ok(advice) => {
                ins::save(&*state.conn()?, &advice).map_err(err)?;
                state.send_ingest(IngestMsg::Scrub); // the previous result must not linger in freed pages
                Ok(InsightRun::Done { advice })
            }
            Err(e) => {
                // A late login failure (the gate saw "unknown"): re-check once and show the gate line.
                let e = if e == ins::RunError::Failed && login_cmds::logged_out_after_failure(&app) { ins::RunError::LoginRequired } else { e };
                Ok(InsightRun::Failed { message: e.to_string(), error: e })
            }
        }
    })
    .await
    .map_err(err)?
    .map_err(login_cmds::CmdError::from)
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum RefineRun {
    Done { token: String, result: dduck_core::brief_refine::Refined, cost_usd: Option<f64>, spent_usd: f64, runs_left: u32 },
    Declined,
    Failed { error: dduck_core::brief_refine::RefineError, message: String, cost_usd: Option<f64>, spent_usd: Option<f64> },
}

fn refine_failed(e: dduck_core::brief_refine::RefineError, cost_usd: Option<f64>, spent_usd: Option<f64>) -> RefineRun {
    RefineRun::Failed { message: e.to_string(), error: e, cost_usd, spent_usd }
}

/// One refining round (brief-grilling): native confirmation once per consent (the configured rounds, rounds × $0.15
/// up to $0.90, 10 minutes, bound to the redact choice and names), then
/// a guarded headless run with no tools in an emptied app-owned folder. Inputs, questions and answers are never
/// stored or logged; errors are fixed messages.
#[tauri::command]
async fn refine_brief(app: AppHandle, req: dduck_core::brief_refine::Request) -> Result<RefineRun, login_cmds::CmdError> {
    // A cancel pressed from here on stops this run wherever it is (D-159); one pressed for an earlier run does not.
    let ticket = app.state::<AppState>().refine_cancel.begin();
    // Login gate first (claude-login): before any claim, confirmation or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    tauri::async_runtime::spawn_blocking(move || -> Res<RefineRun> {
        use dduck_core::brief_refine as br;
        use std::sync::atomic::Ordering as O;
        let state = app.state::<AppState>();
        let cancelled = || ticket.cancelled(&state.refine_cancel);
        if cancelled() {
            return Ok(refine_failed(br::RefineError::Cancelled, None, None));
        }
        if state.refine_busy.swap(true, O::SeqCst) {
            return Ok(refine_failed(br::RefineError::Busy, None, None));
        }
        let _busy = InsightsBusy(&state.refine_busy);
        // Limits come from the merged team config (Rust), not from the request.
        let team = {
            let stored = settings::team_config(&*state.conn()?).map_err(err)?;
            dduck_core::team_config::load(stored.as_deref())
        };
        let rounds = usize::from(team.refine.rounds).clamp(1, br::MAX_ROUNDS);
        if let Err(e) = req.validate(rounds) {
            return Ok(refine_failed(e, None, None));
        }
        let ty = req.task_type.as_deref().and_then(|k| team.get(k));
        let ctx = br::Context {
            type_label: ty.map(|t| t.label.clone()),
            focus: ty.map(|t| t.refine_focus.clone()).unwrap_or_default(),
            guidance: team.refine.guidance.clone(),
            max_questions: br::max_questions(team.refine.max_questions),
            last_round: req.mode == br::Mode::Rewrite || req.history.len() + 1 >= rounds,
            lang: i18n::current(),
        };
        let binding = req.consent_binding();
        let consented = state.refine_consent.lock().map_err(err)?.as_ref().is_some_and(|c| c.usable(req.token.as_deref(), &binding, now_ms()));
        if !consented {
            let names = req.names();
            let names_line = if names.is_empty() {
                String::new()
            } else {
                let shown = names.iter().map(|n| dduck_core::text::display_safe(n, 40)).collect::<Vec<_>>().join(", ");
                let action = tr(if req.redact_names { C::DlgRefineRedact } else { C::DlgRefineKeepNames });
                trp(C::DlgRefineNames, &P::new().num("count", names.len() as i64).text("names", &shown, 2000).text("action", &action, 80))
            };
            let ok = {
                let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
                app.dialog()
                    .message(trp(
                        C::DlgRefineBody,
                        &P::new()
                            .text("limits", &br::confirm_limits(rounds, i18n::current()), 200)
                            .num("rounds", rounds as i64)
                            .lines("names", names_line.split('\n'), 2000, 4),
                    ))
                    .title(tr(C::DlgRefineTitle))
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSendRefine), tr(C::BtnCancel)))
                    .blocking_show()
            };
            if !ok {
                return Ok(RefineRun::Declined);
            }
            let token = dduck_core::cloud_routines::new_nonce().ok_or_else(|| Fail::NonceFailed.now())?;
            *state.refine_consent.lock().map_err(err)? = Some(br::Consent::new(token, now_ms(), rounds, binding));
        }
        let token = state.refine_consent.lock().map_err(err)?.as_ref().map(|c| c.token.clone()).unwrap_or_default();
        let spent = |state: &AppState| state.refine_consent.lock().ok().and_then(|c| c.as_ref().map(|c| c.spent_usd));
        if cancelled() {
            return Ok(refine_failed(br::RefineError::Cancelled, None, spent(&state)));
        }
        // The hook is this executable in deny-all mode (main.rs). If it cannot deny, nothing runs (fail closed).
        // A pass is kept for this binary (path + mtime) for the app run (D-159).
        let exe = std::env::current_exe().map_err(err)?;
        let hook = br::guard_command(&exe);
        if !state.refine_guard.denies(&exe, || dduck_core::cloud_routines::guard_self_test_denies(&hook, br::SELF_TEST_SAMPLE)) {
            return Ok(refine_failed(br::RefineError::Guard, None, spent(&state)));
        }
        if cancelled() {
            return Ok(refine_failed(br::RefineError::Cancelled, None, spent(&state)));
        }
        let base = app_data_dir(&app).map_err(err)?.join("brief-refine");
        let cwd = match br::fresh_cwd(&base) {
            Ok(c) => c,
            Err(e) => return Ok(refine_failed(e, None, spent(&state))),
        };
        let projects = state.config.read().map_err(err)?.path.join("projects");
        // Cleans the folder on every exit path from here on, early returns included.
        let workdir = br::CwdGuard::new(projects, cwd);
        let client = state.client()?;
        let prompt = br::prompt(&req, &ctx);
        // One automatic retry for a malformed answer, inside the same consent and budget.
        let (outcome, cost) = br::run_with_retry(
            || state.refine_consent.lock().ok().and_then(|c| c.as_ref().map(|c| c.run_budget())).unwrap_or(0.0),
            |budget| {
                let raw = client.insights_raw(workdir.path(), &br::args(&hook, budget, prompt.clone()), Duration::from_secs(br::TIMEOUT_SECS), &cancelled);
                let ran = !matches!(raw, Err(dduck_core::insights::RunError::Spawn));
                let o = match raw {
                    Ok(out) => br::parse_output(&out, &ctx),
                    Err(e) => br::Outcome { result: Err(e.into()), cost_usd: None },
                };
                (o, ran)
            },
            |cost, budget| {
                if let Ok(mut g) = state.refine_consent.lock() {
                    if let Some(c) = g.as_mut() {
                        c.charge(cost, budget);
                    }
                }
            },
            cancelled,
        );
        // Parse first, then clean (D-072 order).
        drop(workdir);
        let (spent_usd, runs_left) = {
            let mut g = state.refine_consent.lock().map_err(err)?;
            match g.as_mut() {
                Some(c) => {
                    c.runs += 1;
                    (c.spent_usd, c.runs_left())
                }
                None => (0.0, 0),
            }
        };
        Ok(match outcome.result {
            Ok(result) => RefineRun::Done { token, result, cost_usd: cost, spent_usd, runs_left },
            Err(br::RefineError::Failed) if login_cmds::logged_out_after_failure(&app) => {
                refine_failed(br::RefineError::LoginRequired, cost, Some(spent_usd))
            }
            Err(e) => refine_failed(e, cost, Some(spent_usd)),
        })
    })
    .await
    .map_err(err)?
    .map_err(login_cmds::CmdError::from)
}

#[tauri::command(async)]
fn refine_cancel(state: State<'_, AppState>) {
    state.refine_cancel.cancel();
}

#[tauri::command(async)]
fn usage_insights_cancel(state: State<'_, AppState>) {
    state.insights_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command(async)]
fn usage_insights_last(state: State<'_, AppState>) -> Res<Option<dduck_core::insights::Advice>> {
    dduck_core::insights::load(&*state.conn()?, now_ms()).map_err(err)
}

#[tauri::command(async)]
fn usage_insights_delete(state: State<'_, AppState>) -> Res<()> {
    dduck_core::insights::delete(&*state.conn()?).map_err(err)?;
    state.send_ingest(IngestMsg::Scrub);
    Ok(())
}

/// Opens a link from Claude's answer: https only, full URL shown in a native confirmation first.
#[tauri::command]
async fn open_insight_link(app: AppHandle, url: String) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (url, host) = dduck_core::insights::link_ok(&url).ok_or_else(|| Fail::HttpsOnly.now())?;
        let ok = {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            app.dialog()
                .message(trp(C::DlgLinkBody, &P::new().text("host", &host, 253).checked_url("url", &url)))
                .title(tr(C::DlgLinkTitle))
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnOpen), tr(C::BtnCancel)))
                .blocking_show()
        };
        if !ok {
            return Ok(false);
        }
        open_url(&state, &url).map(|_| true)
    })
    .await
    .map_err(err)?
}

/// A session's flow, read on demand and never stored (D-071). Only sessions D.Duck has ingested.
#[tauri::command(async)]
fn session_timeline(state: State<'_, AppState>, session_id: String) -> Res<Option<dduck_core::timeline::Timeline>> {
    let known: bool = state
        .conn()?
        .query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = ?1)", [&session_id], |r| r.get(0))
        .map_err(err)?;
    if !known {
        return Err(Fail::UnknownSession.now());
    }
    // Serialized, not refused: switching sessions quickly just waits for the previous read.
    let _guard = state.timeline_lock.lock().map_err(err)?;
    let config = state.config.read().map_err(err)?.path.clone();
    let Some(root) = discover::projects_root(&config) else { return Ok(None) };
    Ok(dduck_core::timeline::read(&root, &session_id))
}

/// Transcript records as JSON, read by byte position when the user opens them and never stored, cached or logged
/// (D-074). `spans` reads exactly an item's records, otherwise one page of up to 100 records from `from`, or ending
/// at `before` (`tail` = the end of the file). `agent_id` reads that subagent's transcript in the same session
/// folder. Shares the timeline's single-read lock.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
fn session_raw(
    state: State<'_, AppState>,
    session_id: String,
    agent_id: Option<String>,
    spans: Option<Vec<[u64; 2]>>,
    from: Option<u64>,
    before: Option<u64>,
    tail: Option<bool>,
    reveal: bool,
) -> Res<dduck_core::raw::RawPage> {
    use dduck_core::raw::{RawError, Request, MAX_RECORDS};
    let req = match (spans, from, before, tail.unwrap_or(false)) {
        (Some(s), None, None, false) if !s.is_empty() && s.len() <= MAX_RECORDS => Request::Spans(s),
        (None, Some(f), None, false) => Request::Page { from: f },
        (None, None, Some(b), false) => Request::Before { end: Some(b) },
        (None, None, None, true) => Request::Before { end: None },
        _ => return Err(Fail::BadRawRequest.now()),
    };
    let known: bool = state
        .conn()?
        .query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = ?1)", [&session_id], |r| r.get(0))
        .map_err(err)?;
    if !known {
        return Err(Fail::UnknownSession.now());
    }
    let _guard = state.timeline_lock.lock().map_err(err)?;
    let config = state.config.read().map_err(err)?.path.clone();
    let Some(root) = discover::projects_root(&config) else { return Err(Fail::HistoryFolderMissing.now()) };
    dduck_core::raw::read(&root, &session_id, agent_id.as_deref(), &req, reveal).map_err(|e| {
        match e {
            RawError::NotFound => Fail::SessionFileMissing,
            RawError::Changed => Fail::SessionFileChanged,
            RawError::Invalid => Fail::BadRawRequest,
        }
        .now()
    })
}

#[tauri::command(async)]
fn set_session_label(state: State<'_, AppState>, session_id: String, label: Option<String>) -> Res<()> {
    let conn = state.conn()?;
    let replaced = dduck_core::labels::set(&conn, &session_id, label.as_deref(), chrono_now_ms()).map_err(|e| e.to_string())?;
    drop(conn);
    if replaced {
        state.send_ingest(IngestMsg::Scrub); // the previous title must not linger (on the ingest thread)
    }
    Ok(())
}

#[tauri::command(async)]
fn skill_catalog(state: State<'_, AppState>) -> Res<Vec<dduck_core::skills_catalog::Skill>> {
    let roots = catalog_roots(&state)?;
    let defs = dduck_core::skills_catalog::discover(&roots.config_dir, &roots.project_dirs);
    let used = dduck_core::skills_catalog::usage(&*state.conn()?).map_err(err)?;
    Ok(dduck_core::skills_catalog::catalog(defs, used))
}

/// Connector status via `claude mcp list`, on request only (it starts every configured server).
#[tauri::command]
async fn mcp_status(app: AppHandle, project: Option<String>) -> Res<dduck_core::mcp_list::ConnectorList> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::sync::atomic::Ordering as O;
        let state = app.state::<AppState>();
        if state.mcp_last.lock().map_err(err)?.is_some_and(|t| t.elapsed() < Duration::from_secs(15)) {
            return Err(Fail::JustChecked.now());
        }
        if state.mcp_busy.load(O::SeqCst) {
            return Err(Fail::AlreadyChecking.now());
        }
        // Run in the current project (nav-ia) so its local and .mcp.json servers are checked too; home otherwise.
        // A wrong id is an error (no silent fallback). The project's .mcp.json can start local commands, so a folder
        // Claude does not trust yet is confirmed natively first.
        let cwd = match project.as_deref() {
            None => home_dir(),
            Some(id) => {
                let dir = projects_cmds::usable_folder(&state, id)?;
                let client = state.client()?;
                let config_file = dduck_core::trust::config_file(&home_dir(), client.config_dir_env.as_deref());
                if dduck_core::trust::is_trusted(&config_file, &dir) != Some(true) {
                    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let msg = trp(C::DlgMcpCheckBody, &P::new().text("name", &name, 80));
                    if !add_items::confirm(&app, &tr(C::DlgMcpCheckTitle), msg, &tr(C::BtnCheck), true)? {
                        return Err(ReasonCode::Cancelled.now());
                    }
                }
                dir
            }
        };
        if state.mcp_busy.compare_exchange(false, true, O::SeqCst, O::SeqCst).is_err() {
            return Err(Fail::AlreadyChecking.now());
        }
        let result = state.client().and_then(|client| {
            client.mcp_list_raw(&cwd).map_err(|e| match e {
                CliError::Timeout => Fail::Timeout60.now(),
                _ => Fail::NoConnectorList.now(),
            })
        });
        // Cool-down counts from the end of the run; the busy flag is held for all of it.
        if let Ok(mut last) = state.mcp_last.lock() {
            *last = Some(Instant::now());
        }
        state.mcp_busy.store(false, O::SeqCst);
        Ok(dduck_core::mcp_list::parse(&result?))
    })
    .await
    .map_err(err)?
}

// ---------- plugins (D-075) ----------

/// Installed plugins as the CLI lists them (run from home, allow-listed fields only).
fn installed_plugins(state: &AppState) -> Res<Vec<dduck_core::plugins::Installed>> {
    let raw = state.client()?.plugin_list_raw(&home_dir()).map_err(|e| match e {
        CliError::Timeout => Fail::PluginListTimeout.now(),
        _ => Fail::NoPluginList.now(),
    })?;
    dduck_core::plugins::parse_list(&raw).ok_or_else(|| Fail::PluginListFormat.now())
}

#[tauri::command(async)]
fn plugin_list(state: State<'_, AppState>) -> Res<Vec<dduck_core::plugins::Plugin>> {
    let installed = installed_plugins(&state)?;
    let roots = catalog_roots(&state)?;
    let mut list = dduck_core::plugins::build(&roots.config_dir, installed);
    // Same usage numbers as the skills tab (D-071 matching).
    let defs = dduck_core::skills_catalog::discover(&roots.config_dir, &roots.project_dirs);
    let used = dduck_core::skills_catalog::usage(&*state.conn()?).map_err(err)?;
    dduck_core::plugins::apply_usage(&mut list, &dduck_core::skills_catalog::catalog(defs, used));
    Ok(list)
}

/// Holds `plugin_busy` for one action; released on drop.
struct PluginBusy<'a>(&'a AtomicBool);

impl<'a> PluginBusy<'a> {
    fn take(flag: &'a AtomicBool) -> Res<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| PluginBusy(flag))
            .map_err(|_| Fail::PluginBusy.now())
    }
}

impl Drop for PluginBusy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The installed entry for `id`, re-read from the CLI (the webview's copy is not trusted), unlocked.
fn editable_plugin(state: &AppState, id: &str) -> Res<dduck_core::plugins::Installed> {
    if !dduck_core::plugins::valid_id(id) {
        return Err(Fail::BadPluginId.now());
    }
    let p = installed_plugins(state)?.into_iter().find(|p| p.id == id).ok_or_else(|| Fail::PluginNotInstalled.now())?;
    if p.scope != dduck_core::plugins::Scope::User {
        return Err(Fail::GlobalPluginsOnly.now());
    }
    Ok(p)
}

#[tauri::command(async)]
fn set_plugin_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Res<dduck_core::plugins::Outcome> {
    use dduck_core::plugins::{Action, Outcome};
    let _busy = PluginBusy::take(&state.plugin_busy)?;
    let p = editable_plugin(&state, &id)?;
    if p.enabled == enabled {
        return Ok(Outcome::Unchanged);
    }
    let action = if enabled { Action::Enable } else { Action::Disable };
    let outcome = state.client()?.plugin_action(&home_dir(), action, &id, p.scope).map_err(err)?;
    if outcome != Outcome::Done {
        return Ok(outcome);
    }
    // Success is what the re-read list says, not the exit code.
    let after = installed_plugins(&state)?.into_iter().find(|p| p.id == id);
    Ok(dduck_core::plugins::judge_toggle(outcome, after.as_ref(), enabled))
}

/// Updates one plugin after a native confirmation. Never auto-approves a marketplace command (D-075).
#[tauri::command]
async fn update_plugin(app: AppHandle, id: String) -> Res<dduck_core::plugins::Outcome> {
    tauri::async_runtime::spawn_blocking(move || {
        use dduck_core::plugins::{Action, Outcome};

        let state = app.state::<AppState>();
        let _busy = PluginBusy::take(&state.plugin_busy)?;
        let p = editable_plugin(&state, &id)?;
        use dduck_core::plugins::UpdateState;
        let config = state.config.read().map_err(err)?.path.clone();
        let info = dduck_core::plugins::local_updates(&config, std::slice::from_ref(&p)).remove(&p.id);
        let (name, market) = p.id.split_once('@').unwrap_or((&p.id, ""));
        let current = p.version.clone().unwrap_or_else(|| tr(C::PluginNoVersion));
        let target = match info.map(|i| i.state) {
            Some(UpdateState::Available { latest }) => latest,
            Some(UpdateState::Latest) => tr(C::PluginLatestSame),
            _ => tr(C::PluginLatestUnknown),
        };
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(trp(
                C::DlgPluginUpdateBody,
                &P::new().text("name", name, 100).text("current", &current, 40).text("target", &target, 60).text("market", market, 100),
            ))
            .title(tr(C::DlgPluginUpdateTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnUpdate), tr(C::BtnCancel)))
            .blocking_show();
        drop(_guard);
        if !ok {
            return Ok(Outcome::Cancelled);
        }
        let outcome = state.client()?.plugin_action(&home_dir(), Action::Update, &p.id, p.scope).map_err(err)?;
        if outcome != Outcome::Done {
            return Ok(outcome);
        }
        let after = installed_plugins(&state)?.into_iter().find(|x| x.id == p.id);
        Ok(dduck_core::plugins::judge_update(outcome, &p, after.as_ref()))
    })
    .await
    .map_err(err)?
}

// ---------- tech-lead workflow (D-061, AG-01 … AG-14) ----------

/// Git-free repository check for the worktree option (AG-12).
#[tauri::command(async)]
fn repo_check(path: String) -> dduck_core::team::RepoStatus {
    let p = std::path::Path::new(&path);
    // Only absolute, existing folders; anything else reads as "not a repository".
    if !p.is_absolute() || !p.is_dir() {
        return dduck_core::team::RepoStatus::NotRepo;
    }
    dduck_core::team::repo_status(p, &home_dir())
}

/// Turns Remote Control alerts on (native confirmation) or off (D-067).
#[tauri::command]
async fn set_remote_alerts(app: AppHandle, enabled: bool) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if enabled {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            let ok = app
                .dialog()
                .message(tr(C::DlgRemoteBody))
                .title(tr(C::DlgRemoteTitle))
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTurnOn), tr(C::BtnCancel)))
                .blocking_show();
            if !ok {
                return Ok(false);
            }
        }
        settings::set_remote_alerts(&*state.conn()?, enabled).map_err(err)?;
        Ok(enabled)
    })
    .await
    .map_err(err)?
}

// ---------- task history (D-069) ----------

#[tauri::command(async)]
fn task_records(state: State<'_, AppState>) -> Res<Vec<task_meta::RecordRow>> {
    task_meta::records(&*state.conn()?, 200).map_err(err)
}

#[tauri::command(async)]
fn task_record(state: State<'_, AppState>, id: String) -> Res<Option<task_meta::RecordDetail>> {
    if !task_meta::valid_bg_id(&id) {
        return Err(Fail::BadTaskId.now());
    }
    task_meta::record_detail(&*state.conn()?, &id).map_err(err)
}

#[tauri::command(async)]
fn delete_task_record(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<bool> {
    if !task_meta::valid_bg_id(&id) {
        return Err(Fail::BadTaskId.now());
    }
    let conn = state.conn()?;
    // Same refusals as the 리뷰 tab's delete (review-delete): running (or an alias of it), resume planned, no listing.
    let running = dduck_core::review_inbox::running_map(&state.agents.read().map_err(err)?);
    let known = dduck_core::review_inbox::poll_known() && state.agents_error.read().map_err(err)?.is_none();
    dduck_core::review_inbox::delete_refusal(&conn, &id, &running, known).map_err(delete_fail)?;
    // The report folder (evidence, attachments) goes right away with the record (report-evidence 2.5).
    let gone = task_meta::delete_record_and_report(&conn, &id, reports_root(&app).as_deref()).map_err(err)?;
    let _ = ingest::scrub(&conn); // the instruction must not linger in freed pages or the WAL
    Ok(gone)
}

/// Deletes all history after a native confirmation.
#[tauri::command]
async fn delete_all_task_records(app: AppHandle) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(tr(C::DlgClearHistoryBody))
            .title(tr(C::DlgClearHistoryTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnDelete), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        let conn = state.conn()?;
        task_meta::delete_all_and_reports(&conn, reports_root(&app).as_deref()).map_err(err)?;
        let _ = ingest::scrub(&conn);
        Ok(true)
    })
    .await
    .map_err(err)?
}

/// Turning storage off asks whether to delete what is already stored (security review HIGH).
#[tauri::command]
async fn set_store_task_prompts(app: AppHandle, enabled: bool) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        settings::set_user_pref(&*state.conn()?, settings::STORE_TASK_PROMPTS, if enabled { "1" } else { "0" }).map_err(err)?;
        if enabled {
            return Ok(());
        }
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let wipe = app
            .dialog()
            .message(tr(C::DlgPromptsOffBody))
            .title(tr(C::DlgPromptsOffTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnDelete), tr(C::BtnKeep)))
            .blocking_show();
        if wipe {
            let conn = state.conn()?;
            task_meta::clear_prompts(&conn).map_err(err)?;
            let _ = ingest::scrub(&conn);
        }
        Ok(())
    })
    .await
    .map_err(err)?
}

/// "리뷰했어요": the task no longer counts against review capacity (AG-14).
#[tauri::command(async)]
fn mark_reviewed(state: State<'_, AppState>, id: String) -> Res<bool> {
    dduck_core::claude_cli::check_id(&id).map_err(|e| e.to_string())?;
    task_meta::mark_reviewed(&*state.conn()?, &id, chrono_now_ms()).map_err(err)
}

// ---------- agent profiles (D-054 … D-057) ----------

fn catalog_roots(state: &AppState) -> Res<agents_catalog::Roots> {
    let mut roots = base_catalog_roots(state)?;
    // The projects the tabs use (D-127), so their skills and agents are listed too.
    roots.project_dirs.extend(projects_cmds::picked_folders(state));
    Ok(roots)
}

/// Catalog roots without the current project: default folder, routines and live sessions. `picked_folders` resolves
/// the picked ids against these too, so a project known only as the default folder or a routine still counts.
fn base_catalog_roots(state: &AppState) -> Res<agents_catalog::Roots> {
    let config_dir = state.config.read().map_err(err)?.path.clone();
    let mut project_dirs = Vec::new();
    {
        let conn = state.conn()?;
        if let Some(d) = settings::default_cwd(&conn).map_err(err)? {
            project_dirs.push(d);
        }
        for r in routines::list(&conn).map_err(err)? {
            project_dirs.push(PathBuf::from(r.cwd));
        }
    }
    for s in state.agents.read().map_err(err)?.iter() {
        project_dirs.push(PathBuf::from(&s.cwd));
    }
    Ok(agents_catalog::Roots { config_dir, project_dirs })
}

fn fresh_catalog(state: &AppState) -> Res<Vec<AgentDef>> {
    fresh_catalog_for(state, None)
}

fn fresh_catalog_for(state: &AppState, cwd: Option<&std::path::Path>) -> Res<Vec<AgentDef>> {
    let defs = agents_catalog::discover_for(&catalog_roots(state)?, cwd);
    *state.catalog.write().map_err(err)? = defs.clone();
    Ok(defs)
}

#[tauri::command(async)]
fn agent_catalog(state: State<'_, AppState>, cwd: Option<String>) -> Res<Vec<AgentDef>> {
    fresh_catalog_for(&state, cwd.as_deref().map(std::path::Path::new))
}

#[tauri::command(async)]
fn agent_activity(state: State<'_, AppState>) -> Res<Vec<live_agents::Activity>> {
    let sessions = state.agents.read().map_err(err)?.clone();
    let tags = task_meta::tags(&*state.conn()?, &sessions).map_err(err)?;
    let dir = state.config.read().map_err(err)?.path.clone();
    let persona = |s: &AgentSession| tags.get(&s.key).and_then(|t| t.agent.clone());
    Ok(live_agents::collect(&dir, &sessions, &persona, chrono_now_ms()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefinitionText {
    content: String,
    token: String,
}

#[tauri::command(async)]
fn agent_definition(state: State<'_, AppState>, id: String) -> Res<DefinitionText> {
    let defs = state.catalog.read().map_err(err)?.clone();
    let (content, token) = agents_catalog::load(&defs, &id).map_err(|e| e.to_string())?;
    Ok(DefinitionText { content, token })
}

fn risk_lines(r: &agents_catalog::Risk) -> Vec<String> {
    let mut v = Vec::new();
    if r.hooks {
        v.push(tr(C::RiskHooks));
    }
    if r.mcp_servers {
        v.push(tr(C::RiskMcp));
    }
    if r.danger_mode {
        v.push(tr(C::RiskDangerMode));
    }
    if r.unreadable {
        v.push(tr(C::RiskUnreadable));
    }
    v
}

/// Saves an agent definition the user edited. The webview sends an id, never a path; the backend re-discovers,
/// shows the real frontmatter natively and re-checks the file after approval (D-054).
#[tauri::command]
async fn save_agent_definition(app: AppHandle, id: String, content: String, token: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let cached = state.catalog.read().map_err(err)?.iter().find(|d| d.id == id).and_then(|d| d.file.clone());
        let defs = fresh_catalog(&state)?;
        let still = defs.iter().find(|d| d.id == id).and_then(|d| d.file.clone());
        if cached.is_none() || cached != still {
            return Err(ReasonCode::ListChanged.now());
        }
        let plan = agents_catalog::plan_save(&defs, &id, content, &token).map_err(|e| e.to_string())?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let mut msg = trp(
            C::DlgAgentSaveBody,
            &P::new()
                .text("file", &plan.file_name, 120)
                .text("path", &plan.path.display().to_string(), 200)
                .num("changed", plan.changed_lines as i64)
                .lines("frontmatter", plan.frontmatter_preview.lines(), 300, 80),
        );
        if plan.permission_warning {
            msg.push_str(&format!("\n\n{}", tr(C::DlgAgentSaveWarn)));
        }
        let risks = risk_lines(&plan.risk);
        if !risks.is_empty() {
            msg.push_str(&format!("\n\n{}\n", tr(C::DlgAgentRisks)));
            msg.push_str(&risks.join("\n"));
        }
        msg.push_str(&format!("\n\n{}", tr(C::DlgSaveAsk)));
        let ok = app
            .dialog()
            .message(msg)
            .title(tr(C::DlgAgentSaveTitle))
            .kind(if plan.permission_warning || plan.risk.any() { MessageDialogKind::Warning } else { MessageDialogKind::Info })
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        agents_catalog::commit_save(&plan).map_err(|e| e.to_string())?;
        let _ = fresh_catalog(&state);
        Ok(())
    })
    .await
    .map_err(err)?
}

// ---------- tool permissions (D-076, D-079) ----------

/// Projects the permissions tab may open: the agent catalog's folders plus D.Duck task folders. The webview can only
/// pick from this list; a path it sends is matched against it after canonicalizing.
fn permission_projects(state: &AppState) -> Res<Vec<PathBuf>> {
    let mut dirs = catalog_roots(state)?.project_dirs;
    for r in task_meta::records(&*state.conn()?, 200).map_err(err)? {
        if let Some(c) = r.cwd {
            dirs.push(PathBuf::from(c));
        }
    }
    let home = home_dir().canonicalize().ok();
    let mut out: Vec<PathBuf> = Vec::new();
    for d in dirs {
        let Ok(c) = d.canonicalize() else { continue };
        if c.is_dir() && Some(&c) != home.as_ref() && c.parent().is_some() && !out.contains(&c) {
            out.push(c);
        }
        if out.len() >= 40 {
            break;
        }
    }
    out.sort();
    Ok(out)
}

/// `project` is an opaque id from `projects_known` (project-switcher); home and folder-less projects are refused.
fn permission_project(state: &AppState, project: Option<&str>) -> Res<Option<PathBuf>> {
    let Some(id) = project else { return Ok(None) };
    projects_cmds::usable_folder(state, id).map(Some)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionScopesDto {
    catalog: &'static dduck_core::permissions::Catalog,
    /// The project id echoed back, and its folder name.
    project: Option<String>,
    project_name: Option<String>,
    user: dduck_core::permissions::ScopeView,
    shared: Option<dduck_core::permissions::ScopeView>,
    local: Option<dduck_core::permissions::ScopeView>,
    managed: dduck_core::permissions::Managed,
    agents: Vec<PermAgentDto>,
    modes: [&'static str; 5],
}

/// Agent plus its tool lists exactly as written; the catalog copy is cut for display and must not be saved back.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PermAgentDto {
    #[serde(flatten)]
    def: AgentDef,
    tools_raw: Vec<String>,
    disallowed_raw: Vec<String>,
}

#[tauri::command(async)]
fn permission_scopes(state: State<'_, AppState>, project: Option<String>) -> Res<PermissionScopesDto> {
    use dduck_core::permissions as p;
    let proj = permission_project(&state, project.as_deref())?;
    let config_dir = state.config.read().map_err(err)?.path.clone();
    let defs = fresh_catalog_for(&state, proj.as_deref())?;
    let agents = defs
        .iter()
        .filter(|d| d.file.is_some() && (d.source != agents_catalog::Source::Project || d.usable || proj.is_none()))
        .map(|d| {
            let (tools_raw, disallowed_raw) = agents_catalog::load(&defs, &d.id).map(|(c, _)| agents_catalog::tool_lists_raw(&c)).unwrap_or_default();
            PermAgentDto { def: d.clone(), tools_raw, disallowed_raw }
        })
        .collect();
    Ok(PermissionScopesDto {
        catalog: p::catalog(),
        project: proj.as_ref().and(project.clone()),
        project_name: proj.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()),
        user: p::read_scope(&config_dir.join("settings.json")),
        shared: proj.as_ref().map(|d| p::read_scope(&d.join(".claude/settings.json"))),
        local: proj.as_ref().map(|d| p::read_scope(&d.join(".claude/settings.local.json"))),
        managed: p::managed(&p::default_managed_roots(), &p::default_mdm_files()),
        agents,
        modes: p::MODES,
    })
}

fn capped_lines(items: impl Iterator<Item = String>, max: usize) -> String {
    let all: Vec<String> = items.collect();
    let mut out: Vec<String> = all.iter().take(max).cloned().collect();
    if all.len() > max {
        out.push(trp(C::DlgMore, &P::new().num("count", (all.len() - max) as i64)));
    }
    out.join("\n")
}

/// Saves the `permissions` key of one settings file after a native confirmation that shows the real path and
/// every added/removed rule, with extra warnings for high-risk allows and permissive default modes.
#[tauri::command]
async fn save_permissions(app: AppHandle, scope: String, project: Option<String>, edit: dduck_core::permissions::PermEdit, token: String) -> Res<()> {
    use dduck_core::permissions as p;
    use dduck_core::text::display_safe;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let proj = permission_project(&state, project.as_deref())?;
        let (path, label, title, shared) = match (scope.as_str(), &proj) {
            ("user", _) => (state.config.read().map_err(err)?.path.join("settings.json"), "user", tr(C::PermScopeUser), false),
            ("shared", Some(d)) => (d.join(".claude/settings.json"), "project", tr(C::PermScopeShared), true),
            ("local", Some(d)) => (d.join(".claude/settings.local.json"), "local", tr(C::PermScopeLocal), false),
            _ => return Err(Fail::BadSaveScope.now()),
        };
        let plan = p::plan_save(&path, edit, &token, shared).map_err(|e| e.to_string())?;
        if plan.added.is_empty() && plan.removed.is_empty() && plan.mode_change.is_none() {
            return Err(Fail::NoChanges.now());
        }
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let mut msg = format!("{}\n{}\n", tr(if plan.exists { C::DlgPermEdit } else { C::DlgPermNew }), display_safe(&plan.path.display().to_string(), 300));
        if !plan.added.is_empty() {
            msg.push_str(&format!("\n{}\n{}\n", tr(C::DlgPermAdded), capped_lines(plan.added.iter().map(|(l, r)| format!("  + {l}  {}", display_safe(r, 120))), 25)));
        }
        if !plan.removed.is_empty() {
            msg.push_str(&format!("\n{}\n{}\n", tr(C::DlgPermRemoved), capped_lines(plan.removed.iter().map(|(l, r)| format!("  − {l}  {}", display_safe(r, 120))), 25)));
        }
        if let Some((old, new)) = &plan.mode_change {
            let none = tr(C::DlgNone);
            msg.push_str(&format!("\n{}\n", trp(C::DlgPermMode, &P::new().text("old", old.as_deref().unwrap_or(&none), 40).text("new", new.as_deref().unwrap_or(&none), 40))));
        }
        let mut warn = false;
        if !plan.high_allows.is_empty() {
            warn = true;
            msg.push_str(&format!("\n{}\n{}\n", tr(C::DlgPermHighAllow), capped_lines(plan.high_allows.iter().map(|r| format!("  · {}", display_safe(r, 120))), 10)));
        }
        if !plan.high_denies_removed.is_empty() {
            warn = true;
            msg.push_str(&format!("\n{}\n{}\n", tr(C::DlgPermHighDenyRemoved), capped_lines(plan.high_denies_removed.iter().map(|r| format!("  · {}", display_safe(r, 120))), 10)));
        }
        if plan.risky_mode {
            warn = true;
            msg.push_str(&format!("\n{}\n", tr(C::DlgPermRiskyMode)));
        }
        msg.push_str(&format!("\n{}", tr(C::DlgPermFoot)));
        let ok = app
            .dialog()
            .message(msg)
            .title(title)
            .kind(if warn { MessageDialogKind::Warning } else { MessageDialogKind::Info })
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        let backups = app_data_dir(&app).map_err(err)?.join("backups/permissions");
        p::commit_save(&plan, &backups, label, chrono_now_ms()).map_err(|e| e.to_string())
    })
    .await
    .map_err(err)?
}

/// What the task's folder offers for a role's loadout (agent-loadout): the pinned skills found there (hashed again)
/// and the static MCP server names. Unreadable skills count as changed; an unreadable server list refuses roles that
/// would otherwise allow every server (QA review).
fn loadout_env(state: &AppState, cwd: &std::path::Path, team: Option<&str>) -> dduck_core::loadout::Env {
    use dduck_core::loadout;
    let names: std::collections::BTreeSet<String> = dduck_core::team_config::load(team).types.iter().flat_map(|t| t.def.skills.iter().map(|s| s.name.clone())).collect();
    let Ok(cfg) = state.config.read().map(|c| c.clone()) else { return loadout::Env { mcp_unreadable: true, ..loadout::Env::default() } };
    let canon = std::fs::canonicalize(cwd).ok();
    let scan = if names.is_empty() { loadout::SkillScan::default() } else { loadout::scan_skills(&cfg.path, canon.as_slice(), Some(&names)) };
    let file = dduck_core::mcp_config::user_file(&home_dir(), cfg.explicit.then_some(cfg.path.as_path()));
    let list = dduck_core::mcp_config::list(&file, canon.as_deref());
    let preferred = if dduck_core::preferred::needed(&dduck_core::team_config::load(team).preferred) { preferred_for(&cfg.path, cwd) } else { Vec::new() };
    loadout::Env { skills: scan.options, unreadable_skills: scan.unreadable, mcp_unreadable: !list.unreadable.is_empty(), mcp_servers: list.servers.into_iter().map(|s| s.name).collect(), preferred }
}

/// "먼저 살펴볼 스킬·에이전트" installed for one folder (preferred-skills): the extension and agent catalog readers,
/// with project items from `cwd` and its ancestors only and agents usable there. Read only when something is chosen.
fn preferred_for(config_dir: &std::path::Path, cwd: &std::path::Path) -> Vec<dduck_core::preferred::Available> {
    let canon = std::fs::canonicalize(cwd).ok();
    let skills = dduck_core::skills_catalog::discover(config_dir, canon.as_slice());
    let roots = agents_catalog::Roots { config_dir: config_dir.to_path_buf(), project_dirs: Vec::new() };
    let agents = agents_catalog::discover_for(&roots, canon.as_deref());
    dduck_core::preferred::available(&skills, &agents, true)
}

/// Choices for "먼저 살펴볼 스킬·에이전트": user, plugin and project items of the folders the tabs use. No file paths.
#[tauri::command]
async fn preferred_options(app: AppHandle) -> Res<Vec<dduck_core::preferred::Available>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let roots = catalog_roots(&state)?;
        let skills = dduck_core::skills_catalog::discover(&roots.config_dir, &roots.project_dirs);
        let agents = agents_catalog::discover(&roots);
        Ok(dduck_core::preferred::available(&skills, &agents, false))
    })
    .await
    .map_err(err)?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadoutOptions {
    skills: Vec<dduck_core::loadout::SkillOption>,
    /// Skill names with a definition that could not be read (a pin of that name counts as changed).
    unreadable_skills: Vec<String>,
    mcp: Vec<dduck_core::mcp_config::McpServer>,
    /// Settings files that exist but could not be read, by label.
    unreadable: Vec<&'static str>,
}

/// The static MCP servers of the user file and the projects the tabs use (nothing is started; D-114).
fn loadout_mcp(state: &AppState) -> Res<dduck_core::mcp_config::McpList> {
    let cfg = state.config.read().map_err(err)?.clone();
    let file = dduck_core::mcp_config::user_file(&home_dir(), cfg.explicit.then_some(cfg.path.as_path()));
    let mut list = dduck_core::mcp_config::list(&file, None);
    for p in projects_cmds::picked_folders(state) {
        let more = dduck_core::mcp_config::list(&file, Some(&p));
        for s in more.servers {
            if !list.servers.iter().any(|x| x.name == s.name) {
                list.servers.push(s);
            }
        }
        for u in more.unreadable {
            if !list.unreadable.contains(&u) {
                list.unreadable.push(u);
            }
        }
    }
    Ok(list)
}

/// Choices for "기본 장비": preloadable skills of the user, plugins and the projects the tabs use, and the MCP servers
/// of the user file and those projects.
#[tauri::command]
async fn loadout_options(app: AppHandle) -> Res<LoadoutOptions> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let roots = catalog_roots(&state)?;
        let scan = dduck_core::loadout::scan_skills(&roots.config_dir, &roots.project_dirs, None);
        let list = loadout_mcp(&state)?;
        Ok(LoadoutOptions { skills: scan.options, unreadable_skills: scan.unreadable.into_iter().take(100).collect(), mcp: list.servers, unreadable: list.unreadable })
    })
    .await
    .map_err(err)?
}

/// "기본 장비" of a file agent: its `skills` line and the MCP entries of `tools` / `disallowedTools`. Everything else
/// in those lists must stay as stored (security review). Plugin and built-in agents are read-only. Shown and
/// confirmed natively, leading with what the agent will use; lifting a block is a warning.
#[tauri::command]
async fn save_agent_loadout(app: AppHandle, id: String, project: Option<String>, skills: Vec<String>, tools: Vec<String>, disallowed: Vec<String>, token: String) -> Res<()> {
    use dduck_core::loadout;
    use dduck_core::text::display_safe;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let proj = permission_project(&state, project.as_deref())?;
        let cached = state.catalog.read().map_err(err)?.iter().find(|d| d.id == id).and_then(|d| d.file.clone());
        let defs = fresh_catalog_for(&state, proj.as_deref())?;
        let def = defs.iter().find(|d| d.id == id).cloned().ok_or_else(|| Fail::AgentNotListed.now())?;
        if cached.is_none() || cached != def.file {
            return Err(ReasonCode::ListChanged.now());
        }
        if let Some(r) = def.readonly_reason {
            return Err(r.into());
        }
        let (content, current) = agents_catalog::load(&defs, &id).map_err(|e| e.to_string())?;
        if current != token {
            return Err(ReasonCode::ChangedOutside.now());
        }
        let old_skills = agents_catalog::skills_raw(&content);
        let (old_tools, old_disallowed) = agents_catalog::tool_lists_raw(&content);
        let change = loadout::check_agent_lists(&old_tools, &old_disallowed, &tools, &disallowed)?;
        let new = agents_catalog::with_loadout(&content, &skills, &tools, &disallowed).map_err(|e| e.to_string())?;
        if new == content {
            return Ok(());
        }
        let plan = agents_catalog::plan_save(&defs, &id, new, &token).map_err(|e| e.to_string())?;
        // Strict types that run as this agent take its skills too (security review LOW).
        let team = state.conn().ok().and_then(|c| settings::team_config(&c).ok().flatten());
        let strict_users: Vec<String> = dduck_core::team_config::load(team.as_deref()).types.iter().filter(|t| t.def.strict && t.def.agent.as_deref() == Some(def.name.as_str())).map(|t| t.def.label.clone()).collect();
        let added_skills: Vec<&String> = skills.iter().filter(|s| !old_skills.contains(s)).collect();
        let mcp = loadout_mcp(&state).map(|l| l.servers.into_iter().map(|s| s.name).collect::<Vec<_>>()).unwrap_or_default();
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let list = |v: &[String]| if v.is_empty() { tr(C::DlgNone) } else { display_safe(&v.join(", "), 400) };
        let servers = match loadout::agent_servers(&tools, &disallowed, &mcp) {
            None => tr(C::LoadoutAllServers),
            Some(v) if v.is_empty() => tr(C::LoadoutNoServers),
            Some(v) => display_safe(&v.join(", "), 300),
        };
        let skills_text = if skills.is_empty() { tr(C::LoadoutNone) } else { display_safe(&skills.join(", "), 300) };
        let mut msg = format!("{}\n", trp(C::DlgLoadoutServers, &P::new().text("servers", &servers, 300).text("skills", &skills_text, 300)));
        if tools.is_empty() {
            msg.push_str(&format!("{}\n", tr(C::DlgLoadoutOutside)));
        }
        let mut warn = false;
        if !change.removed_blocks.is_empty() {
            warn = true;
            msg.push_str(&format!("\n{}\n", trp(C::DlgLoadoutUnblock, &P::new().text("list", &list(&change.removed_blocks), 400))));
        }
        if !strict_users.is_empty() && (!added_skills.is_empty() || !change.added_tools.is_empty()) {
            warn = true;
            msg.push_str(&format!("\n{}\n", trp(C::DlgLoadoutStrict, &P::new().text("types", &strict_users.join(", "), 200))));
        }
        msg.push_str(&format!("\n{}\n{}\n\nskills: {} → {}\n", plan.file_name, display_safe(&plan.path.display().to_string(), 300), list(&old_skills), list(&skills)));
        if tools != old_tools {
            msg.push_str(&format!("tools: {} → {}\n", list(&old_tools), list(&tools)));
        }
        if disallowed != old_disallowed {
            msg.push_str(&format!("disallowedTools: {} → {}\n", list(&old_disallowed), list(&disallowed)));
        }
        msg.push_str(&format!("\n{}", tr(C::DlgLoadoutFoot)));
        let ok = app
            .dialog()
            .message(msg)
            .title(tr(C::DlgLoadoutTitle))
            .kind(if warn { MessageDialogKind::Warning } else { MessageDialogKind::Info })
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        agents_catalog::commit_save(&plan).map_err(|e| e.to_string())?;
        let _ = fresh_catalog_for(&state, proj.as_deref());
        Ok(())
    })
    .await
    .map_err(err)?
}

/// Rewrites an agent's `tools`/`disallowedTools` lines only, through the agent definition save path (D-054).
#[tauri::command]
async fn save_agent_tools(app: AppHandle, id: String, project: Option<String>, tools: Vec<String>, disallowed: Vec<String>, token: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let proj = permission_project(&state, project.as_deref())?;
        let cached = state.catalog.read().map_err(err)?.iter().find(|d| d.id == id).and_then(|d| d.file.clone());
        let defs = fresh_catalog_for(&state, proj.as_deref())?;
        let def = defs.iter().find(|d| d.id == id).cloned();
        if cached.is_none() || cached != def.as_ref().and_then(|d| d.file.clone()) {
            return Err(ReasonCode::ListChanged.now());
        }
        def.ok_or_else(|| Fail::AgentNotListed.now())?;
        let (content, current) = agents_catalog::load(&defs, &id).map_err(|e| e.to_string())?;
        if current != token {
            return Err(ReasonCode::ChangedOutside.now());
        }
        let (old_tools, old_disallowed) = agents_catalog::tool_lists_raw(&content);
        let new = agents_catalog::with_tool_lists(&content, &tools, &disallowed).map_err(|e| e.to_string())?;
        let plan = agents_catalog::plan_save(&defs, &id, new, &token).map_err(|e| e.to_string())?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let list = |v: &[String]| if v.is_empty() { tr(C::DlgNone) } else { dduck_core::text::display_safe(&v.join(", "), 400) };
        let mut msg = format!(
            "{}\n{}\n\n{}: {} → {}\ndisallowedTools: {} → {}\n",
            plan.file_name,
            dduck_core::text::display_safe(&plan.path.display().to_string(), 300),
            tr(C::DlgToolsAllowList),
            if old_tools.is_empty() { tr(C::DlgToolsInheritAll) } else { list(&old_tools) },
            if tools.is_empty() { tr(C::DlgToolsInheritAll) } else { list(&tools) },
            list(&old_disallowed),
            list(&disallowed),
        );
        msg.push_str(&format!("\n{}", tr(C::DlgToolsFoot)));
        let ok = app
            .dialog()
            .message(msg)
            .title(tr(C::DlgToolsTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        agents_catalog::commit_save(&plan).map_err(|e| e.to_string())?;
        let _ = fresh_catalog_for(&state, proj.as_deref());
        Ok(())
    })
    .await
    .map_err(err)?
}

/// Rule sets of every scope (managed, local, shared, user). `draft` replaces one scope's saved rules.
fn permission_sets(state: &AppState, proj: Option<&std::path::Path>, draft: Option<(&str, &dduck_core::permissions::PermEdit)>) -> Res<(Vec<dduck_core::permissions::RuleSet>, bool, bool)> {
    use dduck_core::permissions as p;
    let home = home_dir();
    let config_dir = state.config.read().map_err(err)?.path.clone();
    let managed = p::managed(&p::default_managed_roots(), &p::default_mdm_files());
    let set = |label: &str, key: &str, v: &p::ScopeView, managed: bool, root: &std::path::Path| {
        let (allow, ask, deny) = match draft {
            Some((k, e)) if k == key => (e.allow.clone(), e.ask.clone(), e.deny.clone()),
            _ => (v.allow.clone(), v.ask.clone(), v.deny.clone()),
        };
        p::RuleSet { label: label.into(), allow, ask, deny, managed, root: root.to_path_buf() }
    };
    let mut sets: Vec<p::RuleSet> = managed.sources.iter().map(|s| set("관리자", "managed", s, true, std::path::Path::new("/"))).collect();
    if let Some(d) = proj {
        sets.push(set("프로젝트(개인)", "local", &p::read_scope(&d.join(".claude/settings.local.json")), false, d));
        sets.push(set("프로젝트(공유)", "shared", &p::read_scope(&d.join(".claude/settings.json")), false, d));
    }
    sets.push(set("전역", "user", &p::read_scope(&config_dir.join("settings.json")), false, &home));
    Ok((sets, managed.managed_only, managed.mdm.is_some()))
}


/// Which rule decides a tool call, across managed, project and user settings plus an agent's tool lists. Reference
/// only: modes, hooks and CLI flags are not modelled.
#[tauri::command(async)]
fn explain_permission(state: State<'_, AppState>, project: Option<String>, agent_id: Option<String>, call: String) -> Res<dduck_core::permissions::Explanation> {
    use dduck_core::permissions as p;
    let proj = permission_project(&state, project.as_deref())?;
    let home = home_dir();
    let (sets, managed_only, mdm) = permission_sets(&state, proj.as_deref(), None)?;
    let agent = match agent_id {
        Some(id) => {
            let defs = state.catalog.read().map_err(err)?.clone();
            let d = defs.iter().find(|d| d.id == id).cloned().ok_or_else(|| Fail::AgentNotListed.now())?;
            // Full lists from the file: the catalog copy is cut for display.
            let (tools, disallowed) = agents_catalog::load(&defs, &id).map(|(c, _)| agents_catalog::tool_lists_raw(&c)).unwrap_or((d.tools.clone(), d.disallowed_tools.clone()));
            Some((d.name.clone(), (!tools.is_empty()).then_some(tools), disallowed))
        }
        None => None,
    };
    let ctx = p::CallContext { cwd: proj.clone().unwrap_or_else(|| home.clone()), home, agent };
    let mut e = p::explain(&sets, managed_only, &call, &ctx);
    if mdm {
        e.notes.push(Fail::MdmNote.now());
    }
    Ok(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectiveDto {
    rows: Vec<dduck_core::permissions::RowEffect>,
    mdm_note: Option<&'static str>,
}

/// Effective result of every catalog row with all scopes combined; `edit` is the unsaved draft of `scope`.
#[tauri::command(async)]
fn permission_effective(state: State<'_, AppState>, project: Option<String>, scope: String, edit: dduck_core::permissions::PermEdit) -> Res<EffectiveDto> {
    use dduck_core::permissions as p;
    if ![&edit.allow, &edit.ask, &edit.deny].iter().all(|l| l.len() <= p::MAX_RULES_PER_LIST) {
        return Err(Fail::TooManyRules.now());
    }
    let proj = permission_project(&state, project.as_deref())?;
    let home = home_dir();
    let (sets, managed_only, mdm) = permission_sets(&state, proj.as_deref(), Some((scope.as_str(), &edit)))?;
    let ctx = p::CallContext { cwd: proj.unwrap_or_else(|| home.clone()), home, agent: None };
    Ok(EffectiveDto { rows: p::catalog_effective(&sets, managed_only, &ctx), mdm_note: mdm.then(|| Fail::MdmNote.text(dduck_core::i18n::current())) })
}

/// High-risk catalog rows a new allow rule would also let through.
#[tauri::command(async)]
fn permission_rule_covers(rule: String) -> Vec<String> {
    if rule.len() > 2000 {
        return Vec::new();
    }
    dduck_core::permissions::covers_high_risk(&rule)
}

// ---------- team config and agent controls (team-presets-editable, D-081) ----------

fn stored_team(state: &AppState) -> Res<Option<String>> {
    settings::team_config(&*state.conn()?).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TeamConfigDto {
    config: dduck_core::team_config::Merged,
    defaults: dduck_core::team_config::Merged,
    /// Types referenced by task records: hide only, never delete.
    used_types: Vec<String>,
    auto_allowed: Vec<&'static str>,
}

#[tauri::command(async)]
fn team_config(state: State<'_, AppState>) -> Res<TeamConfigDto> {
    use dduck_core::team_config as tc;
    let stored = stored_team(&state)?;
    Ok(TeamConfigDto {
        config: tc::load(stored.as_deref()),
        defaults: tc::defaults(),
        used_types: task_meta::used_types(&*state.conn()?).map_err(err)?,
        auto_allowed: tc::AUTO_ALLOWED.iter().map(|(id, _)| *id).collect(),
    })
}

/// Saves the edited team config as differences from the bundle. Rust validates it and computes which changes
/// weaken a type or a safety rule; only those need the native confirmation (webview flags are not trusted).
#[tauri::command]
async fn save_team_config(app: AppHandle, input: dduck_core::team_config::TeamInput) -> Res<()> {
    use dduck_core::team_config as tc;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let used = task_meta::used_types(&*state.conn()?).map_err(err)?;
        let before = stored_team(&state)?;
        let stored = tc::diff(&input, &used)?;
        // Strict types may not use a "내 에이전트" that can write or run commands (QA review).
        let defs = fresh_catalog(&state)?;
        let tools_of = |name: &str| defs.iter().find(|d| d.name == name && d.primary).map(|d| d.tools.clone());
        let problems = tc::strict_agent_problems(&tc::merge(&stored), tools_of);
        if !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        let risky = tc::risky_changes(&tc::load(before.as_deref()), &tc::merge(&stored));
        if !risky.is_empty() {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            let list = capped_lines(risky.iter().map(|r| format!("· {}", dduck_core::text::display_safe(r, 160))), 15);
            let msg = trp(C::DlgTeamBody, &P::new().lines("list", list.lines(), 200, 16));
            let ok = app
                .dialog()
                .message(msg)
                .title(tr(C::DlgTeamTitle))
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
                .blocking_show();
            if !ok {
                return Err(ReasonCode::Cancelled.now());
            }
            if stored_team(&state)? != before {
                return Err(Fail::TeamChanged.now());
            }
        }
        settings::set_team_config(&*state.conn()?, &stored).map_err(err)
    })
    .await
    .map_err(err)?
}

/// "기본값으로": one item (`rule:<id>`, `type:<key>`) or everything. Defaults are never riskier, so no dialog.
#[tauri::command(async)]
fn reset_team_config(state: State<'_, AppState>, item: Option<String>) -> Res<()> {
    use dduck_core::team_config as tc;
    let conn = state.conn()?;
    match item {
        None => settings::clear_team_config(&conn).map_err(err),
        Some(item) => {
            let mut st: tc::Stored = settings::team_config(&conn).map_err(err)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            tc::reset_item(&mut st, &item)?;
            settings::set_team_config(&conn, &st).map_err(err)
        }
    }
}

/// Settings files whose `Agent(<name>)` deny rules count: global, managed, every known project (shared and
/// personal) and the ancestors of `cwd`.
fn deny_scopes(state: &AppState, cwd: Option<&std::path::Path>) -> Res<Vec<dduck_core::permissions::ScopeView>> {
    use dduck_core::permissions as p;
    let config_dir = state.config.read().map_err(err)?.path.clone();
    let mut out = vec![p::read_scope(&config_dir.join("settings.json"))];
    out.extend(p::managed(&p::default_managed_roots(), &p::default_mdm_files()).sources);
    let mut dirs = permission_projects(state)?;
    if let Some(c) = cwd.and_then(|c| c.canonicalize().ok()) {
        dirs.extend(c.ancestors().take(12).map(std::path::Path::to_path_buf));
    }
    for d in dirs {
        for f in [".claude/settings.json", ".claude/settings.local.json"] {
            let v = p::read_scope(&d.join(f));
            if v.exists {
                out.push(v);
            }
        }
    }
    Ok(out)
}

fn agent_denied(state: &AppState, name: &str, cwd: &std::path::Path) -> bool {
    // Unreadable state counts as "off": better a refused start than starting an agent the user turned off.
    deny_scopes(state, Some(cwd)).map_or(true, |scopes| scopes.iter().any(|v| dduck_core::agent_controls::is_denied(&dduck_core::agent_controls::denied_names(v), name)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentControlsDto {
    /// Off in the global settings (user, built-in and plugin agents).
    user_off: Vec<String>,
    /// Project folder → names off in its personal settings.
    project_off: Vec<(String, Vec<String>)>,
    /// Off through managed or shared project settings: shown read-only.
    elsewhere_off: Vec<String>,
    hidden: Vec<String>,
}

#[tauri::command(async)]
fn agent_controls(state: State<'_, AppState>) -> Res<AgentControlsDto> {
    use dduck_core::agent_controls::denied_names;
    use dduck_core::permissions as p;
    let config_dir = state.config.read().map_err(err)?.path.clone();
    let user_off = denied_names(&p::read_scope(&config_dir.join("settings.json")));
    let mut elsewhere_off: Vec<String> = p::managed(&p::default_managed_roots(), &p::default_mdm_files()).sources.iter().flat_map(denied_names).collect();
    let mut project_off = Vec::new();
    for d in permission_projects(&state)? {
        let local = denied_names(&p::read_scope(&d.join(".claude/settings.local.json")));
        if !local.is_empty() {
            project_off.push((d.display().to_string(), local));
        }
        elsewhere_off.extend(denied_names(&p::read_scope(&d.join(".claude/settings.json"))));
    }
    elsewhere_off.sort();
    elsewhere_off.dedup();
    Ok(AgentControlsDto { user_off, project_off, elsewhere_off, hidden: settings::hidden_agents(&*state.conn()?).map_err(err)? })
}

/// Writes or removes the exact `Agent(<name>)` deny rule through the permissions-only save path. No dialog (the
/// change only takes abilities away or gives back what the user took away); returns a warning to show, if any.
fn write_agent_deny(app: &AppHandle, path: &std::path::Path, name: &str, off: bool) -> Res<()> {
    use dduck_core::permissions as p;
    let view = p::read_scope(path);
    if let Some(r) = &view.readonly_reason {
        return Err(r.clone());
    }
    if view.error.is_some() {
        return Err(Fail::SettingsUnreadable.now());
    }
    let edit = dduck_core::agent_controls::with_agent_denied(&view, name, off);
    let plan = p::plan_save(path, edit, &view.token, false).map_err(|e| e.to_string())?;
    if plan.added.is_empty() && plan.removed.is_empty() {
        return Ok(());
    }
    let backups = app_data_dir(app).map_err(err)?.join("backups/permissions");
    p::commit_save(&plan, &backups, "agent-off", chrono_now_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_agent_enabled(app: AppHandle, id: String, enabled: bool) -> Res<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let config_dir = state.config.read().map_err(err)?.path.clone();
        let def = fresh_catalog(&state)?.into_iter().find(|d| d.id == id).ok_or_else(|| Fail::AgentNotListed.now())?;
        let path = dduck_core::agent_controls::deny_target(&def, &config_dir).ok_or_else(|| Fail::ProjectFolderUnknown.now())?;
        let mut warning = None;
        if def.source == agents_catalog::Source::Project {
            let project = def.scope.as_deref().map(PathBuf::from).ok_or_else(|| Fail::ProjectFolderUnknown.now())?;
            if !permission_projects(&state)?.contains(&project) {
                return Err(Fail::ProjectNotListed.now());
            }
            if !enabled && dduck_core::agent_controls::local_settings_unignored(&project) {
                warning = Some(Fail::LocalNotIgnored.now());
            }
        }
        write_agent_deny(&app, &path, &def.name, !enabled)?;
        Ok(warning)
    })
    .await
    .map_err(err)?
}

#[tauri::command(async)]
fn set_agent_hidden(state: State<'_, AppState>, name: String, hidden: bool) -> Res<()> {
    settings::set_agent_hidden(&*state.conn()?, &name, hidden).map_err(err)
}

#[cfg(target_os = "macos")]
fn move_to_trash(path: &std::path::Path) -> Res<()> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let s = path.to_str().ok_or_else(|| Fail::BadPath.now())?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(s));
    NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, None).map_err(|_| Fail::TrashFailed.now())
}

#[cfg(not(target_os = "macos"))]
fn move_to_trash(_path: &std::path::Path) -> Res<()> {
    Err(Fail::TrashMacOnly.now().into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrashResult {
    name: String,
    project: Option<String>,
    /// A same-named `Agent(<name>)` deny rule or hidden entry remains; the UI offers to clean it up.
    off: bool,
    hidden: bool,
}

/// Moves a user or project agent definition to the Trash. The target comes from the catalog id only; the file is
/// re-checked after the native confirmation, and agents that a running task uses are refused.
#[tauri::command]
async fn trash_agent(app: AppHandle, id: String) -> Res<TrashResult> {
    use dduck_core::agent_controls as ac;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let config_dir = state.config.read().map_err(err)?.path.clone();
        let def = fresh_catalog(&state)?.into_iter().find(|d| d.id == id).ok_or_else(|| Fail::AgentNotListed.now())?;
        let plan = ac::plan_trash(&def, &config_dir).map_err(|e| e.to_string())?;
        let sessions = state.agents.read().map_err(err)?.clone();
        let tags = task_meta::tags(&*state.conn()?, &sessions).map_err(err)?;
        let persona = |s: &AgentSession| tags.get(&s.key).and_then(|t| t.agent.clone());
        let busy = live_agents::collect(&config_dir, &sessions, &persona, chrono_now_ms())
            .iter()
            .any(|a| a.agent == def.name && matches!(a.state, live_agents::ActState::Working | live_agents::ActState::Waiting));
        if busy {
            return Err(Fail::AgentInUse.now());
        }
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(trp(C::DlgAgentTrashBody, &P::new().text("name", &plan.name, 64).text("path", &plan.path.display().to_string(), 300)))
            .title(tr(C::DlgAgentTrashTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTrash), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        ac::recheck(&plan).map_err(|e| e.to_string())?;
        move_to_trash(&plan.path)?;
        let _ = fresh_catalog(&state);
        let target = match &plan.project {
            Some(p) => p.join(".claude/settings.local.json"),
            None => config_dir.join("settings.json"),
        };
        let off = ac::denied_names(&dduck_core::permissions::read_scope(&target)).contains(&plan.name);
        let hidden = settings::hidden_agents(&*state.conn()?).map_err(err)?.contains(&plan.name);
        Ok(TrashResult { name: plan.name, project: plan.project.map(|p| p.display().to_string()), off, hidden })
    })
    .await
    .map_err(err)?
}

/// After a delete: removes the same-named `Agent(<name>)` deny rule (global, or that project's personal settings)
/// and the hidden entry.
#[tauri::command]
async fn forget_agent(app: AppHandle, name: String, project: Option<String>) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if !agents_catalog::valid_name(&name) {
            return Err(Fail::BadAgentName.now());
        }
        let path = match permission_project(&state, project.as_deref())? {
            Some(d) => d.join(".claude/settings.local.json"),
            None => state.config.read().map_err(err)?.path.join("settings.json"),
        };
        if dduck_core::permissions::read_scope(&path).exists {
            write_agent_deny(&app, &path, &name, false)?;
        }
        settings::set_agent_hidden(&*state.conn()?, &name, false).map_err(err)
    })
    .await
    .map_err(err)?
}

/// A start the webview can cancel by id (D-161). Registered when the start begins and removed when it ends, whatever
/// the path; a cancel for an unknown id does nothing.
struct PendingStart {
    app: AppHandle,
    id: Option<String>,
}

impl PendingStart {
    /// Ids come from the webview: short ASCII only, anything else is simply not cancellable.
    fn register(app: &AppHandle, id: Option<String>) -> Self {
        let id = id.filter(|i| !i.is_empty() && i.len() <= 64 && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        if let Some(i) = &id {
            if let Ok(mut m) = app.state::<AppState>().start_pending.lock() {
                m.insert(i.clone(), false);
            }
        }
        PendingStart { app: app.clone(), id }
    }

    fn cancelled(&self) -> bool {
        let Some(id) = &self.id else { return false };
        self.app.state::<AppState>().start_pending.lock().map(|m| m.get(id) == Some(&true)).unwrap_or(false)
    }

    fn check(&self) -> Result<(), CliError> {
        if self.cancelled() { Err(CliError::Cancelled) } else { Ok(()) }
    }
}

impl Drop for PendingStart {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            if let Ok(mut m) = self.app.state::<AppState>().start_pending.lock() {
                m.remove(id);
            }
        }
    }
}

/// "취소" on a "시작하는 중" card. Before `claude --bg` runs the start just ends; if it already ran, the new task is
/// stopped at once, so a cancelled start never leaves a task running. True when the id was a start in flight.
#[tauri::command(async)]
fn start_cancel(state: State<'_, AppState>, start_id: String) -> bool {
    state.start_pending.lock().map(|mut m| m.get_mut(&start_id).map(|v| *v = true).is_some()).unwrap_or(false)
}

/// Starts a background task. Dangerous modes always go through a native Rust dialog, whatever the webview did.
#[tauri::command]
async fn start_task(app: AppHandle, mut spec: TaskSpec, start_id: Option<String>) -> Result<String, CliError> {
    let pending = PendingStart::register(&app, start_id);
    // Step durations only, emitted when dropped (D-159).
    let mut steps = timing::Steps::new("start_task");
    // Login gate first (claude-login): before any check, dialog or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    steps.lap("login_gate");
    pending.check()?;
    spec.validate()?;
    // Presets, team policy and guards; everything injected comes from bundled data (AG-09).
    if spec.remote_control {
        // Only after the user turned it on through the native confirmation in Settings (D-067).
        let allowed = app.state::<AppState>().conn().ok().and_then(|c| settings::remote_alerts(&c).ok()).unwrap_or(false);
        if !allowed {
            return Err(CliError::invalid(ReasonCode::RemoteAlertsOff));
        }
    }
    // The stored team config is merged and re-validated inside `prepare` on every start (D-081).
    let team = app.state::<AppState>().conn().ok().and_then(|c| settings::team_config(&c).ok().flatten());
    let env = loadout_env(&app.state::<AppState>(), &spec.cwd, team.as_deref());
    steps.lap("loadout");
    // 전달 범위 (delivery-guard): computed here, never from the webview; any failure refuses the start.
    let worktree_possible = spec.origin.is_none() && dduck_core::team::repo_status(&spec.cwd, &home_dir()) == dduck_core::team::RepoStatus::Ok;
    let delivery = delivery_cmds::start_guard(&app.state::<AppState>(), &spec.cwd, worktree_possible).map_err(|reason| CliError::invalid(ReasonCode::from_guard(reason)))?;
    if delivery.force_worktree {
        // The default branch is checked out: the task works on its own branch in a worktree.
        spec.worktree = true;
    }
    let dangerous = spec.permission_mode.is_some_and(PermissionMode::is_dangerous);
    if let Some(reason) = dduck_core::delivery_guard::local_only_refusal(delivery.ctx.guard, dangerous, &delivery_cmds::managed_sources(&app.state::<AppState>())) {
        return Err(CliError::invalid(ReasonCode::from_guard(reason)));
    }
    spec.guard_ctx = Some(delivery.ctx.clone());
    // The start language (app-language design 6): one reply-language line, recorded for resumes.
    spec.lang = Some(app_lang(&app));
    steps.lap("delivery_guard");
    spec.prepare_with(&home_dir(), chrono_now_ms(), team.as_deref(), &env)?;
    steps.lap("prepare");
    pending.check()?;
    let task_type = spec.task_type.clone();
    let client = app.state::<AppState>().client().map_err(|_| CliError::Spawn)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut steps = steps;
        let pending = pending;
        // An agent turned off with `Agent(<name>)` anywhere is not started by D.Duck either (D-081).
        if let Some(name) = spec.agent.as_deref().or(spec.inline_role.as_deref()) {
            if agent_denied(&app.state::<AppState>(), name, &spec.cwd) {
                return Err(CliError::invalid(ReasonCode::AgentOff));
            }
        }
        // Persona: must be usable in this folder; risky definitions need the same native approval as a
        // dangerous mode, because their settings may override the explicit permission flag (D-057).
        // The risk comes from a raw scan of cwd, its ancestors and the user directory — not the cached catalog —
        // so files discovery skips (links, oversize, duplicates) still count (security review, D-057).
        let persona_risk = match &spec.agent {
            Some(name) => {
                let state = app.state::<AppState>();
                let defs = fresh_catalog_for(&state, Some(&spec.cwd)).map_err(|_| CliError::Spawn)?;
                let config_dir = state.config.read().map(|c| c.path.clone()).map_err(|_| CliError::Spawn)?;
                let (path, risk) = agents_catalog::persona_risk(&defs, &config_dir, name, &spec.cwd)
                    .ok_or(CliError::invalid(ReasonCode::AgentNotHere))?;
                risk.any().then_some((name.clone(), dduck_core::text::display_safe(&path, 200), risk))
            }
            None => None,
        };
        if spec.strict_type {
            // Strict types never run as a persona that can write or run commands (no `tools` = every tool).
            if let Some(name) = &spec.agent {
                let defs = fresh_catalog_for(&app.state::<AppState>(), Some(&spec.cwd)).map_err(|_| CliError::Spawn)?;
                if agents_catalog::resolve_for_cwd(&defs, name, &spec.cwd).is_none_or(|d| dduck_core::team_config::writes(&d.tools)) {
                    return Err(CliError::invalid(ReasonCode::StrictPersonaEdits));
                }
            }
        }
        if spec.strict_type && persona_risk.is_some() {
            return Err(CliError::invalid(ReasonCode::StrictPersonaCommands));
        }
        if spec.remote_control && persona_risk.is_some() {
            return Err(CliError::invalid(ReasonCode::PersonaNoRemote));
        }
        if spec.sensitive && persona_risk.is_some() {
            return Err(CliError::invalid(ReasonCode::SensitivePersona));
        }
        // A draft built from Claude's web-search-based advice: show the whole brief natively before it runs.
        if spec.origin == Some(dduck_core::claude_cli::Origin::InsightClaude) {
            let state = app.state::<AppState>();
            let _guard = state.confirm_lock.try_lock().map_err(|_| CliError::invalid(ReasonCode::ConfirmOpen))?;
            // Newlines kept; every other control, bidi or invisible character dropped.
            let mut brief: String = spec.prompt.chars().filter(|c| *c == '\n' || !dduck_core::text::is_unsafe_char(*c)).take(4_000).collect();
            if spec.prompt.chars().count() > 4_000 {
                brief.push('…');
            }
            let ok = app
                .dialog()
                .message(trp(C::DlgAdviceBody, &P::new().text("folder", &spec.cwd.display().to_string(), 200).lines("brief", brief.split('\n'), 4_001, 4_000)))
                .title(tr(C::DlgAdviceTitle))
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnStart), tr(C::BtnCancel)))
                .blocking_show();
            if !ok {
                return Err(CliError::DangerNotConfirmed);
            }
        }
        let dangerous_mode = spec.permission_mode.filter(|m| m.is_dangerous());
        let mut limit_auto_ok = false;
        let confirm_state = app.state::<AppState>();
        let _guard = if dangerous_mode.is_some() || persona_risk.is_some() {
            Some(confirm_state.confirm_lock.try_lock().map_err(|_| CliError::invalid(ReasonCode::ConfirmOpen))?)
        } else {
            None
        };
        let approval = match (dangerous_mode, &persona_risk) {
            (None, Some((name, path, risk))) => {
                let ok = app
                    .dialog()
                    .message(trp(
                        C::DlgPersonaBody,
                        &P::new()
                            .text("name", name, 64)
                            .text("path", path, 300)
                            .lines("risks", risk_lines(risk), 120, 8)
                            .text("type", &spec.type_label.clone().unwrap_or_else(|| tr(C::NoType)), 20)
                            .text("mode", spec.permission_mode.map_or("manual", |m| m.as_cli()), 20)
                            .text("sensitive", &tr(if spec.sensitive { C::Yes } else { C::No }), 10)
                            .text("worktree", &tr(if spec.worktree_name.is_some() { C::On } else { C::Off }), 10)
                            .text("folder", &spec.cwd.display().to_string(), 200),
                    ))
                    .title(tr(C::DlgPersonaTitle))
                    .kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnRun), tr(C::BtnCancel)))
                    .blocking_show();
                if !ok {
                    return Err(CliError::DangerNotConfirmed);
                }
                Some(DangerApproval::after_native_confirmation())
            }
            (Some(mode), _) => {
                // Show exactly what will run, so a request altered in the webview cannot be approved blindly.
                let cwd = std::fs::canonicalize(&spec.cwd).unwrap_or_else(|_| spec.cwd.clone());
                let mut preview = dduck_core::text::display_safe(&spec.prompt.replace('\n', " "), 200);
                if spec.prompt.chars().count() > 200 {
                    preview.push('…');
                }
                let name = spec.name.as_deref().map(|n| dduck_core::text::display_safe(n, 64)).unwrap_or_else(|| tr(C::NoName));
                let persona_note = persona_risk
                    .as_ref()
                    .map(|(n, _, r)| trp(C::DlgDangerPersona, &P::new().text("name", n, 64).lines("risks", risk_lines(r), 120, 8)))
                    .unwrap_or_default();
                // Native dialogs have no checkbox: "리셋 후 자동으로 이어서 하기" is its own button, off unless pressed
                // (D-141 (5)). Deploy and phone-controlled tasks never resume on their own, so they get two buttons.
                let offer_auto = task_type.as_deref() != Some("deploy") && !spec.remote_control;
                let (run, run_auto, cancel) = (tr(C::BtnRun), tr(C::BtnRunAuto), tr(C::BtnCancel));
                let buttons = if offer_auto {
                    MessageDialogButtons::YesNoCancelCustom(run.clone(), run_auto.clone(), cancel)
                } else {
                    MessageDialogButtons::OkCancelCustom(run.clone(), cancel)
                };
                let result = app
                    .dialog()
                    .message(trp(
                        C::DlgDangerBody,
                        &P::new()
                            .text("mode", mode.as_cli(), 20)
                            .text("folder", &cwd.display().to_string(), 200)
                            .text("name", &name, 64)
                            .text("prompt", &preview, 201)
                            .lines("persona", persona_note.split('\n'), 200, 12)
                            .lines("auto", if offer_auto { tr(C::DlgDangerAuto) } else { String::new() }.split('\n'), 200, 3),
                    ))
                    .title(tr(C::DlgDangerTitle))
                    .kind(MessageDialogKind::Warning)
                    .buttons(buttons)
                    .blocking_show_with_result();
                match result {
                    tauri_plugin_dialog::MessageDialogResult::Custom(b) if b == run_auto => limit_auto_ok = true,
                    tauri_plugin_dialog::MessageDialogResult::Custom(b) if b == run => {}
                    tauri_plugin_dialog::MessageDialogResult::Ok | tauri_plugin_dialog::MessageDialogResult::Yes => {}
                    _ => return Err(CliError::DangerNotConfirmed),
                }
                Some(DangerApproval::after_native_confirmation())
            }
            (None, None) => None,
        };
        steps.lap("checks_and_confirm");
        // Last point before anything is created: a cancel from here on stops the started task below instead.
        pending.check()?;
        // 작업 보고서: an app-owned folder and its instruction, unless off globally or for this task (D-108).
        let report_id = attach_report(&app, &mut spec, task_type.as_deref());
        steps.lap("report_folder");
        let started = match client.start_background(&spec, approval) {
            Err(CliError::UntrustedWorkspace { stderr_tail }) => trust_then_retry(&app, &client, &spec, stderr_tail),
            other => other,
        };
        steps.lap("claude_bg");
        // Cancelled while `claude --bg` ran: stop what it started, so nothing keeps running behind the user's back.
        if let Ok(id) = &started {
            if pending.cancelled() {
                return match client.stop(id) {
                    Ok(()) => Err(CliError::Cancelled),
                    // Could not stop it: say so; the task shows up on the board where it can be stopped by hand.
                    Err(_) => Err(CliError::KillFailed),
                };
            }
        }
        let mut guard_unrecorded = false;
        if let Ok(id) = &started {
            // Every D.Duck start is recorded; the instruction only while "작업 지시 저장" is on (D-069).
            let who = spec.agent.as_deref().or(spec.inline_role.as_deref());
            let app_state = app.state::<AppState>();
            let cwd_text = spec.cwd.to_string_lossy();
            // Up to 3 tries, each on a fresh pooled connection (D-164): a failed write is retried instead of leaving
            // a running task without its record. The SQLite code of each failure goes to the timing line only.
            let mut recorded = false;
            for attempt in 0..3 {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_millis(150 * attempt));
                }
                let conn = match app_state.conn() {
                    Ok(c) => c,
                    Err(_) => {
                        steps.code("record_conn_err", -1);
                        continue;
                    }
                };
                let keep = settings::store_task_prompts(&conn).unwrap_or(true);
                let rec = task_meta::NewRecord {
                    bg_id: id,
                    name: spec.name.as_deref(),
                    cwd: &cwd_text,
                    agent: who,
                    task_type: task_type.as_deref(),
                    prompt: keep.then_some(spec.prompt.as_str()),
                    brief: if keep { spec.brief.as_ref() } else { None },
                    effort: spec.effort.map(|e| e.as_cli()),
                    worktree: spec.worktree_name.is_some(),
                    permission_mode: Some(spec.permission_mode.unwrap_or(PermissionMode::Manual).as_cli()),
                    report_id: report_id.as_deref(),
                    origin: spec.origin,
                    loadout_dropped: spec.loadout_dropped,
                    now_ms: chrono_now_ms(),
                };
                let result = task_meta::record_start(&conn, &rec)
                    .and_then(|()| task_meta::record_guard(&conn, id, delivery.ctx.guard, delivery.default_branch.as_deref(), delivery.default_head.as_deref()))
                    .and_then(|()| spec.lang.map_or(Ok(()), |l| task_meta::record_lang(&conn, id, l)));
                match result {
                    Ok(()) => {
                        let _ = task_meta::set_limit_fields(&conn, id, spec.remote_name.is_some(), limit_auto_ok);
                        if spec.limit_opt_out {
                            let _ = dduck_core::limit_resume::set_opt_out(&conn, id, true, chrono_now_ms());
                        }
                        if let Some(root) = reports_root(&app) {
                            dduck_core::task_report::sweep(&conn, &root);
                        }
                        recorded = true;
                        break;
                    }
                    Err(e) => steps.code("record_err", sqlite_code(&e)),
                }
            }
            guard_unrecorded = !recorded;
        }
        steps.lap("record");
        if started.is_ok() && !guard_unrecorded {
            poll_soon();
        }
        if guard_unrecorded {
            if let Ok(id) = &started {
                // Never silent (D-164): a task that could not be stopped keeps running without a record.
                if client.stop(id).is_err() {
                    return Err(CliError::invalid(ReasonCode::RecordFailedRunning));
                }
            }
            return Err(CliError::invalid(ReasonCode::RecordFailedStopped));
        }
        started
    })
    .await
    .map_err(|_| CliError::Spawn)?
}

/// The id must belong to a background session in the current list — not just look like an id.
fn ensure_listed_background(state: &AppState, id: &str) -> Result<(), CliError> {
    dduck_core::claude_cli::check_id(id)?;
    let listed = state
        .agents
        .read()
        .map(|a| a.iter().any(|s| s.id.as_deref() == Some(id) && s.kind == SessionKind::Background))
        .unwrap_or(false);
    if listed { Ok(()) } else { Err(CliError::InvalidId) }
}

#[tauri::command]
async fn task_logs(state: State<'_, AppState>, id: String) -> Result<String, CliError> {
    ensure_listed_background(&state, &id)?;
    let client = state.client().map_err(|_| CliError::Spawn)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || client.logs(&id)).await.map_err(|_| CliError::Spawn)??;
    // Kept in memory only; the webview renders it with a terminal emulator, never as HTML.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tauri::command]
async fn stop_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), CliError> {
    ensure_listed_background(&state, &id)?;
    let client = state.client().map_err(|_| CliError::Spawn)?;
    let bg = id.clone();
    tauri::async_runtime::spawn_blocking(move || client.stop(&id)).await.map_err(|_| CliError::Spawn)??;
    // A task the user stopped is never resumed after a usage-limit reset (usage-limit-resume).
    limit_cmds::on_user_stop(&app, &bg);
    Ok(())
}

/// "끝내고 넘기기" (D-167 behaviour, D-190 name): stops a background task normally (the conversation stays, as "중지") and moves it on —
/// an unreviewed report to 리뷰 대기, anything else straight to done. Never skips the review.
#[tauri::command]
async fn finish_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<dduck_core::task_lifecycle::Finished, CliError> {
    ensure_listed_background(&state, &id)?;
    let key = state.agents.read().ok().and_then(|a| a.iter().find(|s| s.id.as_deref() == Some(id.as_str())).map(|s| s.key.clone())).ok_or(CliError::InvalidId)?;
    let client = state.client().map_err(|_| CliError::Spawn)?;
    let bg = id.clone();
    tauri::async_runtime::spawn_blocking(move || client.stop(&id)).await.map_err(|_| CliError::Spawn)??;
    limit_cmds::on_user_stop(&app, &bg);
    let conn = state.conn().map_err(|_| CliError::invalid(ReasonCode::StoppedNotDone))?;
    let since = settings::review_since(&conn, chrono_now_ms()).map_err(|_| CliError::invalid(ReasonCode::StoppedNotDone))?;
    let done = dduck_core::task_lifecycle::finish_stopped(&conn, &bg, &key, since, chrono_now_ms())
        .map_err(|_| CliError::invalid(ReasonCode::StoppedNotDone))?;
    drop(conn);
    poll_soon();
    Ok(done)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachResult {
    opened: bool,
    /// Command to paste into a terminal when automation is unavailable or denied.
    command: String,
}

const TRUST_WAIT: Duration = Duration::from_secs(180);

/// The folder is not trusted yet (D-047): ask, open Claude in that folder so the user can accept the trust prompt,
/// watch Claude's config (read-only) until the folder is trusted, then start the task again.
fn trust_then_retry(app: &AppHandle, client: &ClaudeCli, spec: &TaskSpec, stderr_tail: String) -> Result<String, CliError> {
    let dir = std::fs::canonicalize(&spec.cwd).unwrap_or_else(|_| spec.cwd.clone());
    let ok = app
        .dialog()
        .message(trp(C::DlgTrustBody, &P::new().text("folder", &dir.display().to_string(), 200)))
        .title(tr(C::DlgTrustTitle))
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTrustInTerminal), tr(C::BtnCancel)))
        .blocking_show();
    if !ok || !cli_path::is_shell_safe_path(&dir) {
        return Err(CliError::UntrustedWorkspace { stderr_tail });
    }
    if !open_terminal_os(client, &TerminalTarget::Trust(dir.clone())) {
        return Err(CliError::UntrustedWorkspace { stderr_tail });
    }
    let _ = app.emit("task-start-status", "awaiting_trust");
    let config_file = dduck_core::trust::config_file(&home_dir(), client.config_dir_env.as_deref());
    let deadline = std::time::Instant::now() + TRUST_WAIT;
    while std::time::Instant::now() < deadline {
        if dduck_core::trust::is_trusted(&config_file, &dir) == Some(true) {
            let _ = app.emit("task-start-status", "trusted");
            // A dangerous mode was already approved before the first attempt.
            let approval = spec.permission_mode.filter(|m| m.is_dangerous()).map(|_| DangerApproval::after_native_confirmation());
            return client.start_background(spec, approval);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err(CliError::UntrustedWorkspace { stderr_tail })
}

// ---------- artifacts & routines ----------

#[tauri::command(async)]
fn list_artifacts(state: State<'_, AppState>, query: String) -> Res<Vec<artifacts::ArtifactRow>> {
    artifacts::list(&*state.conn()?, query.chars().take(100).collect::<String>().as_str(), 500).map_err(err)
}

/// Opens a URL in the default browser: one argument to `/usr/bin/open`, no options (D-053).
fn open_external(state: &AppState, url: &str) -> Res<()> {
    if !url.starts_with("https://claude.ai/") {
        return Err(Fail::LinkNotAllowed.now());
    }
    open_url(state, url)
}

/// Opens an https URL the caller has already vetted (and, for untrusted text, confirmed natively).
fn open_url(state: &AppState, url: &str) -> Res<()> {
    {
        let mut last = state.last_open.lock().map_err(err)?;
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return Err(Fail::TryLater.now());
        }
        *last = Some(Instant::now());
    }
    #[cfg(target_os = "macos")]
    {
        let child = std::process::Command::new("/usr/bin/open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        let ok = match child {
            Ok(mut c) => {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    match c.try_wait() {
                        Ok(Some(st)) => break st.success(),
                        Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                        _ => {
                            let _ = c.kill();
                            let _ = c.wait();
                            break false;
                        }
                    }
                }
            }
            Err(_) => false,
        };
        if ok { Ok(()) } else { Err(Fail::BrowserFailed.now()) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(Fail::CopyLinkInstead.now().into())
    }
}

#[tauri::command(async)]
fn open_artifact(state: State<'_, AppState>, artifact_id: String) -> Res<()> {
    let url = artifacts::safe_url(&*state.conn()?, &artifact_id).map_err(err)?.ok_or_else(|| Fail::NotCollectedArtifact.now())?;
    open_external(&state, &url)
}

/// Card preview of a collected artifact from its local source file (D-102). Only the id comes in; the session is
/// taken from the DB row. Failures are a reason code only: no path or content in errors or logs.
#[tauri::command]
async fn artifact_preview(
    app: AppHandle,
    artifact_id: String,
) -> Result<dduck_core::artifact_preview::Preview, dduck_core::artifact_preview::PreviewError> {
    use dduck_core::artifact_preview::{self as ap, PreviewError};
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (session_id, kind) = {
            let conn = state.conn().map_err(|_| PreviewError::Unknown)?;
            ap::lookup(&conn, &artifact_id).map_err(|_| PreviewError::Unknown)?.ok_or(PreviewError::Unknown)?
        };
        if kind == "typed" {
            return Err(PreviewError::Typed);
        }
        let config = state.config.read().map_err(|_| PreviewError::Unknown)?.path.clone();
        let root = discover::projects_root(&config).ok_or(PreviewError::Unknown)?;
        let allowed = ap::Allowed::for_user(&home_dir()).ok_or(PreviewError::Unknown)?;
        ap::preview(&state.previews, &root, &allowed, &session_id, &artifact_id, false)
    })
    .await
    .map_err(|_| PreviewError::Unknown)?
}

/// Lists Claude cloud routines through a guarded headless `claude` run (D-066). Button-triggered only.
#[tauri::command]
async fn cloud_routines(app: AppHandle) -> Result<dduck_core::cloud_routines::CloudList, login_cmds::CmdError> {
    // Login gate first (claude-login): before any claim, confirmation or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::ClaudeAi).await?;
    tauri::async_runtime::spawn_blocking(move || -> Result<_, login_cmds::CmdError> {
        use dduck_core::cloud_routines as cr;
        let state = app.state::<AppState>();
        let _guard = state.cloud_lock.try_lock().map_err(|_| Fail::AlreadyLoading.now())?;
        // App-owned, empty working folder and hook script (0700): no project CLAUDE.md, hooks or settings apply.
        let base = app_data_dir(&app).map_err(err)?.join("cloud-routines");
        let cwd = base.join("cwd");
        std::fs::create_dir_all(&cwd).map_err(err)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
        }
        // The hook is this executable itself in guard mode (main.rs): a real JSON check, no shell script.
        let exe = std::env::current_exe().map_err(err)?;
        let client = state.client()?;
        let config = state.config.read().map_err(err)?.path.clone();
        let projects = config.join("projects");
        let hook = cr::guard_command(&exe);
        // RemoteTrigger is auto-allowed by the CLI, so a hook that cannot run would fail open: check it first.
        if !cr::guard_self_test(&hook, cr::SELF_TEST_DENY, cr::SELF_TEST_ALLOW) {
            return Err(Fail::GuardUnchecked.now().into());
        }
        let result = client.cloud_routines_raw(&cwd, &hook);
        // Parse first: a large response is saved by the CLI under the cwd's project folder, which the cleanup
        // below removes (D-072). Cleaning before parsing deleted the file and every load failed.
        // A login failure is matched by variant (never by message text), re-checked and returned as the gate error.
        let mut login_lost = false;
        let parsed = result
            .map_err(|e| match e {
                CliError::Timeout => Fail::Timeout60.now(),
                CliError::Spawn => Fail::CliSpawn.now(),
                _ => Fail::CliFailedRetry.now(),
            })
            .and_then(|raw| {
                cr::parse_stream(&raw, &projects).map_err(|e| {
                    login_lost = e == cr::CloudError::LoginExpired;
                    e.to_string()
                })
            });
        // Whatever happened, nothing the run stored (it can hold every routine's prompt) stays behind.
        if let Ok(cr::Parsed { leftovers: Some(left), .. }) = &parsed {
            cr::clean(&projects, left);
        }
        cr::clean_cwd_project(&projects, &cwd);
        if login_lost {
            login_cmds::recheck_after_failure(&app);
            return Err(CliError::LoginRequired.into());
        }
        let mut list = parsed?.list.ok_or_else(|| Fail::EmptyReply.now())?;
        // "로컬만" projects never run in the cloud (delivery-guard): mark them; enabling or editing is refused.
        let local = delivery_cmds::cloud_guard(&state);
        for r in list.routines.iter_mut() {
            r.local_only = local.blocks(&r.repo_slugs);
        }
        Ok(list)
    })
    .await
    .map_err(err)?}

#[tauri::command(async)]
fn open_cloud_routines(state: State<'_, AppState>) -> Res<()> {
    open_external(&state, artifacts::CLOUD_ROUTINES_URL)
}

#[tauri::command(async)]
fn open_cloud_routine(state: State<'_, AppState>, id: String) -> Res<()> {
    let url = dduck_core::cloud_routines::routine_url(&id).ok_or_else(|| Fail::BadRoutineId.now())?;
    open_external(&state, &url)
}

// A short-lived command return value serialized once; boxing `Done` would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
enum CloudEdit {
    Declined,
    #[serde(rename_all = "camelCase")]
    Done { routine: dduck_core::cloud_routines::CloudRoutine, cost_usd: Option<f64> },
}

/// One guarded headless RemoteTrigger call with exactly `expected` (D-077): fresh nonce, guard self-test (D-095),
/// run, parse with the allowed-field parser, then clean in the D-072 order. The response (it holds the routine's
/// prompt) stays in memory and is never logged.
fn cloud_call(app: &AppHandle, expected: &serde_json::Value) -> Result<(dduck_core::cloud_routines::CloudRoutine, Option<f64>), login_cmds::CmdError> {
    cloud_exact(app, expected, |body, cost| dduck_core::cloud_routines::parse_one(body).map(|r| (r, cost)))
}

/// One exact guarded `RemoteTrigger` call (`expected` is the only input the hook allows, once) and its parsed result.
/// The raw response stays in memory, is parsed by `parse`, then the saved file and the run's session folder are
/// deleted whatever happened (D-072). Shared by get/update (cloud-routine-edit) and run history/logs (D-166).
fn cloud_exact<T>(
    app: &AppHandle,
    expected: &serde_json::Value,
    parse: impl FnOnce(&str, Option<f64>) -> Result<T, dduck_core::cloud_routines::CloudError>,
) -> Result<T, login_cmds::CmdError> {
    use dduck_core::cloud_routines as cr;
    let state = app.state::<AppState>();
    let base = app_data_dir(app).map_err(err)?.join("cloud-routines");
    let cwd = base.join("cwd");
    let once = base.join("once");
    std::fs::create_dir_all(&cwd).map_err(err)?;
    std::fs::create_dir_all(&once).map_err(err)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(&once, std::fs::Permissions::from_mode(0o700));
    }
    let exe = std::env::current_exe().map_err(err)?;
    let nonce = cr::new_nonce().ok_or_else(|| Fail::NonceFailed.now())?;
    let b64 = cr::encode_expected(expected);
    let hook = cr::edit_guard_command(&exe, &once, &nonce, &b64).ok_or_else(|| Fail::RequestFailed.now())?;
    let marker = once.join(&nonce);
    // The self-test uses the exact hook (and so creates the one-time marker); remove it before the real run.
    let tested = cr::edit_guard_self_test(&hook, expected);
    let _ = std::fs::remove_file(&marker);
    if !tested {
        return Err(Fail::GuardUnchecked.now().into());
    }
    let client = state.client()?;
    let projects = state.config.read().map_err(err)?.path.join("projects");
    let result = client.cloud_edit_raw(&cwd, &cr::edit_args(&hook, expected));
    let _ = std::fs::remove_file(&marker);
    // Login failures are matched by variant, re-checked, and returned as the gate error (claude-login).
    let mut login_lost = false;
    let raw = result
        .map_err(|e| match e {
            CliError::Timeout => Fail::Timeout60.now(),
            CliError::Spawn => Fail::CliSpawn.now(),
            _ => Fail::CliFailed.now(),
        })
        .and_then(|out| {
            cr::read_stream(&out, &projects).map_err(|e| {
                login_lost |= e == cr::CloudError::LoginExpired;
                e.to_string()
            })
        });
    // Parse first, then delete the saved response and the run's session folder (D-072), whatever happened.
    let parsed = raw.as_ref().map_err(Clone::clone).and_then(|r| {
        parse(&r.body, r.cost_usd).map_err(|e| {
            login_lost |= e == cr::CloudError::LoginExpired;
            e.to_string()
        })
    });
    if let Ok(cr::RawResult { leftovers: Some(left), .. }) = &raw {
        cr::clean(&projects, left);
    }
    drop(raw);
    cr::clean_cwd_project(&projects, &cwd);
    if login_lost {
        login_cmds::recheck_after_failure(app);
        return Err(CliError::LoginRequired.into());
    }
    Ok(parsed?)
}

/// A cloud routine's recent runs (D-166): one guarded, read-only `list_runs` call, first page only.
#[tauri::command]
async fn cloud_routine_runs(app: AppHandle, id: String) -> Result<dduck_core::cloud_routines::CloudRuns, login_cmds::CmdError> {
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::ClaudeAi).await?;
    tauri::async_runtime::spawn_blocking(move || -> Result<_, login_cmds::CmdError> {
        use dduck_core::cloud_routines as cr;
        let expected = cr::expected_list_runs(&id).ok_or_else(|| Fail::BadRoutineId.now())?;
        let state = app.state::<AppState>();
        let _run = state.cloud_lock.try_lock().map_err(|_| Fail::CloudBusy.now())?;
        cloud_exact(&app, &expected, cr::parse_runs)
    })
    .await
    .map_err(err)?
}

/// One run's condensed log (D-166): one guarded, read-only `get_run_log` call, newest page only. In memory for
/// display; never stored or logged.
#[tauri::command]
async fn cloud_run_log(app: AppHandle, session_id: String) -> Result<dduck_core::cloud_routines::CloudRunLog, login_cmds::CmdError> {
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::ClaudeAi).await?;
    tauri::async_runtime::spawn_blocking(move || -> Result<_, login_cmds::CmdError> {
        use dduck_core::cloud_routines as cr;
        let expected = cr::expected_run_log(&session_id).ok_or_else(|| Fail::BadRunId.now())?;
        let state = app.state::<AppState>();
        let _run = state.cloud_lock.try_lock().map_err(|_| Fail::CloudBusy.now())?;
        cloud_exact(&app, &expected, cr::parse_log)
    })
    .await
    .map_err(err)?
}

/// Changes a cloud routine's enabled state, name or simple schedule (cloud-routine-edit). Native confirmation, then
/// a guarded `get` (conflict check against what the app showed) and the exact guarded `update`.
#[tauri::command]
async fn update_cloud_routine(app: AppHandle, id: String, before: dduck_core::cloud_routines::Before, edit: dduck_core::cloud_routines::Edit, listed_at: i64) -> Result<CloudEdit, login_cmds::CmdError> {
    // Login gate first (claude-login): before any claim, confirmation or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::ClaudeAi).await?;
    tauri::async_runtime::spawn_blocking(move || -> Result<_, login_cmds::CmdError> {
        use dduck_core::cloud_routines as cr;
        let state = app.state::<AppState>();
        let get = cr::expected_get(&id).ok_or_else(|| Fail::BadRoutineId.now())?;
        let (body, changes) = cr::build_body(&before, &edit, now_ms(), cr::local_offset_min())?;
        let update = cr::expected_update(&id, &body).ok_or_else(|| Fail::CannotChange.now())?;
        let _run = state.cloud_lock.try_lock().map_err(|_| Fail::CloudBusy.now())?;
        let age_min = (now_ms() - listed_at).max(0) / 60_000;
        let ok = {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            let lines: Vec<String> = changes.iter().map(|(label, old, new)| format!("{label}: {old} → {new}")).collect();
            app.dialog()
                .message(trp(C::DlgCloudBody, &P::new().text("name", &before.name, 80).lines("changes", &lines, 300, 20).num("minutes", age_min)))
                .title(tr(C::DlgCloudTitle))
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnChange), tr(C::BtnCancel)))
                .blocking_show()
        };
        if !ok {
            return Ok(CloudEdit::Declined);
        }
        let (now, c1) = cloud_call(&app, &get)?;
        // Turning it off is the only change allowed for a routine on a "로컬만" project's repository.
        if edit.enabled != Some(false) && delivery_cmds::cloud_guard(&state).blocks(&now.repo_slugs) {
            return Err(Fail::CloudBlocked.now().into());
        }
        if !cr::unchanged(&before, &now) {
            return Err(Fail::ChangedElsewhere.now().into());
        }
        let (routine, c2) = cloud_call(&app, &update)?;
        let cost_usd = match (c1, c2) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        };
        Ok(CloudEdit::Done { routine, cost_usd })
    })
    .await
    .map_err(err)?}

#[tauri::command(async)]
fn list_routines(state: State<'_, AppState>) -> Res<Vec<routines::RoutineSummary>> {
    routines::list(&*state.conn()?).map_err(err)
}

/// Full prompt for the editor only; the list carries just its length (D-050).
#[tauri::command(async)]
fn routine_prompt(state: State<'_, AppState>, id: i64) -> Res<Option<String>> {
    routines::prompt_of(&*state.conn()?, id).map_err(err)
}

#[tauri::command(async)]
fn routine_runs(state: State<'_, AppState>, id: i64) -> Res<Vec<routines::RoutineRun>> {
    routines::runs(&*state.conn()?, id, 100).map_err(err)
}

/// Read-only trust check for the form's warning (D-047).
#[tauri::command(async)]
fn folder_trusted(state: State<'_, AppState>, path: String) -> Res<Option<bool>> {
    let env = state.config.read().map_err(err)?.env_value().map(PathBuf::from);
    let file = dduck_core::trust::config_file(&home_dir(), env.as_deref());
    Ok(dduck_core::trust::is_trusted(&file, std::path::Path::new(&path)))
}

fn mode_meaning(m: PermissionMode) -> String {
    match m {
        PermissionMode::Plan => tr(C::ModePlan),
        PermissionMode::Manual => tr(C::ModeManual),
        PermissionMode::AcceptEdits => tr(C::ModeAcceptEdits),
        PermissionMode::DontAsk => tr(C::ModeDontAsk),
        _ => String::new(),
    }
}

/// Creates or updates a routine. New routines and any change to the folder, prompt, permission or schedule need a
/// native confirmation; a name-only change does not (D-049).
#[tauri::command]
async fn save_routine(app: AppHandle, id: Option<i64>, input: RoutineInput) -> Res<i64> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let valid = input.validate().map_err(|e| e.to_string())?;
        if let Some(id) = id {
            let stored = routines::load_valid(&*state.conn()?, id).map_err(err)?.ok_or_else(|| Fail::RoutineNotFound.now())?;
            if routines::digest(&valid) == stored.1 {
                routines::rename(&*state.conn()?, id, &valid.name).map_err(err)?;
                return Ok(id);
            }
        }
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let mut preview = dduck_core::text::display_safe(&valid.prompt.as_str().replace('\n', " "), 200);
        if valid.prompt.as_str().chars().count() > 200 {
            preview.push('…');
        }
        let ok = app
            .dialog()
            .message(trp(
                C::DlgRoutineBody,
                &P::new()
                    .text("name", &valid.name, 64)
                    .text("folder", &valid.cwd.display().to_string(), 200)
                    .text("schedule", &valid.schedule.describe_in(dduck_core::i18n::current()), 120)
                    .text("mode", valid.permission_mode.as_cli(), 20)
                    .text("meaning", &mode_meaning(valid.permission_mode), 200)
                    .num("chars", valid.prompt.as_str().chars().count() as i64)
                    .text("prompt", &preview, 201),
            ))
            .title(tr(if id.is_some() { C::DlgRoutineEditTitle } else { C::DlgRoutineNewTitle }))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnSave), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Err(ReasonCode::Cancelled.now());
        }
        let now = chrono_now_ms();
        let conn = state.conn()?;
        let saved = match id {
            Some(id) => routines::update(&conn, id, &valid, now, &chrono::Local).map(|_| id),
            None => routines::insert(&conn, &valid, now, &chrono::Local),
        }
        .map_err(err)?;
        drop(conn);
        if id.is_some() {
            state.send_ingest(IngestMsg::Scrub); // the previous prompt must not linger in freed pages
        }
        Ok(saved)
    })
    .await
    .map_err(err)?
}

#[tauri::command(async)]
fn set_routine_enabled(state: State<'_, AppState>, id: i64, enabled: bool) -> Res<()> {
    routines::set_enabled(&*state.conn()?, id, enabled, chrono_now_ms(), &chrono::Local).map_err(err)
}

#[tauri::command(async)]
fn delete_routine(state: State<'_, AppState>, id: i64) -> Res<()> {
    routines::delete(&mut *state.conn()?, id).map_err(err)?;
    state.send_ingest(IngestMsg::Scrub);
    Ok(())
}

/// "지금 실행": same claim path as the scheduler; never shows dialogs (D-051).
#[tauri::command]
async fn run_routine_now(app: AppHandle, id: i64) -> Result<(), login_cmds::CmdError> {
    // Login gate first (claude-login): before any claim, confirmation or CLI run. Unknown passes.
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let claim = match routines::claim_now(&mut *state.conn()?, id, chrono_now_ms()) {
            Ok(c) => c,
            Err(routines::RunNowError::Busy) => return Err(Fail::AlreadyRunning.now()),
            Err(routines::RunNowError::NotFound) => return Err(Fail::RoutineNotFound.now()),
            Err(routines::RunNowError::Invalid(kind)) => return Err(Fail::RunInvalid.with(&dduck_core::i18n::Params::new().text("kind", kind, 40))),
        };
        start_claim(&app, claim);
        Ok(())
    })
    .await
    .map_err(err)?
    .map_err(login_cmds::CmdError::from)
}

/// Opens the trust prompt for a routine's stored folder (the webview sends only the id, D-051).
#[tauri::command]
async fn trust_routine_folder(app: AppHandle, id: i64) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let dir = routines::cwd_of(&*state.conn()?, id).map_err(err)?.ok_or_else(|| Fail::RoutineNotFound.now())?;
        if !cli_path::is_shell_safe_path(&dir) {
            return Err(Fail::FolderNameTerminal.now());
        }
        let client = state.client()?;
        Ok(open_terminal_os(&client, &TerminalTarget::Trust(dir)))
    })
    .await
    .map_err(err)?
}

fn cli_error_kind(e: &CliError) -> &'static str {
    match e {
        CliError::Spawn => "spawn",
        CliError::Timeout => "timeout",
        CliError::UntrustedWorkspace { .. } => "untrustedWorkspace",
        CliError::Failed { .. } => "failed",
        CliError::UnsupportedOutput => "unsupportedOutput",
        CliError::InvalidId => "invalidId",
        CliError::InvalidInput { .. } => "invalidInput",
        CliError::DangerNotConfirmed => "dangerNotConfirmed",
        CliError::KillFailed => "killFailed",
        CliError::Cancelled => "cancelled",
        CliError::LoginRequired => dduck_core::auth_status::LOGIN_REQUIRED,
        CliError::ClaudeAiRequired => dduck_core::auth_status::CLAUDE_AI_REQUIRED,
    }
}

// ---------- 작업 보고서 (decision-report, D-108) ----------

/// `<app data>/reports`. Report paths and ids are never logged or sent to the webview.
fn reports_root<R: tauri::Runtime, M: Manager<R>>(app: &M) -> Option<PathBuf> {
    app_data_dir(app).ok().map(|d| d.join(dduck_core::task_report::DIR_NAME))
}

/// Gives a local task its report folder (`--add-dir`) and instruction. `None` when the setting is off, the task
/// unchecked it, it is phone-controlled (remote, not applied), or the folder could not be made safely: the task
/// then starts without a report.
fn attach_report(app: &AppHandle, spec: &mut TaskSpec, task_type: Option<&str>) -> Option<String> {
    use dduck_core::task_report as tr;
    let on = || app.state::<AppState>().conn().ok().and_then(|c| settings::task_report(&c).ok()).unwrap_or(true);
    if !tr::wanted(spec, on) {
        return None;
    }
    let root = reports_root(app)?;
    let id = tr::new_id()?;
    let dir = tr::prepare_dir(&root, &id).ok()?;
    tr::attach(spec, dir.clone(), tr::is_review_type(task_type));
    // A role without a write tool gets `Write` + the report-dir hook only after spike 1.1 (`report_guard::MEASURED`).
    let blocked = delivery_cmds::managed_sources(&app.state::<AppState>()).blocks_hooks();
    let exe = std::env::current_exe().ok();
    if let dduck_core::report_guard::Grant::Granted { settings } =
        dduck_core::report_guard::grant(&mut spec.inline_roles_json, spec.inline_role.as_deref(), Some(dir.as_path()), exe.as_deref(), blocked, dduck_core::report_guard::MEASURED)
    {
        spec.report_settings = Some(settings);
    }
    Some(id)
}

/// List chips: report state for sessions that were given a report (stat only, D-108).
fn add_report_presence(app: &AppHandle, conn: &Connection, sessions: &[AgentSession], tags: &mut std::collections::HashMap<String, task_meta::Tag>) {
    let Some(root) = reports_root(app) else { return };
    for (key, p) in dduck_core::task_report::presence_for(conn, &root, sessions) {
        tags.entry(key).or_default().report = Some(p);
    }
}

/// The report screen of one D.Duck task: the app's own "위임 내용" plus the validated report, or a reason code.
#[tauri::command(async)]
fn task_report(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<dduck_core::task_report::View> {
    if !task_meta::valid_bg_id(&id) {
        return Err(Fail::BadTaskId.now());
    }
    let root = reports_root(&app).ok_or_else(|| Fail::ReportUnreadable.now())?;
    let conn = state.conn()?;
    // delivery-guard post-checks at report time (local git only): stored in task_meta.delivery_alert. A failed check
    // leaves the stored value; the report still opens.
    let _ = dduck_core::delivery_guard::post_check(&conn, &id, Some(&home_dir()), chrono_now_ms() / 1000);
    let target = dduck_core::task_report::target(&conn, &id).map_err(|_| Fail::ReportUnreadable.now())?;
    // The DB lock is released before any report file is read, checked or parsed.
    drop(conn);
    Ok(dduck_core::task_report::view_of(&root, target))
}

/// One evidence preview (report-evidence): by the DB's report id and indexes only. Paths never cross IPC.
#[tauri::command(async)]
fn report_evidence(app: AppHandle, state: State<'_, AppState>, id: String, criterion: usize, item: usize) -> Res<dduck_core::evidence::Preview> {
    if !task_meta::valid_bg_id(&id) {
        return Err(Fail::BadTaskId.now());
    }
    let root = reports_root(&app).ok_or_else(|| Fail::EvidenceUnreadable.now())?;
    let target = dduck_core::task_report::target(&*state.conn()?, &id).map_err(|_| Fail::EvidenceUnreadable.now())?;
    dduck_core::task_report::evidence_preview_of(&root, &target, criterion, item).map_err(|_| Fail::EvidenceUnreadable.now())
}

/// One attachment, cleaned in Rust into a full srcdoc (HTML) or SafeMarkdown text (D-156).
#[tauri::command(async)]
fn report_attachment(app: AppHandle, state: State<'_, AppState>, id: String, index: usize) -> Res<dduck_core::attachment::Opened> {
    if !task_meta::valid_bg_id(&id) {
        return Err(Fail::BadTaskId.now());
    }
    let root = reports_root(&app).ok_or_else(|| Fail::AttachmentUnreadable.now())?;
    let target = dduck_core::task_report::target(&*state.conn()?, &id).map_err(|_| Fail::AttachmentUnreadable.now())?;
    dduck_core::task_report::attachment_open_of(&root, &target, index).map_err(|_| Fail::AttachmentUnreadable.now())
}

// ---------- 리뷰 탭 (review-inbox, D-140) ----------

/// Card summaries of tasks that were given a report. `project` is an opaque id; one that cannot be resolved is an
/// error, never every project. The DB lock is released before any report file is read.
#[tauri::command]
async fn review_inbox(app: AppHandle, project: Option<String>, filter: dduck_core::review_inbox::Filter, offset: usize, limit: usize) -> Res<dduck_core::review_inbox::Inbox> {
    tauri::async_runtime::spawn_blocking(move || {
        use dduck_core::review_inbox as ri;
        if offset > ri::MAX_OFFSET {
            return Err(Fail::BadRequest.now());
        }
        let state = app.state::<AppState>();
        let scope = match project {
            Some(id) => {
                let folder = projects_cmds::usable_folder(&state, &id)?;
                Some(std::fs::canonicalize(folder).map_err(|_| Fail::ProjectFolderMissing.now())?)
            }
            None => None,
        };
        let running = ri::running_map(&state.agents.read().map_err(err)?);
        let (rows, since, report_setting) = {
            let conn = state.conn()?;
            (ri::rows(&conn).map_err(err)?, settings::review_since(&conn, chrono_now_ms()).map_err(err)?, settings::task_report(&conn).map_err(err)?)
        };
        let root = reports_root(&app).ok_or_else(|| Fail::ReportUnreadable.now())?;
        let known = ri::poll_known() && state.agents_error.read().map_err(err)?.is_none();
        let q = ri::Query { filter, scope: scope.as_deref(), offset, limit, since, report_setting, known };
        let mut inbox = ri::build(rows, &root, &running, &q).map_err(|_| Fail::ReviewListUnreadable.now())?;
        // delivery-guard post-checks for finished cards not checked yet (local git only, one page at most).
        let now_s = chrono_now_ms() / 1000;
        for it in inbox.items.iter_mut().filter(|i| i.delivery_alert.is_none() && i.running.is_none()) {
            if let Ok(conn) = state.conn() {
                it.delivery_alert = dduck_core::delivery_guard::post_check(&conn, &it.bg_id, Some(&home_dir()), now_s).ok().flatten();
            }
        }
        Ok(inbox)
    })
    .await
    .map_err(err)?
}

/// "리뷰했어요" from the 리뷰 탭: refused for a running task (by the last listing) or one without a report.
#[tauri::command(async)]
fn inbox_mark_reviewed(state: State<'_, AppState>, id: String) -> Res<()> {
    let running = dduck_core::review_inbox::running_map(&state.agents.read().map_err(err)?);
    let known = dduck_core::review_inbox::poll_known() && state.agents_error.read().map_err(err)?.is_none();
    dduck_core::review_inbox::mark(&*state.conn()?, &id, &running, known, chrono_now_ms()).map_err(|_| Fail::NotReviewable.now())
}

fn delete_fail(e: dduck_core::Error) -> String {
    use dduck_core::review_inbox as ri;
    match e {
        dduck_core::Error::Config(ri::DELETE_RUNNING) => Fail::DeleteRunning.now(),
        dduck_core::Error::Config(ri::DELETE_RESUME_PENDING) => Fail::DeleteResumePending.now(),
        dduck_core::Error::Config(ri::DELETE_CHANGED) => Fail::DeleteChanged.now(),
        dduck_core::Error::Config("task id") => Fail::BadTaskId.now(),
        _ => Fail::DeleteFailed.now(),
    }
}

/// "삭제" from the 리뷰 탭 (review-delete): checks, a native confirmation naming what goes and what stays, then a fresh
/// listing and one transaction that re-checks and deletes. `false` = cancelled. Only the id comes from the webview.
#[tauri::command]
async fn inbox_delete(app: AppHandle, id: String) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        use dduck_core::review_inbox as ri;
        let state = app.state::<AppState>();
        if !task_meta::valid_bg_id(&id) {
            return Err(Fail::BadTaskId.now());
        }
        let known = || -> Res<bool> { Ok(ri::poll_known() && state.agents_error.read().map_err(err)?.is_none()) };
        let plan = {
            let conn = state.conn()?;
            let running = ri::running_map(&state.agents.read().map_err(err)?);
            ri::delete_refusal(&conn, &id, &running, known()?).map_err(delete_fail)?;
            ri::delete_plan(&conn, &id).map_err(delete_fail)?.ok_or_else(|| Fail::DeleteFailed.now())?
        };
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(ri::delete_confirm_text(&plan, i18n::current()))
            .title(tr(C::DlgReviewDeleteTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnDelete), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        // The task may have restarted while the dialog was open: a fresh listing, not the last poll.
        let fresh = state.client()?.list_agents().map_err(|_| Fail::DeleteRunning.now())?;
        let running = ri::running_map(&fresh);
        let listed_key = fresh.iter().find(|s| s.id.as_deref() == Some(id.as_str())).map(|s| s.key.clone());
        let mut conn = state.conn()?;
        let task = ri::delete(&mut conn, &id, &plan, &running, known()?, listed_key.as_deref(), reports_root(&app).as_deref()).map_err(delete_fail)?;
        if task {
            let _ = ingest::scrub(&conn); // the instruction must not linger in freed pages or the WAL
        }
        Ok(true)
    })
    .await
    .map_err(err)?
}

/// "되돌리기": clears "리뷰했어요".
#[tauri::command(async)]
fn unmark_reviewed(state: State<'_, AppState>, id: String) -> Res<bool> {
    dduck_core::review_inbox::unmark(&*state.conn()?, &id).map_err(|_| Fail::BadTaskId.now())
}

/// Starts a claimed run on its own thread. Unattended: no approval object, no dialogs, no terminals (D-051).
fn start_claim(app: &AppHandle, claim: routines::Claim) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let mut spec = claim.spec;
        // "먼저 살펴볼 스킬·에이전트" for the run's folder (preferred-skills), only when something is chosen.
        if let Ok(conn) = state.conn() {
            if routines::wants_preferred(&conn) {
                if let Ok(dir) = state.config.read().map(|c| c.path.clone()) {
                    let installed = preferred_for(&dir, &spec.cwd);
                    routines::add_preferred(&mut spec, &conn, &installed);
                }
            }
        }
        // Logged out: this occurrence is recorded as `login_required`, not held (routines are per occurrence).
        let gate = login_cmds::gate_cli(&app, dduck_core::auth_status::Need::Model).map_err(|e| cli_error_kind(&e));
        let report_id = if gate.is_ok() { attach_report(&app, &mut spec, None) } else { None };
        let result = gate.and_then(|()| match state.client() {
            Ok(client) => client.start_background(&spec, None).map_err(|e| cli_error_kind(&e)),
            Err(_) => Err("spawn"),
        });
        if let Ok(conn) = state.conn() {
            let started = result.is_ok();
            let _ = routines::finish_start(&conn, claim.run_id, result);
            if let (true, Some(id)) = (started, report_id.as_deref()) {
                let _ = dduck_core::task_report::set_run_report(&conn, claim.run_id, id);
            }
            if let Some(root) = reports_root(&app) {
                dduck_core::task_report::sweep(&conn, &root);
            }
        }
        let _ = app.emit("routines-updated", ());
    });
}

const SCHEDULER_EVERY: Duration = Duration::from_secs(20);

fn spawn_scheduler(app: AppHandle) {
    std::thread::spawn(move || {
        if let Ok(conn) = app.state::<AppState>().conn() {
            let _ = routines::close_interrupted(&conn);
        }
        limit_cmds::on_start(&app);
        loop {
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let state = app.state::<AppState>();
                let claims = match state.conn() {
                    Ok(mut conn) => routines::tick(&mut conn, chrono_now_ms(), &chrono::Local).unwrap_or_default(),
                    Err(_) => Vec::new(),
                };
                let any = !claims.is_empty();
                for c in claims {
                    start_claim(&app, c);
                }
                if any {
                    let _ = app.emit("routines-updated", ());
                }
                limit_cmds::on_tick(&app);
            }));
            std::thread::sleep(SCHEDULER_EVERY);
        }
    });
}

/// The session with `key` from the current list (never a value the webview made up).
fn listed_session(state: &AppState, key: &str) -> Result<AgentSession, CliError> {
    state
        .agents
        .read()
        .ok()
        .and_then(|a| a.iter().find(|s| s.key == key).cloned())
        .ok_or(CliError::InvalidId)
}

enum TerminalTarget {
    /// `claude attach <id>` for background sessions (works for stopped ones too).
    Attach(String),
    /// A shell in the session's folder, for sessions running in another terminal.
    Folder(PathBuf),
    /// Interactive `claude` in a folder, so the user can answer its workspace-trust prompt.
    Trust(PathBuf),
}

/// Opens Terminal for any listed session: attached for background sessions, in its folder otherwise (D-043).
/// macOS: osascript receives values only as argv, never spliced into the script (D-025).
#[tauri::command]
async fn open_terminal(state: State<'_, AppState>, key: String) -> Result<AttachResult, CliError> {
    let s = listed_session(&state, &key)?;
    let client = state.client().map_err(|_| CliError::Spawn)?;
    let (target, command) = match (&s.kind, &s.id) {
        (SessionKind::Background, Some(id)) => {
            dduck_core::claude_cli::check_id(id)?;
            (TerminalTarget::Attach(id.clone()), client.attach_command_line(id)?)
        }
        _ => {
            let dir = PathBuf::from(&s.cwd);
            if !dir.is_absolute() || !dir.is_dir() || !cli_path::is_shell_safe_path(&dir) {
                return Err(CliError::invalid(ReasonCode::SessionFolder));
            }
            let command = format!("cd '{}'", s.cwd.replace('\'', r"'\''"));
            (TerminalTarget::Folder(dir), command)
        }
    };
    let opened = tauri::async_runtime::spawn_blocking(move || open_terminal_os(&client, &target)).await.unwrap_or(false);
    Ok(AttachResult { opened, command })
}

#[cfg(target_os = "macos")]
fn open_terminal_os(client: &ClaudeCli, target: &TerminalTarget) -> bool {
    let mut cmd = std::process::Command::new("/usr/bin/osascript");
    match target {
        TerminalTarget::Attach(id) => {
            let env_prefix = match &client.config_dir_env {
                Some(_) => r#""CLAUDE_CONFIG_DIR=" & (quoted form of item 3 of argv) & " " & "#,
                None => "",
            };
            let script = format!(
                r#"tell application "Terminal" to do script {env_prefix}(quoted form of item 1 of argv) & " attach " & (quoted form of item 2 of argv)"#
            );
            cmd.args(["-e", "on run argv", "-e", &script, "-e", "tell application \"Terminal\" to activate", "-e", "end run", "--"]);
            cmd.arg(&client.path).arg(id);
            if let Some(dir) = &client.config_dir_env {
                cmd.arg(dir);
            }
        }
        TerminalTarget::Folder(dir) => {
            let script = r#"tell application "Terminal" to do script "cd " & (quoted form of item 1 of argv)"#;
            cmd.args(["-e", "on run argv", "-e", script, "-e", "tell application \"Terminal\" to activate", "-e", "end run", "--"]);
            cmd.arg(dir);
        }
        TerminalTarget::Trust(dir) => {
            let env_prefix = match &client.config_dir_env {
                Some(_) => r#""CLAUDE_CONFIG_DIR=" & (quoted form of item 3 of argv) & " " & "#,
                None => "",
            };
            let script = format!(
                r#"tell application "Terminal" to do script "cd " & (quoted form of item 1 of argv) & " && " & {env_prefix}(quoted form of item 2 of argv)"#
            );
            cmd.args(["-e", "on run argv", "-e", &script, "-e", "tell application \"Terminal\" to activate", "-e", "end run", "--"]);
            cmd.arg(dir).arg(&client.path);
            if let Some(cfg) = &client.config_dir_env {
                cmd.arg(cfg);
            }
        }
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn open_terminal_os(_client: &ClaudeCli, _target: &TerminalTarget) -> bool {
    // Linux/Windows terminal launching is a follow-up (design Open Questions); the UI offers the copy command.
    false
}

/// Force-stops any listed session (D-043). Background: `claude stop`, then the process if it is still alive.
/// Terminal sessions: a native confirmation (it may drop unsent input), then SIGTERM → SIGKILL on the verified pid.
#[tauri::command]
async fn force_stop(app: AppHandle, key: String) -> Result<(), CliError> {
    let state = app.state::<AppState>();
    let s = listed_session(&state, &key)?;
    let client = state.client().ok();
    tauri::async_runtime::spawn_blocking(move || {
        let pid = s.pid.ok_or(CliError::InvalidId)?;
        if let (SessionKind::Background, Some(id), Some(client)) = (&s.kind, &s.id, &client) {
            let _ = client.stop(id);
            std::thread::sleep(Duration::from_secs(2));
            if !dduck_core::procs::is_own_claude_process(pid) {
                return Ok(());
            }
        } else {
            let name = dduck_core::text::display_safe(&s.name, 64);
            let ok = app
                .dialog()
                .message(trp(C::DlgKillBody, &P::new().text("name", &name, 64)))
                .title(tr(C::DlgKillTitle))
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnForceQuit), tr(C::BtnCancel)))
                .blocking_show();
            if !ok {
                return Err(CliError::DangerNotConfirmed);
            }
        }
        dduck_core::procs::terminate(pid, Duration::from_secs(3)).map_err(|_| CliError::KillFailed)
    })
    .await
    .map_err(|_| CliError::Spawn)?
}

/// The webview may only show the bundled app (and the dev server in debug builds); links, `location` changes and
/// terminal OSC 8 hyperlinks cannot navigate it elsewhere (D-017).
fn navigation_guard() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_webview, url| {
            let host = url.host_str().unwrap_or("");
            match url.scheme() {
                "tauri" => true,
                // Artifact card previews are sandboxed `srcdoc` iframes (D-102); nothing else of `about:` is allowed.
                "about" if url.as_str() == "about:srcdoc" => true,
                "http" | "https" if host == "tauri.localhost" => true,
                "http" if cfg!(debug_assertions) && host == "localhost" && url.port() == Some(1420) => true,
                _ => false,
            }
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    timing::init();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(navigation_guard())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app_data_dir(app)?;
            // One-time markers of cloud-routine edit runs (D-077) never outlive a run; clear any left by a crash.
            let _ = std::fs::remove_dir_all(data_dir.join("cloud-routines").join("once"));
            let db_path = db::adopt_legacy(&data_dir, "dduck.db");
            let conn = db::open(&db_path).map_err(|e| Box::new(std::io::Error::other(e.to_string())))?;
            // One login-shell probe at startup, for the CLI path and CLAUDE_CONFIG_DIR (D-028).
            let probe = cli_path::probe_default_shell();
            let setting_dir = settings::config_dir(&conn).ok().flatten();
            let config = cli_path::effective_config_dir(setting_dir.as_deref(), &probe, &home_dir());
            let prices = PriceTable::load(Some(&data_dir.join("pricing.json")));
            let (tx, rx) = mpsc::channel();
            let state = AppState {
                db: db_pool::DbPool::new(db_path.clone(), conn),
                prices,
                config: RwLock::new(config.clone()),
                probe,
                cli: RwLock::new(Cli { info: None, client: None }),
                agents: RwLock::new(Vec::new()),
                agents_error: RwLock::new(None),
                window_visible: AtomicBool::new(true),
                ingest_tx: Mutex::new(tx.clone()),
                tray_signature: Mutex::new(None),
                catalog: RwLock::new(Vec::new()),
                confirm_lock: Mutex::new(()),
                cloud_lock: Mutex::new(()),
                insights_busy: AtomicBool::new(false),
                insights_cancel: AtomicBool::new(false),
                refine_busy: AtomicBool::new(false),
                refine_cancel: Default::default(),
                refine_guard: Default::default(),
                refine_consent: Mutex::new(None),
                timeline_lock: Mutex::new(()),
                previews: dduck_core::artifact_preview::PreviewCache::new(),
                start_pending: Mutex::new(std::collections::HashMap::new()),
                mcp_busy: std::sync::atomic::AtomicBool::new(false),
                mcp_last: Mutex::new(None),
                plugin_busy: AtomicBool::new(false),
                last_open: Mutex::new(None),
            };
            state.resolve_cli();
            if let Ok(conn) = state.conn() {
                dduck_core::task_report::sweep(&conn, &data_dir.join(dduck_core::task_report::DIR_NAME));
            }
            app.manage(state);
            app.manage(add_items::AddState::new(data_dir.clone()));
            create_tray(&handle)?;
            spawn_ingest_worker(handle.clone(), db_path.clone(), config.path, rx, tx);
            spawn_poller(handle.clone(), db_path);
            spawn_scheduler(handle.clone());
            login_cmds::on_start(&handle);
            let notify_on = app.state::<AppState>().conn().ok().and_then(|c| settings::notify_waiting(&c).ok()).unwrap_or(true);
            if notify_on {
                let _ = handle.notification().request_permission();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(true) = event {
                login_cmds::on_focus(window.app_handle());
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing hides; the tray keeps collection and polling alive (desktop-shell spec).
                api.prevent_close();
                let _ = window.hide();
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.window_visible.store(false, Ordering::Relaxed);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            limit_cmds::limit_status,
            limit_cmds::limit_cancel,
            limit_cmds::set_limit_opt_out,
            limit_cmds::limit_resume_now,
            set_pref,
            get_language,
            set_language,
            get_onboarding_state,
            set_onboarding_state,
            set_store_titles,
            pick_cli_path,
            reset_cli_path,
            pick_config_dir,
            reset_config_dir,
            rebuild_usage,
            pick_directory,
            ingest_files,
            usage_summary,
            usage_timeseries,
            usage_distribution,
            usage_sessions,
            usage_sessions_page,
            usage_insights_local,
            usage_insights_preview,
            usage_insights_claude,
            usage_insights_cancel,
            refine_brief,
            refine_cancel,
            usage_insights_last,
            usage_insights_delete,
            open_insight_link,
            session_timeline,
            session_raw,
            set_session_label,
            skill_catalog,
            mcp_status,
            plugin_list,
            set_plugin_enabled,
            update_plugin,
            add_items::inspect_link,
            add_items::discard_link,
            add_items::install_plugin,
            add_items::add_marketplace,
            add_items::install_skill,
            add_items::authoring_info,
            add_items::create_authoring_item,
            add_items::read_authoring_item,
            add_items::save_authoring_item,
            add_items::start_authoring_task,
            add_items::authoring_jobs,
            add_items::install_authoring_result,
            add_items::discard_authoring_job,
            add_items::install_authoring_command,
            add_items::remove_authoring_command,
            ext_items_cmds::trash_extension_item,
            usage_session_detail,
            usage_filter_options,
            list_agents,
            start_task,
            start_cancel,
            cloud_routine_runs,
            cloud_run_log,
            finish_task,
            team_config,
            save_team_config,
            reset_team_config,
            preferred_options,
            agent_controls,
            set_agent_enabled,
            set_agent_hidden,
            trash_agent,
            forget_agent,
            task_logs,
            stop_task,
            open_terminal,
            force_stop,
            pick_default_cwd,
            reset_default_cwd,
            session_prompt,
            list_artifacts,
            agent_catalog,
            repo_check,
            set_remote_alerts,
            task_records,
            task_record,
            task_report,
            report_evidence,
            report_attachment,
            report_save_cmds::save_report_attachment,
            delete_task_record,
            delete_all_task_records,
            set_store_task_prompts,
            mark_reviewed,
            review_inbox,
            inbox_mark_reviewed,
            inbox_delete,
            unmark_reviewed,
            agent_activity,
            agent_definition,
            save_agent_definition,
            permission_scopes,
            save_permissions,
            save_agent_tools,
            save_agent_loadout,
            loadout_options,
            explain_permission,
            permission_effective,
            permission_rule_covers,
            open_artifact,
            artifact_preview,
            open_cloud_routines,
            cloud_routines,
            update_cloud_routine,
            open_cloud_routine,
            list_routines,
            routine_prompt,
            routine_runs,
            folder_trusted,
            save_routine,
            set_routine_enabled,
            delete_routine,
            run_routine_now,
            trust_routine_folder,
            memory_cmds::memory_projects,
            memory_cmds::memory_entries,
            memory_cmds::memory_read,
            memory_cmds::save_memory,
            memory_cmds::set_memory_enabled,
            memory_cmds::trash_memory,
            memory_cmds::auto_memory_setting,
            memory_cmds::set_auto_memory,
            memory_cmds::instruction_files,
            memory_cmds::open_instruction_file,
            memory_cmds::open_memory_index,
            memory_cmds::rules_list,
            memory_cmds::rule_read,
            memory_cmds::save_rule,
            memory_cmds::set_rule_enabled,
            memory_cmds::trash_rule,
            projects_cmds::projects_known,
            workflow_check_cmds::check_workflow_meta,
            lifecycle_cmds::board_wrapup,
            lifecycle_cmds::wrap_task,
            lifecycle_cmds::unwrap_task,
            lifecycle_cmds::wrapup_check,
            lifecycle_cmds::remove_worktree,
            delivery_cmds::local_only_projects,
            delivery_cmds::set_local_only,
            delivery_cmds::set_local_only_cwd,
            delivery_cmds::delivery_guard_preview,
            projects_cmds::set_page_project,
            projects_cmds::mcp_servers,
            add_items::copy_item,
            login_cmds::auth_state,
            login_cmds::refresh_auth,
            login_cmds::login_status,
            login_cmds::start_login,
            login_cmds::cancel_login,
            login_cmds::open_login_link,
            login_cmds::login_in_terminal,
        ])
        .build(tauri::generate_context!());
    let Ok(app) = app else {
        eprintln!("D.Duck failed to start");
        std::process::exit(1);
    };
    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        RunEvent::Exit => login_cmds::on_exit(),
        _ => {}
    });
}

#[cfg(test)]
mod poll_tests {
    use super::*;

    #[test]
    fn polls_every_second_for_a_while_after_a_start() {
        assert_eq!(poll_interval(true, 1_000, 0), POLL_VISIBLE);
        assert_eq!(poll_interval(false, 1_000, 0), POLL_HIDDEN);
        assert_eq!(poll_interval(false, 1_000, 1_000 + FAST_POLL_FOR_MS), POLL_FAST);
        assert_eq!(poll_interval(true, 1_000 + FAST_POLL_FOR_MS, 1_000 + FAST_POLL_FOR_MS), POLL_VISIBLE, "back to normal after 30 s");
    }
}
