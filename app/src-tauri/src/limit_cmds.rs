//! Usage-limit notifications and resume after the reset (usage-limit-resume, D-139·D-141).
//!
//! The poller turns new limit records into rows and notifications, the scheduler claims due rows, and a resume
//! re-checks everything `start_task` checks before running `claude --bg --resume`. The webview only ever sends row ids
//! and bg ids from the lists it got; nothing here reads transcript text, CLI output beyond the bg id line, or Claude
//! credentials. Notifications carry fixed phrases, a time and counts only.

use dduck_core::i18n::Fail;
use std::path::{Path, PathBuf};

use dduck_core::claude_cli::{CliError, DangerApproval, PermissionMode, ResumeOutput, ResumeSpec, SessionKind, SessionStatus, TaskSpec};
use dduck_core::limit_resume::{self as lr, Claim, Policy, Reason};
use dduck_core::{agents_catalog, settings, task_meta};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;

use crate::{AppState, Res, agent_denied, chrono_now_ms, err, fresh_catalog_for, home_dir, loadout_env, reports_root, risk_lines};
use dduck_core::i18n::{Code as C, Params as P, ReasonCode, tr, trp};

pub fn policy(conn: &Connection) -> Policy {
    Policy { auto_on: settings::limit_auto_resume(conn).unwrap_or(true), shipped: lr::AUTO_RESUME_SHIPPED }
}

/// Poll cycle: link bg ids to sessions, group new limit records, and announce them (one notification per reset time).
pub fn on_poll(app: &AppHandle, sessions: &[dduck_core::claude_cli::AgentSession]) {
    let state = app.state::<AppState>();
    let Ok(mut conn) = state.conn() else { return };
    let _ = task_meta::link_sessions(&conn, sessions);
    let pol = policy(&conn);
    let backfilled = settings::limit_backfilled(&conn).unwrap_or(false);
    let Ok(notices) = lr::sync(&mut conn, chrono_now_ms(), sessions, pol, backfilled) else { return };
    let notify = settings::limit_notify(&conn).unwrap_or(true);
    drop(conn);
    if notify {
        for n in &notices {
            let body = lr::notice_body(n, chrono_now_ms(), &chrono::Local, dduck_core::i18n::current());
            let _ = app.notification().builder().title("D.Duck").body(dduck_core::text::display_safe(&body, 200)).show();
        }
    }
    if !notices.is_empty() {
        let _ = app.emit("limits-updated", ());
    }
}

/// Scheduler tick: at most one claimed resume per tick (the gap and concurrency rules live in core).
pub fn on_tick(app: &AppHandle) {
    // Logged out: `tick` is not called, so due rows stay `auto` and resume after login (claude-login).
    if !crate::login_cmds::should_tick(app) {
        return;
    }
    let state = app.state::<AppState>();
    let claim = match state.conn() {
        Ok(mut conn) => {
            let pol = policy(&conn);
            // Resumed tasks still working count toward the concurrency limit (MAX_CONCURRENT).
            let working: Vec<String> = state
                .agents
                .read()
                .map(|a| a.iter().filter(|s| s.status == SessionStatus::Working).filter_map(|s| s.session_id.clone()).collect())
                .unwrap_or_default();
            lr::tick_with(&mut conn, chrono_now_ms(), pol, &working).ok().flatten()
        }
        Err(_) => None,
    };
    if let Some(c) = claim {
        let app = app.clone();
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resume(&app, &c, None))).unwrap_or(Err(Reason::CliFailed));
            finish(&app, &c, outcome);
        });
    }
}

pub fn on_start(app: &AppHandle) {
    if let Ok(conn) = app.state::<AppState>().conn() {
        let _ = lr::close_interrupted(&conn, chrono_now_ms());
    }
}

/// First full ingest pass done: what it found counts as already announced.
pub fn on_backfill_done(conn: &Connection) {
    if !settings::limit_backfilled(conn).unwrap_or(false) {
        let _ = lr::finish_backfill(conn);
    }
}

fn finish(app: &AppHandle, c: &Claim, outcome: Result<(), Reason>) {
    if let Ok(conn) = app.state::<AppState>().conn() {
        let _ = lr::finish(&conn, c.id, outcome, chrono_now_ms());
    }
    if outcome.is_ok() {
        let _ = app.notification().builder().title("D.Duck").body(lr::started_body(1, dduck_core::i18n::current())).show();
    }
    let _ = app.emit("limits-updated", ());
}

