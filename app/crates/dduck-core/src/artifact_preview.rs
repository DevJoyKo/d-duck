//! Card previews for collected artifacts (D-102, artifact-cards-preview). On request only, the session's transcripts
//! are scanned for the Artifact tool call that published the artifact, and the local source file it named
//! (`.html`/`.htm`/`.md` under the home folder or the user's own Claude temp folder) is read and returned for a
//! sandboxed render. Paths and content are never stored in the DB, logged or put in an error: callers only get a
//! [`Preview`] or a [`PreviewError`] code. An in-memory cache keeps the per-session scan and the last previews.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use crate::timeline;

/// Source files larger than this are not read.
pub const MAX_SOURCE: u64 = 2 * 1024 * 1024;
/// HTML sent to the webview is cut here (`truncated` is set).
pub const MAX_HTML: usize = 512 * 1024;
/// Markdown is rendered as text, so far less is needed.
pub const MAX_MARKDOWN: usize = 64 * 1024;
/// A transcript line longer than this is skipped without being kept in memory.
const MAX_LINE: usize = 1024 * 1024;
/// Scan limits for one session (main transcript + subagents).
const SCAN_BYTES: u64 = 100 * 1024 * 1024;
const SCAN_FILES: usize = 40;
/// Visible text below this, with a `<script`, means the page is drawn by script.
const SCRIPT_ONLY_TEXT: usize = 40;
const PREVIEW_CACHE: usize = 30;
const SESSION_CACHE: usize = 16;
/// How long a session's transcript list is reused before it is discovered again (the files are re-stat'ed anyway).
const FILES_TTL: Duration = Duration::from_secs(60);

static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9-]{1,64}\z").unwrap_or_else(|_| unreachable!()));
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ahttps://claude\.ai/(?:code/)?artifact/([A-Za-z0-9-]{1,64})\z").unwrap_or_else(|_| unreachable!())
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Preview {
    Html {
        text: String,
        truncated: bool,
        /// Little visible text and a `<script`: the sandbox will show next to nothing.
        #[serde(rename = "scriptOnly")]
        script_only: bool,
    },
    Markdown { text: String, truncated: bool },
}

/// Why there is no preview. This code is all that crosses IPC on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewError {
    /// The publishing call named no usable `.html`/`.htm`/`.md` source.
    NoSource,
    /// The source file is gone.
    Missing,
    /// The path or file failed validation (link, outside the allowed folders, `..`, not a regular file, …).
    Refused,
    /// The source is over [`MAX_SOURCE`].
    TooLarge,
    /// Typed artifacts (decks, designs) need script to render.
    Typed,
    /// Unknown id, missing transcript or an internal error.
    Unknown,
}

/// Where sources may be read from: the canonical home folder (minus `Library` and `.claude`) and, optionally, the
/// user's own Claude temp folder.
#[derive(Debug, Clone)]
pub struct Allowed {
    home: PathBuf,
    tmp: Option<PathBuf>,
}

impl Allowed {
    /// The real locations for this user. `None` when the home folder cannot be resolved.
    pub fn for_user(home: &Path) -> Option<Self> {
        let home = crate::platform::canon(home).ok()?;
        Some(Allowed { home, tmp: user_tmp() })
    }

    /// Fixed locations (tests).
    pub fn new(home: PathBuf, tmp: Option<PathBuf>) -> Self {
        Allowed { home, tmp }
    }

    /// Component-wise containment of an already-resolved path.
    fn permits(&self, p: &Path) -> bool {
        if let Some(rest) = p.strip_prefix(&self.home).ok().filter(|r| r.components().next().is_some()) {
            let first = rest.components().next();
            let denied = matches!(first, Some(Component::Normal(c)) if c == "Library" || c == ".claude");
            return !denied;
        }
        self.tmp.as_ref().is_some_and(|t| p.strip_prefix(t).ok().is_some_and(|r| r.components().next().is_some()))
    }
}

/// `/private/tmp/claude-<uid>` when it is a real directory owned by this user.
fn user_tmp() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: getuid has no preconditions and cannot fail.
        let uid = unsafe { libc::getuid() };
        let p = PathBuf::from(format!("/private/tmp/claude-{uid}"));
        let meta = std::fs::symlink_metadata(&p).ok()?;
        (meta.is_dir() && meta.uid() == uid).then_some(p)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Session id and kind (`page`/`typed`) of a collected artifact. `None` for malformed or unknown ids.
