//! D.Duck local routines (artifacts-and-routines design R3–R6, D-049–D-051).
//!
//! Routines run unattended, so everything that decides whether and what to run lives here as plain functions:
//! validation, schedule math, the approval digest, claiming a run atomically and tracking its status. The shell
//! only supplies "start this TaskSpec" — it never shows dialogs or opens terminals on a routine's behalf.

use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration as ChronoDuration, LocalResult, NaiveDate, NaiveTime, TimeZone};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Error;
use crate::claude_cli::{AgentSession, PermissionMode, SessionStatus, TaskSpec};
use crate::parse::Redacted;
use crate::text::is_unsafe_char;

pub const MAX_ROUTINES: i64 = 20;
pub const MAX_ACTIVE_RUNS: i64 = 3;
pub const MISSED_AFTER_MS: i64 = 5 * 60 * 1000;
pub const LOST_AFTER_MS: i64 = 24 * 3600 * 1000;
const MAX_PROMPT_BYTES: usize = 100 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Schedule {
    /// Every day at HH:MM local time.
    Daily { hour: u32, minute: u32 },
    /// On the given weekdays (bit 0 = Monday … bit 6 = Sunday) at HH:MM local time.
    Weekly { weekdays: u8, hour: u32, minute: u32 },
    /// Every N hours (1–24), counted from the routine's anchor.
    Interval { hours: u32 },
}

impl Schedule {
    fn validate(&self) -> Result<(), &'static str> {
        match *self {
            Schedule::Daily { hour, minute } if hour < 24 && minute < 60 => Ok(()),
            Schedule::Weekly { weekdays, hour, minute } if (1..=127).contains(&weekdays) && hour < 24 && minute < 60 => Ok(()),
            Schedule::Interval { hours } if (1..=24).contains(&hours) => Ok(()),
            _ => Err(crate::i18n::Fail::ScheduleRange.str()),
        }
    }

    fn period_ms(&self) -> i64 {
        match self {
            Schedule::Daily { .. } => 24 * 3600 * 1000,
            Schedule::Weekly { .. } => 7 * 24 * 3600 * 1000,
            Schedule::Interval { hours } => i64::from(*hours) * 3600 * 1000,
        }
    }

    /// Canonical text used in the approval digest: always the Korean form, so a language change never
    /// invalidates an approval. Screens and dialogs use [`describe_in`](Self::describe_in).
    pub fn describe(&self) -> String {
        self.describe_in(crate::i18n::Lang::Ko)
    }

    /// The schedule in `lang` (the list and the confirmation dialog).
    pub fn describe_in(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::{Code, Params, msg, schedule_text};
        match *self {
            Schedule::Daily { hour, minute } => schedule_text(lang, None, hour, minute),
            Schedule::Weekly { weekdays, hour, minute } => schedule_text(lang, Some(weekdays), hour, minute),
            Schedule::Interval { hours } => msg(lang, Code::SchedEvery, &Params::new().num("hours", hours.into())),
        }
    }
}

/// What the user typed into the routine form.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineInput {
    pub name: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub permission_mode: PermissionMode,
    pub schedule: Schedule,
}

/// A validated routine definition: name trimmed, folder canonical.
#[derive(Clone, PartialEq)]
pub struct ValidRoutine {
    pub name: String,
    pub cwd: PathBuf,
    pub prompt: Redacted,
    pub permission_mode: PermissionMode,
    pub schedule: Schedule,
}

impl std::fmt::Debug for ValidRoutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidRoutine").field("name", &self.name).field("prompt", &self.prompt).finish_non_exhaustive()
    }
}

/// Modes a routine may use: never the ones that skip permission checks (D-049).
pub fn allowed_mode(m: PermissionMode) -> bool {
    matches!(m, PermissionMode::Manual | PermissionMode::Plan | PermissionMode::AcceptEdits | PermissionMode::DontAsk)
}

impl RoutineInput {
    pub fn validate(&self) -> Result<ValidRoutine, &'static str> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(is_unsafe_char) {
            return Err(crate::i18n::Fail::RoutineName.str());
        }
        if !self.cwd.is_absolute() || self.cwd.to_str().is_none_or(|s| s.chars().any(char::is_control)) {
            return Err(crate::i18n::Fail::TaskDirAbsolute.str());
        }
        let cwd = crate::platform::canon(&self.cwd).map_err(|_| crate::i18n::Fail::TaskDirMissing.str())?;
        if !cwd.is_dir() {
            return Err(crate::i18n::Fail::TaskDirMissing.str());
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(crate::i18n::Fail::RoutinePrompt.str());
        }
        if !allowed_mode(self.permission_mode) {
            return Err(crate::i18n::Fail::RoutineRisky.str());
        }
        self.schedule.validate()?;
        Ok(ValidRoutine {
            name: name.to_string(),
            cwd,
            prompt: Redacted(self.prompt.clone()),
            permission_mode: self.permission_mode,
            schedule: self.schedule.clone(),
        })
    }
}

/// SHA-256 over exactly the fields a user approves in the native dialog (D-049).
pub fn digest(r: &ValidRoutine) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(r.cwd.to_string_lossy().as_bytes());
    h.update([0]);
    h.update(r.prompt.as_str().as_bytes());
    h.update([0]);
    h.update(r.permission_mode.as_cli().as_bytes());
    h.update([0]);
    h.update(r.schedule.describe().as_bytes());
    h.finalize().to_vec()
}

