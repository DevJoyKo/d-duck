//! Usage-limit stops and resume after the reset (usage-limit-resume, D-139·D-141).
//!
//! The ingest worker stores structured limit records (`limit_hits`). This module groups them by
//! (session, reset time), decides which D.Duck tasks may resume on their own, schedules and claims resumes, and builds
//! the fixed notification and banner text. Everything here is pure SQL over ids, times and fixed codes: no transcript
//! text, no CLI output, and nothing that reads Claude credentials. The shell does the re-checks and runs the CLI.

use std::path::{Path, PathBuf};

use chrono::{Datelike, TimeZone, Timelike};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::Error;
use crate::claude_cli::{AgentSession, SessionKind};
use crate::i18n::{Code, Lang, Params};

/// Auto resume ships off until spike 1.1 measures `--bg --resume` (bg id, copy notice, carry-over). While false, every
/// limit stop is shown as "직접 이어야 해요" and nothing runs unattended (design 13).
pub const AUTO_RESUME_SHIPPED: bool = false;
/// The fixed resume prompt (Korean). Never user-editable; a task resumes in its start language (`resume_prompt`).
pub const RESUME_PROMPT: &str = "사용량이 리셋되어 이어서 진행해요. 멈춘 지점부터 계속해 주세요";
pub const RESUME_PROMPT_EN: &str = "The usage limit has reset, so carry on. Continue from where you stopped";

/// The resume prompt in `lang` (app-language design 6: a resume follows the start language).
pub fn resume_prompt(lang: crate::i18n::Lang) -> &'static str {
    match lang {
        crate::i18n::Lang::Ko => RESUME_PROMPT,
        crate::i18n::Lang::En => RESUME_PROMPT_EN,
    }
}
/// Automatic resumes per original task, counted across bg id aliases.
pub const MAX_ATTEMPTS: i64 = 3;
pub const LATE_MS: i64 = 2 * 3600 * 1000;
pub const EXPIRE_MS: i64 = 3 * 24 * 3600 * 1000;
pub const KEEP_MS: i64 = 7 * 24 * 3600 * 1000;
pub const GAP_MS: i64 = 20 * 1000;
pub const MAX_CONCURRENT: i64 = 3;
const JITTER_MIN_MS: i64 = 60 * 1000;
const JITTER_SPAN_MS: i64 = 120 * 1000;
/// A stop whose reset time is unknown is only considered this long after it happened.
const UNKNOWN_WINDOW_MS: i64 = 24 * 3600 * 1000;

/// Why a stop is closed or must be resumed by hand. Stored as these fixed codes only — never stderr or CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Interactive or started outside D.Duck: notify only.
    NotDduck,
    NoReset,
    OptOut,
    Cap,
    Deploy,
    Remote,
    /// auto/bypassPermissions without "리셋 후 자동으로 이어서" at start.
    Danger,
    AutoOff,
    NotShipped,
    UserStopped,
    AlreadyResumed,
    CwdMismatch,
    Untrusted,
    Link,
    RiskChanged,
    CopyStarted,
    CliFailed,
    Attached,
    Interrupted,
    Late,
    Expired,
    Cancelled,
    /// The task record, its session id or its transcript did not match at resume time.
    SessionMismatch,
    /// The delivery scope's hook for a "로컬만" folder could not be prepared.
    GuardHookMissing,
    /// The "로컬만" list or the repository's branches could not be read.
    GuardUnreadable,
    /// A "로컬만" folder that cannot run this resume (dangerous mode or managed hook settings).
    LocalOnlyRefused,
}

const REASONS: [Reason; 26] = [
    Reason::NotDduck,
    Reason::NoReset,
    Reason::OptOut,
    Reason::Cap,
    Reason::Deploy,
    Reason::Remote,
    Reason::Danger,
    Reason::AutoOff,
    Reason::NotShipped,
    Reason::UserStopped,
    Reason::AlreadyResumed,
    Reason::CwdMismatch,
    Reason::Untrusted,
    Reason::Link,
    Reason::RiskChanged,
    Reason::CopyStarted,
    Reason::CliFailed,
    Reason::Attached,
    Reason::Interrupted,
    Reason::Late,
    Reason::Expired,
    Reason::Cancelled,
    Reason::SessionMismatch,
    Reason::GuardHookMissing,
    Reason::GuardUnreadable,
    Reason::LocalOnlyRefused,
];

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::NotDduck => "not_dduck",
            Reason::NoReset => "no_reset",
            Reason::OptOut => "opt_out",
            Reason::Cap => "cap",
            Reason::Deploy => "deploy",
            Reason::Remote => "remote",
            Reason::Danger => "danger",
            Reason::AutoOff => "auto_off",
            Reason::NotShipped => "not_shipped",
            Reason::UserStopped => "user_stopped",
            Reason::AlreadyResumed => "already_resumed",
            Reason::CwdMismatch => "cwd_mismatch",
            Reason::Untrusted => "untrusted",
            Reason::Link => "link",
            Reason::RiskChanged => "risk_changed",
            Reason::CopyStarted => "copy_started",
            Reason::CliFailed => "cli_failed",
            Reason::Attached => "attached",
            Reason::Interrupted => "interrupted",
            Reason::Late => "late",
            Reason::Expired => "expired",
            Reason::Cancelled => "cancelled",
            Reason::SessionMismatch => "session_mismatch",
            Reason::GuardHookMissing => "guard_hook_missing",
            Reason::GuardUnreadable => "guard_unreadable",
            Reason::LocalOnlyRefused => "local_only_refused",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        REASONS.into_iter().find(|r| r.as_str() == s)
    }
}

/// Row states. `auto` waits for its due time, `manual` shows "직접 이어서", `needs_choice` asks after a late wake-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Auto,
    Manual,
    NeedsChoice,
    Starting,
    Resumed,
    Closed,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Auto => "auto",
            State::Manual => "manual",
            State::NeedsChoice => "needs_choice",
            State::Starting => "starting",
            State::Resumed => "resumed",
            State::Closed => "closed",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [State::Auto, State::Manual, State::NeedsChoice, State::Starting, State::Resumed, State::Closed].into_iter().find(|x| x.as_str() == s)
    }
}

/// What D.Duck knows about the task behind a session (from `task_meta`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInfo {
    pub bg_id: String,
    /// The first bg id of this task (aliases share it).
    pub origin_bg_id: String,
    pub permission_mode: Option<String>,
    pub task_type: Option<String>,
    /// `None` for tasks recorded before this change: treated as remote (fail-closed).
    pub remote: Option<bool>,
    pub opt_out: bool,
    pub auto_ok: bool,
}

/// Inputs that decide auto vs manual at scheduling time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Settings "리셋되면 자동으로 이어서 하기".
    pub auto_on: bool,
    /// [`AUTO_RESUME_SHIPPED`], injectable for tests.
    pub shipped: bool,
}

fn dangerous(mode: Option<&str>) -> bool {
    matches!(mode, Some("auto" | "bypassPermissions"))
}