pub fn lookup(conn: &Connection, artifact_id: &str) -> Result<Option<(String, String)>, crate::Error> {
    if !ID_RE.is_match(artifact_id) {
        return Ok(None);
    }
    Ok(conn
        .query_row("SELECT session_id, kind FROM artifacts WHERE artifact_id = ?1", [artifact_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

// ---------- transcript scan ----------

/// What the last publish of an artifact pointed at, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Path(PathBuf),
    None(PreviewError),
}

type Signature = Vec<(PathBuf, Option<SystemTime>, u64)>;

struct SessionEntry {
    files: Vec<PathBuf>,
    found_at: Instant,
    sig: Signature,
    sources: HashMap<String, Source>,
}

struct PreviewEntry {
    path: PathBuf,
    mtime: Option<SystemTime>,
    len: u64,
    preview: Preview,
}

/// In-memory caches only (never written anywhere): per-session scan results keyed by the transcripts' mtime and
/// size, and recent previews keyed by the source's mtime and size.
#[derive(Default)]
pub struct PreviewCache {
    sessions: Mutex<VecDeque<(String, SessionEntry)>>,
    previews: Mutex<VecDeque<(String, PreviewEntry)>>,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self::default()
    }
}

fn signature(files: &[PathBuf]) -> Signature {
    files
        .iter()
        .map(|p| {
            let m = std::fs::symlink_metadata(p).ok();
            (p.clone(), m.as_ref().and_then(|m| m.modified().ok()), m.map(|m| m.len()).unwrap_or(0))
        })
        .collect()
}

fn session_transcripts(projects_root: &Path, session_id: &str) -> Option<Vec<PathBuf>> {
    let f = timeline::session_files(projects_root, session_id)?;
    Some(std::iter::once(f.main.path).chain(f.subagents.into_iter().map(|s| s.path)).take(SCAN_FILES).collect())
}

/// `artifact id → source` for one session, from the cache when the transcripts have not changed.
fn session_sources(cache: &PreviewCache, projects_root: &Path, session_id: &str) -> Result<HashMap<String, Source>, PreviewError> {
    let cached_files = {
        let s = cache.sessions.lock().map_err(|_| PreviewError::Unknown)?;
        s.iter().find(|(k, _)| k == session_id).map(|(_, e)| (e.files.clone(), e.found_at, e.sig.clone(), e.sources.clone()))
    };
    let files = match &cached_files {
        Some((files, at, _, _)) if at.elapsed() < FILES_TTL => files.clone(),
        _ => session_transcripts(projects_root, session_id).ok_or(PreviewError::Unknown)?,
    };
    let sig = signature(&files);
    if let Some((_, _, old_sig, sources)) = &cached_files
        && *old_sig == sig
    {
        return Ok(sources.clone());
    }
    let sources = scan(&files);
    let mut s = cache.sessions.lock().map_err(|_| PreviewError::Unknown)?;
    s.retain(|(k, _)| k != session_id);
    s.push_back((session_id.to_string(), SessionEntry { files, found_at: Instant::now(), sig, sources: sources.clone() }));
    while s.len() > SESSION_CACHE {
        s.pop_front();
    }
    Ok(sources)
}

struct Call {
    cwd: Option<String>,
    input: Value,
}

struct Publish {
    order: (i64, usize, usize),
    tool_use_id: String,
    artifact_id: String,
    typed: bool,
}

/// One pass over the session's transcripts: Artifact calls and their results, paired by `tool_use_id`; the latest
/// publish of each artifact wins.
fn scan(files: &[PathBuf]) -> HashMap<String, Source> {
    let mut budget = SCAN_BYTES;
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut publishes: Vec<Publish> = Vec::new();
    for (fi, path) in files.iter().enumerate() {
        if budget == 0 {
            break;
        }
        let Ok(file) = open_regular(path) else { continue };
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let mut li = 0usize;
        loop {
            let (read, kept) = read_line_capped(&mut reader, &mut line, MAX_LINE);
            if read == 0 {
                break;
            }
            budget = budget.saturating_sub(read as u64);
            li += 1;
            if kept {
                scan_line(&line, fi, li, &mut calls, &mut publishes);
            }
            if budget == 0 {
                break;
            }
        }
    }
    publishes.sort_by_key(|p| p.order);
    let mut out = HashMap::new();
    for p in publishes {
        let source = if p.typed {
            Source::None(PreviewError::Typed)
        } else {
            match calls.get(&p.tool_use_id) {
                Some(c) => resolve(c),
                None => Source::None(PreviewError::NoSource),
            }
        };
        out.insert(p.artifact_id, source);
    }
    out
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn scan_line(line: &[u8], fi: usize, li: usize, calls: &mut HashMap<String, Call>, publishes: &mut Vec<Publish>) {
    let is_call = contains(line, br#""name":"Artifact""#);
    let is_result = !is_call && contains(line, br#""toolUseResult""#) && contains(line, b"/artifact/");
    if !is_call && !is_result {
        return;
    }
    let Ok(rec) = serde_json::from_slice::<Value>(line) else { return };
    let blocks = rec.get("message").and_then(|m| m.get("content")).and_then(Value::as_array);
    if is_call {
        let cwd = rec.get("cwd").and_then(Value::as_str).map(str::to_string);
        for b in blocks.into_iter().flatten() {
            if b.get("type").and_then(Value::as_str) != Some("tool_use") || b.get("name").and_then(Value::as_str) != Some("Artifact") {
                continue;
            }
            let (Some(id), Some(input)) = (b.get("id").and_then(Value::as_str), b.get("input")) else { continue };
            if id.len() <= 128 {
                calls.insert(id.to_string(), Call { cwd: cwd.clone(), input: input.clone() });
            }
        }
        return;
    }
    let Some(r) = rec.get("toolUseResult").and_then(Value::as_object) else { return };
    let Some(artifact_id) = r.get("url").and_then(Value::as_str).and_then(|u| URL_RE.captures(u)).and_then(|c| c.get(1)) else {
        return;
    };
    let ts = rec
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp_millis())
        .unwrap_or(0);
    for b in blocks.into_iter().flatten() {
        if b.get("type").and_then(Value::as_str) != Some("tool_result") || b.get("is_error").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(tid) = b.get("tool_use_id").and_then(Value::as_str) {
            publishes.push(Publish {
                order: (ts, fi, li),
                tool_use_id: tid.to_string(),
                artifact_id: artifact_id.as_str().to_string(),
                typed: r.get("type").is_some_and(Value::is_object),
            });
        }
    }
}

/// Reads one line (up to and excluding `\n`) into `buf`, keeping at most `cap` bytes. Returns the bytes consumed and
/// whether the line was kept whole (an over-long line is drained without being stored).
fn read_line_capped<R: BufRead>(r: &mut R, buf: &mut Vec<u8>, cap: usize) -> (usize, bool) {
    buf.clear();
    let mut consumed = 0usize;
    let mut over = false;
    loop {
        let chunk = match r.fill_buf() {
            Ok(c) if !c.is_empty() => c,
            _ => return (consumed, !over),
        };
        let (take, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        let body = if done { &chunk[..take - 1] } else { &chunk[..take] };
        if !over {
            if buf.len() + body.len() > cap {
                over = true;
                buf.clear();
            } else {
                buf.extend_from_slice(body);
            }
        }
        r.consume(take);
        consumed += take;
        if done {
            return (consumed, !over);
        }
    }
}

fn is_previewable(p: &str) -> bool {
    extension_ok(Path::new(p))
}

fn extension_ok(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "html" | "htm" | "md"))
}

/// The source path of a publishing call, resolved against `root` (if given) or the record's `cwd`. Not validated.
fn resolve(call: &Call) -> Source {
    let input = &call.input;
    if input.get("type_url").is_some() || input.get("asset").and_then(Value::as_bool) == Some(true) {
        return Source::None(PreviewError::Typed);
    }
    let file_path = input.get("file_path").and_then(Value::as_str).filter(|p| is_previewable(p));
    // For a typed-style publish whose page is not the file_path, the first HTML among `files` (string or {from}).
    let from_files = || -> Option<&str> {
        let files = input.get("files")?;
        let values: Vec<&Value> = match files {
            Value::Object(m) => m.values().collect(),
            Value::Array(a) => a.iter().map(|v| v.get("path").unwrap_or(v)).collect(),
            _ => return None,
        };
        values.into_iter().filter_map(|v| v.as_str().or_else(|| v.as_object().filter(|o| !o.contains_key("artifact"))?.get("from")?.as_str())).find(|p| {
            Path::new(p).extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "html" | "htm"))
        })
    };
    let Some(raw) = file_path.or_else(from_files) else { return Source::None(PreviewError::NoSource) };
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') {
        return Source::None(PreviewError::Refused);
    }
    let raw_path = Path::new(raw);
    if raw_path.components().any(|c| c == Component::ParentDir) {
        return Source::None(PreviewError::Refused);
    }
    if raw_path.is_absolute() {
        return Source::Path(raw_path.to_path_buf());
    }
    let Some(cwd) = call.cwd.as_deref().map(Path::new).filter(|c| c.is_absolute()) else {
        return Source::None(PreviewError::Refused);
    };
    let base = match input.get("root").and_then(Value::as_str) {
        Some(root) if Path::new(root).components().any(|c| c == Component::ParentDir) => return Source::None(PreviewError::Refused),
        Some(root) => cwd.join(root),
        None => cwd.to_path_buf(),
    };
    if base.components().any(|c| c == Component::ParentDir) {
        return Source::None(PreviewError::Refused);
    }
    Source::Path(base.join(raw_path))
}