/// How the user approved a resume by hand ("직접 이어서" after the native dialog).
pub struct ManualApproval {
    danger: Option<DangerApproval>,
}

/// Re-checks and runs one resume. `manual` is `Some` for "직접 이어서" (already confirmed natively); `None` for the
/// scheduler, which never shows a dialog and never enters the trust flow.
fn resume(app: &AppHandle, c: &Claim, manual: Option<ManualApproval>) -> Result<(), Reason> {
    let state = app.state::<AppState>();
    let src = {
        let conn = state.conn().map_err(|_| Reason::CliFailed)?;
        task_meta::resume_source(&conn, &c.bg_id).map_err(|_| Reason::CliFailed)?.ok_or(Reason::SessionMismatch)?
    };
    if src.session_id.as_deref() != Some(c.session_id.as_str()) || dduck_core::claude_cli::check_session_id(&c.session_id).is_err() {
        return Err(Reason::SessionMismatch);
    }
    if manual.is_none() {
        if src.remote != Some(false) {
            return Err(Reason::Remote);
        }
        if src.task_type.as_deref() == Some("deploy") {
            return Err(Reason::Deploy);
        }
        if matches!(src.permission_mode.as_deref(), Some("auto" | "bypassPermissions")) && !src.auto_ok {
            return Err(Reason::Danger);
        }
    }
    // Re-read this session's transcript now: a reply after the stop means someone already continued.
    let session_cwd = reingest_and_check(app, &c.session_id)?;
    // The live list: never resume a session that is running; a live idle/waiting one could be attached in a
    // terminal, and attach state cannot be read yet (spike 1.1), so it is left alone (fail-closed, design 9).
    let listed = state.agents.read().map(|a| a.iter().find(|s| s.session_id.as_deref() == Some(c.session_id.as_str())).cloned()).map_err(|_| Reason::CliFailed)?;
    let live_cwd = match &listed {
        Some(s) if s.status == SessionStatus::Working => return Err(Reason::AlreadyResumed),
        Some(s) if !s.status.is_ended() => {
            let ours = s.kind == SessionKind::Background && (s.id.as_deref() == Some(c.bg_id.as_str()));
            return Err(if ours { Reason::Attached } else { Reason::SessionMismatch });
        }
        Some(s) => Some(PathBuf::from(&s.cwd)),
        None => None,
    };
    let session_cwd = live_cwd.filter(|p| p.is_absolute()).or(session_cwd).ok_or(Reason::CwdMismatch)?;
    let start_cwd = PathBuf::from(src.cwd.clone().ok_or(Reason::CwdMismatch)?);
    let client = state.client().map_err(|_| Reason::CliFailed)?;
    let config_file = dduck_core::trust::config_file(&home_dir(), client.config_dir_env.as_deref());
    let cwd = lr::verify_cwd(&start_cwd, src.worktree, &session_cwd, |d| dduck_core::trust::is_trusted(&config_file, d))?;

    // Rebuild the start's arguments from the record (never the webview): no worktree, no phone control.
    let mode = src.permission_mode.as_deref().and_then(parse_mode).unwrap_or(PermissionMode::Manual);
    let mut spec = TaskSpec::basic(cwd.clone(), lr::resume_prompt(src.lang.unwrap_or_else(dduck_core::i18n::current)).to_string(), src.name.clone(), Some(mode));
    spec.task_type = src.task_type.clone();
    spec.agent = if src.task_type.is_some() { None } else { src.agent.clone() };
    spec.brief = src.brief.clone();
    spec.origin = src.origin.as_deref().and_then(dduck_core::claude_cli::Origin::parse);
    spec.effort = src.effort.as_deref().and_then(parse_effort);
    // The start's language; an older task follows the current app language.
    spec.lang = Some(src.lang.unwrap_or_else(dduck_core::i18n::current));
    let team = state.conn().ok().and_then(|c| settings::team_config(&c).ok().flatten());
    let env = loadout_env(&state, &cwd, team.as_deref());
    // Delivery scope recomputed now (delivery-guard), before `prepare` so the team rules carry the local-only line:
    // a folder ticked "로컬만" since the start gets the new scope, and a scope that cannot be computed refuses.
    spec.guard_ctx = Some(resume_guard(&state, &cwd, mode.is_dangerous())?);
    // The start's model arguments, reproduced (model-select design 10); an older task has none.
    // An older task (no record) runs with no model arguments, never re-resolved; a junk record refuses (design 10).
    spec.model_pin_in = Some(match task_meta::model_pin(&*state.conn().map_err(|_| Reason::CliFailed)?, &src.bg_id).map_err(|_| Reason::CliFailed)? {
        task_meta::PinRead::Pin(p) => p,
        task_meta::PinRead::None => dduck_core::model_select::ModelPin { layer: dduck_core::model_select::Layer::Cli, arg: None, role: None },
        task_meta::PinRead::Corrupt => return Err(Reason::RiskChanged),
    });
    spec.prepare_with(&home_dir(), chrono_now_ms(), team.as_deref(), &env).map_err(|_| Reason::RiskChanged)?;
    // Allowlist re-check and the fallback for a pinned model that is gone (design 11). A block = button only.
    let file_model = spec.agent.as_deref().filter(|_| spec.inline_role.is_none()).and_then(|n| crate::model_cmds::agent_file_model(&state, n, &spec.cwd));
    let parts = crate::model_cmds::fallback_parts(app, &spec.cwd);
    spec.apply_model(file_model.as_deref(), &parts.ctx()).map_err(|_| Reason::RiskChanged)?;
    if let (Some(id), Some(root)) = (src.report_id.as_deref(), reports_root(app)) {
        use dduck_core::task_report as tr;
        if let Ok(dir) = tr::prepare_dir(&root, id) {
            let review = tr::is_review_type(spec.task_type.as_deref());
            tr::attach(&mut spec, dir, review);
        }
    }

    // The same checks start_task runs, against today's files (design 5).
    let denied = spec.agent.as_deref().or(spec.inline_role.as_deref()).is_some_and(|name| agent_denied(&state, name, &spec.cwd));
    let persona_risky = match &spec.agent {
        Some(name) => {
            let defs = fresh_catalog_for(&state, Some(&spec.cwd)).map_err(|_| Reason::RiskChanged)?;
            let config_dir = state.config.read().map(|c| c.path.clone()).map_err(|_| Reason::RiskChanged)?;
            let (_, risk) = agents_catalog::persona_risk(&defs, &config_dir, name, &spec.cwd).ok_or(Reason::RiskChanged)?;
            if spec.strict_type && agents_catalog::resolve_for_cwd(&defs, name, &spec.cwd).is_none_or(|d| dduck_core::team_config::writes(&d.tools)) {
                return Err(Reason::RiskChanged);
            }
            risk.any()
        }
        None => false,
    };
    lr::recheck(denied, persona_risky, spec.strict_type, spec.sensitive, manual.is_some())?;
    let approval = match manual {
        Some(m) => m.danger,
        // The user ticked "리셋 후 자동으로 이어서" in the native dangerous-mode dialog at start (D-141 (5)).
        None if mode.is_dangerous() && src.auto_ok => Some(DangerApproval::after_native_confirmation()),
        None => None,
    };
    let mut rspec = ResumeSpec::from_prepared(&spec, &c.session_id, cwd.clone()).map_err(|_| Reason::CliFailed)?;
    rspec.guard = spec.guard_ctx.clone();
    match client.resume_background(&rspec, approval) {
        Ok(ResumeOutput::Started(id)) => {
            if id != c.bg_id {
                if let Ok(conn) = state.conn() {
                    let _ = task_meta::alias(&conn, &c.bg_id, &id, chrono_now_ms());
                }
            }
            // What this resume really passed (a replaced model included), so the next one reproduces it.
            if let (Some(pin), Ok(conn)) = (spec.model_pin.as_ref(), state.conn()) {
                let _ = task_meta::record_model(&conn, &id, pin, spec.model_requested.as_deref());
            }
            Ok(())
        }
        Ok(ResumeOutput::Copy(id)) => {
            if let Some(id) = id.filter(|i| dduck_core::claude_cli::check_id(i).is_ok()) {
                let _ = client.stop(&id);
            }
            Err(Reason::CopyStarted)
        }
        // Never `trust_then_retry`: an unattended resume must not open the trust flow.
        Err(CliError::UntrustedWorkspace { .. }) => Err(Reason::Untrusted),
        Err(_) => Err(Reason::CliFailed),
    }
}