/// `routine-<id>-<slug>`, at most 64 characters; Hangul kept, unsafe characters dropped, spaces to `-`.
pub fn run_name(id: i64, name: &str) -> String {
    let slug: String = name
        .chars()
        .filter(|c| !is_unsafe_char(*c))
        .map(|c| if c.is_whitespace() || c == '=' || c == '\'' || c == '"' { '-' } else { c })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = format!("routine-{id}");
    let full = if slug.is_empty() { base } else { format!("{base}-{slug}") };
    full.chars().take(64).collect()
}

// ---------- schedule math (R4) ----------

/// First local instant at `date` + `time`; a skipped time (spring forward) moves to the first valid minute after it,
/// an ambiguous one (fall back) takes the earlier instant.
fn local_instant<Tz: TimeZone>(tz: &Tz, date: NaiveDate, time: NaiveTime) -> Option<i64> {
    let mut t = date.and_time(time);
    for _ in 0..180 {
        match tz.from_local_datetime(&t) {
            LocalResult::Single(dt) => return Some(dt.timestamp_millis()),
            LocalResult::Ambiguous(a, b) => return Some(a.timestamp_millis().min(b.timestamp_millis())),
            LocalResult::None => t += ChronoDuration::minutes(1),
        }
    }
    None
}

/// Next run strictly after `after_ms`. Daily/weekly work per local calendar date, so a fall-back day never runs twice.
pub fn next_after<Tz: TimeZone>(schedule: &Schedule, anchor_ms: i64, after_ms: i64, tz: &Tz) -> Option<i64> {
    match *schedule {
        Schedule::Interval { hours } => {
            let step = i64::from(hours) * 3600 * 1000;
            if after_ms < anchor_ms {
                return Some(anchor_ms);
            }
            let k = (after_ms - anchor_ms) / step + 1;
            Some(anchor_ms + k * step)
        }
        Schedule::Daily { hour, minute } | Schedule::Weekly { hour, minute, .. } => {
            let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
            let start = tz.timestamp_millis_opt(after_ms).single()?.date_naive();
            for offset in 0..9 {
                let date = start + ChronoDuration::days(offset);
                if let Schedule::Weekly { weekdays, .. } = schedule {
                    let bit = 1u8 << date.weekday().num_days_from_monday();
                    if weekdays & bit == 0 {
                        continue;
                    }
                }
                if let Some(at) = local_instant(tz, date, time) {
                    if at > after_ms {
                        return Some(at);
                    }
                }
            }
            None
        }
    }
}

// ---------- persistence (R3) ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineSummary {
    pub id: i64,
    pub name: String,
    pub cwd: String,
    pub prompt_len: usize,
    pub permission_mode: String,
    pub schedule: Schedule,
    pub schedule_text: String,
    pub enabled: bool,
    pub next_run_at: Option<i64>,
    pub last_status: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineRun {
    pub id: i64,
    pub scheduled_for: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub status: String,
    pub bg_id: Option<String>,
    pub error: Option<String>,
}

fn schedule_columns(s: &Schedule) -> (&'static str, Option<String>, Option<i64>, Option<i64>) {
    match *s {
        Schedule::Daily { hour, minute } => ("daily", Some(format!("{hour:02}:{minute:02}")), None, None),
        Schedule::Weekly { weekdays, hour, minute } => ("weekly", Some(format!("{hour:02}:{minute:02}")), Some(i64::from(weekdays)), None),
        Schedule::Interval { hours } => ("interval", None, None, Some(i64::from(hours))),
    }
}

fn schedule_from(kind: &str, time_hm: Option<String>, weekdays: Option<i64>, hours: Option<i64>) -> Option<Schedule> {
    let hm = || {
        let t = time_hm.as_deref()?;
        let (h, m) = t.split_once(':')?;
        Some((h.parse().ok()?, m.parse().ok()?))
    };
    match kind {
        "daily" => hm().map(|(hour, minute)| Schedule::Daily { hour, minute }),
        "weekly" => {
            let (hour, minute) = hm()?;
            Some(Schedule::Weekly { weekdays: u8::try_from(weekdays?).ok()?, hour, minute })
        }
        "interval" => Some(Schedule::Interval { hours: u32::try_from(hours?).ok()? }),
        _ => None,
    }
}

fn mode_from(s: &str) -> Option<PermissionMode> {
    PermissionMode::ALL.into_iter().find(|m| m.as_cli() == s)
}