// ---------- reading the source ----------

/// Opens without following a final symlink or blocking on a FIFO, close-on-exec.
pub(crate) fn open_flags(path: &Path) -> std::io::Result<File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    opts.open(path)
}

pub(crate) fn single_link_regular(f: &File) -> Option<std::fs::Metadata> {
    let meta = f.metadata().ok()?;
    #[cfg(unix)]
    let single = {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() == 1
    };
    #[cfg(not(unix))]
    let single = true;
    (meta.is_file() && single).then_some(meta)
}

fn open_regular(path: &Path) -> Result<File, PreviewError> {
    let f = open_flags(path).map_err(|_| PreviewError::Unknown)?;
    single_link_regular(&f).ok_or(PreviewError::Unknown)?;
    Ok(f)
}

/// The path the open descriptor really refers to.
pub(crate) fn fd_path(f: &File) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::{CStr, OsStr};
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::io::AsRawFd;
        let mut buf = vec![0u8; libc::PATH_MAX as usize + 1];
        // SAFETY: F_GETPATH writes a NUL-terminated path of at most PATH_MAX bytes into `buf`, which is larger.
        let rc = unsafe { libc::fcntl(f.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) };
        if rc == -1 {
            return None;
        }
        let s = CStr::from_bytes_until_nul(&buf).ok()?;
        Some(PathBuf::from(OsStr::from_bytes(s.to_bytes())))
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        std::fs::read_link(format!("/proc/self/fd/{}", f.as_raw_fd())).ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = f;
        None
    }
}

