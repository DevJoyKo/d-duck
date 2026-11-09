//! 리뷰해야 끝나는 작업 (task-lifecycle, D-149 (7)·D-152 (5)·(11)): the board's 리뷰 대기 and 정리 columns, "정리 끝",
//! and the local-only wrap-up checklist.
//!
//! Lanes are decided for ended tasks only (`review_inbox::running_of` is None, or the CLI no longer lists the task):
//! 1. a "리뷰 전" task (the review tab's own `is_pending`, failed and stopped included) → 리뷰 대기;
//! 2. wrapped up → off the board ("완료");
//! 3. failed and never reviewed → none: 확인 필요 as before;
//! 4. anything else → 정리 ("리뷰했어요 · 정리 남음", or "리뷰 없이 끝났어요" for tasks that were never review
//!    targets: before `review_since`, without a report, or started outside D.Duck — those come from the 24-hour
//!    listing only, so old records never flood the column).
//!
//! The checklist runs local git only, through `delivery_guard::app_git` (fsmonitor and hooks off, no system config,
//! no prompts), with argument arrays and time limits. No `gh`, no network. File names never leave this module.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::sync::LazyLock;

use crate::Error;
use crate::claude_cli::{AgentSession, SessionStatus};
use crate::review_inbox::{self, Running};

/// Settings key for sessions without a D.Duck record that were wrapped up (session keys, newest last).
pub const WRAPPED_KEYS: &str = "wrapped_keys";
const KEEP_WRAPPED_KEYS: usize = 200;
/// Cards per column; the header shows the full total.
pub const MAX_CARDS: usize = 40;
const READ_BOUND: i64 = 5000;
const MAX_ROWS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lane {
    /// 리뷰 대기: the same rows as the review tab's "리뷰 전" (the badge).
    ReviewWait,
    /// 정리: reviewed and not wrapped up, or never a review target.
    Wrapup,
    /// 정리 끝: off the board, "완료" elsewhere.
    Done,
}

/// What the lane rule needs from a D.Duck record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub has_report: bool,
    pub reviewed: bool,
    pub wrapped: bool,
    pub created_at: i64,
}

/// The lane of an ended task (see the module docs). `None` = 확인 필요.
pub fn lane_of(failed: bool, rec: Option<&Facts>, external_wrapped: bool, since: i64) -> Option<Lane> {
    match rec {
        Some(f) => {
            if f.has_report && !f.reviewed && f.created_at >= since {
                Some(Lane::ReviewWait)
            } else if f.wrapped {
                Some(Lane::Done)
            } else if failed && !f.reviewed {
                None
            } else {
                Some(Lane::Wrapup)
            }
        }
        None if external_wrapped => Some(Lane::Done),
        None if failed => None,
        None => Some(Lane::Wrapup),
    }
}

fn ended(s: &AgentSession) -> bool {
    review_inbox::running_of(s.status).is_none()
}

pub fn facts(conn: &Connection, bg_id: &str) -> Result<Option<Facts>, Error> {
    if !crate::task_meta::valid_bg_id(bg_id) {
        return Ok(None);
    }
    Ok(conn
        .query_row("SELECT report_id, reviewed_at, wrapped_at, created_at FROM task_meta WHERE bg_id = ?1", [bg_id], |r| {
            Ok(Facts {
                has_report: r.get::<_, Option<String>>(0)?.is_some_and(|id| crate::task_report::valid_id(&id)),
                reviewed: r.get::<_, Option<i64>>(1)?.is_some(),
                wrapped: r.get::<_, Option<i64>>(2)?.is_some(),
                created_at: r.get(3)?,
            })
        })
        .optional()?)
}

fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 128 && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':')
}

fn wrapped_keys(conn: &Connection) -> Result<Vec<String>, Error> {
    let raw = crate::settings::get(conn, WRAPPED_KEYS)?;
    let list: Vec<String> = raw.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
    Ok(list.into_iter().filter(|k| valid_key(k)).collect())
}

fn put_wrapped_keys(conn: &Connection, keys: &[String]) -> Result<(), Error> {
    let start = keys.len().saturating_sub(KEEP_WRAPPED_KEYS);
    let json = serde_json::to_string(&keys[start..]).map_err(|_| Error::Config("wrapped keys"))?;
    crate::settings::put_guarded(conn, WRAPPED_KEYS, &json)
}

/// Lanes of the listed sessions that have ended, by session key (for the agents payload and the tray).
pub fn lanes(conn: &Connection, sessions: &[AgentSession], since: i64) -> Result<HashMap<String, Lane>, Error> {
    let external: HashSet<String> = wrapped_keys(conn)?.into_iter().collect();
    // One query for every listed id (not one per session).
    let ids: Vec<&str> = sessions.iter().filter(|s| ended(s)).filter_map(|s| s.id.as_deref()).filter(|id| crate::task_meta::valid_bg_id(id)).collect();
    let mut by_id: HashMap<String, Facts> = HashMap::new();
    if !ids.is_empty() {
        let json = serde_json::to_string(&ids).map_err(|_| Error::Config("ids"))?;
        let mut stmt = conn.prepare("SELECT bg_id, report_id, reviewed_at, wrapped_at, created_at FROM task_meta WHERE bg_id IN (SELECT value FROM json_each(?1))")?;
        let rows = stmt.query_map([json], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Facts {
                    has_report: r.get::<_, Option<String>>(1)?.is_some_and(|id| crate::task_report::valid_id(&id)),
                    reviewed: r.get::<_, Option<i64>>(2)?.is_some(),
                    wrapped: r.get::<_, Option<i64>>(3)?.is_some(),
                    created_at: r.get(4)?,
                },
            ))
        })?;
        by_id.extend(rows.filter_map(Result::ok));
    }
    let mut out = HashMap::new();
    for s in sessions.iter().filter(|s| ended(s)) {
        let rec = s.id.as_deref().and_then(|id| by_id.get(id)).copied();
        if let Some(lane) = lane_of(s.status == SessionStatus::Failed, rec.as_ref(), external.contains(&s.key), since) {
            out.insert(s.key.clone(), lane);
        }
    }
    Ok(out)
}

/// 확인 필요: waiting for an answer, or failed without a lane (the sidebar's task badge and the tray number).
pub fn needs_check(sessions: &[AgentSession], lanes: &HashMap<String, Lane>) -> usize {
    sessions
        .iter()
        .filter(|s| s.status == SessionStatus::WaitingInput || (s.status == SessionStatus::Failed && !lanes.contains_key(&s.key)))
        .count()
}

/// Tray tooltip: "D.Duck — 확인 필요 N · 리뷰 대기 M", leaving out a zero side.
pub fn tooltip(check: usize, pending: u32, lang: crate::i18n::Lang) -> String {
    use crate::i18n::{Code, Params, msg};
    let mut parts = Vec::new();
    if check > 0 {
        parts.push(msg(lang, Code::TrayCheck, &Params::new().num("count", check as i64)));
    }
    if pending > 0 {
        parts.push(msg(lang, Code::TrayPending, &Params::new().num("count", i64::from(pending))));
    }
    if parts.is_empty() { "D.Duck".to_string() } else { format!("D.Duck — {}", parts.join(" · ")) }
}

/// Finished-task notification: "리뷰 대기" only when the task lands in that lane (`lane_of`).
pub fn finished_body(name: &str, waits_for_review: bool, lang: crate::i18n::Lang) -> String {
    use crate::i18n::{Code, Params, msg};
    msg(lang, if waits_for_review { Code::NotifyFinishedReview } else { Code::NotifyFinished }, &Params::new().text("name", name, 64))
}

/// Failed-task notification: the task name only.
pub fn failed_body(name: &str, lang: crate::i18n::Lang) -> String {
    crate::i18n::msg(lang, crate::i18n::Code::NotifyFailed, &crate::i18n::Params::new().text("name", name, 64))
}

