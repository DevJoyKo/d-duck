//! 작업 보고서 (decision-report, D-108). Every local task D.Duck starts may get an app-owned folder
//! `<app data>/reports/<id>/` (0700, never a link) opened to Claude with `--add-dir`, and an instruction to keep
//! `report.json` there up to date. The app reads that JSON back (safe open, 1 MB, strict shape and limits) and draws
//! the report itself; "위임 내용" comes from what the app stored at start, never from the agent.
//! Report content, paths and ids are never logged or stored in the DB (only the id) and never put in an error.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::claude_cli::{AgentSession, TaskSpec};

pub const DIR_NAME: &str = "reports";
pub const FILE: &str = "report.json";
/// Larger files are refused ("형식 오류"), never cut.
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const MAX_ITEMS: usize = 30;
pub const MAX_CHARS: usize = 500;
/// Routine runs keep this many reports per routine.
pub const KEEP_PER_ROUTINE: i64 = 20;
/// Unreferenced folders younger than this are left alone (a start may be between creating and recording).
const SWEEP_GRACE_SECS: u64 = 3600;
/// Holds a whole 리뷰 탭 page (review-inbox, 40) so a re-query does not parse again.
const CACHE_ENTRIES: usize = 64;

static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[0-9a-f]{32}\z").unwrap_or_else(|_| unreachable!()));

pub fn valid_id(id: &str) -> bool {
    ID_RE.is_match(id)
}

/// 16 bytes from the OS random source as 32 lowercase hex characters. `None` (no report) when the source fails:
/// there is no weaker fallback.
pub fn new_id() -> Option<String> {
    let mut b = [0u8; 16];
    crate::platform::random_bytes(&mut b).then_some(())?;
    Some(b.iter().map(|x| format!("{x:02x}")).collect())
}