/// Delivery scope for a resume, recomputed now from the "로컬만" list and the repository's branches, with the same
/// hook and refusals as a start (delivery-guard). No worktree on a resume, so a default-branch checkout gets the
/// bare-push rules.
fn resume_guard(state: &AppState, cwd: &Path, dangerous: bool) -> Result<dduck_core::delivery_guard::GuardCtx, Reason> {
    use dduck_core::delivery_guard as dg;
    let start = crate::delivery_cmds::start_guard(state, cwd, false).map_err(guard_reason)?;
    if dg::local_only_refusal(start.ctx.guard, dangerous, &crate::delivery_cmds::managed_sources(state)).is_some() {
        return Err(Reason::LocalOnlyRefused);
    }
    Ok(start.ctx)
}

/// `start_guard`'s fixed messages → fixed reason codes: a missing hook, or anything unreadable.
fn guard_reason(msg: &'static str) -> Reason {
    if msg == dduck_core::delivery_guard::HOOK_MISSING { Reason::GuardHookMissing } else { Reason::GuardUnreadable }
}

/// Ingests this session's main transcript right away and returns its recorded cwd. A reply after the limit stop
/// closes the resume as already continued.
fn reingest_and_check(app: &AppHandle, session_id: &str) -> Result<Option<PathBuf>, Reason> {
    let state = app.state::<AppState>();
    let config = state.config.read().map(|c| c.path.clone()).map_err(|_| Reason::CliFailed)?;
    let root = dduck_core::discover::projects_root(&config).ok_or(Reason::SessionMismatch)?;
    let files: Vec<_> = dduck_core::discover::discover(&root).into_iter().filter(|f| f.context.session_id == session_id).collect();
    let main = files.iter().find(|f| f.context.agent_id.is_none()).ok_or(Reason::SessionMismatch)?;
    if main.path.file_stem().and_then(|s| s.to_str()) != Some(session_id) {
        return Err(Reason::SessionMismatch);
    }
    let mut conn = state.conn().map_err(|_| Reason::CliFailed)?;
    {
        let mut ing = dduck_core::ingest::Ingestor::new(&mut conn, dduck_core::ingest::IngestOptions::default());
        for f in &files {
            let _ = ing.ingest_file(f, |_| {});
        }
    }
    let continued: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM api_calls a WHERE a.session_id = ?1 AND a.ts > (SELECT MAX(hit_ts) FROM limit_resumes WHERE session_id = ?1))",
            [session_id],
            |r| r.get(0),
        )
        .map_err(|_| Reason::CliFailed)?;
    if continued {
        return Err(Reason::AlreadyResumed);
    }
    let cwd: Option<String> = conn.query_row("SELECT project_path FROM sessions WHERE session_id = ?1", [session_id], |r| r.get(0)).ok().flatten();
    Ok(cwd.map(PathBuf::from).filter(|p| p.is_absolute()))
}