/// Whether this task may resume by itself; `Err` says why it needs the user. Order: the most specific reason first.
pub fn eligibility(task: Option<&TaskInfo>, attempts: i64, has_reset: bool, policy: Policy) -> Result<(), Reason> {
    let Some(t) = task else { return Err(Reason::NotDduck) };
    if !has_reset {
        return Err(Reason::NoReset);
    }
    if t.task_type.as_deref() == Some("deploy") {
        return Err(Reason::Deploy);
    }
    if t.remote != Some(false) {
        return Err(Reason::Remote);
    }
    if dangerous(t.permission_mode.as_deref()) && !t.auto_ok {
        return Err(Reason::Danger);
    }
    if t.opt_out {
        return Err(Reason::OptOut);
    }
    if attempts >= MAX_ATTEMPTS {
        return Err(Reason::Cap);
    }
    if !policy.shipped {
        return Err(Reason::NotShipped);
    }
    if !policy.auto_on {
        return Err(Reason::AutoOff);
    }
    Ok(())
}

/// 60–180 s after the reset, spread by session so tasks do not all start in the same second.
pub fn due_at(session_id: &str, resets_at: i64) -> i64 {
    let h = Sha256::digest(format!("{session_id}:{resets_at}").as_bytes());
    let n = i64::from(u16::from_be_bytes([h[0], h[1]]));
    resets_at + JITTER_MIN_MS + n * JITTER_SPAN_MS / i64::from(u16::MAX)
}

fn task_by_session(conn: &Connection, session_id: &str, sessions: &[AgentSession]) -> Result<Option<TaskInfo>, Error> {
    let listed = sessions.iter().find(|s| s.session_id.as_deref() == Some(session_id) && s.kind == SessionKind::Background).and_then(|s| s.id.clone());
    let row = |sql: &str, key: &str| {
        conn.query_row(sql, [key], |r| {
            let bg: String = r.get(0)?;
            Ok(TaskInfo {
                origin_bg_id: r.get::<_, Option<String>>(1)?.unwrap_or_else(|| bg.clone()),
                bg_id: bg,
                permission_mode: r.get(2)?,
                task_type: r.get(3)?,
                remote: r.get::<_, Option<i64>>(4)?.map(|v| v != 0),
                opt_out: r.get::<_, i64>(5)? != 0,
                auto_ok: r.get::<_, i64>(6)? != 0,
            })
        })
        .optional()
    };
    const COLS: &str = "SELECT bg_id, origin_bg_id, permission_mode, task_type, remote, limit_opt_out, limit_auto_ok FROM task_meta";
    if let Some(bg) = listed {
        if let Some(t) = row(&format!("{COLS} WHERE bg_id = ?1"), &bg)? {
            return Ok(Some(t));
        }
    }
    Ok(row(&format!("{COLS} WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1"), session_id)?)
}

/// Automatic resumes so far for the original task (aliases included).
pub fn attempts(conn: &Connection, origin_bg_id: &str) -> Result<i64, Error> {
    Ok(conn.query_row("SELECT COUNT(*) FROM limit_resumes WHERE origin_bg_id = ?1 AND state = 'resumed'", [origin_bg_id], |r| r.get(0))?)
}

/// One OS notification: every new stop with the same reset time, counted by who resumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub resets_at: i64,
    pub auto: usize,
    pub manual: usize,
}

