//! 리뷰 탭 (review-inbox, D-140): D.Duck tasks that were given a 작업 보고서, as card summaries grouped by role in the
//! webview. The list carries only what a card shows and what the verdict line needs (goal cut to 120 characters,
//! the agreed done lines, verdict fields of the report); the full brief, prompt, work steps and evidence are read only
//! when a card is opened, through `task_report::view`. Report ids and paths never leave this module.
//!
//! Flow: [`rows`] under the DB lock, then (lock released) [`build`] decides running/project/page and reads reports.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::claude_cli::{AgentSession, SessionStatus};
use crate::task_report::{self, Met, ReportError, Reversible, Status};

/// Cards per page (same as the 아티팩트 tab); `limit` is clamped to 1..=MAX_LIMIT.
pub const PAGE: usize = 10;
pub const MAX_LIMIT: usize = 40;
/// Evidence shown under a card's criteria preview: first line only.
pub const EVIDENCE_CHARS: usize = 120;
/// Every page of the [`MAX_ROWS`] rows opens (50 pages of 10).
pub const MAX_OFFSET: usize = MAX_ROWS;
pub const GOAL_CHARS: usize = 120;
const DONE_CHARS: usize = 2000;
/// Rows considered, applied after project scoping (the list) or over every project (the badge).
pub const MAX_ROWS: usize = 500;
/// A bound on what one query reads; `task_meta` keeps 200 records, so this never cuts in practice.
const READ_BOUND: i64 = 5000;

static POLL_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The shell records whether the last `claude agents` poll succeeded. Until one has, nobody knows which tasks are
/// still running: the list shows "실행 여부 확인 중", marking is refused and the badge hides.
pub fn note_poll(ok: bool) {
    POLL_OK.store(ok, std::sync::atomic::Ordering::Relaxed);
}

pub fn poll_known() -> bool {
    POLL_OK.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Filter {
    Pending,
    All,
}

/// A task that is not over yet, by the report screen's rule (`isEnded` in report.ts): done, failed, stopped and
/// unknown are over, and so is a task the CLI no longer lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Running {
    Working,
    /// Waiting for the user's answer: "답을 기다려요".
    Waiting,
    /// No good poll yet: "실행 여부 확인 중".
    Checking,
}

pub fn running_of(status: SessionStatus) -> Option<Running> {
    match status {
        SessionStatus::WaitingInput => Some(Running::Waiting),
        SessionStatus::Working | SessionStatus::Idle => Some(Running::Working),
        SessionStatus::Done | SessionStatus::Failed | SessionStatus::Stopped | SessionStatus::Unknown => None,
    }
}

/// Background id → running state, for listed sessions that are not over.
pub fn running_map(sessions: &[AgentSession]) -> HashMap<String, Running> {
    sessions.iter().filter_map(|s| Some((s.id.clone()?, running_of(s.status)?))).collect()
}

/// Every record the review tab, the badge and the board's 리뷰 대기·정리 columns read (review-routine-runs): the
/// `task_meta` records, plus routine runs that were given a report (`routine_runs.report_id`, set by the shell when the
/// run started). A routine run is left out when the same background id has a `task_meta` record with a report (the
/// order `task_report::id_for` uses), and only its newest run with a report counts. The routine's instruction is not
/// read: the card's goal is the routine's name. Wrap it as `FROM {RECORDS} WHERE …`; values only ever go in as `?`.
pub(crate) const RECORDS: &str = "(SELECT bg_id, name, cwd, task_type, created_at, reviewed_at, report_id, brief_json, prompt, delivery_alert,
        wrapped_at, ended_status, 0 AS routine FROM task_meta
    UNION ALL
    SELECT rr.bg_id, r.name, r.cwd, NULL, COALESCE(rr.started_at, rr.scheduled_for), rr.reviewed_at, rr.report_id, NULL, NULL, NULL,
        rr.wrapped_at, CASE rr.status WHEN 'done' THEN 'done' WHEN 'failed' THEN 'failed' WHEN 'stopped' THEN 'stopped' WHEN 'lost' THEN 'unknown' END, 1
    FROM routine_runs rr JOIN routines r ON r.id = rr.routine_id
    WHERE rr.report_id IS NOT NULL AND rr.bg_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM task_meta t WHERE t.bg_id = rr.bg_id AND t.report_id IS NOT NULL)
      AND rr.id = (SELECT MAX(x.id) FROM routine_runs x WHERE x.bg_id = rr.bg_id AND x.report_id IS NOT NULL))";

/// Where a record's 리뷰했어요 and 정리 끝 are stored: its `task_meta` row, or one routine run (by row id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Task,
    Routine(i64),
}

/// The record behind `bg_id`, the same one [`RECORDS`] shows: a `task_meta` record with a report, else the newest
/// routine run with a report, else a `task_meta` record without one. `bool` = it has a report.
pub(crate) fn owner(conn: &Connection, bg_id: &str) -> Result<Option<(Owner, bool)>, Error> {
    use rusqlite::OptionalExtension;
    let task: Option<bool> = conn
        .query_row("SELECT report_id FROM task_meta WHERE bg_id = ?1", [bg_id], |r| r.get::<_, Option<String>>(0))
        .optional()?
        .map(|id| id.is_some_and(|id| task_report::valid_id(&id)));
    if task == Some(true) {
        return Ok(Some((Owner::Task, true)));
    }
    let run: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, report_id FROM routine_runs WHERE bg_id = ?1 AND report_id IS NOT NULL ORDER BY id DESC LIMIT 1",
            [bg_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, _)) = run.filter(|(_, rid)| task_report::valid_id(rid)) {
        return Ok(Some((Owner::Routine(id), true)));
    }
    Ok(task.map(|has| (Owner::Task, has)))
}

/// A routine run the scheduler still counts as running: never marked, whatever the last listing said.
fn routine_live(conn: &Connection, run_id: i64) -> Result<bool, Error> {
    Ok(conn.query_row("SELECT status IN ('starting', 'running', 'waiting') FROM routine_runs WHERE id = ?1", [run_id], |r| r.get(0))?)
}

/// One record: with a report (a review target), or a finished D.Duck task without one (listed under 전체 only, D-196).
/// Internal: holds the report id and the stored instruction.
pub struct Row {
    bg_id: String,
    name: Option<String>,
    cwd: Option<String>,
    task_type: Option<String>,
    created_at: i64,
    reviewed: bool,
    report_id: Option<String>,
    brief: Option<crate::team::Brief>,
    prompt: Option<String>,
    delivery_alert: Option<i64>,
    wrapped: bool,
    routine: bool,
    ended: Option<&'static str>,
}

const ENDED_CODES: [&str; 4] = ["done", "failed", "stopped", "unknown"];