#[cfg(unix)]
fn set_private(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private(_p: &Path) -> std::io::Result<()> {
    Ok(())
}

/// A real directory (not a link) at `p`, created if missing, owner-only.
fn private_dir(p: &Path, create_new: bool) -> Result<(), ReportError> {
    match std::fs::symlink_metadata(p) {
        Ok(_) if create_new => return Err(ReportError::Refused),
        Ok(m) if !m.file_type().is_dir() => return Err(ReportError::Refused),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(p).map_err(|_| ReportError::Unknown)?,
        Err(_) => return Err(ReportError::Unknown),
    }
    // Re-check after creating: a link swapped in meanwhile is refused.
    let m = std::fs::symlink_metadata(p).map_err(|_| ReportError::Unknown)?;
    if !m.file_type().is_dir() {
        return Err(ReportError::Refused);
    }
    set_private(p).map_err(|_| ReportError::Unknown)
}

/// Creates `<root>/<id>/` for a new task. `root` is `<app data>/reports`; the app data folder must already exist.
pub fn prepare_dir(root: &Path, id: &str) -> Result<PathBuf, ReportError> {
    if !valid_id(id) || !root.is_absolute() {
        return Err(ReportError::Refused);
    }
    private_dir(root, false)?;
    let dir = root.join(id);
    private_dir(&dir, true)?;
    Ok(dir)
}

/// Task types whose "decisions" are verdicts: their evidence goes into `criteria` (review and QA, the
/// evidence-strict types of `team_config`).
pub fn is_review_type(task_type: Option<&str>) -> bool {
    crate::team_config::is_evidence_strict(task_type)
}

/// The instruction stays within this many characters (spec "근거를 남기라는 보고서 지시"), path included.
pub const MAX_INSTRUCTION_CHARS: usize = 2000;

/// The instruction appended to the system prompt (format version 2). The path is the app's, never one the agent chose.
pub fn instruction(file: &Path, review: bool) -> String {
    // The folder path appears once (it can be long): everything else is named relative to it (QA MEDIUM).
    let dir = file.parent().unwrap_or(file).display().to_string();
    let name = file.file_name().map_or_else(|| FILE.into(), |n| n.to_string_lossy().into_owned());
    let mut s = format!(
        "[D.Duck 작업 보고서]\n보고서 폴더: {dir}\n이 폴더의 {name} 하나에 JSON으로 보고서를 남긴다.\n\
- 시작하면 status \"in_progress\", 단계·결정·막힘마다 파일 전체를 다시 쓴다. 끝나면 \"done\", 실패면 \"failed\".\n\
- 형식(이 키만, 1MB 이하, 배열 {MAX_ITEMS}개·문자열 {MAX_CHARS}자 이하): \
{{\"version\":2,\"status\":\"\",\"updatedAt\":\"\",\"work\":[{{\"step\":\"\",\"result\":\"\",\"files\":[\"\"]}}],\
\"blocked\":[{{\"what\":\"\",\"cause\":\"\",\"resolution\":\"\",\"open\":false,\"needsUser\":false,\"afterStep\":1}}],\
\"decisions\":[{{\"title\":\"\",\"options\":[\"\"],\"chosen\":\"\",\"reason\":\"\",\"reversible\":\"easy|hard\",\"assumed\":false,\"deviatesFromBrief\":false,\"afterStep\":1}}],\
\"criteria\":[{{\"criterion\":\"\",\"met\":\"yes|partial|no\",\"summary\":\"\",\"evidence\":[\
{{\"type\":\"screenshot\",\"path\":\"evidence/a.png\"}},{{\"type\":\"test-report\",\"path\":\"evidence/t.xml\"}},\
{{\"type\":\"command\",\"cmd\":\"\",\"exitCode\":0,\"output\":\"evidence/o.txt\"}},{{\"type\":\"file\",\"path\":\"src/a.ts\",\"lines\":[1,9]}}]}}],\
\"attachments\":[{{\"file\":\"a.html\",\"title\":\"\",\"kind\":\"html|markdown\"}}],\
\"references\":[{{\"kind\":\"file|url|doc|commit|command\",\"ref\":\"\",\"note\":\"\"}}]}}\n\
- afterStep은 work 단계 번호(1부터). criteria는 '완료 기준' 줄마다 같은 문구로.\n\
- 묻지 않고 정하면 assumed true, 지시와 다르게 하면 deviatesFromBrief true.\n\
- 근거 파일은 폴더의 evidence/ 에만(이름 영문·숫자·.-_ 80자, 기준마다 6개).\n\
- 화면을 바꿨으면 Playwright로 찍는다(page.screenshot). 없으면 partial + summary에 이유.\n\
- 테스트는 리포트 파일: vitest --reporter=junit --outputFile, Playwright --reporter=junit, pytest --junitxml, nextest JUnit, 없으면 {{\"format\":\"dduck-test-summary\",\"passed\":0,\"failed\":0,\"failures\":[]}}.\n\
- 명령 근거는 cmd·exitCode·출력 파일(200줄 이내). 시크릿·토큰·환경변수 값·대화 원문은 출력하거나 옮겨 적지 않는다.\n\
- 근거 없이 met yes를 쓰지 않는다. 못 남겼으면 partial과 이유.\n\
- done이면 결과물을 attachments/result.md 또는 result.html 로 남겨 attachments 맨 앞에 적는다. 조사·정리는 표·목록, PR이면 PR 링크.\n\
- HTML은 파일 하나로: 스크립트·외부 글꼴·원격 그림 없이, 그림은 같은 폴더 png/jpg. 출처는 주소가 보이게 글로도 적는다.\n\
- 이 폴더엔 보고서·근거·첨부만 쓴다. 보고서 때문에 멈추거나 묻지 않고, 못 쓰면 건너뛴다."
    );
    if review {
        s.push_str("\n- 리뷰·검증: decisions는 비우고 판정 근거를 criteria evidence에 남긴다(필수).");
    }
    s
}

/// Whether a starting task gets a report: not when the task unchecked it, not for phone-controlled (remote) tasks,
/// and not when the global setting is off. `setting_on` is read only when the first two pass.
pub fn wanted(spec: &TaskSpec, setting_on: impl FnOnce() -> bool) -> bool {
    let asked = spec.want_report && crate::team_config::is_evidence_strict(spec.task_type.as_deref());
    !spec.skip_report && spec.remote_name.is_none() && (asked || setting_on())
}

/// Gives the task its report folder and instruction.
pub fn attach(spec: &mut TaskSpec, dir: PathBuf, review: bool) {
    spec.report_text = Some(instruction(&dir.join(FILE), review));
    spec.report_dir = Some(dir);
}

// ---------- the report the agent writes ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    InProgress,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub step: String,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    pub what: String,
    #[serde(default)]
    pub cause: String,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub needs_user: bool,
    #[serde(default)]
    pub after_step: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reversible {
    Easy,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub title: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub chosen: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub reversible: Option<Reversible>,
    #[serde(default)]
    pub assumed: bool,
    #[serde(default)]
    pub deviates_from_brief: bool,
    #[serde(default)]
    pub after_step: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Met {
    Yes,
    Partial,
    No,
}

/// `evidence` as written: a sentence (version 1) or the item list (version 2).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum EvidenceField {
    Text(String),
    Items(Vec<crate::evidence::Item>),
}

impl Default for EvidenceField {
    fn default() -> Self {
        EvidenceField::Items(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    pub criterion: String,
    pub met: Met,
    /// The one line a person reads (version 1's `evidence` sentence goes here).
    #[serde(default)]
    pub summary: String,
    /// What the agent wrote; never sent as is.
    #[serde(default, rename = "evidence", skip_serializing)]
    pub written: EvidenceField,
    /// The items as checked by the app (`view`): labels and states only.
    #[serde(default, rename = "evidence", skip_deserializing)]
    pub items: Vec<crate::evidence::ItemView>,
    /// A version 1 criterion: no evidence list ("근거 형식 이전 보고서").
    #[serde(default, skip_deserializing)]
    pub legacy: bool,
}

impl Criterion {
    pub fn evidence(&self) -> &[crate::evidence::Item] {
        match &self.written {
            EvidenceField::Items(v) => v,
            EvidenceField::Text(_) => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    File,
    Url,
    Doc,
    Commit,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub kind: RefKind,
    #[serde(rename = "ref")]
    pub target: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: u32,
    pub status: Status,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub work: Vec<Work>,
    #[serde(default)]
    pub blocked: Vec<Blocked>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    #[serde(default)]
    pub references: Vec<Reference>,
    /// Free-form documents in `attachments/` (D-156), as written; never sent as is.
    #[serde(default, rename = "attachments", skip_serializing)]
    pub attachment_refs: Vec<crate::attachment::Attachment>,
    /// The attachments as checked by the app (`view`): title, kind and state.
    #[serde(default, rename = "attachments", skip_deserializing)]
    pub attachments: Vec<crate::attachment::AttachmentView>,
    /// Version 1 report.
    #[serde(default, skip_deserializing)]
    pub legacy: bool,
}

/// Why there is no report to draw. The only thing that crosses IPC on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportError {
    /// No `report.json` (yet).
    Missing,
    /// Over [`MAX_BYTES`].
    TooLarge,
    /// Not the agreed JSON shape, or over a count/length limit.
    Invalid,
    /// Link, hard link, not a regular file, or the folder was swapped.
    Refused,
    Unknown,
}

fn check_str(s: &str) -> Result<(), ReportError> {
    if s.chars().count() > MAX_CHARS || s.contains('\0') {
        return Err(ReportError::Invalid);
    }
    Ok(())
}

fn check_list<T>(v: &[T]) -> Result<(), ReportError> {
    if v.len() > MAX_ITEMS {
        return Err(ReportError::Invalid);
    }
    Ok(())
}

/// One-line display text: control, bidi and zero-width characters removed.
fn clean(s: &mut String) {
    *s = crate::text::display_safe(s, MAX_CHARS);
}

impl Report {
    fn validate(&self) -> Result<(), ReportError> {
        if self.version != 1 && self.version != 2 {
            return Err(ReportError::Invalid);
        }
        let mut total = 0;
        for c in &self.criteria {
            match (&c.written, self.version) {
                (EvidenceField::Text(t), 1) => check_str(t)?,
                (EvidenceField::Items(v), 1) if v.is_empty() => {}
                (EvidenceField::Items(v), 2) => {
                    if v.len() > crate::evidence::MAX_PER_CRITERION || !v.iter().all(crate::evidence::Item::valid) {
                        return Err(ReportError::Invalid);
                    }
                    total += v.len();
                }
                _ => return Err(ReportError::Invalid),
            }
        }
        if total > crate::evidence::MAX_TOTAL {
            return Err(ReportError::Invalid);
        }
        if self.attachment_refs.len() > crate::attachment::MAX_ATTACHMENTS || (self.version == 1 && !self.attachment_refs.is_empty()) || !self.attachment_refs.iter().all(crate::attachment::Attachment::valid) {
            return Err(ReportError::Invalid);
        }
        if let Some(u) = &self.updated_at {
            if u.chars().count() > 40 {
                return Err(ReportError::Invalid);
            }
            check_str(u)?;
        }
        check_list(&self.work)?;
        check_list(&self.blocked)?;
        check_list(&self.decisions)?;
        check_list(&self.criteria)?;
        check_list(&self.references)?;
        for w in &self.work {
            check_list(&w.files)?;
            for s in [&w.step, &w.result].into_iter().chain(&w.files) {
                check_str(s)?;
            }
        }
        for b in &self.blocked {
            for s in [&b.what, &b.cause, &b.resolution] {
                check_str(s)?;
            }
        }
        for d in &self.decisions {
            check_list(&d.options)?;
            for s in [&d.title, &d.chosen, &d.reason].into_iter().chain(&d.options) {
                check_str(s)?;
            }
        }
        for c in &self.criteria {
            check_str(&c.criterion)?;
            check_str(&c.summary)?;
        }
        for r in &self.references {
            check_str(&r.target)?;
            check_str(&r.note)?;
        }
        Ok(())
    }

    fn clean(mut self) -> Self {
        if let Some(u) = &mut self.updated_at {
            clean(u);
        }
        for w in &mut self.work {
            clean(&mut w.step);
            clean(&mut w.result);
            w.files.iter_mut().for_each(clean);
        }
        for b in &mut self.blocked {
            clean(&mut b.what);
            clean(&mut b.cause);
            clean(&mut b.resolution);
        }
        for d in &mut self.decisions {
            clean(&mut d.title);
            clean(&mut d.chosen);
            clean(&mut d.reason);
            d.options.iter_mut().for_each(clean);
        }
        let legacy = self.version == 1;
        self.legacy = legacy;
        for c in &mut self.criteria {
            clean(&mut c.criterion);
            if let EvidenceField::Text(t) = &c.written {
                if c.summary.is_empty() {
                    c.summary = t.clone();
                }
                c.written = EvidenceField::Items(Vec::new());
            }
            clean(&mut c.summary);
            c.legacy = legacy;
            if let EvidenceField::Items(v) = &mut c.written {
                v.iter_mut().for_each(crate::evidence::Item::clean);
            }
        }
        for a in &mut self.attachment_refs {
            a.clean();
        }
        for r in &mut self.references {
            clean(&mut r.target);
            clean(&mut r.note);
        }
        self
    }
}

/// Parses and checks the agent's JSON. Anything off is refused as a whole, never cut.
pub fn parse(bytes: &[u8]) -> Result<Report, ReportError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ReportError::TooLarge);
    }
    // Some editors and Windows tools write a UTF-8 BOM; serde_json refuses it.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let report: Report = serde_json::from_slice(bytes).map_err(|_| ReportError::Invalid)?;
    report.validate()?;
    Ok(report.clean())
}

// ---------- reading ----------

/// File identity for the cache: mtime, length and inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sig(i64, i64, u64, u64);

fn sig(m: &std::fs::Metadata) -> Sig {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Sig(m.mtime(), m.mtime_nsec(), m.len(), m.ino())
    }
    #[cfg(not(unix))]
    {
        let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i64);
        Sig(t, 0, m.len(), 0)
    }
}