/// Validated open of a source path: allowed location (by resolved parent + name), allowed extension, no symlink,
/// single-link regular file, size, and the location re-checked on the open descriptor.
fn open_source(path: &Path, allowed: &Allowed) -> Result<(File, std::fs::Metadata), PreviewError> {
    if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) || !extension_ok(path) {
        return Err(PreviewError::Refused);
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return Err(PreviewError::Refused) };
    let parent = crate::platform::canon(parent).map_err(|e| io_reason(&e))?;
    let lexical = parent.join(name);
    if !allowed.permits(&lexical) {
        return Err(PreviewError::Refused);
    }
    let file = open_flags(&lexical).map_err(|e| io_reason(&e))?;
    let meta = single_link_regular(&file).ok_or(PreviewError::Refused)?;
    if meta.len() > MAX_SOURCE {
        return Err(PreviewError::TooLarge);
    }
    let real = fd_path(&file).ok_or(PreviewError::Refused)?;
    if !allowed.permits(&real) || !extension_ok(&real) {
        return Err(PreviewError::Refused);
    }
    Ok((file, meta))
}

fn io_reason(e: &std::io::Error) -> PreviewError {
    match e.kind() {
        std::io::ErrorKind::NotFound => PreviewError::Missing,
        _ => PreviewError::Refused, // ELOOP (a symlink), EACCES, ENOTDIR, …
    }
}

fn cut(s: &mut String, max: usize) -> bool {
    if s.len() <= max {
        return false;
    }
    let mut at = max;
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    s.truncate(at);
    true
}

static SCRIPT_STYLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(script|style)\b[^>]*>.*?</(?:script|style)\s*>").unwrap_or_else(|_| unreachable!()));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap_or_else(|_| unreachable!()));