/// Records newest first: those given a report, and task records without one (D-196). Call under the DB lock, then
/// release it before [`build`].
pub fn rows(conn: &Connection) -> Result<Vec<Row>, Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT bg_id, name, cwd, task_type, created_at, reviewed_at, report_id, brief_json, prompt, delivery_alert, wrapped_at, routine, ended_status
         FROM {RECORDS} ORDER BY created_at DESC, bg_id LIMIT ?1"
    ))?;
    let rows = stmt.query_map([READ_BOUND], |r| {
        Ok(Row {
            bg_id: r.get(0)?,
            name: r.get(1)?,
            cwd: r.get(2)?,
            task_type: r.get(3)?,
            created_at: r.get(4)?,
            reviewed: r.get::<_, Option<i64>>(5)?.is_some(),
            report_id: r.get(6)?,
            brief: r.get::<_, Option<String>>(7)?.and_then(|j| serde_json::from_str(&j).ok()),
            prompt: r.get(8)?,
            delivery_alert: r.get::<_, Option<i64>>(9)?.filter(|a| (0..=3).contains(a)),
            wrapped: r.get::<_, Option<i64>>(10)?.is_some(),
            routine: r.get::<_, i64>(11)? != 0,
            ended: r.get::<_, Option<String>>(12)?.and_then(|e| ENDED_CODES.into_iter().find(|c| *c == e)),
        })
    })?;
    // A stored report id that is not a valid one leaves the row out (as before); it never becomes "no report".
    let rows: Vec<Row> = rows.filter_map(Result::ok).filter(|r| r.report_id.as_deref().is_none_or(task_report::valid_id) && crate::task_meta::valid_bg_id(&r.bg_id)).collect();
    // A task record without a report whose id has a routine report is that routine card, never a second one.
    let reported: std::collections::HashSet<String> = rows.iter().filter(|r| r.report_id.is_some()).map(|r| r.bg_id.clone()).collect();
    Ok(rows.into_iter().filter(|r| r.report_id.is_some() || !reported.contains(&r.bg_id)).collect())
}

/// "리뷰 전": has a report, not marked, not wrapped up, not running, started at or after `review_since`.
fn pending(r: &Row, running: &HashMap<String, Running>, since: i64) -> bool {
    r.report_id.is_some() && is_pending(&r.bg_id, r.reviewed, r.wrapped, r.created_at, running, since)
}

/// A wrapped-up record is never "리뷰 전": a routine run wrapped up before it became a review target stays off the
/// badge (a task record cannot be wrapped up while it waits for review).
pub(crate) fn is_pending(bg_id: &str, reviewed: bool, wrapped: bool, created_at: i64, running: &HashMap<String, Running>, since: i64) -> bool {
    !reviewed && !wrapped && !running.contains_key(bg_id) && created_at >= since
}

/// The sidebar badge, every few seconds: four narrow columns, the same id checks, order and 500 cap as the list (over
/// every project), so the two always agree. No instruction text, no file reads.
pub fn pending_count(conn: &Connection, running: &HashMap<String, Running>, since: i64) -> Result<u32, Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT bg_id, created_at, reviewed_at, report_id, wrapped_at FROM {RECORDS} WHERE report_id IS NOT NULL ORDER BY created_at DESC, bg_id LIMIT ?1"
    ))?;
    let rows = stmt.query_map([READ_BOUND], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?.is_some(), r.get::<_, String>(3)?, r.get::<_, Option<i64>>(4)?.is_some()))
    })?;
    let n = rows
        .filter_map(Result::ok)
        .filter(|(bg, _, _, rid, _)| task_report::valid_id(rid) && crate::task_meta::valid_bg_id(bg))
        .take(MAX_ROWS)
        .filter(|(bg, at, reviewed, _, wrapped)| is_pending(bg, *reviewed, *wrapped, *at, running, since))
        .count();
    Ok(n.min(999) as u32)
}

// ---------- what a card gets ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedLine {
    pub what: String,
    pub open: bool,
    pub needs_user: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionLine {
    pub title: String,
    pub reversible: Option<Reversible>,
    pub assumed: bool,
    pub deviates_from_brief: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionLine {
    pub criterion: String,
    pub met: Met,
    /// First line of the summary, at most 120 characters (card preview).
    pub evidence: String,
    /// Evidence items written, and how many of each kind the app found valid (no names, commands or captions).
    pub evidence_count: usize,
    pub verified: VerifiedKinds,
    /// Failed tests in this criterion's valid test reports.
    pub tests_failed: u32,
}

/// Valid items per kind, so the webview applies the role's kinds (review: file only; QA: no file).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedKinds {
    pub screenshot: usize,
    pub test_report: usize,
    pub command: usize,
    pub file: usize,
}

/// The first attachment for a card without criteria: title and the first paragraph that is not the title.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentLine {
    pub title: String,
    pub line: Option<String>,
}

/// The verdict fields of a report: no work steps, files, evidence, reasons, options or references.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub status: Status,
    pub work_count: usize,
    pub blocked: Vec<BlockedLine>,
    pub decisions: Vec<DecisionLine>,
    pub criteria: Vec<CriterionLine>,
    /// Version 1 report ("근거 형식 이전 보고서").
    pub legacy: bool,
    /// Any valid test report anywhere in the report (QA "테스트 리포트 없음").
    pub test_report: bool,
    pub attachment_count: usize,
    /// Attachments that open with content, that cannot be opened, and that are empty after cleaning (counts only,
    /// from the signature cache; no titles but the first).
    pub attachment_valid: usize,
    pub attachment_broken: usize,
    pub attachment_empty: usize,
    pub first_attachment: Option<AttachmentLine>,
}