/// Groups new limit records into rows, closes rows the user already continued, and returns the notices to show.
/// Before the first full ingest pass finished (`backfilled == false`) rows are created as already announced.
pub fn sync(conn: &mut Connection, now: i64, sessions: &[AgentSession], policy: Policy, backfilled: bool) -> Result<Vec<Notice>, Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let incidents: Vec<(String, Option<i64>, i64, bool)> = {
        let mut stmt = tx.prepare(
            "SELECT session_id, resets_at, MIN(ts), MAX(seen) FROM limit_hits
             WHERE (resets_at IS NOT NULL AND resets_at > ?1) OR (resets_at IS NULL AND ts > ?2)
             GROUP BY session_id, resets_at",
        )?;
        let rows = stmt.query_map(params![now - EXPIRE_MS, now - UNKNOWN_WINDOW_MS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (session_id, resets_at, hit_ts, seen) in incidents {
        let incident = resets_at.map_or_else(|| "unknown".to_string(), |r| r.to_string());
        let exists: bool = tx.query_row("SELECT COUNT(*) FROM limit_resumes WHERE session_id = ?1 AND incident = ?2", params![session_id, incident], |r| r.get::<_, i64>(0))? > 0;
        if exists {
            continue;
        }
        let task = task_by_session(&tx, &session_id, sessions)?;
        let tries = match &task {
            Some(t) => attempts(&tx, &t.origin_bg_id)?,
            None => 0,
        };
        let (state, reason, due) = match eligibility(task.as_ref(), tries, resets_at.is_some(), policy) {
            Ok(()) => (State::Auto, None, resets_at.map(|r| due_at(&session_id, r))),
            Err(r) => (State::Manual, Some(r), None),
        };
        let notified = !backfilled || seen;
        tx.execute(
            "INSERT INTO limit_resumes(session_id, incident, resets_at, hit_ts, origin_bg_id, bg_id, permission_mode, task_type, due_at, state, reason, notified, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
            params![
                session_id,
                incident,
                resets_at,
                hit_ts,
                task.as_ref().map(|t| t.origin_bg_id.as_str()),
                task.as_ref().map(|t| t.bg_id.as_str()),
                task.as_ref().and_then(|t| t.permission_mode.as_deref()),
                task.as_ref().and_then(|t| t.task_type.as_deref()),
                due,
                state.as_str(),
                reason.map(Reason::as_str),
                notified,
                now
            ],
        )?;
    }
    // Continued since the stop (a real reply after it): nothing left to resume.
    tx.execute(
        "UPDATE limit_resumes SET state = 'closed', reason = 'already_resumed', updated_at = ?1
         WHERE state IN ('auto', 'manual', 'needs_choice')
           AND EXISTS (SELECT 1 FROM api_calls a WHERE a.session_id = limit_resumes.session_id AND a.ts > limit_resumes.hit_ts)",
        [now],
    )?;
    let notices = {
        let mut stmt = tx.prepare(
            "SELECT resets_at, SUM(state = 'auto'), SUM(state != 'auto') FROM limit_resumes
             WHERE notified = 0 AND resets_at > ?1 AND state IN ('auto', 'manual') GROUP BY resets_at ORDER BY resets_at",
        )?;
        let rows = stmt.query_map([now], |r| Ok(Notice { resets_at: r.get(0)?, auto: r.get::<_, i64>(1)? as usize, manual: r.get::<_, i64>(2)? as usize }))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    tx.execute("UPDATE limit_resumes SET notified = 1 WHERE notified = 0", [])?;
    tx.commit()?;
    Ok(notices)
}

/// Called once the first full ingest pass is done: every limit record found so far counts as already announced.
pub fn finish_backfill(conn: &Connection) -> Result<(), Error> {
    conn.execute("UPDATE limit_hits SET seen = 1 WHERE seen = 0", [])?;
    crate::settings::set_limit_backfilled(conn)
}

/// A resume the scheduler took. The shell re-checks it and reports back with [`finish`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub id: i64,
    pub session_id: String,
    pub bg_id: String,
    pub origin_bg_id: String,
}

/// [`tick_with`] with no session known to be working (tests, or when the live list is unavailable).
pub fn tick(conn: &mut Connection, now: i64, policy: Policy) -> Result<Option<Claim>, Error> {
    tick_with(conn, now, policy, &[])
}

/// One scheduler tick: late rows ask the user, old rows expire, and at most one due row is claimed (`starting`
/// before any CLI call, [`GAP_MS`] between starts). At most [`MAX_CONCURRENT`] resumed tasks run at once: rows being
/// started plus resumed rows whose session is in `working` (session ids the live list shows as working).
pub fn tick_with(conn: &mut Connection, now: i64, policy: Policy, working: &[String]) -> Result<Option<Claim>, Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !(policy.auto_on && policy.shipped) {
        let r = if policy.shipped { Reason::AutoOff } else { Reason::NotShipped };
        tx.execute("UPDATE limit_resumes SET state = 'manual', reason = ?1, updated_at = ?2 WHERE state = 'auto'", params![r.as_str(), now])?;
    }
    tx.execute("UPDATE limit_resumes SET state = 'needs_choice', reason = 'late', updated_at = ?1 WHERE state = 'auto' AND resets_at + ?2 < ?1", params![now, LATE_MS])?;
    tx.execute(
        "UPDATE limit_resumes SET state = 'closed', reason = 'expired', updated_at = ?1
         WHERE state IN ('auto', 'manual', 'needs_choice') AND COALESCE(resets_at, hit_ts) + ?2 < ?1",
        params![now, EXPIRE_MS],
    )?;
    tx.execute("DELETE FROM limit_resumes WHERE state IN ('closed', 'resumed') AND updated_at < ?1", [now - KEEP_MS])?;
    tx.execute("DELETE FROM limit_hits WHERE ts < ?1", [now - KEEP_MS])?;
    let starting: i64 = tx.query_row("SELECT COUNT(*) FROM limit_resumes WHERE state = 'starting'", [], |r| r.get(0))?;
    let resumed_running = {
        let mut stmt = tx.prepare("SELECT DISTINCT session_id FROM limit_resumes WHERE state = 'resumed'")?;
        let ids = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        ids.iter().filter(|id| working.contains(id)).count() as i64
    };
    let running = starting + resumed_running;
    let last: Option<i64> = tx.query_row("SELECT MAX(updated_at) FROM limit_resumes WHERE state IN ('starting', 'resumed')", [], |r| r.get(0))?;
    let mut claim = None;
    if running < MAX_CONCURRENT && last.is_none_or(|t| now - t >= GAP_MS) {
        claim = tx
            .query_row(
                "SELECT id, session_id, bg_id, origin_bg_id FROM limit_resumes
                 WHERE state = 'auto' AND due_at <= ?1 AND bg_id IS NOT NULL AND origin_bg_id IS NOT NULL ORDER BY due_at LIMIT 1",
                [now],
                |r| Ok(Claim { id: r.get(0)?, session_id: r.get(1)?, bg_id: r.get(2)?, origin_bg_id: r.get(3)? }),
            )
            .optional()?;
        if let Some(c) = &claim {
            tx.execute("UPDATE limit_resumes SET state = 'starting', updated_at = ?2 WHERE id = ?1 AND state = 'auto'", params![c.id, now])?;
        }
    }
    tx.commit()?;
    Ok(claim)
}

/// Marks a `starting` row with how it ended. A failed or skipped resume is not retried: the card offers "직접 이어서".
pub fn finish(conn: &Connection, id: i64, outcome: Result<(), Reason>, now: i64) -> Result<(), Error> {
    let (state, reason) = match outcome {
        Ok(()) => (State::Resumed, None),
        Err(r @ (Reason::AlreadyResumed | Reason::UserStopped | Reason::Cancelled)) => (State::Closed, Some(r)),
        Err(r) => (State::Manual, Some(r)),
    };
    conn.execute(
        "UPDATE limit_resumes SET state = ?2, reason = ?3, updated_at = ?4 WHERE id = ?1 AND state IN ('starting', 'auto', 'manual', 'needs_choice')",
        params![id, state.as_str(), reason.map(Reason::as_str), now],
    )?;
    Ok(())
}

/// Gives back a row claimed for "지금 이어서" that did not run (dialog cancelled or busy): it stays on the card.
pub fn unclaim(conn: &Connection, id: i64, now: i64) -> Result<(), Error> {
    conn.execute("UPDATE limit_resumes SET state = 'manual', updated_at = ?2 WHERE id = ?1 AND state = 'starting'", params![id, now])?;
    Ok(())
}

/// At app start: a resume that was running when the app quit is never run again (no double runs).
pub fn close_interrupted(conn: &Connection, now: i64) -> Result<usize, Error> {
    Ok(conn.execute("UPDATE limit_resumes SET state = 'manual', reason = 'interrupted', updated_at = ?1 WHERE state = 'starting'", [now])?)
}

/// The user stopped the task in D.Duck: its waiting resumes are cancelled.
pub fn user_stopped(conn: &Connection, bg_id: &str, now: i64) -> Result<usize, Error> {
    Ok(conn.execute(
        "UPDATE limit_resumes SET state = 'closed', reason = 'user_stopped', updated_at = ?2
         WHERE state IN ('auto', 'manual', 'needs_choice') AND (bg_id = ?1 OR origin_bg_id = ?1)",
        params![bg_id, now],
    )?)
}

/// "이번엔 안 이어요" / "그만두기". Unknown ids → false.
pub fn cancel(conn: &Connection, id: i64, now: i64) -> Result<bool, Error> {
    Ok(conn.execute(
        "UPDATE limit_resumes SET state = 'closed', reason = 'cancelled', updated_at = ?2 WHERE id = ?1 AND state IN ('auto', 'manual', 'needs_choice')",
        params![id, now],
    )? > 0)
}

/// Takes an open row for "지금 이어서" (the shell re-checks and confirms natively). `None` if it is not open.
pub fn claim_manual(conn: &mut Connection, id: i64, now: i64) -> Result<Option<Claim>, Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let c = tx
        .query_row(
            "SELECT id, session_id, bg_id, origin_bg_id FROM limit_resumes WHERE id = ?1 AND state IN ('auto', 'manual', 'needs_choice') AND bg_id IS NOT NULL AND origin_bg_id IS NOT NULL",
            [id],
            |r| Ok(Claim { id: r.get(0)?, session_id: r.get(1)?, bg_id: r.get(2)?, origin_bg_id: r.get(3)? }),
        )
        .optional()?;
    if c.is_some() {
        tx.execute("UPDATE limit_resumes SET state = 'starting', updated_at = ?2 WHERE id = ?1", params![id, now])?;
    }
    tx.commit()?;
    Ok(c)
}

/// Per-task "리셋되면 자동으로 이어서 하기" off/on. Waiting auto rows of that task become manual. Unknown ids → false.
pub fn set_opt_out(conn: &Connection, bg_id: &str, off: bool, now: i64) -> Result<bool, Error> {
    let n = conn.execute("UPDATE task_meta SET limit_opt_out = ?2 WHERE bg_id = ?1", params![bg_id, off])?;
    if off {
        conn.execute(
            "UPDATE limit_resumes SET state = 'manual', reason = 'opt_out', updated_at = ?2 WHERE state = 'auto' AND (bg_id = ?1 OR origin_bg_id = ?1)",
            params![bg_id, now],
        )?;
    }
    Ok(n > 0)
}

/// One open stop for the banner and cards. Ids, times and codes only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub session_id: String,
    pub bg_id: Option<String>,
    pub resets_at: Option<i64>,
    pub due_at: Option<i64>,
    pub state: State,
    pub reason: Option<Reason>,
    /// Automatic resumes already used by this task (for "자동으로 3번 이어서 멈췄어요").
    pub attempts: i64,
    /// The task's name, else the first line of its goal, else its folder's base name (display-safe). Never an id.
    pub name: Option<String>,
    /// When the task was started (UTC ms), to tell same-named tasks apart.
    pub started_at: Option<i64>,
}