/// Stored definition as validated input, for re-checking right before a run (R5).
pub fn load_valid(conn: &Connection, id: i64) -> Result<Option<(ValidRoutine, Vec<u8>)>, Error> {
    let row = conn
        .query_row(
            "SELECT name, cwd, prompt, permission_mode, schedule_kind, time_hm, weekdays, interval_hours, approved_digest
             FROM routines WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Vec<u8>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((name, cwd, prompt, mode, kind, hm, wd, hours, dig)) = row else { return Ok(None) };
    let (Some(permission_mode), Some(schedule)) = (mode_from(&mode), schedule_from(&kind, hm, wd, hours)) else {
        return Ok(None);
    };
    Ok(Some((ValidRoutine { name, cwd: PathBuf::from(cwd), prompt: Redacted(prompt), permission_mode, schedule }, dig)))
}

/// Inserts a new, already-approved routine. Returns its id.
pub fn insert(conn: &Connection, r: &ValidRoutine, now_ms: i64, tz: &impl TimeZone) -> Result<i64, Error> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM routines", [], |x| x.get(0))?;
    if count >= MAX_ROUTINES {
        return Err(Error::Config(crate::i18n::Fail::RoutinesMax.str()));
    }
    let (kind, hm, wd, hours) = schedule_columns(&r.schedule);
    let next = next_after(&r.schedule, now_ms, now_ms, tz);
    conn.execute(
        "INSERT INTO routines(name, cwd, prompt, permission_mode, schedule_kind, time_hm, weekdays, interval_hours, enabled,
                              created_at, anchor_ms, next_run_at, approved_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9, ?10, ?11)",
        params![r.name, r.cwd.to_string_lossy(), r.prompt.as_str(), r.permission_mode.as_cli(), kind, hm, wd, hours, now_ms, next, digest(r)],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Replaces an approved definition (name and approved fields). Re-anchors the schedule; no "missed" entries.
pub fn update(conn: &Connection, id: i64, r: &ValidRoutine, now_ms: i64, tz: &impl TimeZone) -> Result<(), Error> {
    let (kind, hm, wd, hours) = schedule_columns(&r.schedule);
    let next = next_after(&r.schedule, now_ms, now_ms, tz);
    let n = conn.execute(
        "UPDATE routines SET name = ?2, cwd = ?3, prompt = ?4, permission_mode = ?5, schedule_kind = ?6, time_hm = ?7,
                weekdays = ?8, interval_hours = ?9, anchor_ms = ?10, next_run_at = ?11, approved_digest = ?12
         WHERE id = ?1",
        params![id, r.name, r.cwd.to_string_lossy(), r.prompt.as_str(), r.permission_mode.as_cli(), kind, hm, wd, hours, now_ms, next, digest(r)],
    )?;
    if n == 0 { Err(Error::Config(crate::i18n::Fail::RoutineNotFound.str())) } else { Ok(()) }
}

/// Only the name changes: no approval needed (D-049).
pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<(), Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(is_unsafe_char) {
        return Err(Error::Config(crate::i18n::Fail::RoutineName.str()));
    }
    conn.execute("UPDATE routines SET name = ?2 WHERE id = ?1", params![id, name])?;
    Ok(())
}

/// Turning a routine on re-anchors it at `now`, so the off period never produces "missed" runs.
pub fn set_enabled(conn: &Connection, id: i64, enabled: bool, now_ms: i64, tz: &impl TimeZone) -> Result<(), Error> {
    let Some((r, _)) = load_valid(conn, id)? else { return Err(Error::Config(crate::i18n::Fail::RoutineNotFound.str())) };
    let next = if enabled { next_after(&r.schedule, now_ms, now_ms, tz) } else { None };
    conn.execute(
        "UPDATE routines SET enabled = ?2, anchor_ms = CASE WHEN ?2 = 1 THEN ?3 ELSE anchor_ms END, next_run_at = ?4 WHERE id = ?1",
        params![id, enabled, now_ms, next],
    )?;
    Ok(())
}

/// Deletes runs then the routine explicitly (FK cascade is only a backstop). The caller scrubs freed pages.
pub fn delete(conn: &mut Connection, id: i64) -> Result<(), Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM routine_runs WHERE routine_id = ?1", [id])?;
    tx.execute("DELETE FROM routines WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

/// Full prompt for the editor (the list only carries its length, D-050).
pub fn prompt_of(conn: &Connection, id: i64) -> Result<Option<String>, Error> {
    Ok(conn.query_row("SELECT prompt FROM routines WHERE id = ?1", [id], |r| r.get(0)).optional()?)
}

pub fn list(conn: &Connection) -> Result<Vec<RoutineSummary>, Error> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.name, r.cwd, length(CAST(r.prompt AS BLOB)), r.permission_mode, r.schedule_kind, r.time_hm, r.weekdays,
                r.interval_hours, r.enabled, r.next_run_at,
                (SELECT status FROM routine_runs WHERE routine_id = r.id ORDER BY scheduled_for DESC, id DESC LIMIT 1),
                EXISTS(SELECT 1 FROM routine_runs WHERE routine_id = r.id AND status IN ('starting','running','waiting'))
         FROM routines r ORDER BY r.created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
            r.get::<_, bool>(9)?,
            r.get::<_, Option<i64>>(10)?,
            r.get::<_, Option<String>>(11)?,
            r.get::<_, bool>(12)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, cwd, plen, mode, kind, hm, wd, hours, enabled, next, last, active) = row?;
        let Some(schedule) = schedule_from(&kind, hm, wd, hours) else { continue };
        out.push(RoutineSummary {
            id,
            name,
            cwd,
            prompt_len: usize::try_from(plen).unwrap_or(0),
            permission_mode: mode,
            schedule_text: schedule.describe_in(crate::i18n::current()),
            schedule,
            enabled,
            next_run_at: next,
            last_status: last,
            active,
        });
    }
    Ok(out)
}