/// The `ended_status` code of a listed session: how it ended, or `None` while it still runs.
fn ended_code(s: SessionStatus) -> Option<&'static str> {
    match s {
        SessionStatus::Done => Some("done"),
        SessionStatus::Failed => Some("failed"),
        SessionStatus::Stopped => Some("stopped"),
        SessionStatus::Unknown => Some("unknown"),
        SessionStatus::Working | SessionStatus::WaitingInput | SessionStatus::Idle => None,
    }
}

const ENDED_CODES: [&str; 4] = ["done", "failed", "stopped", "unknown"];

/// Keeps how each listed D.Duck task ended (`task_meta.ended_status`), so its record card still says "실패" or
/// "멈췄어요" after the CLI forgets it. A running task clears it. Writes only rows whose value changes.
pub fn note_ended(conn: &Connection, sessions: &[AgentSession]) -> Result<usize, Error> {
    let mut stmt = conn.prepare_cached("UPDATE task_meta SET ended_status = ?2 WHERE bg_id = ?1 AND ended_status IS NOT ?2")?;
    let mut changed = 0;
    for s in sessions {
        let Some(id) = s.id.as_deref().filter(|id| crate::task_meta::valid_bg_id(id)) else { continue };
        changed += stmt.execute(params![id, ended_code(s.status)])?;
    }
    Ok(changed)
}

// ---------- board ----------

/// A D.Duck record on the board: what a card shows, no paths beyond the last folder name, no instruction text but
/// the task name (or the brief goal's first line).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardCard {
    pub bg_id: String,
    pub name: Option<String>,
    pub folder: Option<String>,
    pub task_type: Option<String>,
    pub created_at: i64,
    pub reviewed: bool,
    /// The CLI still lists it (the webview draws the live session's card instead).
    pub listed: bool,
    /// How it last ended while listed (`done`/`failed`/`stopped`/`unknown`); `None` = never seen ended.
    pub ended: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    /// Every matching record (the review column's total is the badge).
    pub total: usize,
    pub items: Vec<BoardCard>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    /// "리뷰 전" records, listed or not: total = `review_inbox::pending_count`.
    pub review_wait: Column,
    /// Reviewed, not wrapped up, not running and not listed (listed ones come with the live sessions).
    pub wrapup: Column,
}

struct Rec {
    bg_id: String,
    name: Option<String>,
    cwd: Option<String>,
    task_type: Option<String>,
    created_at: i64,
    reviewed: bool,
    report_ok: bool,
    goal: Option<String>,
    ended: Option<String>,
}

fn name_of(name: Option<String>, brief: Option<String>) -> Option<String> {
    let n = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let goal = || {
        let b: crate::team::Brief = serde_json::from_str(&brief?).ok()?;
        b.goal.split(['\n', '\r']).map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
    };
    n.or_else(goal).map(|s| crate::text::display_safe(&s, 64))
}

fn read_recs(conn: &Connection, sql: &str, since: Option<i64>) -> Result<Vec<Rec>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let map = |r: &rusqlite::Row<'_>| {
        Ok(Rec {
            bg_id: r.get(0)?,
            name: r.get(1)?,
            cwd: r.get(2)?,
            task_type: r.get(3)?,
            created_at: r.get(4)?,
            reviewed: r.get::<_, Option<i64>>(5)?.is_some(),
            report_ok: r.get::<_, Option<String>>(6)?.is_some_and(|id| crate::task_report::valid_id(&id)),
            goal: r.get(7)?,
            ended: r.get(8)?,
        })
    };
    let rows: Vec<Rec> = match since {
        Some(s) => stmt.query_map(params![READ_BOUND, s], map)?.filter_map(Result::ok).collect(),
        None => stmt.query_map(params![READ_BOUND], map)?.filter_map(Result::ok).collect(),
    };
    Ok(rows.into_iter().filter(|r| crate::task_meta::valid_bg_id(&r.bg_id)).collect())
}

fn card(r: Rec, listed: &HashSet<&str>) -> BoardCard {
    BoardCard {
        listed: listed.contains(r.bg_id.as_str()),
        folder: r.cwd.as_deref().and_then(|c| Path::new(c).file_name()).map(|n| crate::text::display_safe(&n.to_string_lossy(), 64)),
        name: name_of(r.name, r.goal),
        bg_id: r.bg_id,
        task_type: r.task_type,
        created_at: r.created_at,
        reviewed: r.reviewed,
        ended: r.ended.as_deref().and_then(|e| ENDED_CODES.into_iter().find(|c| *c == e)),
    }
}

/// The two DB-backed columns. Same rows, order, id checks and 500 cap as `review_inbox::pending_count`.
/// `wrapup_cards` is how many 정리 cards to return ("더 보기" asks for more), clamped to 40..=500.
pub fn board(conn: &Connection, sessions: &[AgentSession], since: i64, wrapup_cards: usize) -> Result<Board, Error> {
    let running: HashMap<String, Running> = review_inbox::running_map(sessions);
    let listed: HashSet<&str> = sessions.iter().filter_map(|s| s.id.as_deref()).collect();
    let pending: Vec<Rec> = read_recs(
        conn,
        "SELECT bg_id, name, cwd, task_type, created_at, reviewed_at, report_id, brief_json, ended_status FROM task_meta
         WHERE report_id IS NOT NULL ORDER BY created_at DESC, bg_id LIMIT ?1",
        None,
    )?
    .into_iter()
    .filter(|r| r.report_ok)
    .take(MAX_ROWS)
    .filter(|r| review_inbox::is_pending(&r.bg_id, r.reviewed, r.created_at, &running, since))
    .collect();
    let review_wait = Column { total: pending.len(), items: pending.into_iter().take(MAX_CARDS).map(|r| card(r, &listed)).collect() };
    let wrapped: Vec<Rec> = read_recs(
        conn,
        "SELECT bg_id, name, cwd, task_type, created_at, reviewed_at, report_id, brief_json, ended_status FROM task_meta
         WHERE reviewed_at IS NOT NULL AND wrapped_at IS NULL AND created_at >= ?2 ORDER BY created_at DESC, bg_id LIMIT ?1",
        Some(since),
    )?
    .into_iter()
    .filter(|r| !running.contains_key(&r.bg_id) && !listed.contains(r.bg_id.as_str()))
    .collect();
    let cap = wrapup_cards.clamp(MAX_CARDS, MAX_ROWS);
    let wrapup = Column { total: wrapped.len(), items: wrapped.into_iter().take(cap).map(|r| card(r, &listed)).collect() };
    Ok(Board { review_wait, wrapup })
}

// ---------- 정리 끝 ----------

/// A usage-limit resume is still planned for this task (`limit_resumes` auto·manual·needs_choice·starting): it will
/// run again, so it can be neither wrapped up nor have its worktree removed (5.2).
pub fn resume_pending(conn: &Connection, bg_id: &str) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM limit_resumes WHERE bg_id = ?1 AND state IN ('auto', 'manual', 'needs_choice', 'starting'))",
        [bg_id],
        |r| r.get::<_, bool>(0),
    )?)
}

/// `wrap` refused because a usage-limit resume is planned (the command shows `Fail::WrapResumePending`).
pub const RESUME_PENDING: &str = "resume pending";

fn resolve<'a>(sessions: &'a [AgentSession], id: &str) -> (Option<&'a AgentSession>, Option<String>) {
    let listed = sessions.iter().find(|s| s.key == id || s.id.as_deref() == Some(id));
    let bg = listed.and_then(|s| s.id.clone()).or_else(|| crate::task_meta::valid_bg_id(id).then(|| id.to_string()));
    (listed, bg)
}

