//! News tab (news-tab, D-204): the confirmed or daily headless run, its guard, and the list commands.
//!
//! The webview only sends item ids and on/off. Every limit (budget, time, reads, daily spacing, 30-day cap) lives in
//! `dduck_core::news`; the confirmation dialogs are native. The answer is parsed in Rust and only checked fields are
//! stored; stderr and the raw answer are dropped.

use std::sync::atomic::{AtomicBool, Ordering as O};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use dduck_core::i18n::{self, Code as C, Fail, Params as P, ReasonCode, tr, trp};
use dduck_core::news::{self, NewsError};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, IngestMsg, Res, app_data_dir, chrono_now_ms, err, login_cmds, open_url};

/// One news run at a time (manual and daily), claimed before any dialog; and its cancel flag.
static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);
/// The daily run waits this long after the app starts (no paid run during start-up).
const START_GRACE: Duration = Duration::from_secs(60);

struct Busy;

impl Busy {
    fn claim() -> Option<Busy> {
        (!BUSY.swap(true, O::SeqCst)).then_some(Busy)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.store(false, O::SeqCst);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyView {
    enabled: bool,
    next_at_ms: Option<i64>,
    /// The 30-day total reached the cap: the daily run rests.
    capped: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsView {
    items: Vec<news::Stored>,
    status: Option<news::Status>,
    daily: DailyView,
    other_lang: i64,
    running: bool,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NewsRun {
    Done { added: usize, dropped: usize, cost_usd: Option<f64> },
    Declined,
    Failed { error: NewsError, message: String },
}

fn failed(e: NewsError) -> NewsRun {
    NewsRun::Failed { message: e.message(), error: e }
}

#[tauri::command(async)]
pub fn news_list(state: tauri::State<'_, AppState>) -> Res<NewsView> {
    let lang = i18n::current();
    let conn = state.conn()?;
    let d = news::daily(&conn).map_err(err)?;
    Ok(NewsView {
        items: news::list(&conn, lang).map_err(err)?,
        status: news::status(&conn).map_err(err)?,
        daily: DailyView {
            enabled: d.enabled,
            next_at_ms: news::next_daily_ms(d, news::last_try(&conn).map_err(err)?),
            capped: news::month_capped(&conn, chrono_now_ms()).map_err(err)?,
        },
        other_lang: news::count_other_langs(&conn, lang).map_err(err)?,
        running: BUSY.load(O::SeqCst),
    })
}

/// The shared run: guard self-test, app-owned emptied folder, one CLI call, parse, merge, status. `daily` marks the
/// unattended run. The caller holds [`Busy`].
fn run(app: &AppHandle, daily: bool) -> NewsRun {
    let state = app.state::<AppState>();
    let lang = i18n::current();
    CANCEL.store(false, O::SeqCst);
    // Every run counts as a try, so a daily run never follows right after a button press.
    if let Ok(conn) = state.conn() {
        let _ = news::set_last_try(&conn, chrono_now_ms());
    }
    let mut cli_ran = false;
    let outcome = (|| -> Result<news::Parsed, NewsError> {
        let known = state.conn().ok().and_then(|c| news::known(&c, lang).ok()).unwrap_or_default();
        let base = app_data_dir(app).map_err(|_| NewsError::Failed { cost_usd: None })?.join("news");
        let cwd = base.join("cwd");
        let counters = base.join("guard");
        std::fs::create_dir_all(&cwd).map_err(|_| NewsError::Failed { cost_usd: None })?;
        std::fs::create_dir_all(&counters).map_err(|_| NewsError::Failed { cost_usd: None })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
        }
        let exe = std::env::current_exe().map_err(|_| NewsError::Failed { cost_usd: None })?;
        let hook = news::guard_command(&exe, &counters);
        // An old CLI first, with its own reason (also for the daily run): the self-test below would only say "guard".
        if !dduck_core::cli_path::hooks_allowed() {
            return Err(NewsError::CliOutdated);
        }
        // A hook that cannot deny would leave WebFetch open: refuse to start (fail closed).
        if !dduck_core::cloud_routines::guard_self_test(&hook, news::SELF_TEST_DENY, news::SELF_TEST_ALLOW) {
            return Err(NewsError::Guard);
        }
        news::reset_counters(&counters);
        let client = state.client().map_err(|_| NewsError::Spawn)?;
        let projects = state.config.read().map_err(|_| NewsError::Failed { cost_usd: None })?.path.join("projects");
        cli_ran = true;
        let raw = client.insights_raw(&cwd, &news::args(&hook, &known, lang), Duration::from_secs(news::TIMEOUT_SECS), &|| CANCEL.load(O::SeqCst));
        // Parse first, then clean (D-072 order).
        let today = chrono::Local::now().date_naive();
        let parsed = raw.map_err(NewsError::from).and_then(|out| news::parse_output(&out, today));
        dduck_core::cloud_routines::clean_owned_cwd(&projects, &cwd, "news");
        news::reset_counters(&counters);
        parsed
    })();
    let outcome = match outcome {
        Err(NewsError::Failed { .. }) if login_cmds::logged_out_after_failure(app) => Err(NewsError::LoginRequired),
        o => o,
    };
    let now = chrono_now_ms();
    let result = (|| -> Res<NewsRun> {
        let mut conn = state.conn()?;
        let cost = match &outcome {
            Ok(p) => p.cost_usd,
            Err(e) => e.cost_usd(),
        };
        let daily_off = news::after_run(&conn, news::charge(cost, cli_ran && !matches!(outcome, Err(NewsError::Spawn))), now).map_err(err)?;
        Ok(match outcome {
            Ok(p) => {
                let added = news::merge(&mut conn, lang, &p.items, now).map_err(err)?;
                news::save_status(&conn, &news::Status { at_ms: now, ok: true, added, dropped: p.dropped, cost_usd: p.cost_usd, error: None, daily, daily_off })
                    .map_err(err)?;
                NewsRun::Done { added, dropped: p.dropped, cost_usd: p.cost_usd }
            }
            Err(e) => {
                news::save_status(&conn, &news::Status { at_ms: now, ok: false, cost_usd: e.cost_usd(), error: Some(e.code().into()), daily, daily_off, ..Default::default() })
                    .map_err(err)?;
                failed(e)
            }
        })
    })();
    state.send_ingest(IngestMsg::Scrub);
    let _ = app.emit("news-updated", ());
    result.unwrap_or_else(|_| failed(NewsError::Failed { cost_usd: None }))
}

/// "가져오기": login gate, busy claimed, native confirmation with what is sent and the cost, then one run.
#[tauri::command]
pub async fn news_fetch(app: AppHandle) -> Result<NewsRun, login_cmds::CmdError> {
    crate::hook_ready()?;
    login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    tauri::async_runtime::spawn_blocking(move || -> Res<NewsRun> {
        let Some(_busy) = Busy::claim() else { return Ok(failed(NewsError::Busy)) };
        let state = app.state::<AppState>();
        let lang = i18n::current();
        let count = news::known(&*state.conn()?, lang).map_err(err)?.ids.len();
        let ok = {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            app.dialog()
                .message(trp(C::DlgNewsBody, &P::new().text("lang", lang.as_str(), 8).num("count", count as i64).text("cost", &tr(C::NewsCost), 120)))
                .title(tr(C::DlgNewsTitle))
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnFetch), tr(C::BtnCancel)))
                .blocking_show()
        };
        if !ok {
            return Ok(NewsRun::Declined);
        }
        let _ = app.emit("news-updated", ());
        Ok(run(&app, false))
    })
    .await
    .map_err(err)?
    .map_err(login_cmds::CmdError::from)
}