fn parse_mode(s: &str) -> Option<PermissionMode> {
    PermissionMode::ALL.into_iter().find(|m| m.as_cli() == s)
}

fn parse_effort(s: &str) -> Option<dduck_core::team::Effort> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

/// Stopping a task in D.Duck cancels its waiting resumes.
pub fn on_user_stop(app: &AppHandle, bg_id: &str) {
    if let Ok(conn) = app.state::<AppState>().conn() {
        let _ = lr::user_stopped(&conn, bg_id, chrono_now_ms());
    }
    let _ = app.emit("limits-updated", ());
}

// ---------- commands ----------

#[tauri::command(async)]
pub fn limit_status(app: AppHandle) -> Res<lr::View> {
    let state = app.state::<AppState>();
    let conn = state.conn()?;
    let pol = policy(&conn);
    lr::view(&conn, chrono_now_ms(), &chrono::Local, pol, dduck_core::i18n::current()).map_err(err)
}

/// "이번엔 안 이어요" / "그만두기" for one row id from `limit_status`.
#[tauri::command(async)]
pub fn limit_cancel(app: AppHandle, id: i64) -> Res<bool> {
    let state = app.state::<AppState>();
    let ok = lr::cancel(&*state.conn()?, id, chrono_now_ms()).map_err(err)?;
    let _ = app.emit("limits-updated", ());
    Ok(ok)
}

/// Per-task auto resume off/on, by a bg id D.Duck recorded.
#[tauri::command(async)]
pub fn set_limit_opt_out(app: AppHandle, bg_id: String, off: bool) -> Res<bool> {
    if !task_meta::valid_bg_id(&bg_id) {
        return Err(Fail::BadTaskId.now());
    }
    let state = app.state::<AppState>();
    let ok = lr::set_opt_out(&*state.conn()?, &bg_id, off, chrono_now_ms()).map_err(err)?;
    let _ = app.emit("limits-updated", ());
    Ok(ok)
}