/// "정리 끝": only an ended task that is reviewed or was never a review target. `id` is a session key (live card) or
/// a background id (record card). Errors name no ids.
pub fn wrap(conn: &Connection, id: &str, sessions: &[AgentSession], known: bool, since: i64, now_ms: i64) -> Result<(), Error> {
    if !known {
        return Err(Error::Config("running"));
    }
    let (listed, bg) = resolve(sessions, id);
    if listed.is_some_and(|s| !ended(s)) {
        return Err(Error::Config("running"));
    }
    if let Some(bg) = bg.as_deref() {
        if resume_pending(conn, bg)? {
            return Err(Error::Config(RESUME_PENDING));
        }
    }
    if let Some(bg) = bg {
        if let Some(f) = facts(conn, &bg)? {
            if review_inbox::running_map(sessions).contains_key(&bg) {
                return Err(Error::Config("running"));
            }
            if lane_of(false, Some(&f), false, since) == Some(Lane::ReviewWait) {
                return Err(Error::Config("review pending"));
            }
            conn.execute("UPDATE task_meta SET wrapped_at = ?2 WHERE bg_id = ?1 AND wrapped_at IS NULL", params![bg, now_ms])?;
            return Ok(());
        }
    }
    let s = listed.ok_or(Error::Config("unknown task"))?;
    if !valid_key(&s.key) {
        return Err(Error::Config("task id"));
    }
    let mut keys = wrapped_keys(conn)?;
    keys.retain(|k| k != &s.key);
    keys.push(s.key.clone());
    put_wrapped_keys(conn, &keys)
}

/// Where "끝내고 넘기기" sent a task (D-167 behaviour, D-190 name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Finished {
    /// It has a report nobody reviewed yet: 리뷰 대기, as any ended task with a report.
    ReviewWait,
    /// Wrapped up: off the board.
    Done,
}

/// "끝내고 넘기기" (D-190 name) for a background task the caller has just stopped (D-167): the same lane rule as an ended task —
/// an unreviewed report goes to 리뷰 대기 (the review gate is never skipped), anything else is wrapped up at once.
/// `key` is the session key (for a task without a D.Duck record).
pub fn finish_stopped(conn: &Connection, bg_id: &str, key: &str, since: i64, now_ms: i64) -> Result<Finished, Error> {
    if let Some(f) = facts(conn, bg_id)? {
        if lane_of(false, Some(&f), false, since) == Some(Lane::ReviewWait) {
            return Ok(Finished::ReviewWait);
        }
        conn.execute("UPDATE task_meta SET wrapped_at = ?2 WHERE bg_id = ?1 AND wrapped_at IS NULL", params![bg_id, now_ms])?;
        return Ok(Finished::Done);
    }
    if !valid_key(key) {
        return Err(Error::Config("task id"));
    }
    let mut keys = wrapped_keys(conn)?;
    keys.retain(|k| k != key);
    keys.push(key.to_string());
    put_wrapped_keys(conn, &keys)?;
    Ok(Finished::Done)
}

/// "되돌리기" within 5 seconds of "정리 끝". Unknown ids → false.
pub fn unwrap(conn: &Connection, id: &str, sessions: &[AgentSession]) -> Result<bool, Error> {
    let (listed, bg) = resolve(sessions, id);
    let mut changed = false;
    if let Some(bg) = bg {
        changed |= conn.execute("UPDATE task_meta SET wrapped_at = NULL WHERE bg_id = ?1", [bg])? > 0;
    }
    let key = listed.map(|s| s.key.as_str()).unwrap_or(id);
    let mut keys = wrapped_keys(conn)?;
    let before = keys.len();
    keys.retain(|k| k != key);
    if keys.len() != before {
        put_wrapped_keys(conn, &keys)?;
        changed = true;
    }
    Ok(changed)
}

// ---------- 정리 점검표 ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Worktree {
    /// "worktree가 남아 있어요 · <폴더 이름> · <브랜치>"
    Present { folder: String, branch: Option<String> },
    /// "worktree는 정리됐어요"
    Gone,
    /// The folder is a link, outside `<repo>/.claude/worktrees/`, or not known.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrLine {
    pub number: u64,
    /// Matches `^https://github.com/<owner>/<repo>/pull/<n>$`; opened only after the usual confirm.
    pub url: String,
}

/// Why "worktree 정리" is off. The webview maps each to one fixed line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Refusal {
    Running,
    Changes,
    NotChecked,
    NoWorktree,
    /// A usage-limit resume is planned for the task.
    ResumePending,
}

