//! 근거 (report-evidence, D-149 (3)·D-152 (9)): the evidence items a v2 report names for each 완료 기준, checked by the
//! app when the report is shown. The app looks only at form and presence (kind, size, count, structure, place), never
//! at whether the evidence is true. Files live in `<report dir>/evidence/` (one name, no folders); `file` items point
//! into the task folder. Paths, names and contents never reach the DB, a log or an error: callers get [`Status`]
//! codes, labels (last name part only) and, on request, one preview.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use serde::{Deserialize, Serialize};

pub const DIR: &str = "evidence";
pub const MAX_PER_CRITERION: usize = 6;
pub const MAX_TOTAL: usize = 60;
pub const MAX_SCREENSHOT: u64 = 5 * 1024 * 1024;
pub const MAX_TEST_REPORT: u64 = 2 * 1024 * 1024;
pub const MAX_OUTPUT: u64 = 256 * 1024;
/// A `file` item's target (only its line count is read).
pub const MAX_FILE: u64 = 2 * 1024 * 1024;
/// Everything in the report folder (report.json, evidence/, attachments/).
pub const MAX_FOLDER: u64 = 50 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 100;
pub const MAX_SIDE: u32 = 8000;
pub const MAX_PIXELS: u64 = 40_000_000;
pub const MAX_CHARS: usize = 500;
pub const MAX_LINE_SPAN: u32 = 2000;
/// JUnit limits (design 8).
const XML_DEPTH: usize = 32;
const XML_ELEMENTS: usize = 200_000;
pub const MAX_FAILURES: usize = 50;
const FAILURE_CHARS: usize = 200;
/// Command output excerpt: head + tail lines, at most this many bytes.
const HEAD_LINES: usize = 80;
const TAIL_LINES: usize = 120;
const EXCERPT_BYTES: usize = 16 * 1024;
/// A whole 리뷰 탭 page (40 cards) × 60 items fits; least recently used goes first (code review).
const CACHE_ENTRIES: usize = 4096;
/// The folder check stops after this many entries (security MEDIUM: a huge folder must not stall a view).
pub const MAX_FOLDER_STATS: usize = 2000;

static NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9._-]{1,80}\z").unwrap_or_else(|_| unreachable!()));

/// One file name inside `evidence/` or `attachments/`: `[A-Za-z0-9._-]{1,80}`, not only dots.
pub fn valid_name(n: &str) -> bool {
    NAME_RE.is_match(n) && !n.bytes().all(|b| b == b'.') && crate::platform::relative_ok(n)
}

fn ext(n: &str) -> String {
    n.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// `evidence/<name>` with an extension from `allowed`. Returns the name.
fn evidence_name<'a>(path: &'a str, allowed: &[&str]) -> Option<&'a str> {
    let name = path.strip_prefix("evidence/")?;
    (valid_name(name) && allowed.contains(&ext(name).as_str())).then_some(name)
}

pub const SCREENSHOT_EXT: &[&str] = &["png", "jpg", "jpeg"];
pub const TEST_EXT: &[&str] = &["xml", "json"];
pub const OUTPUT_EXT: &[&str] = &["txt", "log"];

// ---------- the items the agent writes ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Item {
    Screenshot {
        path: String,
        #[serde(default)]
        caption: String,
    },
    TestReport {
        path: String,
        #[serde(default)]
        caption: String,
    },
    Command {
        cmd: String,
        #[serde(rename = "exitCode")]
        exit_code: i64,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        caption: String,
    },
    File {
        path: String,
        #[serde(default)]
        lines: Option<[u32; 2]>,
        #[serde(default)]
        caption: String,
    },
}

/// A `file` path: relative to the task folder, at most 500 characters, no `..`, `.`-only or empty parts, not under
/// `.git`.
fn valid_repo_path(p: &str) -> bool {
    if p.is_empty() || p.chars().count() > MAX_CHARS || p.starts_with('/') || p.contains('\\') || p.chars().any(crate::text::is_unsafe_char) {
        return false;
    }
    // Windows: no drive, stream, device name or trailing dot/space either (review H3).
    crate::platform::relative_ok(p) && p.split('/').all(|c| !c.is_empty() && c != "." && c != ".." && !crate::platform::name_is(std::ffi::OsStr::new(c), ".git"))
}

fn ok_text(s: &str) -> bool {
    s.chars().count() <= MAX_CHARS && !s.contains('\0')
}

impl Item {
    /// The report's shape rules for one item (anything off makes the whole report "형식 오류").
    pub fn valid(&self) -> bool {
        match self {
            Item::Screenshot { path, caption } => evidence_name(path, SCREENSHOT_EXT).is_some() && ok_text(caption),
            Item::TestReport { path, caption } => evidence_name(path, TEST_EXT).is_some() && ok_text(caption),
            Item::Command { cmd, output, caption, .. } => {
                ok_text(cmd) && !cmd.trim().is_empty() && ok_text(caption) && output.as_deref().is_none_or(|o| evidence_name(o, OUTPUT_EXT).is_some())
            }
            Item::File { path, lines, caption } => {
                valid_repo_path(path) && ok_text(caption) && lines.is_none_or(|[s, e]| s >= 1 && s <= e && e <= s.saturating_add(MAX_LINE_SPAN))
            }
        }
    }

    /// Display text cleaned (control, bidi and zero-width characters dropped).
    pub fn clean(&mut self) {
        let c = |s: &mut String| *s = crate::text::display_safe(s, MAX_CHARS);
        match self {
            Item::Screenshot { caption, .. } | Item::TestReport { caption, .. } | Item::File { caption, .. } => c(caption),
            Item::Command { cmd, caption, .. } => {
                c(cmd);
                c(caption);
            }
        }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Item::Screenshot { .. } => Kind::Screenshot,
            Item::TestReport { .. } => Kind::TestReport,
            Item::Command { .. } => Kind::Command,
            Item::File { .. } => Kind::File,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Screenshot,
    TestReport,
    Command,
    File,
}

/// What the app found. The only thing about a file that crosses IPC besides the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Valid,
    Missing,
    /// Link, hard link, not a regular file, the folder was swapped, or outside the allowed place.
    Refused,
    TooLarge,
    /// The content is not the kind it claims (structure check failed).
    BadType,
    /// `evidence/` holds more than [`MAX_ENTRIES`] entries.
    TooMany,
}