impl Summary {
    fn of(r: task_report::Report, first: Option<AttachmentLine>) -> Self {
        use crate::evidence::{Kind, Status};
        let test_report = r.criteria.iter().flat_map(|c| &c.items).any(|i| i.kind == Kind::TestReport && i.status == Status::Valid);
        Summary {
            legacy: r.legacy,
            test_report,
            attachment_count: r.attachments.len(),
            attachment_valid: r.attachments.iter().filter(|a| a.status == Status::Valid && !a.empty).count(),
            attachment_broken: r.attachments.iter().filter(|a| a.status != Status::Valid).count(),
            attachment_empty: r.attachments.iter().filter(|a| a.status == Status::Valid && a.empty).count(),
            first_attachment: first,
            status: r.status,
            work_count: r.work.len(),
            blocked: r.blocked.into_iter().map(|b| BlockedLine { what: b.what, open: b.open, needs_user: b.needs_user }).collect(),
            decisions: r
                .decisions
                .into_iter()
                .map(|d| DecisionLine { title: d.title, reversible: d.reversible, assumed: d.assumed, deviates_from_brief: d.deviates_from_brief })
                .collect(),
            criteria: r
                .criteria
                .into_iter()
                .map(|c| {
                    let mut v = VerifiedKinds::default();
                    let mut failed = 0u32;
                    for i in c.items.iter().filter(|i| i.status == Status::Valid) {
                        match i.kind {
                            Kind::Screenshot => v.screenshot += 1,
                            Kind::TestReport => v.test_report += 1,
                            Kind::Command => v.command += 1,
                            Kind::File => v.file += 1,
                        }
                        failed = failed.saturating_add(i.failed.unwrap_or(0));
                    }
                    CriterionLine { evidence: one_line(&c.summary, EVIDENCE_CHARS), evidence_count: c.evidence().len(), verified: v, tests_failed: failed, criterion: c.criterion, met: c.met }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CardReport {
    Ready { summary: Summary },
    Missing,
    /// A finished D.Duck task started without a report (D-196): "보고서 없음", nothing to review or mark.
    None,
    Invalid { code: ReportError },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub bg_id: String,
    pub task_type: Option<String>,
    /// Last folder name only.
    pub folder: Option<String>,
    pub created_at: i64,
    pub reviewed: bool,
    pub running: Option<Running>,
    /// Brief goal → task name → first line of the instruction, at most 120 characters.
    pub goal: Option<String>,
    /// `false` when "작업 지시 저장" was off.
    pub stored: bool,
    /// The agreed 완료 기준, for the verdict's criteria matching. `Some("")` = a brief without done lines; `None` = no
    /// brief (the verdict then treats the agent's criteria as agreed, as the report screen does).
    pub done: Option<String>,
    pub report: CardReport,
    /// delivery-guard post-check bits: 1 = a push to the remote was seen, 2 = the default branch moved. A number
    /// only; `None` = not checked.
    pub delivery_alert: Option<i64>,
    /// "정리 끝" (task-lifecycle): the only state shown as "완료".
    pub wrapped: bool,
    /// A routine run (review-routine-runs): the card says "루틴".
    pub routine: bool,
    /// How the task last ended while listed (`done`/`failed`/`stopped`/`unknown`), for a card without a report.
    pub ended: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inbox {
    pub report_setting: bool,
    /// Matching items in this filter and project (all pages).
    pub total: usize,
    /// "리뷰 전" items outside the picked project, so the numbers add up to the badge.
    pub pending_other: usize,
    /// Any record with a report in this project, whatever the filter: tells "모두 리뷰했어요" from "아직 없어요".
    pub has_any: bool,
    /// Finished D.Duck tasks without a report in this project (D-196): listed under 전체, never counted as 리뷰 전.
    pub no_report: usize,
    /// Tasks with a report in this project that are still running ("모두 리뷰했어요 · 일하는 중 N개").
    pub running: usize,
    /// Matching items per role (all pages), so a group header's count is right before 더 보기.
    pub groups: Vec<GroupTotal>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupTotal {
    pub task_type: Option<String>,
    pub total: usize,
}

fn one_line(s: &str, max: usize) -> String {
    let line = s.split(['\n', '\r']).map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    crate::text::display_safe(line, max)
}

fn goal_of(r: &Row) -> Option<String> {
    let brief_goal = r.brief.as_ref().map(|b| b.goal.trim()).filter(|g| !g.is_empty());
    let name = r.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let first = || r.prompt.as_deref().and_then(|p| p.lines().map(str::trim).find(|l| !l.is_empty()));
    let raw = brief_goal.or(name).or_else(first)?;
    // One line: the brief goal may hold line breaks.
    let line = raw.split(['\n', '\r']).map(str::trim).find(|l| !l.is_empty())?;
    Some(crate::text::display_safe(line, GOAL_CHARS))
}

fn folder_of(cwd: Option<&str>) -> Option<String> {
    Path::new(cwd?).file_name().map(|n| crate::text::display_safe(&n.to_string_lossy(), 64))
}

/// A card's report summary from a validated read (paths and the id stay here).
fn report_card(read: Result<task_report::Report, ReportError>, report_id: &str, root: &Path, cwd: Option<&str>) -> CardReport {
    match read {
        Ok(mut rep) => {
            let dir = std::fs::canonicalize(root).ok().map(|d| d.join(report_id));
            let mut first = None;
            if let Some(dir) = dir.filter(|d| std::fs::symlink_metadata(d).is_ok_and(|m| m.file_type().is_dir())) {
                let cwd = cwd.map(Path::new);
                task_report::annotate(&mut rep, report_id, &dir, cwd);
                if rep.criteria.is_empty() {
                    let size = crate::evidence::measure(&dir);
                    first = rep.attachment_refs.first().map(|a| AttachmentLine { title: a.title.clone(), line: crate::attachment::first_line(report_id, &dir, a, size) });
                }
            }
            CardReport::Ready { summary: Summary::of(rep, first) }
        }
        Err(ReportError::Missing) => CardReport::Missing,
        Err(code) => CardReport::Invalid { code },
    }
}

fn card(r: Row, root: &Path, running: Option<Running>) -> Item {
    let report = match r.report_id.as_deref() {
        None => CardReport::None,
        Some(rid) => report_card(task_report::read(root, rid), rid, root, r.cwd.as_deref()),
    };
    Item {
        goal: goal_of(&r),
        // A routine's instruction lives with the routine (not under "작업 지시 저장"); the card's goal is its name.
        stored: r.routine || r.brief.is_some() || r.prompt.is_some(),
        done: r.brief.as_ref().map(|b| b.done.chars().take(DONE_CHARS).collect::<String>()),
        folder: folder_of(r.cwd.as_deref()),
        bg_id: r.bg_id,
        task_type: r.task_type,
        created_at: r.created_at,
        reviewed: r.reviewed,
        running,
        report,
        delivery_alert: r.delivery_alert,
        wrapped: r.wrapped,
        routine: r.routine,
        ended: r.ended,
    }
}

/// What the tab asks for. `scope` is the picked project's canonical folder (the shell resolves it; an unknown project
/// is an error there, never a fall back to every project).
pub struct Query<'a> {
    pub filter: Filter,
    pub scope: Option<&'a Path>,
    pub offset: usize,
    pub limit: usize,
    pub since: i64,
    pub report_setting: bool,
    /// A good poll has landed ([`poll_known`]); otherwise unreviewed cards are "실행 여부 확인 중".
    pub known: bool,
}

/// Inside the project: the stored folder, resolved (links, `..`), at or under the project folder by path component.
/// A folder that no longer resolves is left out. Each distinct folder is resolved once per call.
fn in_scope(cwd: Option<&str>, scope: &Path, seen: &mut HashMap<String, Option<PathBuf>>) -> bool {
    let Some(cwd) = cwd else { return false };
    let real = seen.entry(cwd.to_string()).or_insert_with(|| std::fs::canonicalize(PathBuf::from(cwd)).ok());
    real.as_ref().is_some_and(|real| real.starts_with(scope))
}

/// Filters, counts and reads one page. Run with the DB lock released: it reads report files.
pub fn build(rows: Vec<Row>, root: &Path, running: &HashMap<String, Running>, q: &Query<'_>) -> Result<Inbox, Error> {
    if q.offset > MAX_OFFSET {
        return Err(Error::Config("offset"));
    }
    // Records without a report never touch the review numbers: the badge's 500 rows are the reported ones only.
    let (reported, plain): (Vec<Row>, Vec<Row>) = rows.into_iter().partition(|r| r.report_id.is_some());
    // Same 500 rows as the badge for "other projects"; the cap applies after scoping for this project's numbers.
    let pending_all = reported.iter().take(MAX_ROWS).filter(|r| pending(r, running, q.since)).count();
    let mut seen = HashMap::new();
    let mut here = |r: &Row| q.scope.is_none_or(|scope| in_scope(r.cwd.as_deref(), scope, &mut seen));
    let scoped: Vec<Row> = reported.into_iter().filter(|r| here(r)).take(MAX_ROWS).collect();
    // Finished only: not running by the last listing, and only once a good poll says so.
    let plain: Vec<Row> = if q.known { plain.into_iter().filter(|r| !running.contains_key(&r.bg_id) && here(r)).collect() } else { Vec::new() };
    let has_any = !scoped.is_empty();
    let no_report = plain.len();
    let running_here = scoped.iter().filter(|r| running.contains_key(&r.bg_id)).count();
    let pending_here = scoped.iter().filter(|r| pending(r, running, q.since)).count();
    let matching: Vec<Row> = match q.filter {
        Filter::Pending => scoped.into_iter().filter(|r| pending(r, running, q.since)).collect(),
        Filter::All => {
            let mut all: Vec<Row> = scoped.into_iter().chain(plain).collect();
            all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.bg_id.cmp(&b.bg_id)));
            all.truncate(MAX_ROWS);
            all
        }
    };
    let total = matching.len();
    let mut groups: Vec<GroupTotal> = Vec::new();
    for r in &matching {
        match groups.iter_mut().find(|g| g.task_type == r.task_type) {
            Some(g) => g.total += 1,
            None => groups.push(GroupTotal { task_type: r.task_type.clone(), total: 1 }),
        }
    }
    let items = matching
        .into_iter()
        .skip(q.offset)
        .take(q.limit.clamp(1, MAX_LIMIT))
        .map(|r| {
            let run = if q.known { running.get(&r.bg_id).copied() } else { (!r.reviewed && r.report_id.is_some()).then_some(Running::Checking) };
            card(r, root, run)
        })
        .collect();
    Ok(Inbox { report_setting: q.report_setting, total, pending_other: pending_all.saturating_sub(pending_here), has_any, no_report, running: running_here, groups, items })
}

/// "리뷰했어요" from the 리뷰 탭: only a record that has a report and is not running (the shell passes the last
/// listing). Errors name no ids.
pub fn mark(conn: &Connection, bg_id: &str, running: &HashMap<String, Running>, known: bool, now_ms: i64) -> Result<(), Error> {
    if !crate::task_meta::valid_bg_id(bg_id) {
        return Err(Error::Config("task id"));
    }
    if !known || running.contains_key(bg_id) {
        return Err(Error::Config("running"));
    }
    let n = match owner(conn, bg_id)? {
        Some((Owner::Task, true)) => conn.execute("UPDATE task_meta SET reviewed_at = ?2 WHERE bg_id = ?1 AND report_id IS NOT NULL", params![bg_id, now_ms])?,
        Some((Owner::Routine(run), true)) => {
            if routine_live(conn, run)? {
                return Err(Error::Config("running"));
            }
            conn.execute("UPDATE routine_runs SET reviewed_at = ?2 WHERE id = ?1", params![run, now_ms])?
        }
        _ => 0,
    };
    if n == 0 {
        return Err(Error::Config("no report"));
    }
    Ok(())
}

/// "되돌리기": clears the mark, only on a record with a report (as [`mark`]), and "정리 끝" with it, so an unreviewed
/// task is never "완료" (task-lifecycle). Unknown ids → false.
pub fn unmark(conn: &Connection, bg_id: &str) -> Result<bool, Error> {
    if !crate::task_meta::valid_bg_id(bg_id) {
        return Err(Error::Config("task id"));
    }
    Ok(match owner(conn, bg_id)? {
        Some((Owner::Task, true)) => conn.execute("UPDATE task_meta SET reviewed_at = NULL, wrapped_at = NULL WHERE bg_id = ?1 AND report_id IS NOT NULL", [bg_id])?,
        Some((Owner::Routine(run), true)) => conn.execute("UPDATE routine_runs SET reviewed_at = NULL, wrapped_at = NULL WHERE id = ?1", [run])?,
        _ => 0,
    } > 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::claude_cli::SessionKind;
    use crate::task_meta::{NewRecord, record_start};

    const CANARY: &str = "CANARY-tail-7f3a91";

    #[allow(clippy::too_many_arguments)]
    fn rec(conn: &Connection, bg: &str, cwd: &str, task_type: &str, report: Option<&str>, at: i64, prompt: Option<&str>, brief: Option<&crate::team::Brief>) {
        record_start(
            conn,
            &NewRecord {
                bg_id: bg,
                name: Some("작업 이름"),
                cwd,
                agent: None,
                task_type: Some(task_type),
                prompt,
                brief,
                effort: None,
                worktree: false,
                permission_mode: None,
                report_id: report,
                origin: None,
                loadout_dropped: 0,
                now_ms: at,
            },
        )
        .unwrap();
    }

    fn session(id: &str, status: SessionStatus) -> AgentSession {
        AgentSession { key: id.into(), pid: None, id: Some(id.into()), session_id: None, name: id.into(), cwd: "/tmp".into(), kind: SessionKind::Background, started_at: 0, status }
    }

    fn id(n: u8) -> String {
        format!("{n:032x}")
    }

    fn write_report(root: &Path, rid: &str, json: &str) {
        let dir = task_report::prepare_dir(root, rid).unwrap();
        std::fs::write(dir.join(task_report::FILE), json).unwrap();
    }

    const REPORT: &str = r#"{"version":1,"status":"done","work":[{"step":"s1","result":"EVIDENCE-SECRET","files":["a.rs"]}],
        "blocked":[],"decisions":[{"title":"시계 주입","options":["a","b"],"chosen":"b","reason":"REASON-SECRET","assumed":true}],
        "criteria":[{"criterion":"20회 통과","met":"yes","evidence":"EVIDENCE_LONG"},{"criterion":"문서","met":"partial","evidence":""}]}"#;

    fn now() -> i64 {
        1_800_000_000_000
    }

    #[test]
    fn running_follows_the_report_screen_rule() {
        assert_eq!(running_of(SessionStatus::WaitingInput), Some(Running::Waiting));
        assert_eq!(running_of(SessionStatus::Working), Some(Running::Working));
        assert_eq!(running_of(SessionStatus::Idle), Some(Running::Working));
        for s in [SessionStatus::Done, SessionStatus::Failed, SessionStatus::Stopped, SessionStatus::Unknown] {
            assert_eq!(running_of(s), None, "{s:?}");
        }
    }

    #[test]
    fn pending_leaves_out_running_waiting_reviewed_and_old_tasks() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        let since = now() - 10_000;
        rec(&conn, "aaaaaaa1", "/tmp", "dev", Some(&id(1)), now(), None, None);
        rec(&conn, "aaaaaaa2", "/tmp", "dev", Some(&id(2)), now() + 1, None, None);
        rec(&conn, "aaaaaaa3", "/tmp", "qa", Some(&id(3)), now() + 2, None, None);
        rec(&conn, "aaaaaaa4", "/tmp", "dev", Some(&id(4)), now() + 3, None, None);
        rec(&conn, "aaaaaaa5", "/tmp", "dev", Some(&id(5)), since - 1, None, None); // before the tab existed
        rec(&conn, "aaaaaaa6", "/tmp", "dev", None, now() + 4, None, None); // no report
        crate::task_meta::mark_reviewed(&conn, "aaaaaaa4", now()).unwrap();
        let running = running_map(&[session("aaaaaaa2", SessionStatus::Working), session("aaaaaaa3", SessionStatus::WaitingInput), session("aaaaaaa1", SessionStatus::Unknown)]);
        assert_eq!(pending_count(&conn, &running, since).unwrap(), 1);
        let q = |filter| Query { filter, scope: None, offset: 0, limit: PAGE, since, report_setting: true, known: true };
        let p = build(rows(&conn).unwrap(), &root, &running, &q(Filter::Pending)).unwrap();
        assert_eq!(p.items.iter().map(|i| i.bg_id.as_str()).collect::<Vec<_>>(), ["aaaaaaa1"]);
        assert_eq!(p.items[0].report, CardReport::Missing, "no report.json yet still counts");
        let all = build(rows(&conn).unwrap(), &root, &running, &q(Filter::All)).unwrap();
        assert_eq!(all.total, 6, "everything with a report, old ones included, plus the finished task without one (D-196)");
        assert_eq!((all.running, all.no_report), (2, 1));
        assert_eq!(all.groups, vec![GroupTotal { task_type: Some("dev".into()), total: 5 }, GroupTotal { task_type: Some("qa".into()), total: 1 }]);
        let by = |b: &str| all.items.iter().find(|i| i.bg_id == b).unwrap().clone();
        assert_eq!(by("aaaaaaa3").running, Some(Running::Waiting));
        assert_eq!(by("aaaaaaa2").running, Some(Running::Working));
        assert!(by("aaaaaaa4").reviewed);
        // Marking is refused for running tasks and tasks without a report; undo works.
        assert!(mark(&conn, "aaaaaaa2", &running, true, now()).is_err());
        assert!(mark(&conn, "aaaaaaa6", &running, true, now()).is_err());
        assert!(mark(&conn, "../x", &running, true, now()).is_err());
        assert!(mark(&conn, "aaaaaaa1", &running, false, now()).is_err(), "no good poll yet");
        assert!(!unmark(&conn, "aaaaaaa6").unwrap(), "no report, nothing to undo");
        // Before a good poll, unreviewed cards are "checking" and cannot be marked.
        let unknown = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { known: false, ..q(Filter::All) }).unwrap();
        let st = |b: &str| unknown.items.iter().find(|i| i.bg_id == b).unwrap().running;
        assert_eq!((st("aaaaaaa1"), st("aaaaaaa2"), st("aaaaaaa4")), (Some(Running::Checking), Some(Running::Checking), None));
        mark(&conn, "aaaaaaa1", &running, true, now()).unwrap();
        assert_eq!(pending_count(&conn, &running, since).unwrap(), 0);
        assert!(unmark(&conn, "aaaaaaa1").unwrap());
        assert!(unmark(&conn, "nope").is_err());
        assert_eq!(pending_count(&conn, &running, since).unwrap(), 1);
        assert!(build(rows(&conn).unwrap(), &root, &running, &Query { offset: MAX_OFFSET + 1, ..q(Filter::All) }).is_err());
    }

    #[test]
    fn badge_matches_the_list_and_a_first_run_starts_at_zero() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        // 30 finished tasks from before the tab existed: the first run shows no badge.
        for i in 0..30u8 {
            rec(&conn, &format!("ee{i:06x}"), "/tmp", "dev", Some(&id(i + 1)), now() - 1000 + i64::from(i), None, None);
        }
        let since = crate::settings::review_since(&conn, now()).unwrap();
        let none = HashMap::new();
        assert_eq!(pending_count(&conn, &none, since).unwrap(), 0);
        // Rows the list would drop (bad ids) are not counted either.
        rec(&conn, "ff000001", "/tmp", "dev", Some(&id(99)), now() + 1, None, None);
        rec(&conn, "ff000002", "/tmp", "dev", Some(&id(98)), now() + 2, None, None);
        conn.execute("UPDATE task_meta SET report_id = 'not-an-id' WHERE bg_id = 'ff000002'", []).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, report_id) VALUES ('BAD-ID', ?1, ?2)", params![now() + 3, id(97)]).unwrap();
        let list = build(rows(&conn).unwrap(), &root, &none, &Query { filter: Filter::Pending, scope: None, offset: 0, limit: PAGE, since, report_setting: true, known: true }).unwrap();
        assert_eq!(list.total, 1);
        assert_eq!(pending_count(&conn, &none, since).unwrap() as usize, list.total);
    }