type Cached = (Sig, Result<Report, ReportError>);

/// Parsed reports in memory only, keyed by id, re-read when the file's signature changes.
static CACHE: LazyLock<Mutex<HashMap<String, Cached>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached(id: &str, s: Sig) -> Option<Result<Report, ReportError>> {
    let c = CACHE.lock().ok()?;
    c.get(id).filter(|(k, _)| *k == s).map(|(_, r)| r.clone())
}

fn remember(id: &str, s: Sig, r: &Result<Report, ReportError>) {
    if let Ok(mut c) = CACHE.lock() {
        if c.len() >= CACHE_ENTRIES && !c.contains_key(id) {
            c.clear();
        }
        c.insert(id.to_string(), (s, r.clone()));
    }
}

/// The report folder for `id`, checked to be a real directory under the real `root`.
fn real_dir(root: &Path, id: &str) -> Result<PathBuf, ReportError> {
    if !valid_id(id) {
        return Err(ReportError::Refused);
    }
    for p in [root.to_path_buf(), root.join(id)] {
        let m = std::fs::symlink_metadata(&p).map_err(|e| io_reason(&e))?;
        if !m.file_type().is_dir() {
            return Err(ReportError::Refused);
        }
    }
    let real_root = crate::platform::canon(root).map_err(|_| ReportError::Unknown)?;
    Ok(real_root.join(id))
}

fn io_reason(e: &std::io::Error) -> ReportError {
    match e.kind() {
        std::io::ErrorKind::NotFound => ReportError::Missing,
        _ => ReportError::Refused,
    }
}

/// Reads `<root>/<id>/report.json`: no links (final or folder), single-link regular file, size before reading, and
/// the open descriptor's real path re-checked.
pub fn read(root: &Path, id: &str) -> Result<Report, ReportError> {
    use crate::artifact_preview::{fd_path, open_flags, single_link_regular};
    let dir = real_dir(root, id)?;
    let path = dir.join(FILE);
    let file = open_flags(&path).map_err(|e| io_reason(&e))?;
    let meta = single_link_regular(&file).ok_or(ReportError::Refused)?;
    if fd_path(&file).map_or(cfg!(windows), |real| real != path) {
        return Err(ReportError::Refused);
    }
    let s = sig(&meta);
    if let Some(hit) = cached(id, s) {
        return hit;
    }
    let result = if meta.len() > MAX_BYTES {
        Err(ReportError::TooLarge)
    } else {
        let mut buf = Vec::with_capacity(meta.len() as usize);
        match file.take(MAX_BYTES + 1).read_to_end(&mut buf) {
            Ok(_) => parse(&buf),
            Err(_) => Err(ReportError::Unknown),
        }
    };
    remember(id, s, &result);
    result
}

/// For the task list chip: stat only, plus an earlier parse result for the same file if one is cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Presence {
    Ready,
    Missing,
    Invalid,
}

pub fn presence(root: &Path, id: &str) -> Presence {
    let Ok(dir) = real_dir(root, id) else { return Presence::Missing };
    match std::fs::symlink_metadata(dir.join(FILE)) {
        Err(_) => Presence::Missing,
        Ok(m) if !m.file_type().is_file() || m.len() > MAX_BYTES => Presence::Invalid,
        Ok(m) => match cached(id, sig(&m)) {
            Some(Err(ReportError::Missing)) => Presence::Missing,
            Some(Err(_)) => Presence::Invalid,
            _ => Presence::Ready,
        },
    }
}

// ---------- ids in the DB ----------