/// "직접 이어서" / "지금 이어서": native confirmation (with the dangerous-mode and persona notes), then the same
/// re-checks as an automatic resume. Returns the fixed reason code when it did not run.
#[tauri::command]
pub async fn limit_resume_now(app: AppHandle, id: i64) -> Result<Option<Reason>, crate::login_cmds::CmdError> {
    // Login gate before the claim and the native dialog (claude-login).
    crate::login_cmds::gate_async(&app, dduck_core::auth_status::Need::Model).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (claim, mode, name) = {
            let mut conn = state.conn()?;
            let row: Option<(String, String)> = conn
                .query_row("SELECT session_id, bg_id FROM limit_resumes WHERE id = ?1 AND bg_id IS NOT NULL", [id], |r| Ok((r.get(0)?, r.get(1)?)))
                .ok();
            let Some((_, bg)) = row else { return Err(Fail::NotResumable.now()) };
            let src = task_meta::resume_source(&conn, &bg).map_err(err)?.ok_or_else(|| Fail::NotResumable.now())?;
            if src.task_type.as_deref() == Some("deploy") {
                // Deploy tasks are never resumed by D.Duck; the user continues them in the terminal.
                return Err(Fail::DeployResumeTerminal.now());
            }
            let mode = src.permission_mode.as_deref().and_then(parse_mode).unwrap_or(PermissionMode::Manual);
            let claim = lr::claim_manual(&mut conn, id, chrono_now_ms()).map_err(err)?.ok_or_else(|| Fail::AlreadyHandled.now())?;
            (claim, mode, src.name)
        };
        let _guard = match state.confirm_lock.try_lock() {
            Ok(g) => g,
            Err(_) => {
                // Nothing ran: the stop stays on its card (never closed as cancelled).
                if let Ok(conn) = state.conn() {
                    let _ = lr::unclaim(&conn, claim.id, chrono_now_ms());
                }
                let _ = app.emit("limits-updated", ());
                return Err(ReasonCode::ConfirmOpen.now());
            }
        };
        let danger_note = if mode.is_dangerous() { trp(C::DlgResumeDanger, &P::new().text("mode", mode.as_cli(), 20)) } else { String::new() };
        let persona_note = persona_note(&app, &claim.bg_id);
        let ok = app
            .dialog()
            .message(trp(
                C::DlgResumeBody,
                &P::new()
                    .text("name", &name.clone().unwrap_or_else(|| tr(C::NoName)), 64)
                    .lines("danger", danger_note.split('\n'), 200, 3)
                    .lines("persona", persona_note.split('\n'), 200, 12),
            ))
            .title(tr(C::DlgResumeTitle))
            .kind(if mode.is_dangerous() { MessageDialogKind::Warning } else { MessageDialogKind::Info })
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnContinue), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            // Back to "직접 이어서": nothing ran.
            if let Ok(conn) = state.conn() {
                let _ = lr::unclaim(&conn, claim.id, chrono_now_ms());
            }
            let _ = app.emit("limits-updated", ());
            return Ok(Some(Reason::Cancelled));
        }
        let approval = ManualApproval { danger: Some(DangerApproval::after_native_confirmation()) };
        let outcome = resume(&app, &claim, Some(approval));
        finish(&app, &claim, outcome);
        Ok(outcome.err())
    })
    .await
    .map_err(err)?
    .map_err(crate::login_cmds::CmdError::from)
}

/// Persona risk lines for the manual dialog (empty when the task has no persona or it is not risky).
fn persona_note(app: &AppHandle, bg_id: &str) -> String {
    let state = app.state::<AppState>();
    let Some(src) = state.conn().ok().and_then(|c| task_meta::resume_source(&c, bg_id).ok().flatten()) else { return String::new() };
    let (Some(name), None, Some(cwd)) = (src.agent.as_deref(), src.task_type.as_deref(), src.cwd.as_deref()) else { return String::new() };
    let cwd = Path::new(cwd);
    let Ok(defs) = fresh_catalog_for(&state, Some(cwd)) else { return String::new() };
    let Ok(config_dir) = state.config.read().map(|c| c.path.clone()) else { return String::new() };
    match agents_catalog::persona_risk(&defs, &config_dir, name, cwd) {
        Some((_, risk)) if risk.any() => trp(C::DlgDangerPersona, &P::new().text("name", name, 64).lines("risks", risk_lines(&risk), 120, 8)),
        _ => String::new(),
    }
}