/// One item as the report screen gets it: no path except a `file` item's repository path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub kind: Kind,
    /// File name (evidence), command text (first 80 characters) or `path:start–end` (file).
    pub label: String,
    pub caption: String,
    pub status: Status,
    /// Failed + errored tests of a valid test report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
}

fn label_of(it: &Item) -> String {
    let last = |p: &str| p.rsplit('/').next().unwrap_or("").to_string();
    match it {
        Item::Screenshot { path, .. } | Item::TestReport { path, .. } => last(path),
        Item::Command { cmd, .. } => crate::text::display_safe(cmd, 80),
        Item::File { path, lines: Some([s, e]), .. } if s == e => format!("{path}:{s}"),
        Item::File { path, lines: Some([s, e]), .. } => format!("{path}:{s}–{e}"),
        Item::File { path, lines: None, .. } => path.clone(),
    }
}

// ---------- safe reading ----------

/// File identity for the caches: mtime, length and inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Sig(i64, i64, u64, u64);

pub(crate) fn sig(m: &std::fs::Metadata) -> Sig {
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

/// The state of a folder inside the report folder (`evidence/` or `attachments/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Sub {
    Ok(PathBuf),
    Bad(Status),
}

/// The report folder measured once per view: under [`MAX_FOLDER`] bytes within [`MAX_FOLDER_STATS`] entries
/// (`TooLarge` past either).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured(Result<(), Status>);

pub fn measure(report_dir: &Path) -> Measured {
    Measured(match folder_bytes(report_dir) {
        Some(n) if n <= MAX_FOLDER => Ok(()),
        _ => Err(Status::TooLarge),
    })
}

/// `<report dir>/<sub>`: a real directory (not a link) with at most `max_entries` entries, in a folder that
/// [`measure`] passed. `report_dir` is the real path (`task_report::real_dir`).
pub(crate) fn sub_dir(report_dir: &Path, sub: &str, max_entries: usize, size: Measured) -> Sub {
    let p = report_dir.join(sub);
    match std::fs::symlink_metadata(&p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Sub::Bad(Status::Missing),
        Err(_) => return Sub::Bad(Status::Refused),
        Ok(m) if !m.file_type().is_dir() => return Sub::Bad(Status::Refused),
        Ok(_) => {}
    }
    let Ok(entries) = std::fs::read_dir(&p) else { return Sub::Bad(Status::Refused) };
    if entries.take(max_entries + 1).count() > max_entries {
        return Sub::Bad(Status::TooMany);
    }
    if let Err(s) = size.0 {
        return Sub::Bad(s);
    }
    Sub::Ok(p)
}

/// Bytes of the report folder's direct files and of the files directly in its sub-folders (no deeper, no links).
/// `None` when the folder cannot be read or holds more than [`MAX_FOLDER_STATS`] entries in all.
fn folder_bytes(dir: &Path) -> Option<u64> {
    let mut total = 0u64;
    let mut seen = 0usize;
    let mut subs = Vec::new();
    for e in std::fs::read_dir(dir).ok()? {
        seen += 1;
        if seen > MAX_FOLDER_STATS {
            return None;
        }
        let Ok(e) = e else { continue };
        let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
        if m.file_type().is_file() {
            total = total.saturating_add(m.len());
        } else if m.file_type().is_dir() {
            subs.push(e.path());
        }
    }
    for sub in subs {
        let Ok(inner) = std::fs::read_dir(&sub) else { continue };
        for f in inner {
            seen += 1;
            if seen > MAX_FOLDER_STATS {
                return None;
            }
            let Ok(f) = f else { continue };
            if let Ok(fm) = std::fs::symlink_metadata(f.path()) {
                if fm.file_type().is_file() {
                    total = total.saturating_add(fm.len());
                }
            }
        }
    }
    Some(total)
}

/// A small least-recently-used map (the evidence, file and attachment checks).
pub(crate) struct Lru<K, V> {
    map: HashMap<K, (V, u64)>,
    tick: u64,
    cap: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Lru<K, V> {
    pub(crate) fn new(cap: usize) -> Self {
        Lru { map: HashMap::new(), tick: 0, cap }
    }

    pub(crate) fn get(&mut self, k: &K) -> Option<V> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(k).map(|(v, at)| {
            *at = t;
            v.clone()
        })
    }

    pub(crate) fn put(&mut self, k: K, v: V) {
        self.tick += 1;
        if self.map.len() >= self.cap && !self.map.contains_key(&k) {
            if let Some(old) = self.map.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| k.clone()) {
                self.map.remove(&old);
            }
        }
        self.map.insert(k, (v, self.tick));
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

/// Opens `<dir>/<name>` the way `report.json` is opened (no link, one hard link, regular file, the descriptor's real
/// path re-checked), refuses it over `max` bytes, and reads it.
pub(crate) fn read_in(dir: &Path, name: &str, max: u64) -> Result<(Vec<u8>, Sig), Status> {
    use crate::artifact_preview::{fd_path, open_flags, single_link_regular};
    if !valid_name(name) {
        return Err(Status::Refused);
    }
    let path = dir.join(name);
    let file = open_flags(&path).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { Status::Missing } else { Status::Refused })?;
    let meta = single_link_regular(&file).ok_or(Status::Refused)?;
    if fd_path(&file).is_some_and(|real| real != path) {
        return Err(Status::Refused);
    }
    if meta.len() > max {
        return Err(Status::TooLarge);
    }
    let mut buf = Vec::with_capacity(meta.len() as usize);
    file.take(max + 1).read_to_end(&mut buf).map_err(|_| Status::Refused)?;
    if buf.len() as u64 > max {
        return Err(Status::TooLarge);
    }
    Ok((buf, sig(&meta)))
}

/// Stat-only signature of `<dir>/<name>` for cache lookups (`None` = cannot tell; read it).
fn stat_sig(dir: &Path, name: &str) -> Option<Sig> {
    let m = std::fs::symlink_metadata(dir.join(name)).ok()?;
    m.file_type().is_file().then(|| sig(&m))
}

// ---------- images: structure walk, no decoding ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image {
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

fn dims_ok(w: u32, h: u32) -> bool {
    w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE && u64::from(w) * u64::from(h) <= MAX_PIXELS
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}