pub fn runs(conn: &Connection, id: i64, limit: usize) -> Result<Vec<RoutineRun>, Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, scheduled_for, started_at, ended_at, status, bg_id, error FROM routine_runs
         WHERE routine_id = ?1 ORDER BY scheduled_for DESC, id DESC LIMIT {}",
        limit.min(500)
    ))?;
    let rows = stmt.query_map([id], |r| {
        Ok(RoutineRun {
            id: r.get(0)?,
            scheduled_for: r.get(1)?,
            started_at: r.get(2)?,
            ended_at: r.get(3)?,
            status: r.get(4)?,
            bg_id: r.get(5)?,
            error: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---------- scheduling decisions and claiming (R5) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Wait,
    Run,
    Missed,
    SkipBusy,
    SkipConcurrency,
}

/// Pure decision for one routine at `now`.
pub fn decide(enabled: bool, next_run_at: Option<i64>, now_ms: i64, busy: bool, active_total: i64) -> Action {
    let Some(due) = next_run_at.filter(|_| enabled) else { return Action::Wait };
    if due > now_ms {
        return Action::Wait;
    }
    if now_ms - due > MISSED_AFTER_MS {
        return Action::Missed;
    }
    if busy {
        return Action::SkipBusy;
    }
    if active_total >= MAX_ACTIVE_RUNS {
        return Action::SkipConcurrency;
    }
    Action::Run
}

/// A claimed run, ready to be started outside the transaction.
#[derive(Debug, Clone)]
pub struct Claim {
    pub run_id: i64,
    pub routine_id: i64,
    pub spec: TaskSpec,
}

pub enum RunNowError {
    Busy,
    NotFound,
    Invalid(&'static str),
}

/// Re-validates a stored routine right before it runs (D-051): the definition, the canonical folder and the digest.
fn verified_spec(conn: &Connection, id: i64) -> Result<TaskSpec, &'static str> {
    let Some((stored, dig)) = load_valid(conn, id).map_err(|_| "invalidRoutine")? else { return Err("invalidRoutine") };
    let input = RoutineInput {
        name: stored.name.clone(),
        cwd: stored.cwd.clone(),
        prompt: stored.prompt.as_str().to_string(),
        permission_mode: stored.permission_mode,
        schedule: stored.schedule.clone(),
    };
    let valid = input.validate().map_err(|_| "invalidRoutine")?;
    if valid.cwd != stored.cwd {
        return Err("folderChanged");
    }
    if digest(&valid) != dig {
        return Err("notApproved");
    }
    if valid.permission_mode.is_dangerous() {
        return Err("invalidRoutine");
    }
    let mut spec = TaskSpec::basic(valid.cwd, valid.prompt.as_str().to_string(), Some(run_name(id, &valid.name)), Some(valid.permission_mode));
    // Routine runs follow the same team rules as composer tasks, including the user's edits (D-080, D-081).
    spec.team_policy = true;
    let stored = crate::settings::team_config(conn).ok().flatten();
    // Delivery scope (delivery-guard): a ticked project runs local-only; an unreadable list refuses the run.
    let home = crate::platform::home();
    // Routines never get a worktree, so a default-branch checkout gets the bare-push rules. The hook is the app
    // executable (tests have none: `local_only` is then refused at spawn).
    let exe = if cfg!(test) { None } else { std::env::current_exe().ok() };
    let global = if cfg!(test) { None } else { home.as_deref().map(crate::delivery_guard::global_config_path) };
    let start = crate::delivery_guard::start_ctx_with(conn, &spec.cwd, home.as_deref(), false, exe.as_deref(), global.as_deref()).map_err(|_| "guardUnreadable")?;
    if crate::delivery_guard::local_only_refusal(start.ctx.guard, false, &crate::delivery_guard::ManagedSources::from_env(None)).is_some() {
        return Err("guardRefused");
    }
    spec.policy_text = Some(crate::team_config::policy_text_guarded(&crate::team_config::load(stored.as_deref()), false, start.ctx.guard));
    spec.guard_ctx = Some(start.ctx);
    Ok(spec)
}

/// Adds the "먼저 살펴볼 스킬·에이전트" line to a claimed run's team rules (preferred-skills). Rebuilt with the same
/// function, the run's own delivery scope (so the local-only line stays) and the current language. `installed` is
/// what the catalogs show for the run's folder. A run that would get too long keeps its rules without the line.
pub fn add_preferred(spec: &mut TaskSpec, conn: &Connection, installed: &[crate::preferred::Available]) {
    let stored = crate::settings::team_config(conn).ok().flatten();
    let config = crate::team_config::load(stored.as_deref());
    let Some(line) = crate::preferred::line(&config.preferred, installed, config.lang, crate::preferred::Ctx::default()) else { return };
    let guard = spec.guard_ctx.as_ref().map_or(crate::delivery_guard::Guard::DraftPr, |g| g.guard);
    let before = spec.policy_text.replace(crate::team_config::policy_text_with(&config, false, guard, None, Some(&line)));
    if !spec.args_fit() {
        spec.policy_text = before;
    }
}

/// Whether a routine's team rules carry the preferred line (the shell reads the catalogs only then).
pub fn wants_preferred(conn: &Connection) -> bool {
    let stored = crate::settings::team_config(conn).ok().flatten();
    crate::preferred::needed(&crate::team_config::load(stored.as_deref()).preferred)
}

fn record(conn: &Connection, id: i64, at: i64, status: &str, error: Option<&str>) -> Result<i64, Error> {
    conn.execute(
        "INSERT INTO routine_runs(routine_id, scheduled_for, started_at, status, error) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, at, if status == "starting" { Some(at) } else { None }, status, error],
    )?;
    Ok(conn.last_insert_rowid())
}

/// One scheduler tick: decides every due routine and claims runs atomically (claim first, start later). Returns the
/// claims to start; skipped, missed and invalid routines are recorded here.
pub fn tick(conn: &mut Connection, now_ms: i64, tz: &impl TimeZone) -> Result<Vec<Claim>, Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let due: Vec<(i64, bool, Option<i64>, i64)> = {
        let mut stmt = tx.prepare("SELECT id, enabled, next_run_at, anchor_ms FROM routines WHERE enabled = 1 AND next_run_at IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut active_total: i64 =
        tx.query_row("SELECT COUNT(*) FROM routine_runs WHERE status IN ('starting','running','waiting')", [], |r| r.get(0))?;
    let mut claims = Vec::new();
    for (id, enabled, next, anchor) in due {
        let Some((r, _)) = load_valid(&tx, id)? else { continue };
        // Clock moved backwards far enough that the stored time is beyond one period: re-plan from now.
        if next.is_some_and(|n| n > now_ms + r.schedule.period_ms()) {
            let fixed = next_after(&r.schedule, anchor, now_ms, tz);
            tx.execute("UPDATE routines SET next_run_at = ?2 WHERE id = ?1", params![id, fixed])?;
            continue;
        }
        let busy: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM routine_runs WHERE routine_id = ?1 AND status IN ('starting','running','waiting'))",
            [id],
            |x| x.get(0),
        )?;
        let action = decide(enabled, next, now_ms, busy, active_total);
        let due_at = next.unwrap_or(now_ms);
        match action {
            Action::Wait => continue,
            Action::Missed => {
                record(&tx, id, due_at, "missed", None)?;
            }
            Action::SkipBusy => {
                record(&tx, id, due_at, "skipped", Some("busy"))?;
            }
            Action::SkipConcurrency => {
                record(&tx, id, due_at, "skipped", Some("concurrency"))?;
            }
            Action::Run => match verified_spec(&tx, id) {
                Ok(spec) => {
                    let run_id = record(&tx, id, due_at, "starting", None)?;
                    active_total += 1;
                    claims.push(Claim { run_id, routine_id: id, spec });
                }
                Err(kind) => {
                    record(&tx, id, due_at, "start_failed", Some(kind))?;
                }
            },
        }
        // Move the schedule before anything starts, so a crash cannot run the same slot twice.
        let following = next_after(&r.schedule, anchor, now_ms.max(due_at), tz);
        tx.execute("UPDATE routines SET next_run_at = ?2 WHERE id = ?1", params![id, following])?;
    }
    tx.commit()?;
    Ok(claims)
}

/// "지금 실행": same claim path, regardless of the schedule or the enabled flag (which it does not change).
pub fn claim_now(conn: &mut Connection, id: i64, now_ms: i64) -> Result<Claim, RunNowError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| RunNowError::NotFound)?;
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM routines WHERE id = ?1)", [id], |r| r.get(0)).unwrap_or(false);
    if !exists {
        return Err(RunNowError::NotFound);
    }
    let busy: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM routine_runs WHERE routine_id = ?1 AND status IN ('starting','running','waiting'))",
            [id],
            |r| r.get(0),
        )
        .unwrap_or(true);
    if busy {
        return Err(RunNowError::Busy);
    }
    let spec = match verified_spec(&tx, id) {
        Ok(s) => s,
        Err(kind) => {
            let _ = record(&tx, id, now_ms, "start_failed", Some(kind));
            let _ = tx.commit();
            return Err(RunNowError::Invalid(kind));
        }
    };
    let run_id = record(&tx, id, now_ms, "starting", None).map_err(|_| RunNowError::NotFound)?;
    tx.commit().map_err(|_| RunNowError::NotFound)?;
    Ok(Claim { run_id, routine_id: id, spec })
}