/// Open and recently finished stops, newest reset first.
pub fn status(conn: &Connection, now: i64) -> Result<Vec<Entry>, Error> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.session_id, r.bg_id, r.resets_at, r.due_at, r.state, r.reason,
                (SELECT COUNT(*) FROM limit_resumes x WHERE x.origin_bg_id = r.origin_bg_id AND x.state = 'resumed')
         FROM limit_resumes r
         WHERE r.state IN ('auto', 'manual', 'needs_choice', 'starting') OR (r.state = 'resumed' AND r.updated_at > ?1)
         ORDER BY COALESCE(r.resets_at, r.hit_ts) DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([now - 10 * 60 * 1000], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get(3)?, r.get(4)?, r.get::<_, String>(5)?, r.get::<_, Option<String>>(6)?, r.get(7)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, session_id, bg_id, resets_at, due_at, state, reason, attempts) = row?;
        let Some(state) = State::parse(&state) else { continue };
        let (name, started_at) = match bg_id.as_deref().map(|b| task_name(conn, b)).transpose()?.flatten() {
            Some((n, t)) => (n, Some(t)),
            None => (None, None),
        };
        out.push(Entry { id, session_id, bg_id, resets_at, due_at, state, reason: reason.as_deref().and_then(Reason::parse), attempts, name, started_at });
    }
    Ok(out)
}