impl Refusal {
    /// The fixed line the shell shows for a refused "worktree 정리". `NotChecked` is a refusal before any delete,
    /// never "worktree를 지우지 못했어요".
    pub fn fail(self) -> crate::i18n::Fail {
        use crate::i18n::Fail;
        match self {
            Refusal::Running => Fail::WrapRunning,
            Refusal::Changes => Fail::WrapChanges,
            Refusal::NotChecked => Fail::WrapNotChecked,
            Refusal::NoWorktree => Fail::NoWorktree,
            Refusal::ResumePending => Fail::WrapResumePending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// `None` = not a worktree task.
    pub worktree: Option<Worktree>,
    /// Uncommitted changes (tracked and untracked), a number only. `None` = not checked (no repo, git failed).
    pub changes: Option<usize>,
    pub pr: Option<PrLine>,
    /// `local_only` / `draft_pr`, `None` for older tasks.
    pub guard: Option<&'static str>,
    /// `None` = "worktree 정리" can run.
    pub refusal: Option<Refusal>,
    /// `Some(ResumePending)` = "정리 끝" is off too (a usage-limit resume is planned).
    pub wrap_refusal: Option<Refusal>,
}

static PR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\Ahttps://github\.com/[\w.-]+/[\w.-]+/pull/(\d{1,9})\z").unwrap_or_else(|_| unreachable!()));

pub fn pr_of(urls: impl IntoIterator<Item = String>) -> Option<PrLine> {
    urls.into_iter().find_map(|u| {
        let n = PR_RE.captures(&u)?.get(1)?.as_str().parse().ok()?;
        Some(PrLine { number: n, url: u })
    })
}

/// A task's record for the checklist, read under the DB lock; git runs after the lock is released.
pub struct Record {
    cwd: Option<String>,
    worktree: bool,
    guard: Option<String>,
    report_id: Option<String>,
    session_cwd: Option<String>,
    /// A usage-limit resume is planned (`resume_pending`).
    pub resume_pending: bool,
}

pub fn record(conn: &Connection, bg_id: &str) -> Result<Option<Record>, Error> {
    if !crate::task_meta::valid_bg_id(bg_id) {
        return Err(Error::Config("task id"));
    }
    Ok(conn
        .query_row(
            "SELECT t.cwd, t.worktree, t.delivery_guard, t.report_id, s.project_path,
                    EXISTS(SELECT 1 FROM limit_resumes l WHERE l.bg_id = t.bg_id AND l.state IN ('auto', 'manual', 'needs_choice', 'starting'))
             FROM task_meta t LEFT JOIN sessions s ON s.session_id = t.session_id WHERE t.bg_id = ?1",
            [bg_id],
            |r| {
                Ok(Record {
                    cwd: r.get(0)?,
                    worktree: r.get::<_, i64>(1)? != 0,
                    guard: r.get(2)?,
                    report_id: r.get(3)?,
                    session_cwd: r.get(4)?,
                    resume_pending: r.get(5)?,
                })
            },
        )
        .optional()?)
}

enum Located {
    Present(PathBuf),
    Gone,
    Unknown,
}

/// The worktree folder of a task started in `start` (D-141 (2)): the session folder must be a direct child of
/// `<start>/.claude/worktrees/`, a real folder, not a link.
fn locate(start: &Path, session_cwd: Option<&Path>) -> Located {
    let Some(sc) = session_cwd.filter(|p| p.is_absolute()) else { return Located::Unknown };
    let Some(name) = sc.file_name().filter(|n| Path::new(n).components().count() == 1 && *n != "..") else { return Located::Unknown };
    let Some(parent) = sc.parent() else { return Located::Unknown };
    let Ok(start) = std::fs::canonicalize(start) else { return Located::Unknown };
    let base_lex = start.join(".claude").join("worktrees");
    // The worktrees folder must be the real one: no link anywhere under the repository (a linked `.claude` fails).
    let base = match std::fs::canonicalize(&base_lex) {
        Ok(b) if b == base_lex => b,
        Ok(_) => return Located::Unknown,
        // The whole folder is gone: only when the session folder was exactly there.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && parent == base_lex => return Located::Gone,
        Err(_) => return Located::Unknown,
    };
    let parent_real = std::fs::canonicalize(parent).ok();
    if parent_real.as_deref() != Some(base.as_path()) && parent != base {
        return Located::Unknown;
    }
    let path = base.join(name);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Located::Gone,
        Ok(m) if m.file_type().is_dir() => {
            let real_ok = std::fs::canonicalize(&path).is_ok_and(|r| r == path && r.starts_with(&base) && r != start);
            if real_ok && listed_worktree(&start, &path) { Located::Present(path) } else { Located::Unknown }
        }
        _ => Located::Unknown,
    }
}

/// `git worktree list --porcelain` of the repository names this folder.
fn listed_worktree(repo: &Path, path: &Path) -> bool {
    let Some(out) = git_out(repo, &["worktree", "list", "--porcelain"], 2, 256 * 1024) else { return false };
    out.lines().filter_map(|l| l.strip_prefix("worktree ")).any(|w| std::fs::canonicalize(w).is_ok_and(|w| w == path))
}

fn git_out(dir: &Path, args: &[&str], secs: u64, cap: u64) -> Option<String> {
    let mut cmd = crate::delivery_guard::app_git(dir);
    cmd.args(args);
    let out = crate::process::run(&mut cmd, Duration::from_secs(secs), cap, crate::process::Keep::Head, 4 * 1024).ok()?;
    if !out.status.success() || out.stdout.len() as u64 >= cap {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `status --porcelain=v1 -uall`: lines only, names dropped right here.
fn count_changes(dir: &Path, secs: u64) -> Option<usize> {
    git_out(dir, &["status", "--porcelain=v1", "-uall"], secs, 8 * 1024 * 1024).map(|s| s.lines().filter(|l| !l.is_empty()).count())
}

fn branch_of(dir: &Path) -> Option<String> {
    let b = git_out(dir, &["rev-parse", "--abbrev-ref", "HEAD"], 2, 4096)?;
    let b = b.trim();
    (b != "HEAD" && crate::delivery_guard::valid_branch(b)).then(|| crate::text::display_safe(b, 100))
}

fn folder_name(p: &Path) -> String {
    p.file_name().map(|n| crate::text::display_safe(&n.to_string_lossy(), 64)).unwrap_or_default()
}

/// The checklist, read when a 정리 card is opened. `live_cwd` is the listed session's folder, if it is listed.
pub fn check(rec: &Record, live_cwd: Option<&Path>, running: bool, reports_root: Option<&Path>) -> Check {
    let guard = rec.guard.as_deref().and_then(crate::delivery_guard::Guard::from_code).map(crate::delivery_guard::Guard::code);
    let pr = match (reports_root, rec.report_id.as_deref()) {
        (Some(root), Some(rid)) if crate::task_report::valid_id(rid) => {
            crate::task_report::read(root, rid).ok().and_then(|r| pr_of(r.references.into_iter().map(|x| x.target)))
        }
        _ => None,
    };
    let start = rec.cwd.as_deref().map(PathBuf::from);
    let session_cwd = live_cwd.map(Path::to_path_buf).or_else(|| rec.session_cwd.as_deref().map(PathBuf::from));
    let (worktree, changes, refusal) = if rec.worktree {
        match start.as_deref().map(|s| locate(s, session_cwd.as_deref())).unwrap_or(Located::Unknown) {
            Located::Present(p) => {
                let changes = count_changes(&p, 2);
                let refusal = if running {
                    Some(Refusal::Running)
                } else if rec.resume_pending {
                    Some(Refusal::ResumePending)
                } else {
                    match changes {
                        Some(0) => None,
                        Some(_) => Some(Refusal::Changes),
                        None => Some(Refusal::NotChecked),
                    }
                };
                (Some(Worktree::Present { folder: folder_name(&p), branch: branch_of(&p) }), changes, refusal)
            }
            Located::Gone => (Some(Worktree::Gone), None, Some(Refusal::NoWorktree)),
            Located::Unknown => (Some(Worktree::Unknown), None, Some(Refusal::NoWorktree)),
        }
    } else {
        let changes = start.as_deref().filter(|s| s.is_absolute() && s.is_dir()).and_then(|s| count_changes(s, 2));
        (None, changes, Some(Refusal::NoWorktree))
    };
    let wrap_refusal = rec.resume_pending.then_some(Refusal::ResumePending);
    Check { worktree, changes, pr, guard, refusal, wrap_refusal }
}

// ---------- worktree 정리 ----------

/// What the native confirm shows and what the removal runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePlan {
    pub repo: PathBuf,
    pub path: PathBuf,
    pub folder: String,
    pub branch: Option<String>,
    /// Ignored files git deletes with the worktree (`!!` lines). `None` = too many to count in time.
    pub ignored: Option<usize>,
    /// Where the task started and the session folder, to locate the worktree again after the confirm.
    start: PathBuf,
    session: Option<PathBuf>,
}

impl RemovePlan {
    pub fn confirm_text(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::{Code, Params, msg, text};
        let mut lines = vec![text(lang, Code::WtAsk).to_string(), msg(lang, Code::WtFolder, &Params::new().text("path", &self.folder, 300))];
        if let Some(b) = &self.branch {
            lines.push(msg(lang, Code::WtBranch, &Params::new().text("branch", b, 120)));
        }
        match self.ignored {
            Some(0) => {}
            Some(n) => lines.push(msg(lang, Code::WtIgnoredCount, &Params::new().num("count", n as i64))),
            None => lines.push(text(lang, Code::WtIgnored).to_string()),
        }
        lines.push(text(lang, Code::WtKeep).to_string());
        lines.join("\n")
    }
}

fn count_ignored(dir: &Path) -> Option<usize> {
    git_out(dir, &["status", "--porcelain=v1", "--ignored", "-uall"], 10, 32 * 1024 * 1024).map(|s| s.lines().filter(|l| l.starts_with("!! ")).count())
}

/// Checks before the confirm: not running, a real worktree folder in place, no changes or untracked files.
pub fn remove_plan(rec: &Record, live_cwd: Option<&Path>, running: bool) -> Result<RemovePlan, Refusal> {
    if running {
        return Err(Refusal::Running);
    }
    if rec.resume_pending {
        return Err(Refusal::ResumePending);
    }
    if !rec.worktree {
        return Err(Refusal::NoWorktree);
    }
    let start = rec.cwd.as_deref().map(PathBuf::from).ok_or(Refusal::NoWorktree)?;
    let session_cwd = live_cwd.map(Path::to_path_buf).or_else(|| rec.session_cwd.as_deref().map(PathBuf::from));
    let Located::Present(path) = locate(&start, session_cwd.as_deref()) else { return Err(Refusal::NoWorktree) };
    match count_changes(&path, 10) {
        Some(0) => {}
        Some(_) => return Err(Refusal::Changes),
        None => return Err(Refusal::NotChecked),
    }
    let repo = std::fs::canonicalize(&start).map_err(|_| Refusal::NoWorktree)?;
    Ok(RemovePlan { folder: folder_name(&path), branch: branch_of(&path), ignored: count_ignored(&path), repo, path, start, session: session_cwd })
}

/// After the confirm: checks the changes once more, then `git worktree remove <path>` (no `--force`, 10 s). The
/// branch stays.
pub fn remove(plan: &RemovePlan) -> Result<(), Refusal> {
    // The folder may have been swapped while the dialog was open: it must still be the same real worktree.
    match locate(&plan.start, plan.session.as_deref()) {
        Located::Present(p) if p == plan.path => {}
        _ => return Err(Refusal::NoWorktree),
    }
    match count_changes(&plan.path, 10) {
        Some(0) => {}
        Some(_) => return Err(Refusal::Changes),
        None => return Err(Refusal::NotChecked),
    }
    let mut cmd = crate::delivery_guard::app_git(&plan.repo);
    cmd.args(["worktree", "remove"]).arg(&plan.path);
    let out = crate::process::run(&mut cmd, Duration::from_secs(10), 64 * 1024, crate::process::Keep::Head, 4 * 1024).map_err(|_| Refusal::NotChecked)?;
    if out.status.success() { Ok(()) } else { Err(Refusal::NotChecked) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::claude_cli::SessionKind;
    use crate::task_meta::{NewRecord, record_start};
    use std::process::Command;

    const SINCE: i64 = 1_800_000_000_000;

    fn rec(conn: &Connection, bg: &str, cwd: &str, report: Option<&str>, at: i64, worktree: bool) {
        record_start(
            conn,
            &NewRecord {
                bg_id: bg,
                name: Some("작업 이름"),
                cwd,
                agent: None,
                task_type: Some("dev"),
                prompt: None,
                brief: None,
                effort: None,
                worktree,
                permission_mode: None,
                report_id: report,
                origin: None,
                loadout_dropped: 0,
                now_ms: at,
            },
        )
        .unwrap();
    }

    fn session(key: &str, id: Option<&str>, status: SessionStatus) -> AgentSession {
        AgentSession { key: key.into(), pid: None, id: id.map(Into::into), session_id: None, name: key.into(), cwd: "/tmp".into(), kind: SessionKind::Background, started_at: 0, status }
    }

    fn rid(n: u8) -> String {
        format!("{n:032x}")
    }

    fn reviewed(conn: &Connection, bg: &str) {
        crate::task_meta::mark_reviewed(conn, bg, SINCE + 50).unwrap();
    }

    #[test]
    fn finish_stopped_keeps_the_review_gate() {
        let conn = crate::db::open_in_memory().unwrap();
        let rec = |bg: &str, report: Option<&str>| {
            conn.execute("INSERT INTO task_meta(bg_id, created_at, report_id) VALUES (?1, ?2, ?3)", params![bg, SINCE + 1, report]).unwrap();
        };
        rec("aaaa1111", Some("0123456789abcdef0123456789abcdef"));
        rec("bbbb2222", None);
        assert_eq!(finish_stopped(&conn, "aaaa1111", "k1", SINCE, SINCE + 9).unwrap(), Finished::ReviewWait, "unreviewed report waits for review");
        assert!(!facts(&conn, "aaaa1111").unwrap().unwrap().wrapped);
        assert_eq!(finish_stopped(&conn, "bbbb2222", "k2", SINCE, SINCE + 9).unwrap(), Finished::Done);
        assert!(facts(&conn, "bbbb2222").unwrap().unwrap().wrapped);
        conn.execute("UPDATE task_meta SET reviewed_at = 1 WHERE bg_id = 'aaaa1111'", []).unwrap();
        assert_eq!(finish_stopped(&conn, "aaaa1111", "k1", SINCE, SINCE + 9).unwrap(), Finished::Done, "reviewed → done");
        // No D.Duck record (started elsewhere): wrapped by session key.
        assert_eq!(finish_stopped(&conn, "cccc3333", "sess-key-1", SINCE, SINCE + 9).unwrap(), Finished::Done);
        assert!(wrapped_keys(&conn).unwrap().contains(&"sess-key-1".to_string()));
        assert!(finish_stopped(&conn, "dddd4444", "bad key!", SINCE, SINCE + 9).is_err());
    }

    #[test]
    fn lane_rule_in_order() {
        let f = |has_report, reviewed, wrapped, created_at| Facts { has_report, reviewed, wrapped, created_at };
        assert_eq!(lane_of(false, Some(&f(true, false, false, SINCE)), false, SINCE), Some(Lane::ReviewWait));
        assert_eq!(lane_of(true, Some(&f(true, false, false, SINCE)), false, SINCE), Some(Lane::ReviewWait), "failed with a report waits for review");
        assert_eq!(lane_of(true, Some(&f(false, false, false, SINCE)), false, SINCE), None, "failed without a report: 확인 필요");
        assert_eq!(lane_of(true, Some(&f(true, false, false, SINCE - 1)), false, SINCE), None, "failed before review_since: 확인 필요");
        assert_eq!(lane_of(false, Some(&f(true, false, false, SINCE - 1)), false, SINCE), Some(Lane::Wrapup), "old task goes straight to 정리");
        assert_eq!(lane_of(false, Some(&f(true, true, false, SINCE)), false, SINCE), Some(Lane::Wrapup));
        assert_eq!(lane_of(true, Some(&f(true, true, false, SINCE)), false, SINCE), Some(Lane::Wrapup), "reviewed failure can be wrapped up");
        assert_eq!(lane_of(false, Some(&f(true, true, true, SINCE)), false, SINCE), Some(Lane::Done));
        assert_eq!(lane_of(false, None, false, SINCE), Some(Lane::Wrapup), "external session: 리뷰 없이 끝났어요");
        assert_eq!(lane_of(true, None, false, SINCE), None);
        assert_eq!(lane_of(false, None, true, SINCE), Some(Lane::Done));
    }

    #[test]
    fn review_column_equals_the_badge_with_failed_and_stopped_tasks() {
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "aaaaaaa1", "/tmp", Some(&rid(1)), SINCE + 1, false); // done, listed
        rec(&conn, "aaaaaaa2", "/tmp", Some(&rid(2)), SINCE + 2, false); // failed, listed
        rec(&conn, "aaaaaaa3", "/tmp", Some(&rid(3)), SINCE + 3, false); // stopped, listed
        rec(&conn, "aaaaaaa4", "/tmp", Some(&rid(4)), SINCE + 4, false); // gone from the listing
        rec(&conn, "aaaaaaa5", "/tmp", Some(&rid(5)), SINCE + 5, false); // still working
        rec(&conn, "aaaaaaa6", "/tmp", Some(&rid(6)), SINCE - 1, false); // before review_since
        rec(&conn, "aaaaaaa7", "/tmp", None, SINCE + 7, false); // failed, no report
        let sessions = vec![
            session("k1", Some("aaaaaaa1"), SessionStatus::Done),
            session("k2", Some("aaaaaaa2"), SessionStatus::Failed),
            session("k3", Some("aaaaaaa3"), SessionStatus::Stopped),
            session("k5", Some("aaaaaaa5"), SessionStatus::Working),
            session("k6", Some("aaaaaaa6"), SessionStatus::Done),
            session("k7", Some("aaaaaaa7"), SessionStatus::Failed),
        ];
        let b = board(&conn, &sessions, SINCE, MAX_CARDS).unwrap();
        let badge = review_inbox::pending_count(&conn, &review_inbox::running_map(&sessions), SINCE).unwrap();
        assert_eq!(b.review_wait.total, badge as usize);
        assert_eq!(b.review_wait.total, 4);
        let ids: Vec<_> = b.review_wait.items.iter().map(|c| (c.bg_id.as_str(), c.listed)).collect();
        assert_eq!(ids, [("aaaaaaa4", false), ("aaaaaaa3", true), ("aaaaaaa2", true), ("aaaaaaa1", true)]);
        let l = lanes(&conn, &sessions, SINCE).unwrap();
        assert_eq!(l.get("k2"), Some(&Lane::ReviewWait), "failed report task is not in 확인 필요");
        assert_eq!(l.get("k3"), Some(&Lane::ReviewWait));
        assert_eq!(l.get("k6"), Some(&Lane::Wrapup), "first run: old tasks do not flood 리뷰 대기");
        assert_eq!(l.get("k7"), None);
        assert!(!l.contains_key("k5"));
        assert_eq!(needs_check(&sessions, &l), 1, "only the failed task without a report");
        // Every listed ended session lands in exactly one lane, and none in two.
        assert!(b.wrapup.items.is_empty());
        let r_badge = badge;
        assert_eq!(tooltip(needs_check(&sessions, &l), r_badge, crate::i18n::Lang::Ko), "D.Duck — 확인 필요 1 · 리뷰 대기 4");
    }

    #[test]
    fn many_pending_cards_are_capped_but_the_total_is_not() {
        let conn = crate::db::open_in_memory().unwrap();
        for i in 0..45u8 {
            rec(&conn, &format!("bbbbbb{i:02x}"), "/tmp", Some(&rid(i + 1)), SINCE + i64::from(i), false);
        }
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        assert_eq!(b.review_wait.total, 45);
        assert_eq!(b.review_wait.items.len(), MAX_CARDS);
        assert_eq!(b.review_wait.total, review_inbox::pending_count(&conn, &HashMap::new(), SINCE).unwrap() as usize);
    }

    #[test]
    fn a_record_card_keeps_how_the_task_ended_after_it_leaves_the_listing() {
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "eeeeeee1", "/tmp", Some(&rid(1)), SINCE + 1, false);
        rec(&conn, "eeeeeee2", "/tmp", Some(&rid(2)), SINCE + 2, false);
        rec(&conn, "eeeeeee3", "/tmp", Some(&rid(3)), SINCE + 3, false);
        rec(&conn, "eeeeeee4", "/tmp", Some(&rid(4)), SINCE + 4, false); // never seen ended
        let listed = [
            session("k1", Some("eeeeeee1"), SessionStatus::Failed),
            session("k2", Some("eeeeeee2"), SessionStatus::Stopped),
            session("k3", Some("eeeeeee3"), SessionStatus::Done),
        ];
        assert_eq!(note_ended(&conn, &listed).unwrap(), 3);
        assert_eq!(note_ended(&conn, &listed).unwrap(), 0, "unchanged rows are not written again");
        // The CLI forgot them: the cards still say how each ended.
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        let ended: HashMap<_, _> = b.review_wait.items.iter().map(|c| (c.bg_id.as_str(), c.ended)).collect();
        assert_eq!(ended["eeeeeee1"], Some("failed"));
        assert_eq!(ended["eeeeeee2"], Some("stopped"));
        assert_eq!(ended["eeeeeee3"], Some("done"));
        assert_eq!(ended["eeeeeee4"], None);
        // Resumed: running again clears it; anything not in the known set is dropped on read.
        note_ended(&conn, &[session("k1", Some("eeeeeee1"), SessionStatus::Working)]).unwrap();
        conn.execute("UPDATE task_meta SET ended_status = 'weird' WHERE bg_id = 'eeeeeee2'", []).unwrap();
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        assert!(b.review_wait.items.iter().all(|c| c.bg_id != "eeeeeee1" || c.ended.is_none()));
        assert!(b.review_wait.items.iter().all(|c| c.bg_id != "eeeeeee2" || c.ended.is_none()));
    }

    #[test]
    fn thirty_old_report_tasks_do_not_flood_review_wait_and_only_the_last_24_hours_show() {
        let conn = crate::db::open_in_memory().unwrap();
        let now = SINCE + 1000;
        let day = 24 * 3_600_000;
        let mut listing = Vec::new();
        for i in 0..30u8 {
            let bg = format!("0dd0dd{i:02x}");
            rec(&conn, &bg, "/tmp", Some(&rid(i + 1)), SINCE - 1 - i64::from(i), false); // before review_since
            let key = format!("old-{i}");
            // Half ended more than 24 hours ago (first seen then), half within the last 24 hours.
            let seen = if i % 2 == 0 { now - day - 1 } else { now - 60_000 };
            conn.execute("INSERT INTO session_observations(session_id, ended_observed_at) VALUES (?1, ?2)", params![key, seen]).unwrap();
            listing.push(session(&key, Some(&bg), if i % 3 == 0 { SessionStatus::Stopped } else { SessionStatus::Done }));
        }
        let visible = crate::tracker::visible_sessions(&conn, listing, now).unwrap();
        assert_eq!(visible.len(), 15, "only the last 24 hours stay listed");
        let b = board(&conn, &visible, SINCE, MAX_CARDS).unwrap();
        assert_eq!(b.review_wait.total, 0, "리뷰 대기 stays empty");
        assert_eq!(b.review_wait.total as u32, review_inbox::pending_count(&conn, &review_inbox::running_map(&visible), SINCE).unwrap());
        assert_eq!(b.wrapup.total, 0, "unreviewed old records are not 정리 records");
        let l = lanes(&conn, &visible, SINCE).unwrap();
        assert_eq!(l.len(), 15);
        assert!(l.values().all(|x| *x == Lane::Wrapup), "the 24-hour ones show in 정리 as 리뷰 없이 끝났어요");
    }

    #[test]
    fn wrapup_column_returns_more_cards_on_request() {
        let conn = crate::db::open_in_memory().unwrap();
        for i in 0..45u8 {
            let bg = format!("ffffff{i:02x}");
            rec(&conn, &bg, "/tmp", Some(&rid(i + 1)), SINCE + i64::from(i), false);
            reviewed(&conn, &bg);
        }
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        assert_eq!((b.wrapup.total, b.wrapup.items.len()), (45, MAX_CARDS));
        let b = board(&conn, &[], SINCE, 80).unwrap();
        assert_eq!(b.wrapup.items.len(), 45, "더 보기 shows the rest");
        assert_eq!(board(&conn, &[], SINCE, 0).unwrap().wrapup.items.len(), MAX_CARDS, "never fewer than 40");
    }

    #[test]
    fn finished_notification_says_review_wait_only_for_tasks_in_that_lane() {
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "abab0001", "/tmp", Some(&rid(1)), SINCE + 1, false);
        rec(&conn, "abab0002", "/tmp", Some(&rid(2)), SINCE - 1, false); // before review_since
        let s = [session("k1", Some("abab0001"), SessionStatus::Done), session("k2", Some("abab0002"), SessionStatus::Done)];
        let l = lanes(&conn, &s, SINCE).unwrap();
        let body = |k: &str| finished_body("x", l.get(k) == Some(&Lane::ReviewWait), crate::i18n::Lang::Ko);
        assert_eq!(body("k1"), "오리가 끝냈어요 · 리뷰 대기: x");
        assert_eq!(body("k2"), "오리가 끝냈어요: x", "an old report task goes to 정리, not 리뷰 대기");
    }