/// Result of starting a claimed run.
pub fn finish_start(conn: &Connection, run_id: i64, result: Result<String, &str>) -> Result<(), Error> {
    match result {
        Ok(bg_id) => conn.execute("UPDATE routine_runs SET status = 'running', bg_id = ?2 WHERE id = ?1", params![run_id, bg_id])?,
        Err(kind) => conn.execute(
            "UPDATE routine_runs SET status = 'start_failed', error = ?2, ended_at = started_at WHERE id = ?1",
            params![run_id, kind],
        )?,
    };
    Ok(())
}

/// At startup: runs left `starting` by a crash or quit are closed (never re-started).
pub fn close_interrupted(conn: &Connection) -> Result<usize, Error> {
    Ok(conn.execute(
        "UPDATE routine_runs SET status = 'start_failed', error = 'interrupted', ended_at = started_at WHERE status = 'starting'",
        [],
    )?)
}

// ---------- status tracking (R6) ----------

type ActiveRun = (i64, Option<String>, Option<i64>, Option<i64>);

/// Applies one successful `claude agents` snapshot to active runs.
pub fn track(conn: &Connection, sessions: &[AgentSession], now_ms: i64) -> Result<(), Error> {
    let active: Vec<ActiveRun> = {
        let mut stmt = conn.prepare("SELECT id, bg_id, last_seen_at, started_at FROM routine_runs WHERE status IN ('running','waiting')")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (run_id, bg_id, last_seen, started) in active {
        let found = bg_id.as_deref().and_then(|b| sessions.iter().find(|s| s.id.as_deref() == Some(b)));
        match found {
            Some(s) => {
                let status = match s.status {
                    SessionStatus::Working => Some("running"),
                    SessionStatus::WaitingInput => Some("waiting"),
                    SessionStatus::Done => Some("done"),
                    SessionStatus::Failed => Some("failed"),
                    SessionStatus::Stopped => Some("stopped"),
                    SessionStatus::Idle | SessionStatus::Unknown => None,
                };
                let ended = status.is_some_and(|st| matches!(st, "done" | "failed" | "stopped"));
                conn.execute(
                    "UPDATE routine_runs SET last_seen_at = ?2, status = COALESCE(?3, status),
                            ended_at = CASE WHEN ?4 THEN ?2 ELSE ended_at END WHERE id = ?1",
                    params![run_id, now_ms, status, ended],
                )?;
            }
            None => {
                let since = last_seen.or(started).unwrap_or(now_ms);
                if now_ms - since > LOST_AFTER_MS {
                    conn.execute("UPDATE routine_runs SET status = 'lost', ended_at = ?2 WHERE id = ?1", params![run_id, now_ms])?;
                }
            }
        }
    }
    Ok(())
}

/// Whether `dir` (a stored routine folder) can be shown in the trust flow.
pub fn cwd_of(conn: &Connection, id: i64) -> Result<Option<PathBuf>, Error> {
    Ok(conn.query_row("SELECT cwd FROM routines WHERE id = ?1", [id], |r| r.get::<_, String>(0)).optional()?.map(PathBuf::from))
}

pub fn is_dir(p: &Path) -> bool {
    p.is_dir()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::platform::CanonExt;
    use super::*;
    use chrono::{FixedOffset, Utc};
    use chrono_tz::America::New_York;

    fn ms(s: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
    }

    fn input(dir: &Path) -> RoutineInput {
        RoutineInput {
            name: "아침 점검".into(),
            cwd: dir.to_path_buf(),
            prompt: "테스트를 돌리고 실패를 요약해 줘".into(),
            permission_mode: PermissionMode::Plan,
            schedule: Schedule::Daily { hour: 9, minute: 0 },
        }
    }

    #[test]
    fn validation_rules() {
        let d = tempfile::tempdir().unwrap();
        assert!(input(d.path()).validate().is_ok());
        let spaced = d.path().join("내 프로젝트 폴더");
        std::fs::create_dir(&spaced).unwrap();
        assert!(RoutineInput { cwd: spaced, ..input(d.path()) }.validate().is_ok(), "spaces and Hangul are fine");
        for bad in [
            RoutineInput { permission_mode: PermissionMode::BypassPermissions, ..input(d.path()) },
            RoutineInput { permission_mode: PermissionMode::Auto, ..input(d.path()) },
            RoutineInput { name: "  ".into(), ..input(d.path()) },
            RoutineInput { name: "evil\u{202e}name".into(), ..input(d.path()) },
            RoutineInput { cwd: d.path().join("missing"), ..input(d.path()) },
            RoutineInput { prompt: "".into(), ..input(d.path()) },
            RoutineInput { schedule: Schedule::Interval { hours: 25 }, ..input(d.path()) },
            RoutineInput { schedule: Schedule::Daily { hour: 24, minute: 0 }, ..input(d.path()) },
            RoutineInput { schedule: Schedule::Weekly { weekdays: 0, hour: 9, minute: 0 }, ..input(d.path()) },
        ] {
            assert!(bad.validate().is_err(), "{:?}", bad.name);
        }
    }

    #[test]
    fn run_names() {
        assert_eq!(run_name(3, "아침 점검"), "routine-3-아침-점검");
        assert_eq!(run_name(7, "  "), "routine-7");
        assert_eq!(run_name(1, "a=b 'c\""), "routine-1-a-b-c");
        assert!(run_name(12, &"긴이름".repeat(40)).chars().count() <= 64);
    }

    #[test]
    fn daily_weekly_interval() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let daily = Schedule::Daily { hour: 9, minute: 0 };
        assert_eq!(next_after(&daily, 0, ms("2026-11-07T08:59:00+09:00"), &kst), Some(ms("2026-11-07T09:00:00+09:00")));
        assert_eq!(next_after(&daily, 0, ms("2026-11-07T09:00:00+09:00"), &kst), Some(ms("2026-11-08T09:00:00+09:00")), "strictly after");
        // 2026-11-07 is a Saturday; Monday + Wednesday.
        let weekly = Schedule::Weekly { weekdays: 0b000_0101, hour: 9, minute: 0 };
        assert_eq!(next_after(&weekly, 0, ms("2026-11-07T10:00:00+09:00"), &kst), Some(ms("2026-11-09T09:00:00+09:00")));
        let interval = Schedule::Interval { hours: 2 };
        let anchor = ms("2026-11-07T10:00:00+09:00");
        assert_eq!(next_after(&interval, anchor, anchor, &kst), Some(anchor + 2 * 3600 * 1000));
        assert_eq!(next_after(&interval, anchor, anchor + 5 * 3600 * 1000, &kst), Some(anchor + 6 * 3600 * 1000));
    }

    #[test]
    fn dst_transitions() {
        // US fall back 2026-11-01: 01:30 happens twice; the routine runs once that day, at the earlier one.
        let r = Schedule::Daily { hour: 1, minute: 30 };
        let first = next_after(&r, 0, ms("2026-11-01T00:00:00-04:00"), &New_York).unwrap();
        assert_eq!(first, ms("2026-11-01T01:30:00-04:00"));
        let second = next_after(&r, 0, first, &New_York).unwrap();
        assert_eq!(second, ms("2026-11-02T01:30:00-05:00"), "no second run on the fall-back day");
        // US spring forward 2026-03-08: 02:30 does not exist → first valid minute after it (03:00 EDT).
        let r = Schedule::Daily { hour: 2, minute: 30 };
        assert_eq!(next_after(&r, 0, ms("2026-03-08T00:00:00-05:00"), &New_York), Some(ms("2026-03-08T03:00:00-04:00")));
    }

    #[test]
    fn decisions() {
        let now = 1_000_000_000;
        assert_eq!(decide(true, Some(now + 1), now, false, 0), Action::Wait);
        assert_eq!(decide(false, Some(now - 1), now, false, 0), Action::Wait, "off routines never run");
        assert_eq!(decide(true, Some(now - 1000), now, false, 0), Action::Run);
        assert_eq!(decide(true, Some(now - MISSED_AFTER_MS - 1), now, false, 0), Action::Missed, "slept or closed");
        assert_eq!(decide(true, Some(now - 1000), now, true, 0), Action::SkipBusy);
        assert_eq!(decide(true, Some(now - 1000), now, false, MAX_ACTIVE_RUNS), Action::SkipConcurrency);
    }

    fn setup() -> (tempfile::TempDir, Connection, i64) {
        let d = tempfile::tempdir().unwrap();
        let conn = crate::db::open_in_memory().unwrap();
        let v = input(d.path()).validate().unwrap();
        let id = insert(&conn, &v, ms("2026-11-07T08:00:00+09:00"), &FixedOffset::east_opt(9 * 3600).unwrap()).unwrap();
        (d, conn, id)
    }

    #[test]
    fn routine_runs_carry_the_users_team_rules() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, _id) = setup();
        let mut input = crate::team_config::TeamInput {
            rules: crate::team_config::defaults().rules.iter().map(|r| crate::team_config::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: String::new() }).collect(),
            types: crate::team_config::defaults().types.iter().map(|t| t.def.clone()).collect(),
            refine: None,
            preferred: None,
        };
        input.rules.push(crate::team_config::RuleInput { id: "u-ko".into(), text: "커밋 메시지는 한국어로 쓴다".into(), extra: String::new() });
        crate::settings::set_team_config(&conn, &crate::team_config::diff(&input, &[]).unwrap()).unwrap();
        let claims = tick(&mut conn, ms("2026-11-07T09:00:10+09:00"), &kst).unwrap();
        let args: Vec<String> = claims[0].spec.bg_args().iter().map(|a| a.to_string_lossy().to_string()).collect();
        assert!(args.iter().any(|a| a.contains("8. 커밋 메시지는 한국어로 쓴다")), "{args:?}");
    }

    #[test]
    fn routine_in_a_local_only_project_gets_the_rule_and_bad_list_refuses() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (d, mut conn, _id) = setup();
        let dir = d.path().canon().unwrap();
        crate::delivery_guard::add(&conn, &dir, None).unwrap();
        let claims = tick(&mut conn, ms("2026-11-07T09:00:10+09:00"), &kst).unwrap();
        assert!(claims[0].spec.policy_text.as_deref().unwrap().contains(crate::delivery_guard::LOCAL_ONLY_RULE));
        // A damaged list never falls back to the default scope.
        let (_d2, conn2, id2) = setup();
        crate::settings::put_guarded(&conn2, crate::delivery_guard::LOCAL_ONLY_PROJECTS, "{").unwrap();
        assert_eq!(verified_spec(&conn2, id2).err(), Some("guardUnreadable"));
    }

    #[test]
    fn routine_runs_get_the_preferred_line_and_keep_the_local_only_rule() {
        use crate::preferred::{Available, Kind, PreferredItem};
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (d, mut conn, _id) = setup();
        crate::delivery_guard::add(&conn, &d.path().canon().unwrap(), None).unwrap();
        assert!(wants_preferred(&conn), "the bundled default (mattpocock-skills) is a list");
        let mut input = crate::team_config::TeamInput {
            rules: crate::team_config::defaults().rules.iter().map(|r| crate::team_config::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: String::new() }).collect(),
            types: crate::team_config::defaults().types.iter().map(|t| t.def.clone()).collect(),
            refine: None,
            preferred: Some(vec![PreferredItem { kind: Kind::Skill, name: "tdd".into(), source: "user".into(), scope: None, marketplace: None }]),
        };
        let mut cleared = input.clone();
        cleared.preferred = Some(Vec::new());
        crate::settings::set_team_config(&conn, &crate::team_config::diff(&cleared, &[]).unwrap()).unwrap();
        assert!(!wants_preferred(&conn), "a cleared list reads no catalogs");
        input.preferred.as_mut().unwrap().push(PreferredItem { kind: Kind::Skill, name: "gone".into(), source: "user".into(), scope: None, marketplace: None });
        crate::settings::set_team_config(&conn, &crate::team_config::diff(&input, &[]).unwrap()).unwrap();
        assert!(wants_preferred(&conn));
        let mut claims = tick(&mut conn, ms("2026-11-07T09:00:10+09:00"), &kst).unwrap();
        let installed = [Available { kind: Kind::Skill, name: "tdd".into(), source: "user".into(), scope: None, description: Some("테스트 먼저".into()), members: vec![], writes: false, writing_members: vec![], marketplace: None, enabled: true }];
        add_preferred(&mut claims[0].spec, &conn, &installed);
        let p = claims[0].spec.policy_text.clone().unwrap();
        assert_eq!(p.matches("작업을 시작하기 전에").count(), 1, "{p}");
        assert!(p.contains("스킬 tdd 「테스트 먼저」") && !p.contains("gone"));
        assert!(p.contains(crate::delivery_guard::LOCAL_ONLY_RULE), "the run's own scope is kept");
        // Nothing installed: the rules stay as they were.
        let before = claims[0].spec.policy_text.clone();
        let mut again = claims.remove(0).spec;
        again.policy_text = before.clone().map(|t| t.replace("작업을 시작하기 전에", "X"));
        let kept = again.policy_text.clone();
        add_preferred(&mut again, &conn, &[]);
        assert_eq!(again.policy_text, kept);
    }

    #[test]
    fn tick_claims_once_and_moves_schedule_first() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, id) = setup();
        let at = ms("2026-11-07T09:00:10+09:00");
        let claims = tick(&mut conn, at, &kst).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].spec.name.as_deref(), Some(format!("routine-{id}-아침-점검").as_str()));
        assert_eq!(claims[0].spec.permission_mode, Some(PermissionMode::Plan));
        assert!(claims[0].spec.team_policy, "routines run under the team rules");
        assert!(claims[0].spec.bg_args().iter().any(|x| x.to_string_lossy().starts_with("--append-system-prompt=[D.Duck 팀 규칙]")));
        // A second tick in the same minute claims nothing: the slot already moved to tomorrow.
        assert!(tick(&mut conn, at + 20_000, &kst).unwrap().is_empty());
        let next: i64 = conn.query_row("SELECT next_run_at FROM routines WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(next, ms("2026-11-08T09:00:00+09:00"));
        finish_start(&conn, claims[0].run_id, Ok("ab12cd34".into())).unwrap();
        assert!(matches!(claim_now(&mut conn, id, at + 30_000), Err(RunNowError::Busy)));
    }

    /// claude-login: a scheduled occurrence that comes due while logged out is recorded with the fixed code (the shell
    /// passes `cli_error_kind(CliError::LoginRequired)`) and is not held for later.
    #[test]
    fn logged_out_occurrence_is_recorded_as_login_required() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, id) = setup();
        let at = ms("2026-11-07T09:00:10+09:00");
        let claims = tick(&mut conn, at, &kst).unwrap();
        finish_start(&conn, claims[0].run_id, Err(crate::auth_status::LOGIN_REQUIRED)).unwrap();
        let (status, error): (String, Option<String>) =
            conn.query_row("SELECT status, error FROM routine_runs WHERE id = ?1", [claims[0].run_id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((status.as_str(), error.as_deref()), ("start_failed", Some("login_required")));
        assert!(tick(&mut conn, at + 20_000, &kst).unwrap().is_empty(), "not retried: the next occurrence is tomorrow");
        assert!(claim_now(&mut conn, id, at + 30_000).is_ok(), "nothing is left running");
    }

    #[test]
    fn missed_while_asleep_and_no_missed_after_enabling() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, id) = setup();
        assert!(tick(&mut conn, ms("2026-11-07T10:00:00+09:00"), &kst).unwrap().is_empty());
        let st: String = conn.query_row("SELECT status FROM routine_runs WHERE routine_id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(st, "missed");
        set_enabled(&conn, id, false, ms("2026-11-08T08:00:00+09:00"), &kst).unwrap();
        set_enabled(&conn, id, true, ms("2026-11-10T10:00:00+09:00"), &kst).unwrap();
        tick(&mut conn, ms("2026-11-10T10:00:30+09:00"), &kst).unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM routine_runs WHERE status = 'missed'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "the off period adds no missed runs");
    }

    #[test]
    fn tampered_definition_is_not_run() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, id) = setup();
        conn.execute("UPDATE routines SET prompt = 'rm -rf everything' WHERE id = ?1", [id]).unwrap();
        assert!(tick(&mut conn, ms("2026-11-07T09:00:05+09:00"), &kst).unwrap().is_empty());
        let (st, err): (String, String) =
            conn.query_row("SELECT status, error FROM routine_runs WHERE routine_id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((st.as_str(), err.as_str()), ("start_failed", "notApproved"));
    }

    #[test]
    fn run_now_keeps_disabled_state() {
        let kst = FixedOffset::east_opt(9 * 3600).unwrap();
        let (_d, mut conn, id) = setup();
        set_enabled(&conn, id, false, ms("2026-11-07T08:30:00+09:00"), &kst).unwrap();
        let c = claim_now(&mut conn, id, ms("2026-11-07T08:31:00+09:00")).ok().unwrap();
        assert_eq!(c.routine_id, id);
        let enabled: bool = conn.query_row("SELECT enabled FROM routines WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert!(!enabled);
    }

    #[test]
    fn tracking_and_lost_runs() {
        use crate::claude_cli::SessionKind;
        let (_d, mut conn, id) = setup();
        let c = claim_now(&mut conn, id, 1_000).ok().unwrap();
        finish_start(&conn, c.run_id, Ok("ab12cd34".into())).unwrap();
        let s = |status| AgentSession {
            key: "k".into(),
            pid: Some(1),
            id: Some("ab12cd34".into()),
            session_id: Some("k".into()),
            name: "routine".into(),
            cwd: "/w".into(),
            kind: SessionKind::Background,
            started_at: 0,
            status,
        };
        track(&conn, &[s(SessionStatus::WaitingInput)], 2_000).unwrap();
        let st = |conn: &Connection| -> String { conn.query_row("SELECT status FROM routine_runs", [], |r| r.get(0)).unwrap() };
        assert_eq!(st(&conn), "waiting");
        track(&conn, &[s(SessionStatus::Idle)], 3_000).unwrap();
        assert_eq!(st(&conn), "waiting", "idle/unknown leave the status alone");
        track(&conn, &[s(SessionStatus::Done)], 4_000).unwrap();
        assert_eq!(st(&conn), "done");
        // Another run that vanishes from the list is closed as lost after 24h of successful polls.
        let c = claim_now(&mut conn, id, 5_000).ok().unwrap();
        finish_start(&conn, c.run_id, Ok("ffff0000".into())).unwrap();
        track(&conn, &[], 5_000 + LOST_AFTER_MS + 1).unwrap();
        let lost: i64 = conn.query_row("SELECT COUNT(*) FROM routine_runs WHERE status = 'lost'", [], |r| r.get(0)).unwrap();
        assert_eq!(lost, 1);
    }

    #[test]
    fn deleting_scrubs_prompt_from_disk() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("data/dduck.db");
        {
            let mut conn = crate::db::open(&path).unwrap();
            let v = RoutineInput { prompt: format!("CANARY_ROUTINEPROMPT {}", "x".repeat(90_000)), ..input(d.path()) }.validate().unwrap();
            let id = insert(&conn, &v, 0, &Utc).unwrap();
            delete(&mut conn, id).unwrap();
            crate::ingest::scrub(&conn).unwrap();
        }
        for e in std::fs::read_dir(path.parent().unwrap()).unwrap() {
            let bytes = std::fs::read(e.unwrap().path()).unwrap();
            assert!(!bytes.windows(20).any(|w| w == b"CANARY_ROUTINEPROMPT"), "prompt bytes left after delete");
        }
    }
}