/// The report id recorded for a background task (composer start or routine run).
pub fn id_for(conn: &Connection, bg_id: &str) -> Result<Option<String>, Error> {
    let own: Option<String> = conn
        .query_row("SELECT report_id FROM task_meta WHERE bg_id = ?1", [bg_id], |r| r.get(0))
        .optional()?
        .flatten();
    let id = match own {
        Some(id) => Some(id),
        None => conn
            .query_row("SELECT report_id FROM routine_runs WHERE bg_id = ?1 AND report_id IS NOT NULL ORDER BY id DESC LIMIT 1", [bg_id], |r| r.get(0))
            .optional()?,
    };
    Ok(id.filter(|i: &String| valid_id(i)))
}

/// Report state per session key, for sessions that have a report id.
pub fn presence_for(conn: &Connection, root: &Path, sessions: &[AgentSession]) -> HashMap<String, Presence> {
    sessions
        .iter()
        .filter_map(|s| {
            let id = id_for(conn, s.id.as_deref()?).ok().flatten()?;
            Some((s.key.clone(), presence(root, &id)))
        })
        .collect()
}

/// Records a routine run's report and keeps only the newest [`KEEP_PER_ROUTINE`] per routine (older ids are cleared;
/// [`sweep`] removes their folders).
pub fn set_run_report(conn: &Connection, run_id: i64, report_id: &str) -> Result<(), Error> {
    if !valid_id(report_id) {
        return Err(Error::Config("report id"));
    }
    conn.execute("UPDATE routine_runs SET report_id = ?2 WHERE id = ?1", params![run_id, report_id])?;
    conn.execute(
        "UPDATE routine_runs SET report_id = NULL
         WHERE routine_id = (SELECT routine_id FROM routine_runs WHERE id = ?1) AND report_id IS NOT NULL
           AND id NOT IN (SELECT id FROM routine_runs WHERE routine_id = (SELECT routine_id FROM routine_runs WHERE id = ?1)
                          AND report_id IS NOT NULL ORDER BY scheduled_for DESC, id DESC LIMIT ?2)",
        params![run_id, KEEP_PER_ROUTINE],
    )?;
    Ok(())
}