    #[test]
    fn reviewing_moves_a_card_to_wrapup_and_wrapping_takes_it_off() {
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "ccccccc1", "/Users/me/proj", Some(&rid(1)), SINCE + 1, false);
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        assert_eq!((b.review_wait.total, b.wrapup.total), (1, 0));
        assert!(wrap(&conn, "ccccccc1", &[], true, SINCE, SINCE + 9).is_err(), "리뷰 대기 cannot be wrapped up");
        reviewed(&conn, "ccccccc1");
        let b = board(&conn, &[], SINCE, MAX_CARDS).unwrap();
        assert_eq!((b.review_wait.total, b.wrapup.total), (0, 1));
        assert_eq!(b.wrapup.items[0].folder.as_deref(), Some("proj"), "folder name only");
        assert_eq!(b.wrapup.items[0].name.as_deref(), Some("작업 이름"));
        wrap(&conn, "ccccccc1", &[], true, SINCE, SINCE + 9).unwrap();
        assert_eq!(board(&conn, &[], SINCE, MAX_CARDS).unwrap().wrapup.total, 0);
        assert!(facts(&conn, "ccccccc1").unwrap().unwrap().wrapped);
        // 5-second undo.
        assert!(unwrap(&conn, "ccccccc1", &[]).unwrap());
        assert_eq!(board(&conn, &[], SINCE, MAX_CARDS).unwrap().wrapup.total, 1);
        // Undoing the review also undoes 정리 끝: never an unreviewed "완료".
        wrap(&conn, "ccccccc1", &[], true, SINCE, SINCE + 9).unwrap();
        assert!(review_inbox::unmark(&conn, "ccccccc1").unwrap());
        let f = facts(&conn, "ccccccc1").unwrap().unwrap();
        assert!(!f.wrapped && !f.reviewed);
        assert_eq!(board(&conn, &[], SINCE, MAX_CARDS).unwrap().review_wait.total, 1);
    }

    #[test]
    fn running_tasks_and_unknown_polls_cannot_be_wrapped() {
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "ddddddd1", "/tmp", None, SINCE + 1, false);
        let live = [session("k1", Some("ddddddd1"), SessionStatus::Working)];
        assert!(wrap(&conn, "k1", &live, true, SINCE, SINCE + 2).is_err());
        assert!(wrap(&conn, "ddddddd1", &live, true, SINCE, SINCE + 2).is_err());
        let done = [session("k1", Some("ddddddd1"), SessionStatus::Done)];
        assert!(wrap(&conn, "k1", &done, false, SINCE, SINCE + 2).is_err(), "no good poll yet");
        wrap(&conn, "k1", &done, true, SINCE, SINCE + 2).unwrap();
        assert_eq!(lanes(&conn, &done, SINCE).unwrap().get("k1"), Some(&Lane::Done));
    }

    #[test]
    fn external_sessions_are_wrapped_by_key_and_kept_to_200() {
        let conn = crate::db::open_in_memory().unwrap();
        let s = [session("11111111-2222-4333-8444-555555555555", None, SessionStatus::Done)];
        assert_eq!(lanes(&conn, &s, SINCE).unwrap().get(&s[0].key), Some(&Lane::Wrapup));
        wrap(&conn, &s[0].key, &s, true, SINCE, SINCE).unwrap();
        assert_eq!(lanes(&conn, &s, SINCE).unwrap().get(&s[0].key), Some(&Lane::Done));
        assert!(unwrap(&conn, &s[0].key, &s).unwrap());
        assert_eq!(lanes(&conn, &s, SINCE).unwrap().get(&s[0].key), Some(&Lane::Wrapup));
        assert!(wrap(&conn, "not-listed", &s, true, SINCE, SINCE).is_err());
        for i in 0..210 {
            let k = format!("pid:{i}");
            let one = [session(&k, None, SessionStatus::Stopped)];
            wrap(&conn, &k, &one, true, SINCE, SINCE).unwrap();
        }
        let keys = wrapped_keys(&conn).unwrap();
        assert_eq!(keys.len(), KEEP_WRAPPED_KEYS);
        assert_eq!(keys.last().map(String::as_str), Some("pid:209"));
    }

    #[test]
    fn tooltip_and_notification_copy() {
        use crate::i18n::Lang::{En, Ko};
        assert_eq!(tooltip(0, 0, Ko), "D.Duck");
        assert_eq!(tooltip(1, 2, Ko), "D.Duck — 확인 필요 1 · 리뷰 대기 2");
        assert_eq!(tooltip(1, 1, Ko), "D.Duck — 확인 필요 1 · 리뷰 대기 1");
        assert_eq!(tooltip(0, 3, Ko), "D.Duck — 리뷰 대기 3");
        assert_eq!(finished_body("fix-auth", true, Ko), "오리가 끝냈어요 · 리뷰 대기: fix-auth");
        assert_eq!(finished_body("fix-auth", false, Ko), "오리가 끝냈어요: fix-auth");
        assert!(!finished_body("x", true, Ko).contains("완료"));
        assert_eq!(failed_body("fix-auth", Ko), "작업이 실패했어요: fix-auth");
        assert_eq!(tooltip(1, 2, En), "D.Duck — Needs you 1 · Waiting for review 2");
        assert_eq!(finished_body("fix-auth", true, En), "Your duck finished · Waiting for review: fix-auth");
        assert_eq!(failed_body("a\u{202E}b\n", En), "Task failed: ab", "name stays display-safe");
        assert_eq!(finished_body(&"x".repeat(200), false, En).chars().count(), "Your duck finished: ".len() + 64);
    }

    #[test]
    fn pr_line_takes_the_first_github_pull_url_only() {
        let urls = ["https://example.com/pull/1", "https://github.com/o/r/pull/42/files", "https://github.com/o-1/r.s/pull/42", "https://github.com/o/r/pull/7"];
        assert_eq!(pr_of(urls.iter().map(|s| s.to_string())), Some(PrLine { number: 42, url: "https://github.com/o-1/r.s/pull/42".into() }));
        assert_eq!(pr_of(["http://github.com/o/r/pull/1".to_string()]), None);
    }

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success(), "{args:?}");
    }

    /// A repo with a commit and a worktree `.claude/worktrees/fix-auth` on branch `dduck/fix-auth`.
    fn repo_with_worktree(base: &Path) -> (PathBuf, PathBuf) {
        let repo = base.join("proj");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "t@example.invalid"]);
        git(&repo, &["config", "user.name", "T"]);
        std::fs::write(repo.join(".gitignore"), "build/\n").unwrap();
        git(&repo, &["add", ".gitignore"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let wt = repo.join(".claude/worktrees/fix-auth");
        git(&repo, &["worktree", "add", "-q", "-b", "dduck/fix-auth", wt.to_str().unwrap()]);
        (repo, wt)
    }

    fn report(root: &Path, id: &str) {
        let dir = crate::task_report::prepare_dir(root, id).unwrap();
        std::fs::write(
            dir.join(crate::task_report::FILE),
            r#"{"version":1,"status":"done","work":[],"blocked":[],"decisions":[],"criteria":[],
               "references":[{"kind":"url","ref":"https://github.com/o/r/pull/42","note":""}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn checklist_reads_worktree_branch_changes_pr_and_never_runs_fsmonitor() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, wt) = repo_with_worktree(&base);
        let marker = base.join("PWNED");
        let script = base.join("fsmon.sh");
        std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        git(&repo, &["config", "core.fsmonitor", script.to_str().unwrap()]);
        let root = base.join("reports");
        report(&root, &rid(1));
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "eeeeeee1", repo.to_str().unwrap(), Some(&rid(1)), SINCE + 1, true);
        crate::task_meta::record_guard(&conn, "eeeeeee1", crate::delivery_guard::Guard::LocalOnly, None, None).unwrap();
        let c = check(&record(&conn, "eeeeeee1").unwrap().unwrap(), Some(&wt), false, Some(&root));
        assert_eq!(c.worktree, Some(Worktree::Present { folder: "fix-auth".into(), branch: Some("dduck/fix-auth".into()) }));
        assert_eq!(c.changes, Some(0));
        assert_eq!(c.pr.as_ref().map(|p| p.number), Some(42));
        assert_eq!(c.guard, Some("local_only"));
        assert_eq!(c.refusal, None);
        assert!(!marker.exists(), "the checklist must not run core.fsmonitor");
        // Two uncommitted changes: refused, numbers only.
        std::fs::write(wt.join("SECRET-NAME.txt"), "x").unwrap();
        std::fs::write(wt.join(".gitignore"), "build/\nmore\n").unwrap();
        let c = check(&record(&conn, "eeeeeee1").unwrap().unwrap(), Some(&wt), false, Some(&root));
        assert_eq!((c.changes, c.refusal), (Some(2), Some(Refusal::Changes)));
        assert!(!serde_json::to_string(&c).unwrap().contains("SECRET-NAME"), "no file names");
        assert_eq!(remove_plan(&record(&conn, "eeeeeee1").unwrap().unwrap(), Some(&wt), false), Err(Refusal::Changes));
        assert_eq!(remove_plan(&record(&conn, "eeeeeee1").unwrap().unwrap(), Some(&wt), true), Err(Refusal::Running));
        assert!(!marker.exists());
    }

    #[test]
    fn removing_a_clean_worktree_counts_ignored_files_and_keeps_the_branch() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, wt) = repo_with_worktree(&base);
        std::fs::create_dir(wt.join("build")).unwrap();
        for i in 0..120 {
            std::fs::write(wt.join(format!("build/o{i}.bin")), "x").unwrap();
        }
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "fffffff1", repo.to_str().unwrap(), None, SINCE + 1, true);
        let plan = remove_plan(&record(&conn, "fffffff1").unwrap().unwrap(), Some(&wt), false).unwrap();
        assert_eq!(plan.ignored, Some(120));
        assert_eq!(
            plan.confirm_text(crate::i18n::Lang::Ko),
            "worktree를 지울까요?\n폴더: fix-auth\n브랜치: dduck/fix-auth\n무시된 파일 약 120개(빌드 결과 등)도 함께 지워져요\n브랜치와 커밋은 남겨요"
        );
        remove(&plan).unwrap();
        assert!(!wt.exists());
        let out = Command::new("git").arg("-C").arg(&repo).args(["branch", "--list", "dduck/fix-auth"]).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("dduck/fix-auth"), "branch stays");
        let c = check(&record(&conn, "fffffff1").unwrap().unwrap(), Some(&wt), false, None);
        assert_eq!(c.worktree, Some(Worktree::Gone));
        assert_eq!(c.refusal, Some(Refusal::NoWorktree));
    }

    fn plan_resume(conn: &Connection, bg: &str, state: &str) {
        conn.execute(
            "INSERT INTO limit_resumes(session_id, incident, resets_at, hit_ts, origin_bg_id, bg_id, due_at, state, created_at, updated_at)
             VALUES ('s-' || ?1, '1', 2, 1, ?1, ?1, 3, ?2, 1, 1)",
            params![bg, state],
        )
        .unwrap();
    }

    #[test]
    fn a_planned_limit_resume_blocks_wrap_up_and_worktree_removal() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, wt) = repo_with_worktree(&base);
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "abcd0001", repo.to_str().unwrap(), Some(&rid(1)), SINCE + 1, true);
        reviewed(&conn, "abcd0001");
        plan_resume(&conn, "abcd0001", "auto");
        assert!(resume_pending(&conn, "abcd0001").unwrap());
        // 정리 끝 refused with its own reason.
        let e = wrap(&conn, "abcd0001", &[], true, SINCE, SINCE + 9).unwrap_err();
        assert!(matches!(e, Error::Config(RESUME_PENDING)));
        assert!(!facts(&conn, "abcd0001").unwrap().unwrap().wrapped);
        // The checklist says why; worktree 정리 is refused before any delete.
        let r = record(&conn, "abcd0001").unwrap().unwrap();
        let c = check(&r, Some(&wt), false, None);
        assert_eq!((c.refusal, c.wrap_refusal), (Some(Refusal::ResumePending), Some(Refusal::ResumePending)));
        assert_eq!(remove_plan(&r, Some(&wt), false), Err(Refusal::ResumePending));
        assert!(wt.exists());
        // needs_choice and manual count too; closed, resumed and other tasks' rows do not.
        for st in ["manual", "needs_choice", "starting"] {
            conn.execute("UPDATE limit_resumes SET state = ?1", [st]).unwrap();
            assert!(resume_pending(&conn, "abcd0001").unwrap(), "{st}");
        }
        for st in ["closed", "resumed"] {
            conn.execute("UPDATE limit_resumes SET state = ?1", [st]).unwrap();
            assert!(!resume_pending(&conn, "abcd0001").unwrap(), "{st}");
        }
        let r = record(&conn, "abcd0001").unwrap().unwrap();
        assert_eq!(check(&r, Some(&wt), false, None).wrap_refusal, None);
        assert!(remove_plan(&r, Some(&wt), false).is_ok());
        wrap(&conn, "abcd0001", &[], true, SINCE, SINCE + 9).unwrap();
    }

    #[test]
    fn refusals_map_to_fixed_lines_and_not_checked_is_not_a_failed_delete() {
        use crate::i18n::{Fail, Lang};
        assert_eq!(Refusal::NotChecked.fail(), Fail::WrapNotChecked);
        assert_ne!(Refusal::NotChecked.fail(), Fail::WorktreeRemoveFailed);
        assert_eq!(Refusal::Running.fail(), Fail::WrapRunning);
        assert_eq!(Refusal::Changes.fail(), Fail::WrapChanges);
        assert_eq!(Refusal::NoWorktree.fail(), Fail::NoWorktree);
        assert_eq!(Refusal::ResumePending.fail(), Fail::WrapResumePending);
        assert_eq!(Refusal::NotChecked.fail().text(Lang::Ko), "변경을 확인하지 못해 worktree를 지울 수 없어요");
        assert_eq!(Refusal::ResumePending.fail().text(Lang::Ko), "한도 뒤 이어서 하기가 예약돼 있어 정리할 수 없어요");
    }

    #[test]
    fn a_linked_claude_folder_is_not_a_worktree() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (_repo, _wt) = repo_with_worktree(&base);
        // Another repository whose `.claude` is a link into the first one's.
        let other = base.join("other");
        std::fs::create_dir(&other).unwrap();
        git(&other, &["init", "-q", "-b", "main"]);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("proj/.claude"), other.join(".claude")).unwrap();
            let conn = crate::db::open_in_memory().unwrap();
            rec(&conn, "fffffff3", other.to_str().unwrap(), None, SINCE + 1, true);
            let session = other.join(".claude/worktrees/fix-auth");
            let r = record(&conn, "fffffff3").unwrap().unwrap();
            assert_eq!(remove_plan(&r, Some(&session), false), Err(Refusal::NoWorktree));
            assert_eq!(check(&r, Some(&session), false, None).worktree, Some(Worktree::Unknown));
        }
    }

    #[test]
    fn a_folder_swapped_after_the_confirm_is_not_removed() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, wt) = repo_with_worktree(&base);
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "fffffff5", repo.to_str().unwrap(), None, SINCE + 1, true);
        let plan = remove_plan(&record(&conn, "fffffff5").unwrap().unwrap(), Some(&wt), false).unwrap();
        git(&repo, &["worktree", "remove", wt.to_str().unwrap()]);
        std::fs::create_dir(&wt).unwrap();
        std::fs::write(wt.join("keep.txt"), "user file").unwrap();
        assert_eq!(remove(&plan), Err(Refusal::NoWorktree));
        assert!(wt.join("keep.txt").exists());
    }

    #[test]
    fn a_plain_folder_under_worktrees_is_not_removed() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, _wt) = repo_with_worktree(&base);
        let stray = repo.join(".claude/worktrees/stray");
        std::fs::create_dir(&stray).unwrap();
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "fffffff4", repo.to_str().unwrap(), None, SINCE + 1, true);
        let r = record(&conn, "fffffff4").unwrap().unwrap();
        assert_eq!(remove_plan(&r, Some(&stray), false), Err(Refusal::NoWorktree), "not in git worktree list");
    }

    #[test]
    fn status_never_runs_a_planted_clean_filter() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (_repo, wt) = repo_with_worktree(&base);
        let marker = base.join("FILTER-RAN");
        let script = base.join("clean.sh");
        std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(wt.join(".gitattributes"), "*.txt filter=evil\n").unwrap();
        std::fs::write(wt.join("a.txt"), "hi\n").unwrap();
        git(&wt, &["add", "."]);
        git(&wt, &["-c", "user.email=t@example.invalid", "-c", "user.name=T", "commit", "-q", "-m", "attrs"]);
        git(&wt, &["config", "filter.evil.clean", script.to_str().unwrap()]);
        // Same size, new content: git has to re-read the file through the clean filter.
        std::fs::write(wt.join("a.txt"), "ho\n").unwrap();
        let plain = Command::new("git").arg("-C").arg(&wt).args(["-c", "core.fsmonitor=false", "status", "--porcelain=v1"]).output().unwrap();
        assert!(plain.status.success());
        let plain_ran = marker.exists();
        let _ = std::fs::remove_file(&marker);
        std::fs::write(wt.join("a.txt"), "hu\n").unwrap();
        assert_eq!(count_changes(&wt, 5), Some(1));
        assert!(!marker.exists(), "the app's status must not run a filter driver (plain git ran it: {plain_ran})");
        let _ = crate::delivery_guard::start_branch(&wt).unwrap();
        assert!(!marker.exists());
    }

    #[test]
    fn worktree_outside_the_folder_or_a_link_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (repo, _wt) = repo_with_worktree(&base);
        let conn = crate::db::open_in_memory().unwrap();
        rec(&conn, "fffffff2", repo.to_str().unwrap(), None, SINCE + 1, true);
        let elsewhere = base.join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        assert_eq!(remove_plan(&record(&conn, "fffffff2").unwrap().unwrap(), Some(&elsewhere), false), Err(Refusal::NoWorktree));
        assert_eq!(check(&record(&conn, "fffffff2").unwrap().unwrap(), Some(&elsewhere), false, None).worktree, Some(Worktree::Unknown));
        #[cfg(unix)]
        {
            let link = repo.join(".claude/worktrees/linked");
            std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
            assert_eq!(remove_plan(&record(&conn, "fffffff2").unwrap().unwrap(), Some(&link), false), Err(Refusal::NoWorktree));
        }
    }
}