/// Little visible text (tags, scripts and styles removed) and a `<script`.
pub fn script_only(html: &str) -> bool {
    if !html.to_ascii_lowercase().contains("<script") {
        return false;
    }
    let without = SCRIPT_STYLE_RE.replace_all(html, " ");
    let text = TAG_RE.replace_all(&without, " ");
    text.chars().filter(|c| !c.is_whitespace()).count() < SCRIPT_ONLY_TEXT
}

fn read_preview(mut file: File, real_md: bool) -> Result<Preview, PreviewError> {
    let mut buf = Vec::new();
    (&mut file).take(MAX_SOURCE + 1).read_to_end(&mut buf).map_err(|_| PreviewError::Refused)?;
    if buf.len() as u64 > MAX_SOURCE {
        return Err(PreviewError::TooLarge);
    }
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if real_md {
        let truncated = cut(&mut text, MAX_MARKDOWN);
        Ok(Preview::Markdown { text, truncated })
    } else {
        let script_only = script_only(&text);
        let truncated = cut(&mut text, MAX_HTML);
        Ok(Preview::Html { text, truncated, script_only })
    }
}

/// Preview of `artifact_id` from session `session_id`. `typed` is the DB row's kind (typed artifacts are not scanned).
pub fn preview(
    cache: &PreviewCache,
    projects_root: &Path,
    allowed: &Allowed,
    session_id: &str,
    artifact_id: &str,
    typed: bool,
) -> Result<Preview, PreviewError> {
    if !ID_RE.is_match(artifact_id) {
        return Err(PreviewError::Unknown);
    }
    if typed {
        return Err(PreviewError::Typed);
    }
    let sources = session_sources(cache, projects_root, session_id)?;
    let path = match sources.get(artifact_id) {
        Some(Source::Path(p)) => p.clone(),
        Some(Source::None(e)) => return Err(*e),
        None => return Err(PreviewError::NoSource),
    };
    preview_file(cache, allowed, artifact_id, &path)
}