/// PNG: signature, then chunks in order; IHDR (13 bytes) first, every length inside the file, IEND last and at the end.
fn png(b: &[u8]) -> Option<Image> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !b.starts_with(SIG) {
        return None;
    }
    let mut at = SIG.len();
    let mut dims = None;
    let mut first = true;
    loop {
        let len = be32(b, at)? as usize;
        let kind = b.get(at + 4..at + 8)?;
        let end = at.checked_add(12)?.checked_add(len)?;
        if end > b.len() {
            return None;
        }
        if first {
            if kind != b"IHDR" || len != 13 {
                return None;
            }
            dims = Some((be32(b, at + 8)?, be32(b, at + 12)?));
            first = false;
        } else if kind == b"IHDR" {
            return None;
        }
        at = end;
        if kind == b"IEND" {
            break;
        }
    }
    if at != b.len() {
        return None;
    }
    let (w, h) = dims?;
    dims_ok(w, h).then_some(Image { mime: "image/png", width: w, height: h })
}

/// JPEG: `FF D8`, then marker segments by their length fields; a frame header (SOF0–SOF15 without C4, C8, CC)
/// before the first SOS, every segment inside the file, and `FF D9` at the end.
fn jpeg(b: &[u8]) -> Option<Image> {
    if !b.starts_with(&[0xFF, 0xD8]) || !b.ends_with(&[0xFF, 0xD9]) || b.len() < 4 {
        return None;
    }
    let mut at = 2;
    let mut dims = None;
    loop {
        if *b.get(at)? != 0xFF {
            return None;
        }
        // Fill bytes.
        while *b.get(at + 1)? == 0xFF {
            at += 1;
        }
        let m = *b.get(at + 1)?;
        at += 2;
        match m {
            0xD8 | 0x00 => return None,
            0x01 | 0xD0..=0xD7 => continue,
            0xD9 => return None,
            _ => {}
        }
        let len = be16(b, at)? as usize;
        if len < 2 || at + len > b.len() {
            return None;
        }
        if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            if len < 8 {
                return None;
            }
            let h = be16(b, at + 3)?;
            let w = be16(b, at + 5)?;
            dims = Some((u32::from(w), u32::from(h)));
        }
        if m == 0xDA {
            let (w, h) = dims?;
            return dims_ok(w, h).then_some(Image { mime: "image/jpeg", width: w, height: h });
        }
        at += len;
    }
}

/// The image kind from its content (never the extension), with its size checked.
pub fn image_info(b: &[u8]) -> Option<Image> {
    png(b).or_else(|| jpeg(b))
}

// ---------- test reports ----------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSummary {
    pub tool: String,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    /// `classname.name`, at most 50 × 200 characters.
    pub failures: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryJson {
    format: String,
    #[serde(default)]
    tool: String,
    passed: u32,
    failed: u32,
    #[serde(default)]
    skipped: u32,
    #[serde(default)]
    failures: Vec<SummaryFailure>,
}

#[derive(Deserialize)]
struct SummaryFailure {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    message: String,
}

fn summary_json(b: &[u8]) -> Option<TestSummary> {
    let s: SummaryJson = serde_json::from_slice(b).ok()?;
    if s.format != "dduck-test-summary" || s.failures.len() > 10_000 {
        return None;
    }
    Some(TestSummary {
        tool: crate::text::display_safe(&s.tool, 40),
        passed: s.passed,
        failed: s.failed,
        skipped: s.skipped,
        failures: s.failures.iter().take(MAX_FAILURES).map(|f| crate::text::display_safe(&f.name, FAILURE_CHARS)).collect(),
    })
}

fn known_entity(name: &str) -> bool {
    matches!(name, "lt" | "gt" | "amp" | "apos" | "quot")
}