/// The name the user gave the task, else the first line of the goal they wrote (both D.Duck form input).
/// Falls back to the folder's base name. Returns the task's start time too (to tell same-named tasks apart).
fn task_name(conn: &Connection, bg_id: &str) -> Result<Option<(Option<String>, i64)>, Error> {
    // name, brief_json, cwd, created_at
    type MetaRow = (Option<String>, Option<String>, Option<String>, i64);
    let row: Option<MetaRow> = conn
        .query_row("SELECT name, brief_json, cwd, created_at FROM task_meta WHERE bg_id = ?1", [bg_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;
    let Some((name, brief, cwd, started)) = row else { return Ok(None) };
    Ok(Some((pick_name(name, brief, cwd), started)))
}

fn pick_name(name: Option<String>, brief: Option<String>, cwd: Option<String>) -> Option<String> {
    let clean = |s: &str| Some(crate::text::display_safe(s.lines().next().unwrap_or("").trim(), 40)).filter(|t| !t.trim().is_empty());
    if let Some(n) = name.as_deref().and_then(clean) {
        return Some(n);
    }
    let goal = brief.and_then(|j| serde_json::from_str::<crate::team::Brief>(&j).ok()).map(|b| b.goal);
    if let Some(g) = goal.as_deref().and_then(clean) {
        return Some(g);
    }
    let folder = cwd.as_deref().and_then(|c| Path::new(c).file_name()).and_then(|f| f.to_str()).map(str::to_string);
    folder.as_deref().and_then(clean)
}

/// What the tasks screen shows: one banner line for the coming reset, cards' entries with a local time label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub entries: Vec<EntryView>,
    pub banner: Option<Banner>,
    /// Rows waiting for "지금 이어서 / 그만두기" after a late wake-up.
    pub late: usize,
    /// Open stops whose reset time is unknown.
    pub unknown: usize,
    pub auto_shipped: bool,
    pub auto_on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    #[serde(flatten)]
    pub entry: Entry,
    /// Local reset time ("오후 1:50", or with date and weekday beyond 24 hours).
    pub label: Option<String>,
    pub due_label: Option<String>,
    /// Local start time ("오후 3:12"), appended when two buttons would read the same.
    pub started_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Banner {
    pub resets_at: i64,
    pub label: String,
    pub auto: usize,
    pub manual: usize,
    pub text: String,
}

/// `label`, `text` and the entries' labels are fallback text in `lang`; the webview words the banner and times from
/// `resetsAt`, `auto`, `manual` and the entries' ms fields (app-language design 3).
pub fn view<Tz: TimeZone>(conn: &Connection, now: i64, tz: &Tz, policy: Policy, lang: Lang) -> Result<View, Error> {
    let entries = status(conn, now)?;
    let upcoming: Vec<&Entry> = entries.iter().filter(|e| matches!(e.state, State::Auto | State::Manual) && e.resets_at.is_some_and(|r| r > now)).collect();
    let banner = upcoming.iter().filter_map(|e| e.resets_at).min().map(|first| {
        let auto = upcoming.iter().filter(|e| e.state == State::Auto).count();
        let manual = upcoming.len() - auto;
        let label = reset_label(first, now, tz, lang);
        let counts = counts_text(auto, manual, lang);
        let text = crate::i18n::msg(lang, Code::LimitBanner, &Params::new().text("time", &label, 40).text("counts", &counts, 200));
        Banner { resets_at: first, text, label, auto, manual }
    });
    let late = entries.iter().filter(|e| e.state == State::NeedsChoice).count();
    let unknown = entries.iter().filter(|e| e.resets_at.is_none() && e.state == State::Manual).count();
    let entries = entries
        .into_iter()
        .map(|e| EntryView {
            label: e.resets_at.map(|r| reset_label(r, now, tz, lang)),
            due_label: e.due_at.map(|d| reset_label(d, now, tz, lang)),
            started_label: e.started_at.map(|t| reset_label(t, t, tz, lang)),
            entry: e,
        })
        .collect();
    Ok(View { entries, banner, late, unknown, auto_shipped: policy.shipped, auto_on: policy.auto_on })
}

// ---------- text ----------

/// "오후 1:50", or with the date and weekday when the reset is more than 24 hours away ("11월 12일(목) 오전 9:00").
pub fn reset_label<Tz: TimeZone>(resets_at: i64, now: i64, tz: &Tz, lang: Lang) -> String {
    let Some(t) = tz.timestamp_millis_opt(resets_at).earliest() else { return crate::i18n::text(lang, Code::TimeUnknown).into() };
    let (pm, h12) = t.hour12();
    let date = (resets_at - now > 24 * 3600 * 1000).then(|| (t.month(), t.day(), t.weekday().num_days_from_monday() as usize));
    crate::i18n::clock_label(lang, pm, h12, t.minute(), date)
}

/// "N개는 이어서 할게요 · M개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요", leaving out the empty side.
pub fn counts_text(auto: usize, manual: usize, lang: Lang) -> String {
    let n = |code, count: usize| crate::i18n::msg(lang, code, &Params::new().num("count", count as i64));
    let mut parts = Vec::new();
    if auto > 0 {
        parts.push(n(Code::LimitAuto, auto));
    }
    if manual > 0 {
        parts.push(n(Code::LimitManual, manual));
    }
    if auto > 0 {
        parts.push(crate::i18n::text(lang, Code::LimitAwake).to_string());
    }
    parts.join(" · ")
}

/// The OS notification body for one reset time. Fixed phrases, a time and counts only.
pub fn notice_body<Tz: TimeZone>(n: &Notice, now: i64, tz: &Tz, lang: Lang) -> String {
    let counts = counts_text(n.auto, n.manual, lang);
    let head = crate::i18n::msg(lang, Code::LimitNotice, &Params::new().text("time", &reset_label(n.resets_at, now, tz, lang), 40));
    if counts.is_empty() { head } else { format!("{head} · {counts}") }
}

/// The notification when resumes start.
pub fn started_body(n: usize, lang: Lang) -> String {
    crate::i18n::msg(lang, Code::LimitStarted, &Params::new().num("count", n as i64))
}

/// The start-time risk checks repeated right before a resume (design 5): an agent turned off, or a persona whose
/// definition can run commands, sends the task to "직접 이어서" (refused outright for strict and sensitive tasks, as
/// at start). `manual` is a resume the user confirmed natively.
pub fn recheck(agent_denied: bool, persona_risky: bool, strict: bool, sensitive: bool, manual: bool) -> Result<(), Reason> {
    if agent_denied || (persona_risky && (strict || sensitive || !manual)) {
        return Err(Reason::RiskChanged);
    }
    Ok(())
}

// ---------- folder check ----------

/// Checks the folder a resume would run in (design 6). `session_cwd` is the session's real cwd (from `claude agents`
/// or the transcript), `start_cwd` the folder recorded at start. Returns the canonical path to run in.
pub fn verify_cwd(start_cwd: &Path, worktree: bool, session_cwd: &Path, trusted: impl Fn(&Path) -> Option<bool>) -> Result<PathBuf, Reason> {
    if !session_cwd.is_absolute() || has_link(session_cwd) {
        return Err(Reason::Link);
    }
    let cwd = std::fs::canonicalize(session_cwd).map_err(|_| Reason::CwdMismatch)?;
    let start = std::fs::canonicalize(start_cwd).map_err(|_| Reason::CwdMismatch)?;
    let ok = if worktree {
        cwd.parent().is_some_and(|p| p == start.join(".claude").join("worktrees"))
    } else {
        cwd == start
    };
    if !ok || !cwd.is_dir() {
        return Err(Reason::CwdMismatch);
    }
    if trusted(&cwd) != Some(true) {
        return Err(Reason::Untrusted);
    }
    Ok(cwd)
}

/// Any symlink on the path (each ancestor checked with `symlink_metadata`).
fn has_link(p: &Path) -> bool {
    p.ancestors().any(|a| a.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::i18n::Lang::{En, Ko};
    use crate::claude_cli::SessionStatus;

    const H: i64 = 3600 * 1000;
    const T0: i64 = 1_794_100_000_000;
    const ON: Policy = Policy { auto_on: true, shipped: true };

    fn hit(conn: &Connection, uuid: &str, session: &str, ts: i64, resets_at: Option<i64>) {
        conn.execute(
            "INSERT OR IGNORE INTO limit_hits(record_uuid, session_id, is_subagent, ts, resets_at, kind) VALUES (?1, ?2, 0, ?3, ?4, 'five_hour')",
            params![uuid, session, ts, resets_at],
        )
        .unwrap();
    }

    fn task(conn: &Connection, bg: &str, session: &str, mode: &str) {
        conn.execute("INSERT INTO task_meta(bg_id, created_at, session_id, permission_mode, remote, cwd) VALUES (?1, 1, ?2, ?3, 0, '/w')", params![bg, session, mode]).unwrap();
    }

    fn rows(conn: &Connection) -> Vec<(String, String, Option<String>)> {
        let mut s = conn.prepare("SELECT session_id, state, reason FROM limit_resumes ORDER BY session_id").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    }

    fn db() -> Connection {
        crate::db::open_in_memory().unwrap()
    }

    fn bg_session(id: &str, session: &str) -> AgentSession {
        AgentSession { key: session.into(), pid: Some(1), id: Some(id.into()), session_id: Some(session.into()), name: "job".into(), cwd: "/w".into(), kind: SessionKind::Background, started_at: 0, status: SessionStatus::Idle }
    }

    #[test]
    fn eligibility_rules() {
        let t = TaskInfo { bg_id: "b".into(), origin_bg_id: "b".into(), permission_mode: Some("acceptEdits".into()), task_type: None, remote: Some(false), opt_out: false, auto_ok: false };
        assert_eq!(eligibility(Some(&t), 0, true, ON), Ok(()));
        assert_eq!(eligibility(None, 0, true, ON), Err(Reason::NotDduck));
        assert_eq!(eligibility(Some(&t), 0, false, ON), Err(Reason::NoReset));
        assert_eq!(eligibility(Some(&TaskInfo { task_type: Some("deploy".into()), auto_ok: true, ..t.clone() }), 0, true, ON), Err(Reason::Deploy));
        assert_eq!(eligibility(Some(&TaskInfo { remote: None, ..t.clone() }), 0, true, ON), Err(Reason::Remote), "unknown remote is fail-closed");
        assert_eq!(eligibility(Some(&TaskInfo { remote: Some(true), ..t.clone() }), 0, true, ON), Err(Reason::Remote));
        for m in ["auto", "bypassPermissions"] {
            assert_eq!(eligibility(Some(&TaskInfo { permission_mode: Some(m.into()), ..t.clone() }), 0, true, ON), Err(Reason::Danger));
            assert_eq!(eligibility(Some(&TaskInfo { permission_mode: Some(m.into()), auto_ok: true, ..t.clone() }), 0, true, ON), Ok(()), "ticked at start");
        }
        assert_eq!(eligibility(Some(&TaskInfo { opt_out: true, ..t.clone() }), 0, true, ON), Err(Reason::OptOut));
        assert_eq!(eligibility(Some(&t), 3, true, ON), Err(Reason::Cap));
        assert_eq!(eligibility(Some(&t), 0, true, Policy { auto_on: true, shipped: false }), Err(Reason::NotShipped));
        assert_eq!(eligibility(Some(&t), 0, true, Policy { auto_on: false, shipped: true }), Err(Reason::AutoOff));
    }

    #[test]
    fn reasons_are_fixed_codes() {
        for r in REASONS {
            assert_eq!(Reason::parse(r.as_str()), Some(r));
        }
        assert_eq!(Reason::parse("Error: stderr tail"), None);
        let conn = db();
        conn.execute("INSERT INTO limit_resumes(session_id, incident, hit_ts, state, reason, created_at, updated_at) VALUES ('s', 'x', 1, 'manual', 'free text', 1, 1)", []).unwrap();
        assert_eq!(status(&conn, 1).unwrap()[0].reason, None, "unknown stored codes are not shown");
    }

    #[test]
    fn due_time_is_jittered_between_60_and_180_seconds() {
        for s in ["a", "b", "c", "d", "e"] {
            let d = due_at(s, T0) - T0;
            assert!((60_000..=180_000).contains(&d), "{d}");
        }
        assert_ne!(due_at("a", T0), due_at("b", T0));
    }

    #[test]
    fn same_reset_across_sessions_notifies_once_with_counts() {
        let mut conn = db();
        crate::settings::set_limit_backfilled(&conn).unwrap();
        task(&conn, "b1", "s1", "acceptEdits");
        task(&conn, "b2", "s2", "manual");
        task(&conn, "b3", "s3", "bypassPermissions");
        let reset = T0 + 3 * H;
        for (i, s) in ["s1", "s2", "s3"].iter().enumerate() {
            hit(&conn, &format!("u{i}"), s, T0, Some(reset));
        }
        // Main and sub-agent records of the same stop are one incident.
        hit(&conn, "u-sub", "s1", T0 + 1000, Some(reset));
        let n = sync(&mut conn, T0 + 5000, &[], ON, true).unwrap();
        assert_eq!(n, vec![Notice { resets_at: reset, auto: 2, manual: 1 }]);
        assert_eq!(rows(&conn).len(), 3);
        assert_eq!(rows(&conn)[2], ("s3".into(), "manual".into(), Some("danger".into())));
        // Re-reading the same records: no new rows, no repeat notice.
        hit(&conn, "u0", "s1", T0, Some(reset));
        assert!(sync(&mut conn, T0 + 9000, &[], ON, true).unwrap().is_empty());
        assert_eq!(rows(&conn).len(), 3);
    }

    #[test]
    fn notices_need_a_future_reset_and_backfill_is_silent() {
        let mut conn = db();
        hit(&conn, "old", "s1", T0, Some(T0 + H));
        // First ingest not finished: rows are made as already announced.
        assert!(sync(&mut conn, T0 + 1000, &[], ON, false).unwrap().is_empty());
        finish_backfill(&conn).unwrap();
        hit(&conn, "old2", "s2", T0, Some(T0 + H));
        assert_eq!(sync(&mut conn, T0 + 2000, &[], ON, true).unwrap().len(), 1, "new stop after backfill");
        // A stop whose reset already passed is not announced.
        hit(&conn, "past", "s3", T0 - 4 * H, Some(T0 - H));
        assert!(sync(&mut conn, T0 + 3000, &[], ON, true).unwrap().is_empty());
        // Records seen during the backfill stay quiet even if their row is made later.
        let mut c2 = db();
        hit(&c2, "x", "s9", T0, Some(T0 + H));
        finish_backfill(&c2).unwrap();
        assert!(sync(&mut c2, T0 + 1, &[], ON, true).unwrap().is_empty());
    }

    #[test]
    fn interactive_sessions_and_unknown_resets_are_manual() {
        let mut conn = db();
        hit(&conn, "u1", "term", T0, Some(T0 + H));
        task(&conn, "b2", "s2", "acceptEdits");
        hit(&conn, "u2", "s2", T0, None);
        let n = sync(&mut conn, T0 + 1, &[], ON, true).unwrap();
        assert_eq!(n, vec![Notice { resets_at: T0 + H, auto: 0, manual: 1 }], "unknown resets are not announced");
        let r = rows(&conn);
        assert_eq!(r[0], ("s2".into(), "manual".into(), Some("no_reset".into())));
        assert_eq!(r[1], ("term".into(), "manual".into(), Some("not_dduck".into())));
    }

    #[test]
    fn flag_off_or_setting_off_makes_everything_manual() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        hit(&conn, "u1", "s1", T0, Some(T0 + H));
        let n = sync(&mut conn, T0, &[], Policy { auto_on: true, shipped: false }, true).unwrap();
        assert_eq!(n, vec![Notice { resets_at: T0 + H, auto: 0, manual: 1 }]);
        assert_eq!(rows(&conn)[0].2.as_deref(), Some("not_shipped"));
        assert!(tick(&mut conn, T0 + 2 * H, Policy { auto_on: true, shipped: false }).unwrap().is_none(), "nothing runs");
        // Turning the setting off moves waiting auto rows to manual.
        let mut c2 = db();
        task(&c2, "b1", "s1", "acceptEdits");
        hit(&c2, "u1", "s1", T0, Some(T0 + H));
        sync(&mut c2, T0, &[], ON, true).unwrap();
        assert_eq!(rows(&c2)[0].1, "auto");
        assert!(tick(&mut c2, T0 + 1, Policy { auto_on: false, shipped: true }).unwrap().is_none());
        assert_eq!(rows(&c2)[0], ("s1".into(), "manual".into(), Some("auto_off".into())));
    }

    #[test]
    fn listed_bg_session_finds_its_task_without_session_column() {
        let mut conn = db();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, permission_mode, remote) VALUES ('ab12', 1, 'plan', 0)", []).unwrap();
        hit(&conn, "u1", "s1", T0, Some(T0 + H));
        sync(&mut conn, T0, &[bg_session("ab12", "s1")], ON, true).unwrap();
        assert_eq!(rows(&conn)[0].1, "auto");
    }

    #[test]
    fn scheduler_claims_after_due_with_gap_and_limit() {
        let mut conn = db();
        let reset = T0 + H;
        for i in 0..5 {
            task(&conn, &format!("b{i}"), &format!("s{i}"), "acceptEdits");
            hit(&conn, &format!("u{i}"), &format!("s{i}"), T0, Some(reset));
        }
        sync(&mut conn, T0, &[], ON, true).unwrap();
        assert!(tick(&mut conn, reset + 30_000, ON).unwrap().is_none(), "before the 60 s jitter");
        let first = tick(&mut conn, reset + 181_000, ON).unwrap().expect("due");
        assert!(tick(&mut conn, reset + 190_000, ON).unwrap().is_none(), "20 s gap");
        let second = tick(&mut conn, reset + 202_000, ON).unwrap().expect("after the gap");
        assert_ne!(first.id, second.id);
        tick(&mut conn, reset + 223_000, ON).unwrap().expect("third");
        assert!(tick(&mut conn, reset + 300_000, ON).unwrap().is_none(), "three running");
        finish(&conn, first.id, Ok(()), reset + 301_000).unwrap();
        assert!(tick(&mut conn, reset + 330_000, ON).unwrap().is_some(), "a slot freed");
        // The claimed row is never claimed twice.
        assert!(claim_manual(&mut conn, first.id, reset + 331_000).unwrap().is_none());
    }

    #[test]
    fn logged_out_keeps_auto_rows_until_login() {
        use crate::auth_status::{judge, should_tick};
        let mut conn = db();
        let reset = T0 + H;
        task(&conn, "b1", "s1", "acceptEdits");
        hit(&conn, "u1", "s1", T0, Some(reset));
        sync(&mut conn, T0, &[], ON, true).unwrap();
        let count = |c: &Connection| -> i64 { c.query_row("SELECT COUNT(*) FROM limit_resumes WHERE state IN ('starting', 'resumed')", [], |r| r.get(0)).unwrap() };
        let before = rows(&conn);
        assert_eq!(before[0].1, "auto");
        // The shell's loop: `should_tick` first, `tick` only when it allows.
        let out = judge(Some(1), br#"{"loggedIn":false,"authMethod":"none"}"#, 0);
        for step in 0..10 {
            if should_tick(&out) {
                tick(&mut conn, reset + 181_000 + step * 20_000, ON).unwrap();
            }
        }
        assert_eq!(rows(&conn), before, "still auto, no reason, no starting/manual");
        assert_eq!(count(&conn), 0, "nothing counted toward the cap");
        let back = judge(Some(0), br#"{"loggedIn":true,"authMethod":"claude.ai"}"#, 0);
        assert!(should_tick(&back));
        assert!(tick(&mut conn, reset + 400_000, ON).unwrap().is_some(), "first tick after login claims it");
    }

    #[test]
    fn late_wake_up_asks_and_old_rows_expire() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        hit(&conn, "u1", "s1", T0, Some(T0 + H));
        sync(&mut conn, T0, &[], ON, true).unwrap();
        // 30 minutes late: still runs.
        assert!(tick(&mut conn, T0 + H + 30 * 60_000, ON).unwrap().is_some());
        let mut c2 = db();
        task(&c2, "b1", "s1", "acceptEdits");
        hit(&c2, "u1", "s1", T0, Some(T0 + H));
        sync(&mut c2, T0, &[], ON, true).unwrap();
        // Overnight: asks instead.
        assert!(tick(&mut c2, T0 + 9 * H, ON).unwrap().is_none());
        assert_eq!(rows(&c2)[0], ("s1".into(), "needs_choice".into(), Some("late".into())));
        tick(&mut c2, T0 + H + EXPIRE_MS + 1, ON).unwrap();
        assert_eq!(rows(&c2)[0].2.as_deref(), Some("expired"));
        tick(&mut c2, T0 + H + EXPIRE_MS + KEEP_MS + 2, ON).unwrap();
        assert!(rows(&c2).is_empty(), "closed rows are cleaned after 7 days");
    }

    #[test]
    fn stopped_already_continued_and_interrupted() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        task(&conn, "b2", "s2", "acceptEdits");
        task(&conn, "b3", "s3", "acceptEdits");
        for s in ["s1", "s2", "s3"] {
            hit(&conn, &format!("u-{s}"), s, T0, Some(T0 + H));
        }
        sync(&mut conn, T0, &[], ON, true).unwrap();
        assert_eq!(user_stopped(&conn, "b1", T0 + 1).unwrap(), 1);
        conn.execute("INSERT INTO api_calls(message_id, request_id, session_id, ts, model) VALUES ('m', 'r', 's2', ?1, 'claude-x')", [T0 + 100]).unwrap();
        sync(&mut conn, T0 + 200, &[], ON, true).unwrap();
        let c = tick(&mut conn, T0 + H + 200_000, ON).unwrap().expect("s3 is due");
        assert_eq!(c.session_id, "s3");
        assert_eq!(close_interrupted(&conn, T0 + 2 * H).unwrap(), 1, "app quit mid-start");
        let r = rows(&conn);
        assert_eq!(r[0].2.as_deref(), Some("user_stopped"));
        assert_eq!(r[1].2.as_deref(), Some("already_resumed"));
        assert_eq!(r[2], ("s3".into(), "manual".into(), Some("interrupted".into())));
    }

    #[test]
    fn cap_counts_across_aliases_and_same_reset_waits() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        let mut reset = T0 + H;
        for i in 0..3 {
            hit(&conn, &format!("u{i}"), "s1", reset - H, Some(reset));
            sync(&mut conn, reset - H, &[], ON, true).unwrap();
            let c = tick(&mut conn, reset + 200_000, ON).unwrap().expect("due");
            assert_eq!(c.origin_bg_id, "b1");
            finish(&conn, c.id, Ok(()), reset + 201_000).unwrap();
            if i == 0 {
                // The resume got a new bg id: the alias row keeps the origin.
                conn.execute("INSERT INTO task_meta(bg_id, created_at, session_id, permission_mode, remote, origin_bg_id) VALUES ('b1x', 2, 's1', 'acceptEdits', 0, 'b1')", []).unwrap();
                // Hitting the limit again with the same reset time joins the finished incident: nothing new.
                hit(&conn, "again", "s1", reset + 300_000, Some(reset));
                assert!(sync(&mut conn, reset + 300_001, &[], ON, true).unwrap().is_empty());
            }
            reset += 5 * H;
        }
        assert_eq!(attempts(&conn, "b1").unwrap(), 3);
        hit(&conn, "u-last", "s1", reset - H, Some(reset));
        sync(&mut conn, reset - H, &[], ON, true).unwrap();
        let last = status(&conn, reset - H).unwrap().into_iter().find(|e| e.resets_at == Some(reset)).unwrap();
        assert_eq!((last.state, last.reason, last.attempts), (State::Manual, Some(Reason::Cap), 3));
    }

    #[test]
    fn opt_out_and_cancel() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        hit(&conn, "u1", "s1", T0, Some(T0 + H));
        sync(&mut conn, T0, &[], ON, true).unwrap();
        assert!(set_opt_out(&conn, "b1", true, T0 + 1).unwrap());
        assert!(!set_opt_out(&conn, "nope", true, T0 + 1).unwrap());
        assert_eq!(rows(&conn)[0].2.as_deref(), Some("opt_out"));
        let id = status(&conn, T0).unwrap()[0].id;
        assert!(cancel(&conn, id, T0 + 2).unwrap());
        assert!(!cancel(&conn, id, T0 + 3).unwrap());
    }

    #[test]
    fn texts() {
        let tz = chrono_tz::Asia::Seoul;
        let reset = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 11, 9, 13, 50, 0).unwrap().timestamp_millis();
        let now = reset - 3 * H;
        assert_eq!(reset_label(reset, now, &tz, Ko), "오후 1:50");
        assert_eq!(notice_body(&Notice { resets_at: reset, auto: 2, manual: 1 }, now, &tz, Ko), "사용량 한도에 닿았어요 · 오후 1:50 리셋 · 2개는 이어서 할게요 · 1개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요");
        assert_eq!(notice_body(&Notice { resets_at: reset, auto: 0, manual: 2 }, now, &tz, Ko), "사용량 한도에 닿았어요 · 오후 1:50 리셋 · 2개는 직접 이어야 해요");
        let weekly = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 11, 12, 9, 0, 0).unwrap().timestamp_millis();
        assert_eq!(reset_label(weekly, now, &tz, Ko), "11월 12일(목) 오전 9:00");
        assert_eq!(started_body(2, Ko), "작업 2개를 이어서 시작했어요");
        assert_eq!(counts_text(0, 0, Ko), "");
        assert_eq!(reset_label(chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 11, 9, 0, 10, 0).unwrap().timestamp_millis(), now - 20 * H, &tz, Ko), "오전 12:10");
        // English: the same facts, English clock and date.
        assert_eq!(reset_label(reset, now, &tz, En), "1:50 PM");
        assert_eq!(reset_label(weekly, now, &tz, En), "Thu, Nov 12, 9:00 AM");
        assert_eq!(
            notice_body(&Notice { resets_at: reset, auto: 2, manual: 1 }, now, &tz, En),
            "Usage limit reached · Resets at 1:50 PM · 2 continue on their own · 1 need you to continue · Your Mac needs to be awake"
        );
        assert_eq!(started_body(1, En), "Continued tasks: 1");
    }

    #[test]
    fn view_banner_counts_and_labels() {
        let tz = chrono_tz::Asia::Seoul;
        let mut conn = db();
        let reset = chrono::TimeZone::with_ymd_and_hms(&tz, 2026, 11, 9, 13, 50, 0).unwrap().timestamp_millis();
        task(&conn, "b1", "s1", "acceptEdits");
        task(&conn, "b2", "s2", "plan");
        task(&conn, "b3", "s3", "bypassPermissions");
        for s in ["s1", "s2", "s3"] {
            hit(&conn, &format!("u{s}"), s, reset - H, Some(reset));
        }
        sync(&mut conn, reset - H, &[], ON, true).unwrap();
        let en = view(&conn, reset - H, &tz, ON, En).unwrap();
        assert_eq!(en.banner.unwrap().text, "Usage limit · resets 1:50 PM · 2 continue on their own · 1 need you to continue · Your Mac needs to be awake");
        assert!(en.entries.iter().all(|e| e.label.as_deref() == Some("1:50 PM")));
        let v = view(&conn, reset - H, &tz, ON, Ko).unwrap();
        let b = v.banner.unwrap();
        assert_eq!(b.text, "사용량 한도 · 오후 1:50 리셋 · 2개는 이어서 할게요 · 1개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요");
        assert_eq!(v.entries.len(), 3);
        assert!(v.entries.iter().all(|e| e.label.as_deref() == Some("오후 1:50")));
        let json = serde_json::to_value(&v.entries[0]).unwrap();
        for k in ["id", "sessionId", "bgId", "resetsAt", "dueAt", "state", "reason", "attempts", "name", "startedAt", "label", "dueLabel", "startedLabel"] {
            assert!(json.get(k).is_some(), "{k}");
        }
        // Setting off: everything is "직접".
        tick(&mut conn, reset - H + 1, Policy { auto_on: false, shipped: true }).unwrap();
        let v = view(&conn, reset - H + 1, &tz, Policy { auto_on: false, shipped: true }, Ko).unwrap();
        assert_eq!(v.banner.unwrap().text, "사용량 한도 · 오후 1:50 리셋 · 3개는 직접 이어야 해요");
    }

    #[test]
    fn running_resumed_tasks_count_toward_the_limit() {
        let mut conn = db();
        let reset = T0 + H;
        for i in 0..5 {
            task(&conn, &format!("b{i}"), &format!("s{i}"), "acceptEdits");
            hit(&conn, &format!("u{i}"), &format!("s{i}"), T0, Some(reset));
        }
        sync(&mut conn, T0, &[], ON, true).unwrap();
        let mut t = reset + 181_000;
        let mut done = Vec::new();
        for _ in 0..3 {
            let c = tick(&mut conn, t, ON).unwrap().expect("due");
            finish(&conn, c.id, Ok(()), t + 1).unwrap();
            done.push(c.session_id);
            t += 21_000;
        }
        // Three resumed sessions still working: no fourth start, whatever the starting count.
        assert!(tick_with(&mut conn, t, ON, &done).unwrap().is_none());
        // One of them finished: a slot frees.
        assert!(tick_with(&mut conn, t + 1, ON, &done[..2]).unwrap().is_some());
    }

    #[test]
    fn unclaim_keeps_the_card_and_names_never_show_ids() {
        let mut conn = db();
        task(&conn, "b1", "s1", "acceptEdits");
        conn.execute("UPDATE task_meta SET name = NULL, brief_json = ?1 WHERE bg_id = 'b1'", [r#"{"goal":"결제 화면 고치기\n세부","outOfScope":"","constraints":"","done":""}"#]).unwrap();
        task(&conn, "b2", "s2", "acceptEdits");
        conn.execute("UPDATE task_meta SET name = 'release-notes' WHERE bg_id = 'b2'", []).unwrap();
        hit(&conn, "u1", "s1", T0, Some(T0 + H));
        hit(&conn, "u2", "s2", T0, Some(T0 + H));
        hit(&conn, "u3", "term", T0, Some(T0 + H));
        sync(&mut conn, T0, &[], ON, true).unwrap();
        let st = status(&conn, T0).unwrap();
        let name = |s: &str| st.iter().find(|e| e.session_id == s).unwrap().name.clone();
        assert_eq!(name("s1").as_deref(), Some("결제 화면 고치기"), "first line of the goal");
        assert_eq!(name("s2").as_deref(), Some("release-notes"));
        assert_eq!(name("term"), None, "no task, no name — the UI never falls back to an id");
        assert_eq!(pick_name(None, None, Some("/Users/me/Agents/dev_team".into())).as_deref(), Some("dev_team"), "folder base name last");
        assert_eq!(pick_name(Some(" ".into()), None, None), None);
        let id = st.iter().find(|e| e.session_id == "s2").unwrap().id;
        let c = claim_manual(&mut conn, id, T0 + 1).unwrap().unwrap();
        unclaim(&conn, c.id, T0 + 2).unwrap();
        let e = status(&conn, T0 + 3).unwrap().into_iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.state, State::Manual, "back on the card, not closed");
    }

    #[test]
    fn risk_rechecks() {
        assert_eq!(recheck(false, false, false, false, false), Ok(()));
        assert_eq!(recheck(true, false, false, false, true), Err(Reason::RiskChanged), "agent turned off since start");
        assert_eq!(recheck(false, true, false, false, false), Err(Reason::RiskChanged), "persona became risky: not unattended");
        assert_eq!(recheck(false, true, false, false, true), Ok(()), "by hand after the native dialog");
        assert_eq!(recheck(false, true, true, false, true), Err(Reason::RiskChanged), "strict types never run risky personas");
        assert_eq!(recheck(false, true, false, true, true), Err(Reason::RiskChanged));
    }

    #[test]
    fn folder_checks() {
        let d = tempfile::tempdir().unwrap();
        let repo = std::fs::canonicalize(d.path()).unwrap().join("repo");
        let wt = repo.join(".claude/worktrees/fix-1");
        std::fs::create_dir_all(&wt).unwrap();
        let other = repo.join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        let yes = |_: &Path| Some(true);
        assert_eq!(verify_cwd(&repo, true, &wt, yes), Ok(wt.clone()));
        assert_eq!(verify_cwd(&repo, false, &repo, yes), Ok(repo.clone()));
        assert_eq!(verify_cwd(&repo, true, &other, yes), Err(Reason::CwdMismatch), "worktree task outside .claude/worktrees");
        assert_eq!(verify_cwd(&repo, false, &other, yes), Err(Reason::CwdMismatch));
        assert_eq!(verify_cwd(&repo, false, &repo, |_| None), Err(Reason::Untrusted));
        assert_eq!(verify_cwd(&repo, false, &repo, |_| Some(false)), Err(Reason::Untrusted));
        #[cfg(unix)]
        {
            let link = repo.join("link");
            std::os::unix::fs::symlink(&repo, &link).unwrap();
            assert_eq!(verify_cwd(&repo, false, &link, yes), Err(Reason::Link));
        }
        assert_eq!(verify_cwd(&repo, false, Path::new("relative"), yes), Err(Reason::Link));
    }

    #[test]
    fn nothing_but_ids_and_codes_reach_the_db() {
        // The parse → ingest → schedule path with a canary in the message text and upgrade paths.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("t.db");
        let mut conn = crate::db::open(&db_path).unwrap();
        let reset = T0 / 1000 + 3600;
        let line = format!(
            r#"{{"type":"assistant","uuid":"LIM1","timestamp":"2026-11-09T03:00:00.000Z","isApiErrorMessage":true,"error":"rate_limit","quotaLimits":{{"status":"rejected","resetsAt":{reset},"rateLimitType":"five_hour","upgradePaths":["CANARY_UP"]}},"message":{{"model":"<synthetic>","content":[{{"type":"text","text":"CANARY_LIMIT_BODY"}}]}}}}"#
        );
        let ctx = crate::parse::FileContext { session_id: "s1".into(), agent_id: None };
        let evs = crate::parse::parse_line(&line, &ctx).unwrap();
        for e in evs {
            if let crate::parse::Event::LimitHit(h) = e {
                conn.execute("INSERT OR IGNORE INTO limit_hits(record_uuid, session_id, is_subagent, ts, resets_at, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![h.record_uuid, h.session_id, h.is_subagent, h.ts, h.resets_at, h.kind]).unwrap();
            }
        }
        sync(&mut conn, T0, &[], ON, true).unwrap();
        drop(conn);
        for f in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(f.unwrap().path()).unwrap();
            assert!(!bytes.windows(6).any(|w| w == b"CANARY"));
        }
    }
}