/// Removes report folders no record points at any more (pruned or deleted history, old routine runs), older than
/// an hour. Only names that are report ids, only real directories. Returns how many were removed.
pub fn sweep(conn: &Connection, root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else { return 0 };
    let mut keep = std::collections::HashSet::new();
    for sql in ["SELECT report_id FROM task_meta WHERE report_id IS NOT NULL", "SELECT report_id FROM routine_runs WHERE report_id IS NOT NULL"] {
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                keep.extend(rows.filter_map(Result::ok));
            }
        }
    }
    let mut n = 0;
    for e in entries.filter_map(Result::ok) {
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        if !valid_id(name) || keep.contains(name) {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
        let old = m.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() >= SWEEP_GRACE_SECS);
        if m.file_type().is_dir() && old && std::fs::remove_dir_all(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

// ---------- what the report screen gets ----------

/// "위임 내용(합의)": what the app had when it started the task. Not written by the agent.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delegated {
    pub name: Option<String>,
    pub brief: Option<crate::team::Brief>,
    /// Plain instruction when there is no structured brief (routines, older records), at most 2,000 characters.
    pub prompt: Option<String>,
    pub task_type: Option<String>,
    pub permission_mode: Option<String>,
    /// Last folder name only.
    pub folder: Option<String>,
    /// `false` when "작업 지시 저장" was off: the screen says "저장 안 함".
    pub stored: bool,
    pub routine: bool,
    /// 전달 범위 at start (delivery-guard); `None` for older tasks and routines.
    pub delivery_guard: Option<crate::delivery_guard::Guard>,
    /// Post-check bits (`delivery_guard::ALERT_*`); `None` = not checked or nothing to check.
    pub delivery_alert: Option<i64>,
}

fn folder_name(cwd: Option<&str>) -> Option<String> {
    let p = Path::new(cwd?);
    p.file_name().map(|n| crate::text::display_safe(&n.to_string_lossy(), 64))
}

pub fn delegated(conn: &Connection, bg_id: &str) -> Result<Option<Delegated>, Error> {
    let own = conn
        .query_row("SELECT name, brief_json, prompt, task_type, permission_mode, cwd, delivery_guard, delivery_alert FROM task_meta WHERE bg_id = ?1", [bg_id], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
            ))
        })
        .optional()?;
    if let Some((name, brief, prompt, task_type, mode, cwd, guard, alert)) = own {
        let brief: Option<crate::team::Brief> = brief.and_then(|j| serde_json::from_str(&j).ok());
        let stored = brief.is_some() || prompt.is_some();
        return Ok(Some(Delegated {
            name,
            prompt: if brief.is_none() { prompt.map(|p| p.chars().take(2000).collect()) } else { None },
            brief,
            task_type,
            permission_mode: mode,
            folder: folder_name(cwd.as_deref()),
            stored,
            routine: false,
            delivery_guard: guard.as_deref().and_then(crate::delivery_guard::Guard::from_code),
            delivery_alert: alert.filter(|a| (0..=3).contains(a)),
        }));
    }
    let routine = conn
        .query_row(
            "SELECT r.name, r.prompt, r.permission_mode, r.cwd FROM routine_runs rr JOIN routines r ON r.id = rr.routine_id
             WHERE rr.bg_id = ?1 ORDER BY rr.id DESC LIMIT 1",
            [bg_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        )
        .optional()?;
    Ok(routine.map(|(name, prompt, mode, cwd)| Delegated {
        name: Some(name),
        brief: None,
        prompt: Some(prompt.chars().take(2000).collect()),
        task_type: None,
        permission_mode: Some(mode),
        folder: folder_name(Some(&cwd)),
        stored: true,
        routine: true,
        delivery_guard: None,
        delivery_alert: None,
    }))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum View {
    /// No report was asked for (setting off, unchecked, remote control, or not a D.Duck task).
    Off { delegated: Option<Delegated> },
    Missing { delegated: Option<Delegated> },
    Invalid { delegated: Option<Delegated>, code: ReportError },
    Ready { delegated: Option<Delegated>, report: Report },
}

/// Everything the report screen needs for one background task. Paths never leave this function.
pub fn view(conn: &Connection, root: &Path, bg_id: &str) -> Result<View, Error> {
    Ok(view_of(root, target(conn, bg_id)?))
}

/// What the DB knows about a task's report. Read under the DB lock; the file work ([`view_of`],
/// [`evidence_preview_of`], [`attachment_open_of`]) runs after the lock is released (code review).
#[derive(Debug, Clone)]
pub struct Target {
    delegated: Option<Delegated>,
    id: Option<String>,
    cwd: Option<PathBuf>,
}

pub fn target(conn: &Connection, bg_id: &str) -> Result<Target, Error> {
    Ok(Target { delegated: delegated(conn, bg_id)?, id: id_for(conn, bg_id)?, cwd: task_cwd(conn, bg_id) })
}

/// The report screen from a [`Target`]: reads files only.
pub fn view_of(root: &Path, t: Target) -> View {
    let delegated = t.delegated;
    let Some(id) = t.id else { return View::Off { delegated } };
    match read(root, &id) {
        Ok(mut report) => {
            if let Ok(dir) = real_dir(root, &id) {
                annotate(&mut report, &id, &dir, t.cwd.as_deref());
            }
            View::Ready { delegated, report }
        }
        Err(ReportError::Missing) => View::Missing { delegated },
        Err(code) => View::Invalid { delegated, code },
    }
}

// ---------- evidence and attachments (report-evidence) ----------

/// The folder a task worked in (`file` evidence is relative to it).
fn task_cwd(conn: &Connection, bg_id: &str) -> Option<PathBuf> {
    let own: Option<String> = conn.query_row("SELECT cwd FROM task_meta WHERE bg_id = ?1", [bg_id], |r| r.get(0)).optional().ok().flatten().flatten();
    let cwd = match own {
        Some(c) => Some(c),
        None => conn
            .query_row("SELECT r.cwd FROM routine_runs rr JOIN routines r ON r.id = rr.routine_id WHERE rr.bg_id = ?1 ORDER BY rr.id DESC LIMIT 1", [bg_id], |r| r.get(0))
            .optional()
            .ok()
            .flatten(),
    };
    cwd.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// Fills each criterion's checked items and the attachment tabs. `dir` is the real report folder.
pub fn annotate(report: &mut Report, id: &str, dir: &Path, cwd: Option<&Path>) {
    use crate::evidence::{Ctx, Folder};
    let ctx = Ctx { report_id: id, report_dir: dir, cwd };
    let any = report.criteria.iter().any(|c| !c.evidence().is_empty());
    // The folder is measured once per view (security MEDIUM), only when something in it is named.
    let size = (any || !report.attachment_refs.is_empty()).then(|| crate::evidence::measure(dir));
    let folder = size.filter(|_| any).map(|m| Folder::open_with(dir, m));
    for c in &mut report.criteria {
        c.items = match &folder {
            Some(f) => c.evidence().iter().map(|it| crate::evidence::view(&ctx, f, it)).collect(),
            None => Vec::new(),
        };
    }
    report.attachments = match size {
        Some(m) if !report.attachment_refs.is_empty() => crate::attachment::views(id, dir, &report.attachment_refs, m),
        _ => Vec::new(),
    };
}

/// The report id and real folder of a task, and its parsed report.
fn report_of(root: &Path, t: &Target) -> Result<(String, PathBuf, Report), ReportError> {
    let id = t.id.clone().ok_or(ReportError::Missing)?;
    let report = read(root, &id)?;
    let dir = real_dir(root, &id)?;
    Ok((id, dir, report))
}

/// One evidence preview, by the DB's report id and indexes only (never a path from the webview).
pub fn evidence_preview(conn: &Connection, root: &Path, bg_id: &str, criterion: usize, item: usize) -> Result<crate::evidence::Preview, ReportError> {
    evidence_preview_of(root, &target(conn, bg_id).map_err(|_| ReportError::Unknown)?, criterion, item)
}

pub fn evidence_preview_of(root: &Path, t: &Target, criterion: usize, item: usize) -> Result<crate::evidence::Preview, ReportError> {
    let (id, dir, report) = report_of(root, t)?;
    let it = report.criteria.get(criterion).and_then(|c| c.evidence().get(item)).ok_or(ReportError::Missing)?;
    Ok(crate::evidence::preview(&crate::evidence::Ctx { report_id: &id, report_dir: &dir, cwd: t.cwd.as_deref() }, it))
}

/// One attachment, cleaned for the sandboxed view.
pub fn attachment_open(conn: &Connection, root: &Path, bg_id: &str, index: usize) -> Result<crate::attachment::Opened, ReportError> {
    attachment_open_of(root, &target(conn, bg_id).map_err(|_| ReportError::Unknown)?, index)
}

pub fn attachment_open_of(root: &Path, t: &Target, index: usize) -> Result<crate::attachment::Opened, ReportError> {
    let (_, dir, report) = report_of(root, t)?;
    let a = report.attachment_refs.get(index).ok_or(ReportError::Missing)?;
    Ok(crate::attachment::open(&dir, a))
}

/// One attachment's original for "저장" (report-copy-save): its file name and kind, and the bytes or why they
/// were refused. Read now, never from a cache; the caller writes them only where the user chose.
pub fn attachment_original_of(root: &Path, t: &Target, index: usize) -> Result<(crate::attachment::Attachment, Result<Vec<u8>, crate::evidence::Status>), ReportError> {
    let (_, dir, report) = report_of(root, t)?;
    let a = report.attachment_refs.get(index).ok_or(ReportError::Missing)?.clone();
    let bytes = crate::attachment::read_original(&dir, &a);
    Ok((a, bytes))
}

/// Removes `<root>/<id>/` now (a deleted record): only a report id, only a real directory under the real root.
pub fn remove_folder(root: &Path, id: &str) -> bool {
    let Ok(dir) = real_dir(root, id) else { return false };
    match std::fs::symlink_metadata(&dir) {
        Ok(m) if m.file_type().is_dir() => std::fs::remove_dir_all(&dir).is_ok(),
        _ => false,
    }
}

/// Report ids of the records about to be deleted (one bg id, or all with `None`).
pub fn ids_of_records(conn: &Connection, bg_id: Option<&str>) -> Vec<String> {
    let rows: Result<Vec<String>, _> = match bg_id {
        Some(b) => conn
            .prepare("SELECT report_id FROM task_meta WHERE bg_id = ?1 AND report_id IS NOT NULL")
            .and_then(|mut st| st.query_map([b], |r| r.get(0))?.collect()),
        None => conn.prepare("SELECT report_id FROM task_meta WHERE report_id IS NOT NULL").and_then(|mut st| st.query_map([], |r| r.get(0))?.collect()),
    };
    rows.unwrap_or_default().into_iter().filter(|i| valid_id(i)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"version":1,"status":"in_progress","updatedAt":"2026-11-08T10:00:00Z",
        "work":[{"step":"테스트 재현","result":"20회 중 3회 실패","files":["src/auth.ts"]}],
        "blocked":[{"what":"DB 접속","cause":"권한","resolution":"","open":true,"needsUser":true,"afterStep":1}],
        "decisions":[{"title":"시계 주입","options":["fake timer","clock 인자"],"chosen":"clock 인자","reason":"단순","reversible":"easy","assumed":true}],
        "criteria":[{"criterion":"20회 연속 통과","met":"partial","evidence":"17/20"}],
        "references":[{"kind":"url","ref":"https://example.com","note":"문서"}],
        "extra":"ignored"}"#;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let r = crate::platform::canon(t.path()).unwrap().join(DIR_NAME);
        (t, r)
    }

    #[test]
    fn ids_are_random_hex_and_checked() {
        let a = new_id().unwrap();
        assert!(valid_id(&a) && a != new_id().unwrap());
        for bad in ["", "../x", &"A".repeat(32), &"a".repeat(31), &"a".repeat(33), "0123456789abcdef0123456789abcde\n"] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn folder_is_private_and_links_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        assert_eq!(std::fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(prepare_dir(&root, &id), Err(ReportError::Refused), "an existing folder is never reused");
        assert_eq!(prepare_dir(&root, "../etc"), Err(ReportError::Refused));
        // A link in place of the folder (or the root) is refused.
        let other = tempfile::tempdir().unwrap();
        let id2 = new_id().unwrap();
        std::os::unix::fs::symlink(other.path(), root.join(&id2)).unwrap();
        assert_eq!(prepare_dir(&root, &id2), Err(ReportError::Refused));
        assert_eq!(read(&root, &id2), Err(ReportError::Refused));
        let t2 = tempfile::tempdir().unwrap();
        let linked_root = t2.path().join("reports");
        std::os::unix::fs::symlink(other.path(), &linked_root).unwrap();
        assert_eq!(prepare_dir(&linked_root, &new_id().unwrap()), Err(ReportError::Refused));
    }

    #[test]
    fn instruction_names_the_path_and_rules() {
        let p = Path::new("/data/reports/abc/report.json");
        let t = instruction(p, false);
        assert!(t.contains("/data/reports/abc") && t.contains("report.json") && t.contains("in_progress") && t.contains("deviatesFromBrief"));
        assert!(t.contains("시크릿") && t.contains("환경변수") && t.contains("대화 원문"));
        assert!(!t.contains("판정 근거"));
        assert!(instruction(p, true).contains("판정 근거"));
        assert!(is_review_type(Some("qa")) && is_review_type(Some("review")) && !is_review_type(Some("dev")) && !is_review_type(None));
    }

    #[test]
    fn spec_gets_add_dir_and_instruction_after_team_rules() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        let args = |s: &TaskSpec| -> Vec<String> { s.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect() };
        assert!(!args(&spec).iter().any(|a| a.starts_with("--add-dir") || a.starts_with("--append-system-prompt")));
        attach(&mut spec, PathBuf::from("/data/reports/x"), false);
        let a = args(&spec);
        assert!(a.contains(&"--add-dir=/data/reports/x".to_string()));
        assert!(a.iter().any(|x| x.starts_with("--append-system-prompt=[D.Duck 작업 보고서]")));
        spec.prepare(Path::new("/nonexistent-home"), 0, None).unwrap();
        let a = args(&spec);
        let sys = a.iter().find(|x| x.starts_with("--append-system-prompt=")).unwrap();
        assert!(sys.starts_with("--append-system-prompt=[D.Duck 팀 규칙]") && sys.contains("[D.Duck 작업 보고서]"));
        assert_eq!(a.iter().filter(|x| x.starts_with("--append-system-prompt=")).count(), 1);
        assert_eq!(&a[a.len() - 2..], ["--", "p"]);
    }

    #[test]
    fn report_is_wanted_only_for_local_tasks_with_the_setting_on() {
        let dir = tempfile::tempdir().unwrap();
        let spec = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        assert!(wanted(&spec, || true));
        assert!(!wanted(&spec, || false), "global setting off");
        let mut skipped = spec.clone();
        skipped.skip_report = true;
        assert!(!wanted(&skipped, || panic!("the setting is not read for an unchecked task")));
        // Review/QA asked for one with the setting off; other types cannot.
        let mut qa = spec.clone();
        qa.want_report = true;
        qa.task_type = Some("qa".into());
        assert!(wanted(&qa, || false));
        qa.task_type = Some("dev".into());
        assert!(!wanted(&qa, || false));
        let mut remote = spec.clone();
        remote.remote_name = Some("dduck-x".into());
        assert!(!wanted(&remote, || panic!("the setting is not read for a remote task")));
    }

    #[test]
    fn version_two_evidence_and_version_one_compat() {
        let v1 = parse(GOOD.as_bytes()).unwrap();
        assert!(v1.legacy && v1.criteria[0].legacy && v1.criteria[0].summary == "17/20" && v1.criteria[0].evidence().is_empty());
        let json = serde_json::to_value(&v1).unwrap();
        assert_eq!(json["criteria"][0]["evidence"], serde_json::json!([]), "the v1 sentence never goes out as evidence");
        let v2 = r#"{"version":2,"status":"done","criteria":[{"criterion":"다크 모드","met":"yes","summary":"읽혀요",
            "evidence":[{"type":"screenshot","path":"evidence/list-dark.png","caption":"목록"},{"type":"test-report","path":"evidence/vitest.xml"},
            {"type":"command","cmd":"npm test","exitCode":0,"output":"evidence/out.txt"},{"type":"file","path":"src/a.ts","lines":[10,42]}]}],
            "attachments":[{"file":"compare.html","title":"DB 후보 비교","kind":"html"}]}"#;
        let r = parse(v2.as_bytes()).unwrap();
        assert!(!r.legacy && r.criteria[0].evidence().len() == 4 && r.attachment_refs.len() == 1);
        let item = |e: &str| format!(r#"{{"version":2,"status":"done","criteria":[{{"criterion":"c","met":"yes","evidence":[{e}]}}]}}"#);
        for bad in [
            item(r#"{"type":"screenshot","path":"../other/shot.png"}"#),
            item(r#"{"type":"screenshot","path":"/tmp/shot.png"}"#),
            item(r#"{"type":"screenshot","path":"evidence/sub/shot.png"}"#),
            item(r#"{"type":"screenshot","path":"evidence/shot.svg"}"#),
            item(r#"{"type":"link","url":"https://x"}"#),
            item(r#"{"type":"file","path":".git/config"}"#),
            item(&[r#"{"type":"file","path":"a"}"#; 7].join(",")),
            r#"{"version":2,"status":"done","criteria":[{"criterion":"c","met":"yes","evidence":"글만"}]}"#.to_string(),
            r#"{"version":1,"status":"done","criteria":[{"criterion":"c","met":"yes","evidence":[{"type":"file","path":"a"}]}]}"#.to_string(),
            r#"{"version":2,"status":"done","attachments":[{"file":"a.md","title":"t","kind":"html"}]}"#.to_string(),
            format!(r#"{{"version":2,"status":"done","attachments":[{}]}}"#, [r#"{"file":"a.md","kind":"markdown"}"#; 11].join(",")),
        ] {
            assert_eq!(parse(bad.as_bytes()).err(), Some(ReportError::Invalid), "{bad}");
        }
        // 60 items over the report at most.
        let crit = |n: usize| format!(r#"{{"criterion":"c{n}","met":"yes","evidence":[{}]}}"#, [r#"{"type":"file","path":"a"}"#; 6].join(","));
        let many = |k: usize| format!(r#"{{"version":2,"status":"done","criteria":[{}]}}"#, (0..k).map(crit).collect::<Vec<_>>().join(","));
        assert!(parse(many(10).as_bytes()).is_ok());
        assert_eq!(parse(many(11).as_bytes()).err(), Some(ReportError::Invalid));
    }

    #[test]
    fn instruction_v2_rules_and_length() {
        // A realistic long app-data path (~200 characters) is counted: the whole text stays within the limit.
        let long = format!("/Users/{}/Library/Application Support/com.dduck.app/reports/{}", "u".repeat(80), "0123456789abcdef".repeat(4));
        assert!(long.chars().count() >= 200);
        let p = PathBuf::from(&long).join(FILE);
        for review in [false, true] {
            let t = instruction(&p, review);
            let n = t.chars().count();
            assert!(n <= MAX_INSTRUCTION_CHARS, "{n} characters");
            assert_eq!(t.matches(&long).count(), 1, "the path appears once");
            for must in ["evidence/", "report.json", "Playwright", "--reporter=junit", "--junitxml", "근거 없이 met yes를 쓰지 않는다", "\"version\":2", "attachments/", "attachments/result.md", "주소가 보이게"] {
                assert!(t.contains(must), "{must}");
            }
            assert!(!t.contains("screencapture"));
        }
    }

    #[test]
    fn view_checks_evidence_and_sends_no_paths() {
        let conn = crate::db::open_in_memory().unwrap();
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        task(&conn, "ab12cd34", Some(&id));
        std::fs::create_dir(dir.join("evidence")).unwrap();
        std::fs::write(dir.join("evidence/list-dark.png"), crate::evidence::tests::png_bytes(10, 10)).unwrap();
        std::fs::write(dir.join(FILE), r#"{"version":2,"status":"done","criteria":[{"criterion":"기준 1","met":"yes","summary":"s",
            "evidence":[{"type":"screenshot","path":"evidence/list-dark.png","caption":"CANARY-CAPTION"},{"type":"screenshot","path":"evidence/gone.png"}]}]}"#).unwrap();
        let View::Ready { report, .. } = view(&conn, &root, "ab12cd34").unwrap() else { panic!() };
        let st: Vec<_> = report.criteria[0].items.iter().map(|i| (i.label.as_str(), i.status)).collect();
        assert_eq!(st, [("list-dark.png", crate::evidence::Status::Valid), ("gone.png", crate::evidence::Status::Missing)]);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains(&*root.to_string_lossy()) && !json.contains("evidence/") && !json.contains(&id));
        let crate::evidence::Preview::Image { width, .. } = evidence_preview(&conn, &root, "ab12cd34", 0, 0).unwrap() else { panic!() };
        assert_eq!(width, 10);
        assert_eq!(evidence_preview(&conn, &root, "ab12cd34", 0, 9).err(), Some(ReportError::Missing));
        assert_eq!(evidence_preview(&conn, &root, "ffffffff", 0, 0).err(), Some(ReportError::Missing));
    }

    #[test]
    fn parse_accepts_a_bom() {
        let mut b = b"\xEF\xBB\xBF".to_vec();
        b.extend_from_slice(GOOD.as_bytes());
        assert_eq!(parse(&b).unwrap().work.len(), 1);
        let mut twice = b"\xEF\xBB\xBF".to_vec();
        twice.extend_from_slice(&b);
        assert_eq!(parse(&twice).err(), Some(ReportError::Invalid), "only one leading BOM is dropped");
    }

    #[test]
    fn parse_accepts_the_shape_and_refuses_the_rest() {
        let r = parse(GOOD.as_bytes()).unwrap();
        assert_eq!(r.status, Status::InProgress);
        assert_eq!(r.decisions[0].reversible, Some(Reversible::Easy));
        assert_eq!(r.references[0].kind, RefKind::Url);
        assert_eq!(r.blocked[0].after_step, Some(1));
        let long = "가".repeat(MAX_CHARS + 1);
        let many = vec![r#"{"step":"s"}"#; MAX_ITEMS + 1].join(",");
        for bad in [
            "not json".to_string(),
            r#"{"version":3,"status":"done"}"#.to_string(),
            r#"{"version":1,"status":"finished"}"#.to_string(),
            r#"{"version":1,"status":"done","criteria":[{"criterion":"c","met":"maybe"}]}"#.to_string(),
            r#"{"version":1,"status":"done","references":[{"kind":"script","ref":"x"}]}"#.to_string(),
            r#"{"version":1,"status":"done","work":[{"result":"no step"}]}"#.to_string(),
            format!(r#"{{"version":1,"status":"done","work":[{{"step":"{long}"}}]}}"#),
            format!(r#"{{"version":1,"status":"done","work":[{many}]}}"#),
            r#"{"version":1,"status":"done","work":[{"step":"a\u0000b"}]}"#.to_string(),
        ] {
            assert_eq!(parse(bad.as_bytes()).err(), Some(ReportError::Invalid), "{bad}");
        }
        let huge = vec![b' '; MAX_BYTES as usize + 1];
        assert_eq!(parse(&huge).err(), Some(ReportError::TooLarge));
        // Control and bidi characters are dropped from display text.
        let r = parse(r#"{"version":1,"status":"done","work":[{"step":"ok\u202etxt\nnext"}]}"#.as_bytes()).unwrap();
        assert_eq!(r.work[0].step, "oktxtnext");
    }

    #[test]
    fn read_checks_the_file_and_follows_changes() {
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::Missing));
        assert_eq!(presence(&root, &id), Presence::Missing);
        std::fs::write(dir.join(FILE), GOOD).unwrap();
        assert_eq!(read(&root, &id).unwrap().work.len(), 1);
        assert_eq!(presence(&root, &id), Presence::Ready);
        std::fs::write(dir.join(FILE), r#"{"version":1,"status":"done","work":[{"step":"a"},{"step":"b"}]}"#).unwrap();
        let r = read(&root, &id).unwrap();
        assert_eq!((r.status, r.work.len()), (Status::Done, 2), "a rewritten file is read again");
        std::fs::write(dir.join(FILE), "{broken").unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::Invalid));
        assert_eq!(presence(&root, &id), Presence::Invalid);
        std::fs::write(dir.join(FILE), vec![b' '; MAX_BYTES as usize + 10]).unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::TooLarge));
        assert_eq!(presence(&root, &id), Presence::Invalid);
        assert_eq!(read(&root, "../../etc"), Err(ReportError::Refused));
    }

    #[cfg(unix)]
    #[test]
    fn linked_or_shared_report_files_are_refused() {
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.json");
        std::fs::write(&secret, GOOD).unwrap();
        std::os::unix::fs::symlink(&secret, dir.join(FILE)).unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::Refused));
        std::fs::remove_file(dir.join(FILE)).unwrap();
        std::fs::hard_link(&secret, dir.join(FILE)).unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::Refused));
        std::fs::remove_file(dir.join(FILE)).unwrap();
        std::fs::create_dir(dir.join(FILE)).unwrap();
        assert_eq!(read(&root, &id), Err(ReportError::Refused));
        assert_eq!(presence(&root, &id), Presence::Invalid);
    }

    #[cfg(unix)]
    #[test]
    fn fifo_does_not_block() {
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        let c = std::ffi::CString::new(dir.join(FILE).to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(read(&root, &id), Err(ReportError::Refused));
    }

    fn task(conn: &Connection, bg: &str, report: Option<&str>) {
        let brief = crate::team::Brief { goal: "목표".into(), out_of_scope: "".into(), constraints: "".into(), done: "기준 1\n기준 2".into() };
        crate::task_meta::record_start(
            conn,
            &crate::task_meta::NewRecord {
                bg_id: bg,
                name: Some("job"),
                cwd: "/Users/me/proj",
                agent: None,
                task_type: Some("dev"),
                prompt: Some("p"),
                brief: Some(&brief),
                effort: None,
                worktree: false,
                permission_mode: Some("plan"),
                report_id: report,
                origin: None,
                loadout_dropped: 0,
                now_ms: 1,
            },
        )
        .unwrap();
    }

    #[test]
    fn view_uses_the_db_id_and_the_apps_brief() {
        let conn = crate::db::open_in_memory().unwrap();
        let (_t, root) = root();
        let id = new_id().unwrap();
        let dir = prepare_dir(&root, &id).unwrap();
        task(&conn, "ab12cd34", Some(&id));
        task(&conn, "cd34ef56", None);
        let View::Missing { delegated: Some(d) } = view(&conn, &root, "ab12cd34").unwrap() else { panic!() };
        assert_eq!(d.brief.unwrap().done, "기준 1\n기준 2");
        assert_eq!((d.permission_mode.as_deref(), d.folder.as_deref(), d.stored), (Some("plan"), Some("proj"), true));
        std::fs::write(dir.join(FILE), GOOD).unwrap();
        let v = view(&conn, &root, "ab12cd34").unwrap();
        assert!(matches!(v, View::Ready { .. }));
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains(&id) && !json.contains(&*root.to_string_lossy()), "no id or path crosses IPC");
        assert!(matches!(view(&conn, &root, "cd34ef56").unwrap(), View::Off { .. }));
        assert!(matches!(view(&conn, &root, "00000000").unwrap(), View::Off { delegated: None }));
        std::fs::write(dir.join(FILE), "[]").unwrap();
        assert!(matches!(view(&conn, &root, "ab12cd34").unwrap(), View::Invalid { code: ReportError::Invalid, .. }));
        // A tampered id in the DB is never turned into a path.
        conn.execute("UPDATE task_meta SET report_id = '../../x' WHERE bg_id = 'ab12cd34'", []).unwrap();
        assert!(matches!(view(&conn, &root, "ab12cd34").unwrap(), View::Off { .. }));
    }

    #[test]
    fn routine_runs_keep_twenty_and_sweep_removes_the_rest() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO routines(id, name, cwd, prompt, permission_mode, schedule_kind, created_at, anchor_ms, approved_digest)
             VALUES (1, '아침 점검', '/w', '점검해 줘', 'plan', 'daily', 0, 0, x'00')",
            [],
        )
        .unwrap();
        let (_t, root) = root();
        let mut ids = Vec::new();
        for i in 0..25i64 {
            conn.execute("INSERT INTO routine_runs(routine_id, scheduled_for, status, bg_id) VALUES (1, ?1, 'running', ?2)", params![i, format!("{i:08x}")]).unwrap();
            let id = new_id().unwrap();
            prepare_dir(&root, &id).unwrap();
            set_run_report(&conn, conn.last_insert_rowid(), &id).unwrap();
            ids.push(id);
        }
        let kept: i64 = conn.query_row("SELECT COUNT(*) FROM routine_runs WHERE report_id IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(kept, KEEP_PER_ROUTINE);
        assert_eq!(id_for(&conn, &format!("{:08x}", 24)).unwrap().as_deref(), Some(ids[24].as_str()));
        assert_eq!(id_for(&conn, "00000000").unwrap(), None, "the oldest run lost its report");
        let d = delegated(&conn, &format!("{:08x}", 24)).unwrap().unwrap();
        assert!(d.routine && d.prompt.as_deref() == Some("점검해 줘"));
        // Fresh folders survive the grace period; nothing referenced is touched.
        assert_eq!(sweep(&conn, &root), 0);
        // Age the unreferenced ones (and one referenced) past the grace period.
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(SWEEP_GRACE_SECS + 60);
        for id in &ids {
            crate::testkit::set_mtime(&root.join(id), old);
        }
        std::fs::create_dir(root.join("not-a-report")).unwrap();
        assert_eq!(sweep(&conn, &root), 5);
        assert!(!root.join(&ids[0]).exists() && root.join(&ids[24]).exists() && root.join("not-a-report").exists());
    }
}