fn preview_file(cache: &PreviewCache, allowed: &Allowed, artifact_id: &str, path: &Path) -> Result<Preview, PreviewError> {
    let (file, meta) = open_source(path, allowed)?;
    let mtime = meta.modified().ok();
    {
        let p = cache.previews.lock().map_err(|_| PreviewError::Unknown)?;
        if let Some((_, e)) = p.iter().find(|(k, e)| k == artifact_id && e.path == path && e.mtime == mtime && e.len == meta.len()) {
            return Ok(e.preview.clone());
        }
    }
    let is_md = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md"));
    let preview = read_preview(file, is_md)?;
    let mut p = cache.previews.lock().map_err(|_| PreviewError::Unknown)?;
    p.retain(|(k, _)| k != artifact_id);
    p.push_back((artifact_id.to_string(), PreviewEntry { path: path.to_path_buf(), mtime, len: meta.len(), preview: preview.clone() }));
    while p.len() > PREVIEW_CACHE {
        p.pop_front();
    }
    Ok(preview)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fx {
        _dir: tempfile::TempDir,
        home: PathBuf,
        allowed: Allowed,
    }

    fn fx() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let base = crate::platform::canon(dir.path()).unwrap();
        let home = base.join("home");
        std::fs::create_dir_all(home.join("proj")).unwrap();
        std::fs::create_dir_all(base.join("outside")).unwrap();
        let allowed = Allowed::new(home.clone(), None);
        Fx { _dir: dir, home, allowed }
    }

    fn call_line(id: &str, cwd: &str, input: Value) -> String {
        serde_json::json!({
            "type": "assistant", "cwd": cwd, "timestamp": "2026-11-08T00:00:00Z",
            "message": { "content": [ { "type": "tool_use", "id": id, "name": "Artifact", "input": input } ] }
        })
        .to_string()
    }

    fn result_line(tid: &str, artifact: &str, ts: &str, typed: bool) -> String {
        let mut r = serde_json::json!({ "url": format!("https://claude.ai/code/artifact/{artifact}"), "title": "t", "version": "v1" });
        if typed {
            r["type"] = serde_json::json!({ "name": "Slides" });
        }
        serde_json::json!({
            "type": "user", "timestamp": ts, "toolUseResult": r,
            "message": { "content": [ { "type": "tool_result", "tool_use_id": tid, "content": "ok" } ] }
        })
        .to_string()
    }

    fn transcript(fx: &Fx, lines: &[String]) -> PathBuf {
        let p = fx.home.join("t.jsonl");
        let mut f = File::create(&p).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        p
    }

    fn get(fx: &Fx, lines: &[String], artifact: &str) -> Result<Preview, PreviewError> {
        let t = transcript(fx, lines);
        let sources = scan(&[t]);
        match sources.get(artifact) {
            Some(Source::Path(p)) => preview_file(&PreviewCache::new(), &fx.allowed, artifact, p),
            Some(Source::None(e)) => Err(*e),
            None => Err(PreviewError::NoSource),
        }
    }

    fn publish(fx: &Fx, path: &str, artifact: &str) -> Vec<String> {
        vec![
            call_line("tu1", fx.home.to_str().unwrap(), serde_json::json!({ "file_path": path })),
            result_line("tu1", artifact, "2026-11-08T00:00:01Z", false),
        ]
    }

    #[test]
    fn reads_the_last_publish_of_the_matching_artifact() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/old.html"), "<p>old</p>").unwrap();
        std::fs::write(fx.home.join("proj/new.html"), "<h1>새 보고서</h1>").unwrap();
        std::fs::write(fx.home.join("proj/other.html"), "<p>other</p>").unwrap();
        let cwd = fx.home.to_str().unwrap();
        let lines = vec![
            call_line("a", cwd, serde_json::json!({ "file_path": "proj/old.html" })),
            result_line("a", "art1", "2026-11-08T00:00:01Z", false),
            call_line("b", cwd, serde_json::json!({ "file_path": "proj/other.html" })),
            result_line("b", "art2", "2026-11-08T00:00:02Z", false),
            call_line("c", cwd, serde_json::json!({ "file_path": "proj/new.html", "url": "https://claude.ai/code/artifact/art1" })),
            result_line("c", "art1", "2026-11-08T00:00:03Z", false),
        ];
        match get(&fx, &lines, "art1").unwrap() {
            Preview::Html { text, truncated, script_only } => {
                assert!(text.contains("새 보고서"));
                assert!(!truncated && !script_only);
            }
            other => panic!("{other:?}"),
        }
        // A different artifact id gets its own source, not art1's.
        assert!(matches!(get(&fx, &lines, "art2").unwrap(), Preview::Html { text, .. } if text.contains("other")));
        assert_eq!(get(&fx, &lines, "art3"), Err(PreviewError::NoSource), "unknown artifact id");
    }

    #[test]
    fn relative_path_uses_root_then_cwd() {
        let fx = fx();
        std::fs::create_dir_all(fx.home.join("proj/site")).unwrap();
        std::fs::write(fx.home.join("proj/site/index.html"), "<p>hello</p>").unwrap();
        let cwd = fx.home.join("proj");
        let lines = vec![
            call_line("a", cwd.to_str().unwrap(), serde_json::json!({ "file_path": "index.html", "root": "site" })),
            result_line("a", "art1", "2026-11-08T00:00:01Z", false),
            call_line("b", cwd.to_str().unwrap(), serde_json::json!({ "file_path": "site/index.html" })),
            result_line("b", "art2", "2026-11-08T00:00:01Z", false),
            call_line("c", "relative/cwd", serde_json::json!({ "file_path": "index.html" })),
            result_line("c", "art3", "2026-11-08T00:00:01Z", false),
        ];
        assert!(get(&fx, &lines, "art1").is_ok());
        assert!(get(&fx, &lines, "art2").is_ok());
        assert_eq!(get(&fx, &lines, "art3"), Err(PreviewError::Refused), "relative cwd is refused");
    }

    #[test]
    fn files_map_accepts_strings_and_from_only() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/page.html"), "<p>page</p>").unwrap();
        let cwd = fx.home.join("proj");
        let c = cwd.to_str().unwrap();
        let lines = vec![
            call_line("a", c, serde_json::json!({ "file_path": "data.json", "files": { "x.html": { "artifact": "https://claude.ai/artifact/z", "path": "a.html" }, "p.html": { "from": "page.html" } } })),
            result_line("a", "art1", "2026-11-08T00:00:01Z", false),
            call_line("b", c, serde_json::json!({ "file_path": "data.json", "files": { "x.html": { "artifact": "https://claude.ai/artifact/z", "path": "a.html" } } })),
            result_line("b", "art2", "2026-11-08T00:00:01Z", false),
        ];
        assert!(matches!(get(&fx, &lines, "art1").unwrap(), Preview::Html { text, .. } if text.contains("page")));
        assert_eq!(get(&fx, &lines, "art2"), Err(PreviewError::NoSource));
    }

    #[test]
    fn typed_artifacts_are_not_read() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/deck.html"), "<p>deck</p>").unwrap();
        let p = fx.home.join("proj/deck.html");
        let lines = vec![
            call_line("a", fx.home.to_str().unwrap(), serde_json::json!({ "file_path": p.to_str().unwrap() })),
            result_line("a", "art1", "2026-11-08T00:00:01Z", true),
        ];
        assert_eq!(get(&fx, &lines, "art1"), Err(PreviewError::Typed));
        let cache = PreviewCache::new();
        assert_eq!(preview(&cache, &fx.home, &fx.allowed, "s", "art1", true), Err(PreviewError::Typed));
    }

    #[test]
    fn symlinks_are_refused() {
        let fx = fx();
        let outside = fx.home.parent().unwrap().join("outside/secret.html");
        std::fs::write(&outside, "secret").unwrap();
        let link = fx.home.join("proj/link.html");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert_eq!(get(&fx, &publish(&fx, link.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused));
        // A symlinked directory that leads outside home is caught by the resolved parent.
        let dir_link = fx.home.join("proj/out");
        std::os::unix::fs::symlink(fx.home.parent().unwrap().join("outside"), &dir_link).unwrap();
        let via = dir_link.join("secret.html");
        assert_eq!(get(&fx, &publish(&fx, via.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused));
    }

    #[test]
    fn hard_links_are_refused() {
        let fx = fx();
        let a = fx.home.join("proj/a.html");
        std::fs::write(&a, "<p>a</p>").unwrap();
        std::fs::hard_link(&a, fx.home.join("proj/b.html")).unwrap();
        assert_eq!(get(&fx, &publish(&fx, a.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused));
    }

    #[test]
    fn parent_components_are_refused() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/a.html"), "x").unwrap();
        let p = format!("{}/proj/../proj/a.html", fx.home.display());
        assert_eq!(get(&fx, &publish(&fx, &p, "art1"), "art1"), Err(PreviewError::Refused));
        let rel = vec![
            call_line("a", fx.home.join("proj").to_str().unwrap(), serde_json::json!({ "file_path": "../proj/a.html" })),
            result_line("a", "art1", "2026-11-08T00:00:01Z", false),
        ];
        assert_eq!(get(&fx, &rel, "art1"), Err(PreviewError::Refused));
    }

    #[test]
    fn outside_home_and_denied_folders_are_refused() {
        let fx = fx();
        let outside = fx.home.parent().unwrap().join("outside/page.html");
        std::fs::write(&outside, "<p>x</p>").unwrap();
        assert_eq!(get(&fx, &publish(&fx, outside.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused));
        for d in ["Library", ".claude"] {
            std::fs::create_dir_all(fx.home.join(d)).unwrap();
            let p = fx.home.join(d).join("p.html");
            std::fs::write(&p, "<p>x</p>").unwrap();
            assert_eq!(get(&fx, &publish(&fx, p.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused), "{d}");
        }
        // The temp folder is allowed only when configured.
        let tmp = fx.home.parent().unwrap().join("outside");
        let with_tmp = Fx { allowed: Allowed::new(fx.home.clone(), Some(tmp)), home: fx.home.clone(), _dir: tempfile::tempdir().unwrap() };
        assert!(get(&with_tmp, &publish(&with_tmp, outside.to_str().unwrap(), "art1"), "art1").is_ok());
    }

    #[test]
    fn extension_must_be_allowed() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/a.json"), "{}").unwrap();
        let p = fx.home.join("proj/a.json");
        assert_eq!(get(&fx, &publish(&fx, p.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::NoSource));
        assert_eq!(open_source(&p, &fx.allowed).err(), Some(PreviewError::Refused));
    }

    #[test]
    fn too_large_and_missing() {
        let fx = fx();
        let big = fx.home.join("proj/big.html");
        std::fs::write(&big, vec![b'a'; MAX_SOURCE as usize + 1]).unwrap();
        assert_eq!(get(&fx, &publish(&fx, big.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::TooLarge));
        let gone = fx.home.join("proj/gone.html");
        assert_eq!(get(&fx, &publish(&fx, gone.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Missing));
        let gone_dir = fx.home.join("nope/gone.html");
        assert_eq!(get(&fx, &publish(&fx, gone_dir.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Missing));
    }

    #[test]
    fn fifo_is_refused_without_blocking() {
        let fx = fx();
        let p = fx.home.join("proj/pipe.html");
        let c = std::ffi::CString::new(p.to_str().unwrap()).unwrap();
        // SAFETY: valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(get(&fx, &publish(&fx, p.to_str().unwrap(), "art1"), "art1"), Err(PreviewError::Refused));
    }

    #[test]
    fn html_is_truncated_and_markdown_kept() {
        let fx = fx();
        let p = fx.home.join("proj/long.html");
        std::fs::write(&p, "가".repeat(MAX_HTML)).unwrap();
        match get(&fx, &publish(&fx, p.to_str().unwrap(), "art1"), "art1").unwrap() {
            Preview::Html { text, truncated, .. } => {
                assert!(truncated);
                assert!(text.len() <= MAX_HTML);
            }
            other => panic!("{other:?}"),
        }
        let md = fx.home.join("proj/notes.MD");
        std::fs::write(&md, "# 제목\n본문").unwrap();
        assert_eq!(get(&fx, &publish(&fx, md.to_str().unwrap(), "art1"), "art1").unwrap(), Preview::Markdown { text: "# 제목\n본문".into(), truncated: false });
    }

    #[test]
    fn script_only_pages_are_flagged() {
        assert!(script_only("<html><body><div id=root></div><script src=x.js></script></body></html>"));
        assert!(script_only("<div id=root></div><SCRIPT>render(\"a long string that is not visible text at all\")</SCRIPT>"));
        assert!(!script_only("<p>충분히 긴 본문 텍스트가 여기에 있어요. 사십 자를 넘기도록 조금 더 길게 이어서 써요. 이 정도면 충분해요.</p><script></script>"));
        assert!(!script_only("<p>short</p>"));
    }

    #[test]
    fn long_lines_are_skipped_and_unrelated_lines_not_parsed() {
        let fx = fx();
        std::fs::write(fx.home.join("proj/a.html"), "<p>a</p>").unwrap();
        let mut lines = vec![format!("{{\"x\":\"{}\",\"name\":\"Artifact\"}}", "a".repeat(MAX_LINE + 10)), "not json \"name\":\"Artifact\"".into()];
        lines.extend(publish(&fx, fx.home.join("proj/a.html").to_str().unwrap(), "art1"));
        assert!(get(&fx, &lines, "art1").is_ok());
    }

    #[test]
    fn read_line_capped_drains_long_lines() {
        let data = b"abc\ndefghij\nk".to_vec();
        let mut r = BufReader::with_capacity(2, &data[..]);
        let mut buf = Vec::new();
        assert_eq!(read_line_capped(&mut r, &mut buf, 5), (4, true));
        assert_eq!(buf, b"abc");
        assert_eq!(read_line_capped(&mut r, &mut buf, 5), (8, false));
        assert!(buf.is_empty());
        assert_eq!(read_line_capped(&mut r, &mut buf, 5), (1, true));
        assert_eq!(buf, b"k");
        assert_eq!(read_line_capped(&mut r, &mut buf, 5).0, 0);
    }

    #[test]
    fn preview_cache_follows_source_changes() {
        let fx = fx();
        let p = fx.home.join("proj/a.html");
        std::fs::write(&p, "<p>one</p>").unwrap();
        let cache = PreviewCache::new();
        assert!(matches!(preview_file(&cache, &fx.allowed, "art1", &p).unwrap(), Preview::Html { text, .. } if text.contains("one")));
        std::fs::write(&p, "<p>two, longer</p>").unwrap();
        assert!(matches!(preview_file(&cache, &fx.allowed, "art1", &p).unwrap(), Preview::Html { text, .. } if text.contains("two")));
    }

    #[test]
    fn lookup_validates_ids() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO artifacts VALUES ('ok', 'https://claude.ai/artifact/ok', 't', 'typed', 1, 'sess', 1, 5)", []).unwrap();
        assert_eq!(lookup(&conn, "ok").unwrap(), Some(("sess".into(), "typed".into())));
        assert_eq!(lookup(&conn, "../x").unwrap(), None);
        assert_eq!(lookup(&conn, "nope").unwrap(), None);
    }
}