/// JUnit XML with quick-xml events: a document type or processing instruction refuses the file (no string search),
/// only the five predefined entities and character references, depth 32, 200,000 elements.
fn junit(b: &[u8]) -> Option<TestSummary> {
    use quick_xml::events::Event;
    let text = std::str::from_utf8(b).ok()?;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<String> = Vec::new();
    let mut elements = 0usize;
    let mut seen_root = false;
    // Root `testsuites` attributes, each one optional: a missing one falls back to the suites (QA LOW).
    let mut root_counts: [Option<u32>; 4] = [None; 4];
    let mut suite_counts: [u32; 4] = [0; 4];
    let mut suite_seen = false;
    let mut tool = String::new();
    let mut cases = 0u32;
    let mut case_failed = 0u32;
    let mut case_skipped = 0u32;
    let mut failures: Vec<String> = Vec::new();
    // The open testcase: (name, failed, skipped).
    let mut case: Option<(String, bool, bool)> = None;
    let attrs_of = |e: &quick_xml::events::BytesStart| -> Option<HashMap<String, String>> {
        let mut m = HashMap::new();
        for a in e.attributes() {
            let a = a.ok()?;
            let key = a.key.local_name().as_ref().to_string();
            let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok()?.into_owned();
            m.insert(key, v);
        }
        Some(m)
    };
    let num = |m: &HashMap<String, String>, k: &str| m.get(k).and_then(|v| v.trim().parse::<u32>().ok());
    loop {
        let ev = reader.read_event().ok()?;
        let (start, empty) = match &ev {
            Event::Start(e) => (Some(e.clone()), false),
            Event::Empty(e) => (Some(e.clone()), true),
            _ => (None, false),
        };
        if let Some(e) = start {
            elements += 1;
            if elements > XML_ELEMENTS || stack.len() >= XML_DEPTH {
                return None;
            }
            let name = e.local_name().as_ref().to_string();
            let attrs = attrs_of(&e)?;
            if stack.is_empty() {
                if seen_root || !matches!(name.as_str(), "testsuites" | "testsuite") {
                    return None;
                }
                seen_root = true;
                tool = crate::text::display_safe(attrs.get("name").map_or("", String::as_str), 40);
            }
            let parent = stack.last().map(String::as_str);
            match name.as_str() {
                "testsuites" if stack.is_empty() => {
                    root_counts = [num(&attrs, "tests"), num(&attrs, "failures"), num(&attrs, "errors"), num(&attrs, "skipped").or_else(|| num(&attrs, "disabled"))];
                }
                // Only top-level suites are summed (nested suites would count twice).
                "testsuite" if matches!(parent, None | Some("testsuites")) => {
                    if let Some(t) = num(&attrs, "tests") {
                        suite_seen = true;
                        suite_counts[0] = suite_counts[0].saturating_add(t);
                        suite_counts[1] = suite_counts[1].saturating_add(num(&attrs, "failures").unwrap_or(0));
                        suite_counts[2] = suite_counts[2].saturating_add(num(&attrs, "errors").unwrap_or(0));
                        suite_counts[3] = suite_counts[3].saturating_add(num(&attrs, "skipped").unwrap_or(0));
                    }
                }
                "testcase" => {
                    cases = cases.saturating_add(1);
                    let n = attrs.get("name").cloned().unwrap_or_default();
                    let full = match attrs.get("classname").filter(|c| !c.is_empty()) {
                        Some(c) => format!("{c}.{n}"),
                        None => n,
                    };
                    case = Some((full, false, false));
                    if empty {
                        close_case(&mut case, &mut case_failed, &mut case_skipped, &mut failures);
                    }
                }
                "failure" | "error" if parent == Some("testcase") => {
                    if let Some(c) = case.as_mut() {
                        c.1 = true;
                    }
                }
                "skipped" if parent == Some("testcase") => {
                    if let Some(c) = case.as_mut() {
                        c.2 = true;
                    }
                }
                _ => {}
            }
            if !empty {
                stack.push(name);
            }
            continue;
        }
        match ev {
            Event::End(_) => {
                let name = stack.pop()?;
                if name == "testcase" {
                    close_case(&mut case, &mut case_failed, &mut case_skipped, &mut failures);
                }
            }
            Event::Decl(_) => {
                if seen_root || elements > 0 {
                    return None;
                }
            }
            Event::DocType(_) | Event::PI(_) => return None,
            Event::GeneralRef(r) => {
                if !r.is_char_ref() && !known_entity(&r) {
                    return None;
                }
                if r.is_char_ref() && r.resolve_char_ref().ok().flatten().is_none() {
                    return None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root || !stack.is_empty() {
        return None;
    }
    // Field by field: the root's number, else the top-level suites' sum, else the counted testcases.
    let fallback = if suite_seen { suite_counts } else { [cases, case_failed, 0, case_skipped] };
    let pick = |i: usize| root_counts[i].unwrap_or(fallback[i]);
    let (tests, failed, errors, skipped) = (pick(0), pick(1), pick(2), pick(3));
    let bad = failed.saturating_add(errors);
    Some(TestSummary { tool, passed: tests.saturating_sub(bad).saturating_sub(skipped), failed: bad, skipped, failures })
}

fn close_case(case: &mut Option<(String, bool, bool)>, failed: &mut u32, skipped: &mut u32, names: &mut Vec<String>) {
    if let Some((name, f, s)) = case.take() {
        if f {
            *failed = failed.saturating_add(1);
            if names.len() < MAX_FAILURES {
                names.push(crate::text::display_safe(&name, FAILURE_CHARS));
            }
        } else if s {
            *skipped = skipped.saturating_add(1);
        }
    }
}

/// A test report: the D.Duck summary JSON (`{`) or JUnit XML.
pub fn test_summary(b: &[u8]) -> Option<TestSummary> {
    let b = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b);
    let first = b.iter().find(|c| !c.is_ascii_whitespace())?;
    if *first == b'{' { summary_json(b) } else { junit(b) }
}

// ---------- command output ----------

/// Head and tail lines of a command's output, display-safe, at most 16 KB. `true` when lines were left out.
pub fn excerpt(b: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(b);
    let lines: Vec<&str> = text.lines().collect();
    let pick: Vec<&str> = if lines.len() > HEAD_LINES + TAIL_LINES {
        lines[..HEAD_LINES].iter().chain(std::iter::once(&"…")).chain(&lines[lines.len() - TAIL_LINES..]).copied().collect()
    } else {
        lines.clone()
    };
    let mut out = String::new();
    let mut cut = lines.len() > HEAD_LINES + TAIL_LINES;
    for l in pick {
        let clean: String = l.chars().map(|c| if c == '\t' { ' ' } else { c }).filter(|c| !crate::text::is_unsafe_char(*c)).collect();
        if out.len() + clean.len() + 1 > EXCERPT_BYTES {
            cut = true;
            break;
        }
        out.push_str(&clean);
        out.push('\n');
    }
    (out, cut)
}

// ---------- checking items ----------

/// What a report check knows about the task: the real report folder and the task folder (`file` items).
pub struct Ctx<'a> {
    pub report_id: &'a str,
    pub report_dir: &'a Path,
    pub cwd: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Checked {
    status: Status,
    failed: Option<u32>,
}

type CacheKey = (String, String, Sig);
static CACHE: LazyLock<Mutex<Lru<CacheKey, Checked>>> = LazyLock::new(|| Mutex::new(Lru::new(CACHE_ENTRIES)));
/// Line counts of `file` targets by real path and signature: a poll does not read the file again.
type LineCache = Lru<(PathBuf, Sig), Option<usize>>;
static LINES: LazyLock<Mutex<LineCache>> = LazyLock::new(|| Mutex::new(Lru::new(CACHE_ENTRIES)));

fn cache_get(k: &CacheKey) -> Option<Checked> {
    CACHE.lock().ok()?.get(k)
}

fn cache_put(k: CacheKey, v: Checked) {
    if let Ok(mut c) = CACHE.lock() {
        c.put(k, v);
    }
}

/// Checks an evidence file by kind: image structure, test report parse, or command output without NUL.
fn check_file(ctx: &Ctx, dir: &Path, name: &str, kind: Kind) -> Checked {
    let max = match kind {
        Kind::Screenshot => MAX_SCREENSHOT,
        Kind::TestReport => MAX_TEST_REPORT,
        _ => MAX_OUTPUT,
    };
    let key_of = |s: Sig| (ctx.report_id.to_string(), name.to_string(), s);
    if let Some(hit) = stat_sig(dir, name).and_then(|s| cache_get(&key_of(s))) {
        return hit;
    }
    let (bytes, s) = match read_in(dir, name, max) {
        Ok(x) => x,
        Err(status) => return Checked { status, failed: None },
    };
    let checked = match kind {
        Kind::Screenshot => Checked { status: if image_info(&bytes).is_some() { Status::Valid } else { Status::BadType }, failed: None },
        Kind::TestReport => match test_summary(&bytes) {
            Some(t) => Checked { status: Status::Valid, failed: Some(t.failed) },
            None => Checked { status: Status::BadType, failed: None },
        },
        _ => Checked { status: if bytes.contains(&0) { Status::BadType } else { Status::Valid }, failed: None },
    };
    cache_put(key_of(s), checked);
    checked
}

/// `cwd` resolved, the target resolved under it by path component, not under `.git`, a regular file of at most
/// 2 MB with at least `end` lines.
fn check_repo_file(cwd: Option<&Path>, path: &str, lines: Option<[u32; 2]>) -> Status {
    let Some(real_cwd) = cwd.and_then(|c| crate::platform::canon(c).ok()) else { return Status::Missing };
    let target = real_cwd.join(path);
    let real = match crate::platform::canon(&target) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Status::Missing,
        Err(_) => return Status::Refused,
    };
    let Ok(rel) = real.strip_prefix(&real_cwd) else { return Status::Refused };
    if rel.components().any(|c| matches!(c, Component::Normal(n) if n == ".git")) {
        return Status::Refused;
    }
    let Ok(m) = std::fs::metadata(&real) else { return Status::Missing };
    if !m.is_file() {
        return Status::Refused;
    }
    if m.len() > MAX_FILE {
        return Status::TooLarge;
    }
    let Some([_, end]) = lines else { return Status::Valid };
    let key = (real.clone(), sig(&m));
    let count = match LINES.lock().ok().and_then(|mut c| c.get(&key)) {
        Some(hit) => hit,
        None => {
            let Ok(f) = crate::artifact_preview::open_flags(&real) else { return Status::Refused };
            let mut buf = Vec::new();
            let count = if f.take(MAX_FILE + 1).read_to_end(&mut buf).is_err() || buf.contains(&0) {
                None
            } else {
                Some(buf.split(|b| *b == b'\n').count() - usize::from(buf.ends_with(b"\n")))
            };
            if let Ok(mut c) = LINES.lock() {
                c.put(key, count);
            }
            count
        }
    };
    match count {
        None => Status::BadType,
        Some(n) if n >= end as usize => Status::Valid,
        Some(_) => Status::Missing,
    }
}

/// The evidence folder once per report check.
pub struct Folder(Sub);

impl Folder {
    /// Measures the report folder itself (one preview, tests).
    pub fn open(report_dir: &Path) -> Self {
        Self::open_with(report_dir, measure(report_dir))
    }

    /// With the view's one measurement.
    pub fn open_with(report_dir: &Path, size: Measured) -> Self {
        Folder(sub_dir(report_dir, DIR, MAX_ENTRIES, size))
    }
}

/// One item's view: its label, caption and what the app found.
pub fn view(ctx: &Ctx, folder: &Folder, it: &Item) -> ItemView {
    let in_folder = |path: &str, kind: Kind| -> Checked {
        match &folder.0 {
            Sub::Bad(s) => Checked { status: *s, failed: None },
            Sub::Ok(dir) => check_file(ctx, dir, path.strip_prefix("evidence/").unwrap_or(""), kind),
        }
    };
    let (checked, exit_code) = match it {
        Item::Screenshot { path, .. } => (in_folder(path, Kind::Screenshot), None),
        Item::TestReport { path, .. } => (in_folder(path, Kind::TestReport), None),
        Item::Command { output: Some(o), exit_code, .. } => (in_folder(o, Kind::Command), Some(*exit_code)),
        Item::Command { output: None, exit_code, .. } => (Checked { status: Status::Missing, failed: None }, Some(*exit_code)),
        Item::File { path, lines, .. } => (Checked { status: check_repo_file(ctx.cwd, path, *lines), failed: None }, None),
    };
    let caption = match it {
        Item::Screenshot { caption, .. } | Item::TestReport { caption, .. } | Item::Command { caption, .. } | Item::File { caption, .. } => caption.clone(),
    };
    ItemView {
        kind: it.kind(),
        label: label_of(it),
        caption,
        status: checked.status,
        failed: checked.failed.filter(|_| checked.status == Status::Valid),
        exit_code,
    }
}

// ---------- one preview, on request ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Preview {
    /// `data:` URL with the MIME the structure check found.
    Image { data_url: String, width: u32, height: u32 },
    Tests { summary: TestSummary },
    Command { cmd: String, exit_code: i64, excerpt: String, cut: bool },
    /// Path and line range only (the file's content is not sent).
    File { path: String, lines: Option<[u32; 2]> },
    Invalid { status: Status },
}

/// The preview of one item. Reads the file again (never from a cache) with the same checks as [`view`].
pub fn preview(ctx: &Ctx, it: &Item) -> Preview {
    use base64::Engine;
    let folder = Folder::open(ctx.report_dir);
    let read = |path: &str, max: u64| -> Result<Vec<u8>, Status> {
        match &folder.0 {
            Sub::Bad(s) => Err(*s),
            Sub::Ok(dir) => read_in(dir, path.strip_prefix("evidence/").unwrap_or(""), max).map(|(b, _)| b),
        }
    };
    match it {
        Item::Screenshot { path, .. } => match read(path, MAX_SCREENSHOT) {
            Err(status) => Preview::Invalid { status },
            Ok(b) => match image_info(&b) {
                Some(img) => Preview::Image {
                    data_url: format!("data:{};base64,{}", img.mime, base64::engine::general_purpose::STANDARD.encode(&b)),
                    width: img.width,
                    height: img.height,
                },
                None => Preview::Invalid { status: Status::BadType },
            },
        },
        Item::TestReport { path, .. } => match read(path, MAX_TEST_REPORT) {
            Err(status) => Preview::Invalid { status },
            Ok(b) => test_summary(&b).map_or(Preview::Invalid { status: Status::BadType }, |summary| Preview::Tests { summary }),
        },
        Item::Command { cmd, exit_code, output, .. } => match output.as_deref().map(|o| read(o, MAX_OUTPUT)) {
            None => Preview::Invalid { status: Status::Missing },
            Some(Err(status)) => Preview::Invalid { status },
            Some(Ok(b)) if b.contains(&0) => Preview::Invalid { status: Status::BadType },
            Some(Ok(b)) => {
                let (excerpt, cut) = excerpt(&b);
                Preview::Command { cmd: cmd.clone(), exit_code: *exit_code, excerpt, cut }
            }
        },
        Item::File { path, lines, .. } => match check_repo_file(ctx.cwd, path, *lines) {
            Status::Valid => Preview::File { path: path.clone(), lines: *lines },
            status => Preview::Invalid { status },
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub(crate) mod tests {
    use super::*;

    /// A valid PNG of `w`×`h` (structure only: IHDR, one IDAT, IEND; CRCs are not checked).
    pub fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
        let chunk = |b: &mut Vec<u8>, kind: &[u8], data: &[u8]| {
            b.extend_from_slice(&(data.len() as u32).to_be_bytes());
            b.extend_from_slice(kind);
            b.extend_from_slice(data);
            b.extend_from_slice(&[0, 0, 0, 0]);
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut b, b"IHDR", &ihdr);
        chunk(&mut b, b"IDAT", &[1, 2, 3]);
        chunk(&mut b, b"IEND", &[]);
        b
    }

    pub fn jpeg_bytes(w: u16, h: u16) -> Vec<u8> {
        let mut b = vec![0xFF, 0xD8];
        b.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46]);
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        b.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        b.extend_from_slice(&[0x12, 0x34, 0xFF, 0xD9]);
        b
    }

    #[test]
    fn names_and_items_follow_the_shape() {
        for ok in ["shot.png", "list-dark_2.PNG", "a.b.c.jpg"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "..", ".", "a/b.png", "a b.png", &"a".repeat(81), "가.png", "a\0.png"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        let s = |p: &str| Item::Screenshot { path: p.into(), caption: String::new() };
        assert!(s("evidence/shot.png").valid());
        for bad in ["../other/shot.png", "/tmp/shot.png", "evidence/sub/shot.png", "evidence/shot.svg", "evidence/shot.html", "shot.png", "evidence/../x.png"] {
            assert!(!s(bad).valid(), "{bad}");
        }
        let f = |p: &str, l: Option<[u32; 2]>| Item::File { path: p.into(), lines: l, caption: String::new() };
        assert!(f("src/a.ts", Some([10, 42])).valid());
        for (p, l) in [("../a.ts", None), ("/etc/passwd", None), (".git/config", None), ("a/.git/HEAD", None), ("src/a.ts", Some([0, 1])), ("src/a.ts", Some([5, 4])), ("src/a.ts", Some([1, 2002]))] {
            assert!(!f(p, l).valid(), "{p} {l:?}");
        }
        let c = Item::Command { cmd: "npm test".into(), exit_code: 0, output: Some("evidence/out.png".into()), caption: String::new() };
        assert!(!c.valid(), "command output is txt/log only");
    }

    #[test]
    fn images_are_walked_not_trusted() {
        assert_eq!(image_info(&png_bytes(1440, 900)), Some(Image { mime: "image/png", width: 1440, height: 900 }));
        assert_eq!(image_info(&jpeg_bytes(800, 600)).map(|i| (i.mime, i.width)), Some(("image/jpeg", 800)));
        // Too large, truncated, no IHDR first, trailing bytes, HTML with a .png name.
        assert!(image_info(&png_bytes(8001, 10)).is_none());
        assert!(image_info(&png_bytes(8000, 8000)).is_none(), "64M pixels");
        let p = png_bytes(10, 10);
        assert!(image_info(&p[..p.len() - 3]).is_none());
        let mut no_ihdr = p.clone();
        no_ihdr[12..16].copy_from_slice(b"IDAT");
        assert!(image_info(&no_ihdr).is_none());
        let mut trailing = p.clone();
        trailing.push(0);
        assert!(image_info(&trailing).is_none());
        assert!(image_info(b"<html><script>x</script></html>").is_none());
        let j = jpeg_bytes(10, 10);
        let mut long_seg = j.clone();
        long_seg[4..6].copy_from_slice(&[0xFF, 0xF0]);
        assert!(image_info(&long_seg).is_none(), "segment past the end");
        let mut no_sof = j.clone();
        no_sof[9] = 0xC4;
        assert!(image_info(&no_sof).is_none(), "SOS before a frame header");
        assert!(image_info(&j[..j.len() - 2]).is_none(), "cut before EOI");
    }

    const VITEST: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<testsuites name="vitest tests" tests="51" failures="2" errors="0" time="1.2">
  <testsuite name="src/a.test.ts" tests="51" failures="2" errors="0" skipped="1">
    <testcase classname="src/a.test.ts" name="draws &amp; counts"></testcase>
    <testcase classname="src/a.test.ts" name="fails one"><failure message="x">expected 1 &#x3C; 2</failure></testcase>
    <testcase classname="src/a.test.ts" name="fails two"><failure/></testcase>
    <testcase classname="src/a.test.ts" name="skips"><skipped/></testcase>
  </testsuite>
</testsuites>"#;

    #[test]
    fn junit_counts_and_failure_names() {
        let t = test_summary(VITEST.as_bytes()).unwrap();
        assert_eq!((t.tool.as_str(), t.passed, t.failed, t.skipped), ("vitest tests", 48, 2, 1));
        assert_eq!(t.failures, ["src/a.test.ts.fails one", "src/a.test.ts.fails two"]);
        // pytest style: one testsuite root, no testsuites.
        let py = r#"<testsuite name="pytest" tests="3" failures="0" errors="1" skipped="0"><testcase classname="t" name="a"/><testcase classname="t" name="b"><error/></testcase><testcase classname="t" name="c"/></testsuite>"#;
        let t = test_summary(py.as_bytes()).unwrap();
        assert_eq!((t.passed, t.failed), (2, 1));
        // No count attributes: testcases are counted.
        let bare = r#"<testsuites><testsuite name="n"><testcase name="a"/><testcase name="b"><failure/></testcase></testsuite></testsuites>"#;
        let t = test_summary(bare.as_bytes()).unwrap();
        assert_eq!((t.passed, t.failed, t.failures.len()), (1, 1, 1));
    }

    #[test]
    fn missing_root_attributes_fall_back_to_the_suites() {
        let x = r#"<testsuites tests="5"><testsuite name="a" tests="3" failures="1" errors="1" skipped="0"><testcase name="x"/></testsuite><testsuite name="b" tests="2" failures="1"><testcase name="y"/></testsuite></testsuites>"#;
        let t = test_summary(x.as_bytes()).unwrap();
        assert_eq!((t.passed, t.failed, t.skipped), (2, 3, 0), "failures and errors come from the suites");
        let root_wins = r#"<testsuites tests="4" failures="0"><testsuite tests="4" failures="2"/></testsuites>"#;
        assert_eq!(test_summary(root_wins.as_bytes()).unwrap().failed, 0, "a root attribute that is there is used");
    }

    #[test]
    fn hostile_xml_is_refused() {
        let laughs = r#"<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;&lol;">]><testsuites><testsuite><testcase name="&lol2;"/></testsuite></testsuites>"#;
        assert!(test_summary(laughs.as_bytes()).is_none());
        let pi = r#"<?xml version="1.0"?><?xml-stylesheet href="x"?><testsuites/>"#;
        assert!(test_summary(pi.as_bytes()).is_none());
        let unknown = r#"<testsuites><testsuite><testcase name="a">&nbsp;</testcase></testsuite></testsuites>"#;
        assert!(test_summary(unknown.as_bytes()).is_none());
        let unknown_attr = r#"<testsuites name="&foo;"/>"#;
        assert!(test_summary(unknown_attr.as_bytes()).is_none());
        let deep = format!("<testsuites>{}{}</testsuites>", "<a>".repeat(40), "</a>".repeat(40));
        assert!(test_summary(deep.as_bytes()).is_none());
        assert!(test_summary(&VITEST.as_bytes()[..VITEST.len() - 20]).is_none(), "cut file");
        assert!(test_summary(b"<html><body>no</body></html>").is_none());
        assert!(test_summary(b"<testsuites/><testsuites/>").is_none(), "two roots");
    }

    #[test]
    fn summary_json_format() {
        let j = r#"{"format":"dduck-test-summary","tool":"cargo test","passed":40,"failed":1,"skipped":0,"failures":[{"name":"core::a","message":"boom"}]}"#;
        let t = test_summary(j.as_bytes()).unwrap();
        assert_eq!((t.tool.as_str(), t.passed, t.failed, t.failures[0].as_str()), ("cargo test", 40, 1, "core::a"));
        assert!(test_summary(br#"{"format":"other","passed":1,"failed":0}"#).is_none());
    }

    #[test]
    fn excerpt_keeps_head_and_tail() {
        let text: String = (1..=500).map(|i| format!("line {i}\n")).collect();
        let (e, cut) = excerpt(text.as_bytes());
        assert!(cut && e.starts_with("line 1\n") && e.contains("line 80\n…\nline 381\n") && e.ends_with("line 500\n"));
        assert!(!e.contains("line 81\n"));
        let (e, cut) = excerpt(b"ok\x1b[31m red\n");
        assert!(!cut && !e.contains('\x1b'));
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let real = crate::platform::canon(t.path()).unwrap();
        let rep = real.join("rep");
        std::fs::create_dir_all(rep.join(DIR)).unwrap();
        let cwd = real.join("proj");
        std::fs::create_dir_all(cwd.join("src")).unwrap();
        std::fs::create_dir_all(cwd.join(".git")).unwrap();
        std::fs::write(cwd.join(".git/config"), "x\n").unwrap();
        std::fs::write(cwd.join("src/a.ts"), "a\n".repeat(50)).unwrap();
        (t, rep, cwd)
    }

    fn shot(name: &str) -> Item {
        Item::Screenshot { path: format!("evidence/{name}"), caption: String::new() }
    }

    #[test]
    fn items_are_checked_in_place() {
        let (_t, rep, cwd) = setup();
        let ctx = Ctx { report_id: "r1", report_dir: &rep, cwd: Some(&cwd) };
        std::fs::write(rep.join("evidence/ok.png"), png_bytes(100, 80)).unwrap();
        std::fs::write(rep.join("evidence/fake.png"), "<html>").unwrap();
        std::fs::write(rep.join("evidence/v.xml"), VITEST).unwrap();
        std::fs::write(rep.join("evidence/out.txt"), "ok\n").unwrap();
        std::fs::write(rep.join("evidence/nul.txt"), b"a\0b").unwrap();
        let folder = Folder::open(&rep);
        let st = |it: &Item| view(&ctx, &folder, it).status;
        assert_eq!(st(&shot("ok.png")), Status::Valid);
        assert_eq!(st(&shot("fake.png")), Status::BadType);
        assert_eq!(st(&shot("gone.png")), Status::Missing);
        let v = view(&ctx, &folder, &Item::TestReport { path: "evidence/v.xml".into(), caption: String::new() });
        assert_eq!((v.status, v.failed, v.label.as_str()), (Status::Valid, Some(2), "v.xml"));
        let cmd = |o: &str| Item::Command { cmd: "npm test".into(), exit_code: 1, output: Some(format!("evidence/{o}")), caption: String::new() };
        assert_eq!(st(&cmd("out.txt")), Status::Valid);
        assert_eq!(st(&cmd("nul.txt")), Status::BadType);
        let file = |p: &str, l: Option<[u32; 2]>| Item::File { path: p.into(), lines: l, caption: String::new() };
        let v = view(&ctx, &folder, &file("src/a.ts", Some([10, 42])));
        assert_eq!((v.status, v.label.as_str()), (Status::Valid, "src/a.ts:10–42"));
        assert_eq!(st(&file("src/a.ts", Some([10, 51]))), Status::Missing, "past the last line");
        assert_eq!(st(&file("src/none.ts", None)), Status::Missing);
        // `.git` reached through a link inside the folder is refused after resolving.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(cwd.join(".git"), cwd.join("g")).unwrap();
            assert_eq!(st(&file("g/config", None)), Status::Refused);
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("x.ts"), "x").unwrap();
            std::os::unix::fs::symlink(outside.path().join("x.ts"), cwd.join("src/out.ts")).unwrap();
            assert_eq!(st(&file("src/out.ts", None)), Status::Refused);
        }
        // The task folder (worktree) is gone.
        let gone = Ctx { report_id: "r1", report_dir: &rep, cwd: Some(Path::new("/nonexistent/worktree")) };
        assert_eq!(view(&gone, &folder, &file("src/a.ts", None)).status, Status::Missing);
        // The view carries no path of the report folder.
        let json = serde_json::to_string(&view(&ctx, &folder, &shot("ok.png"))).unwrap();
        assert!(!json.contains(&*rep.to_string_lossy()) && !json.contains("evidence/"));
    }

    #[cfg(unix)]
    #[test]
    fn links_and_special_files_are_refused() {
        let (_t, rep, cwd) = setup();
        let ctx = Ctx { report_id: "r2", report_dir: &rep, cwd: Some(&cwd) };
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("id_rsa");
        std::fs::write(&secret, png_bytes(10, 10)).unwrap();
        std::os::unix::fs::symlink(&secret, rep.join("evidence/link.png")).unwrap();
        std::fs::hard_link(&secret, rep.join("evidence/hard.png")).unwrap();
        let c = std::ffi::CString::new(rep.join("evidence/fifo.png").to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let folder = Folder::open(&rep);
        for n in ["link.png", "hard.png", "fifo.png"] {
            assert_eq!(view(&ctx, &folder, &shot(n)).status, Status::Refused, "{n}");
            assert_eq!(preview(&ctx, &shot(n)), Preview::Invalid { status: Status::Refused });
        }
        // A linked evidence folder refuses everything in it.
        std::fs::rename(rep.join(DIR), rep.join("old")).unwrap();
        std::os::unix::fs::symlink(outside.path(), rep.join(DIR)).unwrap();
        assert_eq!(view(&ctx, &Folder::open(&rep), &shot("id_rsa.png")).status, Status::Refused);
    }

    #[test]
    fn too_many_entries_and_too_large() {
        let (_t, rep, cwd) = setup();
        let ctx = Ctx { report_id: "r3", report_dir: &rep, cwd: Some(&cwd) };
        for i in 0..=MAX_ENTRIES {
            std::fs::write(rep.join(format!("evidence/f{i}.txt")), "x").unwrap();
        }
        let item = Item::Command { cmd: "ls".into(), exit_code: 0, output: Some("evidence/f1.txt".into()), caption: String::new() };
        assert_eq!(view(&ctx, &Folder::open(&rep), &item).status, Status::TooMany);
        std::fs::remove_file(rep.join("evidence/f0.txt")).unwrap();
        assert_eq!(view(&ctx, &Folder::open(&rep), &item).status, Status::Valid);
        std::fs::write(rep.join("evidence/big.png"), vec![0u8; MAX_SCREENSHOT as usize + 1]).unwrap();
        std::fs::remove_file(rep.join("evidence/f1.txt")).unwrap();
        assert_eq!(view(&ctx, &Folder::open(&rep), &shot("big.png")).status, Status::TooLarge);
    }

    #[test]
    fn huge_folders_stop_early_and_the_cache_keeps_recent_entries() {
        let (_t, rep, cwd) = setup();
        let ctx = Ctx { report_id: "r5", report_dir: &rep, cwd: Some(&cwd) };
        std::fs::write(rep.join("evidence/ok.png"), png_bytes(4, 4)).unwrap();
        assert_eq!(measure(&rep), Measured(Ok(())));
        let junk = rep.join("junk");
        std::fs::create_dir(&junk).unwrap();
        for i in 0..MAX_FOLDER_STATS {
            std::fs::write(junk.join(format!("{i}")), "").unwrap();
        }
        assert_eq!(measure(&rep), Measured(Err(Status::TooLarge)));
        assert_eq!(view(&ctx, &Folder::open(&rep), &shot("ok.png")).status, Status::TooLarge);
        let mut lru = Lru::new(2);
        lru.put(1, "a");
        lru.put(2, "b");
        assert_eq!(lru.get(&1), Some("a"));
        lru.put(3, "c");
        assert_eq!((lru.get(&1), lru.get(&2), lru.get(&3), lru.len()), (Some("a"), None, Some("c"), 2), "the least recently used goes");
    }

    #[test]
    fn previews() {
        let (_t, rep, cwd) = setup();
        let ctx = Ctx { report_id: "r4", report_dir: &rep, cwd: Some(&cwd) };
        std::fs::write(rep.join("evidence/ok.jpg"), jpeg_bytes(30, 20)).unwrap();
        std::fs::write(rep.join("evidence/v.xml"), VITEST).unwrap();
        std::fs::write(rep.join("evidence/out.log"), "a\nb\n").unwrap();
        let Preview::Image { data_url, width, .. } = preview(&ctx, &shot("ok.jpg")) else { panic!() };
        assert!(data_url.starts_with("data:image/jpeg;base64,") && width == 30);
        let Preview::Tests { summary } = preview(&ctx, &Item::TestReport { path: "evidence/v.xml".into(), caption: String::new() }) else { panic!() };
        assert_eq!((summary.passed, summary.failed, summary.skipped), (48, 2, 1));
        let Preview::Command { excerpt, exit_code, .. } = preview(&ctx, &Item::Command { cmd: "x".into(), exit_code: 3, output: Some("evidence/out.log".into()), caption: String::new() }) else { panic!() };
        assert_eq!((excerpt.as_str(), exit_code), ("a\nb\n", 3));
        assert_eq!(preview(&ctx, &Item::File { path: "src/a.ts".into(), lines: Some([1, 2]), caption: String::new() }), Preview::File { path: "src/a.ts".into(), lines: Some([1, 2]) });
    }
}

/// Review H3 [CI]: report evidence on Windows never names a share, a stream, a device or another drive.
#[cfg(all(test, windows))]
mod windows_paths {
    #[test]
    fn evidence_files_follow_the_windows_rule() {
        for bad in [r"\\host\share\a.png", "//host/share/a.png", "a.md:secret", "docs/a.md:s", "CON", "nul", "docs/nul.txt", "C:/Users/me/a.md", "C:a.md", ".GIT/config", "dir./a.md"] {
            assert!(!super::valid_repo_path(bad), "{bad}");
        }
        assert!(super::valid_repo_path("src/main.rs"));
        for bad in ["CON", "nul.png", "a.png:x", "a."] {
            assert!(!super::valid_name(bad), "{bad}");
        }
        assert!(super::valid_name("shot.png"));
        assert!(!crate::install_link::valid_part("CON") && !crate::install_link::valid_part("aux.md"));
    }
}