    #[test]
    fn list_carries_a_summary_only() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        let long = format!("{}{CANARY}", "가".repeat(GOAL_CHARS));
        rec(&conn, "bbbbbbb1", "/tmp", "dev", Some(&id(1)), now(), Some(&format!("{long}\n둘째 줄 {CANARY}")), None);
        // No name: the goal falls to the instruction's first line.
        conn.execute("UPDATE task_meta SET name = NULL WHERE bg_id = 'bbbbbbb1'", []).unwrap();
        let brief = crate::team::Brief { goal: "목표 한 줄".into(), out_of_scope: format!("범위 밖 {CANARY}"), constraints: format!("제약 {CANARY}"), done: "20회 통과\n문서".into() };
        rec(&conn, "bbbbbbb2", "/tmp", "dev", Some(&id(2)), now() + 1, Some(&format!("전체 지시 {CANARY}")), Some(&brief));
        // Evidence longer than a card line: only the first 120 characters travel.
        write_report(&root, &id(2), &REPORT.replace("EVIDENCE_LONG", &format!("{}EVIDENCE-SECRET", "근".repeat(EVIDENCE_CHARS))));
        let inbox = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { filter: Filter::All, scope: None, offset: 0, limit: PAGE, since: 0, report_setting: true, known: true }).unwrap();
        let json = serde_json::to_string(&inbox).unwrap();
        for secret in [CANARY, "EVIDENCE-SECRET", "REASON-SECRET", "a.rs", &id(1), &id(2), root.to_str().unwrap()] {
            assert!(!json.contains(secret), "{secret} leaked");
        }
        // The delivery-guard flag is the one allowed new field: a number, no path or branch.
        assert!(json.contains("\"deliveryAlert\":null"), "{json}");
        conn.execute("UPDATE task_meta SET delivery_alert = 1 WHERE bg_id = 'bbbbbbb1'", []).unwrap();
        conn.execute("UPDATE task_meta SET delivery_alert = 99 WHERE bg_id = 'bbbbbbb2'", []).unwrap();
        let inbox = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { filter: Filter::All, scope: None, offset: 0, limit: PAGE, since: 0, report_setting: true, known: true }).unwrap();
        assert_eq!(inbox.items.iter().find(|i| i.bg_id == "bbbbbbb1").unwrap().delivery_alert, Some(1));
        assert_eq!(inbox.items.iter().find(|i| i.bg_id == "bbbbbbb2").unwrap().delivery_alert, None, "out-of-range bits are dropped");
        let plain = inbox.items.iter().find(|i| i.bg_id == "bbbbbbb1").unwrap();
        assert_eq!(plain.goal.as_deref().map(|g| g.chars().count()), Some(GOAL_CHARS));
        let briefed = inbox.items.iter().find(|i| i.bg_id == "bbbbbbb2").unwrap();
        assert_eq!(briefed.goal.as_deref(), Some("목표 한 줄"));
        assert_eq!(briefed.done.as_deref(), Some("20회 통과\n문서"));
        let CardReport::Ready { summary } = &briefed.report else { panic!("ready") };
        assert_eq!(summary.criteria[0].evidence, "근".repeat(EVIDENCE_CHARS), "one card line of evidence only");
        assert_eq!((summary.work_count, summary.criteria.len(), summary.decisions[0].assumed), (1, 2, true));
    }

    #[test]
    fn cards_count_evidence_without_names_and_show_the_first_attachment() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(t.path()).unwrap();
        let root = base.join(task_report::DIR_NAME);
        let cwd = base.join("proj");
        std::fs::create_dir_all(cwd.join("src")).unwrap();
        std::fs::write(cwd.join("src/a.ts"), "x\n".repeat(50)).unwrap();
        let cwd_s = cwd.to_string_lossy().to_string();
        rec(&conn, "ccccccc1", &cwd_s, "qa", Some(&id(1)), now(), None, None);
        write_report(&root, &id(1), r#"{"version":2,"status":"done","criteria":[
            {"criterion":"c1","met":"yes","summary":"s","evidence":[{"type":"screenshot","path":"evidence/SHOT-NAME.png","caption":"CAPTION-SECRET"},{"type":"test-report","path":"evidence/t.xml"}]},
            {"criterion":"c2","met":"yes","summary":"s","evidence":[{"type":"file","path":"src/a.ts","lines":[1,9]},{"type":"command","cmd":"CMD-SECRET","exitCode":0,"output":"evidence/o.txt"}]}]}"#);
        let ev = root.join(id(1)).join("evidence");
        std::fs::create_dir(&ev).unwrap();
        std::fs::write(ev.join("SHOT-NAME.png"), crate::evidence::tests::png_bytes(4, 4)).unwrap();
        std::fs::write(ev.join("t.xml"), r#"<testsuites tests="3" failures="1"><testsuite name="s"><testcase name="a"><failure/></testcase></testsuite></testsuites>"#).unwrap();
        rec(&conn, "ccccccc2", &cwd_s, "research", Some(&id(2)), now() + 1, None, None);
        write_report(&root, &id(2), r#"{"version":2,"status":"done","attachments":[{"file":"c.html","title":"DB 후보 비교","kind":"html"}]}"#);
        let at = root.join(id(2)).join("attachments");
        std::fs::create_dir(&at).unwrap();
        std::fs::write(at.join("c.html"), format!("<h1>DB 후보 비교</h1><p>세 후보를 비용·운영으로 비교했어요</p><p>BODY-SECRET {}</p>", "가".repeat(200))).unwrap();
        let inbox = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { filter: Filter::All, scope: None, offset: 0, limit: PAGE, since: 0, report_setting: true, known: true }).unwrap();
        let json = serde_json::to_string(&inbox).unwrap();
        for secret in ["SHOT-NAME", "CAPTION-SECRET", "CMD-SECRET", "t.xml", "src/a.ts", "c.html", "BODY-SECRET"] {
            assert!(!json.contains(secret), "{secret} leaked");
        }
        let card = |b: &str| match &inbox.items.iter().find(|i| i.bg_id == b).unwrap().report {
            CardReport::Ready { summary } => summary.clone(),
            _ => panic!(),
        };
        let qa = card("ccccccc1");
        assert!(qa.test_report && !qa.legacy);
        assert_eq!((qa.criteria[0].evidence_count, qa.criteria[0].verified.screenshot, qa.criteria[0].verified.test_report, qa.criteria[0].tests_failed), (2, 1, 1, 1));
        assert_eq!((qa.criteria[1].verified.file, qa.criteria[1].verified.command), (1, 0));
        let research = card("ccccccc2");
        assert_eq!((research.attachment_count, research.attachment_valid, research.attachment_broken, research.attachment_empty), (1, 1, 0, 0));
        let first = research.first_attachment.unwrap();
        assert_eq!((first.title.as_str(), first.line.as_deref()), ("DB 후보 비교", Some("세 후보를 비용·운영으로 비교했어요")));
    }

    #[cfg(unix)]
    #[test]
    fn project_scope_resolves_folders_by_component() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(t.path()).unwrap();
        let root = base.join(task_report::DIR_NAME);
        for d in ["proj", "proj/.claude/worktrees/w1", "proj-other", "elsewhere"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        std::os::unix::fs::symlink(base.join("proj"), base.join("link")).unwrap();
        let p = |d: &str| base.join(d).to_string_lossy().into_owned();
        rec(&conn, "ccccccc1", &p("proj"), "dev", Some(&id(1)), now(), None, None);
        rec(&conn, "ccccccc2", &p("proj/.claude/worktrees/w1"), "dev", Some(&id(2)), now() + 1, None, None);
        rec(&conn, "ccccccc3", &p("proj-other"), "dev", Some(&id(3)), now() + 2, None, None);
        rec(&conn, "ccccccc4", &p("link"), "qa", Some(&id(4)), now() + 3, None, None);
        rec(&conn, "ccccccc5", &p("gone"), "dev", Some(&id(5)), now() + 4, None, None);
        rec(&conn, "ccccccc6", &p("elsewhere"), "dev", Some(&id(6)), now() + 5, None, None);
        let scope = base.join("proj");
        let inbox = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { filter: Filter::Pending, scope: Some(&scope), offset: 0, limit: PAGE, since: 0, report_setting: true, known: true }).unwrap();
        let mut got: Vec<&str> = inbox.items.iter().map(|i| i.bg_id.as_str()).collect();
        got.sort_unstable();
        assert_eq!(got, ["ccccccc1", "ccccccc2", "ccccccc4"], "not proj-other, not a folder that is gone");
        assert_eq!(inbox.pending_other, 3);
    }

    #[test]
    fn pages_hold_ten() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        for i in 0..25u8 {
            rec(&conn, &format!("dd{i:06x}"), "/tmp", "dev", Some(&id(i + 1)), now() + i64::from(i), None, None);
        }
        let q = |offset| Query { filter: Filter::All, scope: None, offset, limit: PAGE, since: 0, report_setting: false, known: true };
        let first = build(rows(&conn).unwrap(), &root, &HashMap::new(), &q(0)).unwrap();
        assert_eq!((first.total, first.items.len(), first.report_setting), (25, PAGE, false));
        assert_eq!(first.items[0].bg_id, "dd000018", "newest first");
        assert_eq!(build(rows(&conn).unwrap(), &root, &HashMap::new(), &q(2 * PAGE)).unwrap().items.len(), 5);
        let big = Query { limit: 1000, ..q(0) };
        assert_eq!(build(rows(&conn).unwrap(), &root, &HashMap::new(), &big).unwrap().items.len(), 25.min(MAX_LIMIT));
    }

    // ---------- review-routine-runs ----------

    const ROUTINE_PROMPT: &str = "ROUTINE-PROMPT-CANARY check the notices";

    fn routine(conn: &Connection, id: i64, name: &str) {
        conn.execute(
            "INSERT INTO routines(id, name, cwd, prompt, permission_mode, schedule_kind, created_at, anchor_ms, approved_digest)
             VALUES (?1, ?2, '/w/notices', ?3, 'manual', 'daily', 0, 0, x'00')",
            params![id, name, ROUTINE_PROMPT],
        )
        .unwrap();
    }

    /// A routine run as `start_claim` leaves it: `routine_runs.bg_id`, and `report_id` set by `set_run_report`.
    fn run(conn: &Connection, routine_id: i64, bg: &str, status: &str, report: Option<&str>, at: i64) -> i64 {
        conn.execute(
            "INSERT INTO routine_runs(routine_id, scheduled_for, started_at, status, bg_id) VALUES (?1, ?2, ?2, ?3, ?4)",
            params![routine_id, at, status, bg],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        if let Some(r) = report {
            task_report::set_run_report(conn, id, r).unwrap();
        }
        id
    }

    #[test]
    fn a_routine_runs_report_shows_in_the_inbox_and_the_badge() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        let since = now() - 10_000;
        routine(&conn, 1, "아침 공고 확인");
        run(&conn, 1, "bba9b3f7", "done", Some(&id(41)), now());
        run(&conn, 1, "cca9b3f7", "done", None, now() + 1); // no report: not a review target
        run(&conn, 1, "dda9b3f7", "done", Some(&id(42)), since - 1); // before the review tab existed
        write_report(&root, &id(41), REPORT);
        let none = HashMap::new();
        let q = |filter| Query { filter, scope: None, offset: 0, limit: PAGE, since, report_setting: true, known: true };
        let p = build(rows(&conn).unwrap(), &root, &none, &q(Filter::Pending)).unwrap();
        assert_eq!(p.items.iter().map(|i| i.bg_id.as_str()).collect::<Vec<_>>(), ["bba9b3f7"]);
        let it = &p.items[0];
        assert!(it.routine);
        assert_eq!((it.goal.as_deref(), it.folder.as_deref(), it.task_type.as_deref(), it.stored), (Some("아침 공고 확인"), Some("notices"), None, true));
        assert!(matches!(it.report, CardReport::Ready { .. }), "the report is read like a task's");
        assert_eq!(pending_count(&conn, &none, since).unwrap(), 1, "badge counts it");
        let all = build(rows(&conn).unwrap(), &root, &none, &q(Filter::All)).unwrap();
        assert_eq!(all.items.iter().map(|i| i.bg_id.as_str()).collect::<Vec<_>>(), ["bba9b3f7", "dda9b3f7"], "older one in 전체 only");
        // Still running by the listing → not pending.
        let running = running_map(&[session("bba9b3f7", SessionStatus::Working)]);
        assert_eq!(pending_count(&conn, &running, since).unwrap(), 0);
        // The list never carries the routine's instruction.
        let json = serde_json::to_string(&all).unwrap();
        assert!(!json.contains("ROUTINE-PROMPT-CANARY"));
        assert!(!json.contains(&id(41)), "no report id to the webview");
    }

    #[test]
    fn a_task_record_with_a_report_wins_over_a_routine_run_with_the_same_id() {
        let conn = crate::db::open_in_memory().unwrap();
        routine(&conn, 1, "루틴");
        rec(&conn, "aaaa0001", "/tmp", "dev", Some(&id(1)), now(), None, None);
        run(&conn, 1, "aaaa0001", "done", Some(&id(2)), now());
        // A record without a report does not hide the routine's report (as `task_report::id_for`).
        rec(&conn, "aaaa0002", "/tmp", "dev", None, now(), None, None);
        run(&conn, 1, "aaaa0002", "done", Some(&id(3)), now() + 1);
        // Two runs with one id: the newest with a report counts once.
        run(&conn, 1, "aaaa0003", "done", Some(&id(4)), now() + 2);
        run(&conn, 1, "aaaa0003", "done", Some(&id(5)), now() + 3);
        let rows = rows(&conn).unwrap();
        let got: Vec<(String, Option<String>, bool)> = rows.iter().map(|r| (r.bg_id.clone(), r.report_id.clone(), r.routine)).collect();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(got.contains(&("aaaa0001".into(), Some(id(1)), false)));
        assert!(got.contains(&("aaaa0002".into(), Some(id(3)), true)), "one card, not a second no-report one");
        assert!(got.contains(&("aaaa0003".into(), Some(id(5)), true)));
    }

    #[test]
    fn marking_a_routine_run_stores_it_on_that_run() {
        let conn = crate::db::open_in_memory().unwrap();
        let since = now() - 10_000;
        routine(&conn, 1, "루틴");
        let older = run(&conn, 1, "bba9b3f7", "done", None, now() - 5);
        let r = run(&conn, 1, "bba9b3f7", "done", Some(&id(41)), now());
        let none = HashMap::new();
        mark(&conn, "bba9b3f7", &none, true, now() + 9).unwrap();
        let at = |run: i64| conn.query_row("SELECT reviewed_at FROM routine_runs WHERE id = ?1", [run], |r| r.get::<_, Option<i64>>(0)).unwrap();
        assert_eq!((at(r), at(older)), (Some(now() + 9), None), "only the run with the report");
        assert_eq!(pending_count(&conn, &none, since).unwrap(), 0);
        assert!(unmark(&conn, "bba9b3f7").unwrap());
        assert_eq!(at(r), None);
        assert_eq!(pending_count(&conn, &none, since).unwrap(), 1);
        // The scheduler still counts it running: refused whatever the listing says.
        conn.execute("UPDATE routine_runs SET status = 'running' WHERE id = ?1", [r]).unwrap();
        assert!(mark(&conn, "bba9b3f7", &none, true, now()).is_err());
        // A run without a report cannot be marked.
        run(&conn, 1, "eea9b3f7", "done", None, now());
        assert!(mark(&conn, "eea9b3f7", &none, true, now()).is_err());
        assert!(!unmark(&conn, "eea9b3f7").unwrap());
    }

    #[test]
    fn every_page_of_the_counted_rows_opens() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        for n in 0..300u32 {
            conn.execute("INSERT INTO task_meta(bg_id, created_at, report_id) VALUES (?1, ?2, ?3)", params![format!("ab{n:06x}"), now() + i64::from(n), format!("{n:032x}")]).unwrap();
        }
        let q = Query { filter: Filter::All, scope: None, offset: 290, limit: PAGE, since: 0, report_setting: true, known: true };
        let last = build(rows(&conn).unwrap(), &root, &HashMap::new(), &q).unwrap();
        assert_eq!((last.total, last.items.len()), (300, 10), "page 30 opens (was refused past page 21)");
        assert!(build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { offset: MAX_ROWS + 1, ..q }).is_err());
    }

    #[test]
    fn the_routine_keep_rule_drops_the_oldest_card_and_record_deletion_keeps_routine_cards() {
        let conn = crate::db::open_in_memory().unwrap();
        routine(&conn, 1, "루틴");
        for n in 0..=task_report::KEEP_PER_ROUTINE {
            run(&conn, 1, &format!("ab{n:06x}"), "done", Some(&format!("{n:032x}")), now() + n);
        }
        let ids: Vec<String> = rows(&conn).unwrap().into_iter().filter(|r| r.report_id.is_some()).map(|r| r.bg_id).collect();
        assert_eq!(ids.len(), task_report::KEEP_PER_ROUTINE as usize, "21st report: the oldest card goes, reviewed or not");
        assert!(!ids.contains(&"ab000000".to_string()));
        rec(&conn, "aaaa0001", "/tmp", "dev", Some(&id(200)), now(), None, None);
        crate::task_meta::delete_all(&conn).unwrap();
        assert_eq!(rows(&conn).unwrap().len(), task_report::KEEP_PER_ROUTINE as usize, "routine cards stay; the routine owns them");
    }

    #[test]
    fn a_finished_task_without_a_report_is_listed_under_all_only() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap().join(task_report::DIR_NAME);
        let since = now() - 10_000;
        rec(&conn, "ee6cd22a", "/w/dev_team", "research", None, now(), Some("NO-REPORT-PROMPT-CANARY 조사해 줘"), None);
        conn.execute("UPDATE task_meta SET ended_status = 'failed' WHERE bg_id = 'ee6cd22a'", []).unwrap();
        rec(&conn, "f00dbabe", "/w/dev_team", "dev", None, now() + 1, None, None); // still running
        rec(&conn, "aaaa0001", "/w/dev_team", "dev", Some(&id(1)), now() - 1, None, None);
        let running = running_map(&[session("f00dbabe", SessionStatus::Working)]);
        let q = |filter| Query { filter, scope: None, offset: 0, limit: PAGE, since, report_setting: true, known: true };
        // Not 리뷰 전, not on the badge.
        let p = build(rows(&conn).unwrap(), &root, &running, &q(Filter::Pending)).unwrap();
        assert_eq!(p.items.iter().map(|i| i.bg_id.as_str()).collect::<Vec<_>>(), ["aaaa0001"]);
        assert_eq!(pending_count(&conn, &running, since).unwrap(), 1, "badge unchanged");
        // 전체: newest first, the running one left out.
        let all = build(rows(&conn).unwrap(), &root, &running, &q(Filter::All)).unwrap();
        assert_eq!(all.items.iter().map(|i| i.bg_id.as_str()).collect::<Vec<_>>(), ["ee6cd22a", "aaaa0001"]);
        assert_eq!((all.total, all.no_report, all.has_any), (2, 1, true));
        let it = &all.items[0];
        assert_eq!((&it.report, it.ended, it.task_type.as_deref(), it.folder.as_deref(), it.goal.as_deref()), (&CardReport::None, Some("failed"), Some("research"), Some("dev_team"), Some("작업 이름")));
        assert!(!serde_json::to_string(&all).unwrap().contains("NO-REPORT-PROMPT-CANARY"), "no instruction text");
        // Nothing to mark or undo.
        assert!(mark(&conn, "ee6cd22a", &running, true, now()).is_err());
        assert!(!unmark(&conn, "ee6cd22a").unwrap());
        // Before a good poll nobody knows it has finished: left out.
        let unknown = build(rows(&conn).unwrap(), &root, &HashMap::new(), &Query { known: false, ..q(Filter::All) }).unwrap();
        assert_eq!((unknown.total, unknown.no_report), (1, 0));
        // Only no-report records in a project: not "has a report", but counted for the 전체 view.
        let only = crate::db::open_in_memory().unwrap();
        rec(&only, "ee6cd22a", "/w/x", "research", None, now(), None, None);
        let v = build(rows(&only).unwrap(), &root, &HashMap::new(), &q(Filter::All)).unwrap();
        assert_eq!((v.has_any, v.no_report, v.items.len()), (false, 1, 1));
    }
}