#[tauri::command(async)]
pub fn news_cancel() {
    CANCEL.store(true, O::SeqCst);
}

#[tauri::command(async)]
pub fn news_mark_seen(state: tauri::State<'_, AppState>, id: String) -> Res<bool> {
    news::mark_seen(&*state.conn()?, &id, i18n::current(), chrono_now_ms()).map_err(err)
}

/// On asks natively first (once per consent); off never asks. Returns the new state.
#[tauri::command]
pub async fn news_set_daily(app: AppHandle, enabled: bool) -> Res<bool> {
    if enabled && !cfg!(target_os = "macos") {
        return Err(ReasonCode::MacOnly.now());
    }
    if enabled && !dduck_core::cli_path::hooks_allowed() {
        return Err(ReasonCode::CliUpdateNeeded.now());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if enabled {
            let ok = {
                let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
                app.dialog()
                    .message(tr(C::DlgNewsDailyBody))
                    .title(tr(C::DlgNewsDailyTitle))
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTurnOn), tr(C::BtnCancel)))
                    .blocking_show()
            };
            if !ok {
                return Ok(false);
            }
        }
        news::set_daily(&*state.conn()?, enabled, chrono_now_ms()).map_err(err)?;
        Ok(enabled)
    })
    .await
    .map_err(err)?
}

/// "원문 열기": the stored link of this item, re-checked against the allowlist, in the default browser.
#[tauri::command(async)]
pub fn news_open_source(state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    let url = news::source_of(&*state.conn()?, &id, i18n::current()).map_err(err)?.ok_or_else(|| Fail::NewsNoSource.now())?;
    open_url(&state, &url)
}

/// Scheduler tick: claims busy and starts the daily run on its own thread, so the loop never waits for it.
pub fn on_tick(app: &AppHandle) {
    if STARTED.elapsed() < START_GRACE || BUSY.load(O::SeqCst) {
        return;
    }
    let due = app.state::<AppState>().conn().ok().and_then(|c| news::daily_due(&c, chrono_now_ms()).ok()).unwrap_or(false);
    // Logged out: no try is recorded, so it runs soon after login.
    if !due || !login_cmds::should_tick(app) {
        return;
    }
    let Some(busy) = Busy::claim() else { return };
    // No try recorded means no run (it would retry every tick).
    let recorded = app.state::<AppState>().conn().ok().is_some_and(|c| news::set_last_try(&c, chrono_now_ms()).is_ok());
    if !recorded {
        return;
    }
    // An open News tab shows "가져오는 중" right away.
    let _ = app.emit("news-updated", ());
    let app = app.clone();
    std::thread::spawn(move || {
        let _busy = busy;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&app, true)));
    });
}

/// Touches the start clock (called once at app start).
pub fn on_start() {
    LazyLock::force(&STARTED);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_is_one_at_a_time_and_released_on_drop() {
        let a = Busy::claim();
        assert!(a.is_some());
        assert!(Busy::claim().is_none());
        drop(a);
        assert!(Busy::claim().is_some());
    }
}
