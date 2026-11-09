//! Transcript records shown as indented JSON when the user opens them (D-074, partly replacing D-071 for this view).
//! Records are read straight from the session's own JSONL (or a subagent's) by byte position, returned for display
//! and dropped: nothing here is stored, logged or cached. String values are masked with [`timeline::redact`] unless
//! the user turned masking off for this view, base64 payloads are folded to their size, and control/bidi characters
//! are made visible.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::text::is_unsafe_char;
use crate::timeline::{self, Span};

/// Records per request (item spans or one page).
pub const MAX_RECORDS: usize = 100;
/// A longer record is not parsed: its first part is shown as masked plain text.
const MAX_LINE: usize = 1024 * 1024;
/// Extra bytes kept past the cut so a secret straddling it is masked before the cut.
const MASK_MARGIN: usize = 64 * 1024;
/// Scan limits for one page (a huge line still has to be skipped to find the next record).
const SCAN_BYTES: u64 = 512 * 1024 * 1024;
const SCAN_TIME: Duration = Duration::from_secs(10);
/// Output limit for one page; the page ends early and `next` continues from there.
const PAGE_TEXT: usize = 24 * 1024 * 1024;
/// Base64 strings at least this long are folded to "[이미지 n KB]".
const FOLD_MIN: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Exactly these records (from a timeline item).
    Spans(Vec<Span>),
    /// Up to [`MAX_RECORDS`] records starting at this byte (0 or just after a newline).
    Page { from: u64 },
    /// Up to [`MAX_RECORDS`] records ending at this byte (a record boundary); `None` = the end of the file.
    Before { end: Option<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawError {
    /// Malformed id or the transcript is not there.
    NotFound,
    /// The file shrank or the position is no longer a record boundary.
    Changed,
    /// Too many spans.
    Invalid,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRecord {
    /// Byte position of the record in its file (a stable key; not shown).
    pub offset: u64,
    /// Header label: "도구 결과" for tool results, otherwise the record's `type` (e.g. "user").
    pub kind: Option<String>,
    /// The record's `timestamp` field.
    pub ts: Option<String>,
    /// Bookkeeping without prose (tool results, assistant chunks without text, other record types): folded by default.
    pub minor: bool,
    /// Indented JSON, or plain text when the record was too long or not JSON.
    pub text: String,
    pub json: bool,
    /// Longer than 1 MB: only the first part is in `text`.
    pub truncated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPage {
    pub records: Vec<RawRecord>,
    /// Byte position where this page starts; `0` means the start of the file.
    pub start: u64,
    /// Byte position to continue from, `None` at the end of the file.
    pub next: Option<u64>,
    pub file_len: u64,
    /// The scan limit was reached before the page filled (`next` still continues when it can).
    pub stopped: bool,
}

static B64_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""(data|base64)"\s*:\s*"([A-Za-z0-9+/=]{1024,})"#).unwrap_or_else(|_| unreachable!())
});

/// Over-long records are not parsed: a secret-named key's string value is masked in the text, escaped or not.
static SECRET_FIELD_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(\\?"[A-Za-z0-9_-]*(?:key|token|secret|passw(?:or)?d|credential|authorization)[A-Za-z0-9_-]*\\?"\s*:\s*\\?")[^"\\]*"#)
        .unwrap_or_else(|_| unreachable!())
});

fn file_for(projects_root: &Path, session_id: &str, agent_id: Option<&str>) -> Result<PathBuf, RawError> {
    let files = timeline::session_files(projects_root, session_id).ok_or(RawError::NotFound)?;
    match agent_id {
        None => Ok(files.main.path),
        Some(a) if crate::discover::is_agent_id(a) => files
            .subagents
            .into_iter()
            .find(|f| f.context.agent_id.as_deref() == Some(a))
            .map(|f| f.path)
            .ok_or(RawError::NotFound),
        Some(_) => Err(RawError::NotFound),
    }
}

/// Reads records of `session_id` (or of its subagent `agent_id`) for display. `reveal` turns masking off.
pub fn read(projects_root: &Path, session_id: &str, agent_id: Option<&str>, req: &Request, reveal: bool) -> Result<RawPage, RawError> {
    let path = file_for(projects_root, session_id, agent_id)?;
    let mut f = std::fs::File::open(&path).map_err(|_| RawError::NotFound)?;
    let file_len = f.metadata().map_err(|_| RawError::NotFound)?.len();
    match req {
        Request::Spans(spans) => {
            if spans.len() > MAX_RECORDS {
                return Err(RawError::Invalid);
            }
            let mut records = Vec::with_capacity(spans.len());
            for &[start, len] in spans {
                records.push(read_span(&mut f, file_len, start, len, reveal)?);
            }
            Ok(RawPage { records, start: 0, next: None, file_len, stopped: false })
        }
        Request::Page { from } => read_page(f, file_len, *from, None, reveal),
        Request::Before { end } => {
            let end = end.unwrap_or(file_len);
            if end > file_len || !at_boundary(&mut f, end)? && end != file_len {
                return Err(RawError::Changed);
            }
            let start = start_before(&mut f, end)?;
            read_page(f, file_len, start, Some(end), reveal)
        }
    }
}

/// True when `pos` is the start of a record: the file start or just after a newline.
fn at_boundary(f: &mut std::fs::File, pos: u64) -> Result<bool, RawError> {
    if pos == 0 {
        return Ok(true);
    }
    let mut b = [0u8; 1];
    f.seek(SeekFrom::Start(pos - 1)).map_err(|_| RawError::Changed)?;
    f.read_exact(&mut b).map_err(|_| RawError::Changed)?;
    Ok(b[0] == b'\n')
}

/// Where the page of up to [`MAX_RECORDS`] non-empty records ending at `end` starts, scanning backward in chunks.
/// Stops early (fewer records) at the scan limit.
fn start_before(f: &mut std::fs::File, end: u64) -> Result<u64, RawError> {
    const CHUNK: u64 = 256 * 1024;
    let started = Instant::now();
    let mut pos = end;
    let mut line_end = end;
    let mut count = 0usize;
    let mut buf = vec![0u8; CHUNK as usize];
    while pos > 0 {
        if end - pos > SCAN_BYTES || started.elapsed() > SCAN_TIME {
            // Fewer records: start right after the last boundary seen.
            return Ok(if line_end == end { end } else { line_end + 1 });
        }
        let lo = pos.saturating_sub(CHUNK);
        let n = (pos - lo) as usize;
        f.seek(SeekFrom::Start(lo)).map_err(|_| RawError::Changed)?;
        f.read_exact(&mut buf[..n]).map_err(|_| RawError::Changed)?;
        for i in (0..n).rev() {
            if buf[i] != b'\n' {
                continue;
            }
            let at = lo + i as u64;
            if line_end > at + 1 {
                count += 1;
                if count == MAX_RECORDS {
                    return Ok(at + 1);
                }
            }
            line_end = at;
        }
        pos = lo;
    }
    Ok(0)
}

fn read_span(f: &mut std::fs::File, file_len: u64, start: u64, len: u64, reveal: bool) -> Result<RawRecord, RawError> {
    let end = start.checked_add(len).ok_or(RawError::Invalid)?;
    if len == 0 || end > file_len || !at_boundary(f, start)? {
        return Err(RawError::Changed);
    }
    if end < file_len {
        let mut b = [0u8; 1];
        f.seek(SeekFrom::Start(end)).map_err(|_| RawError::Changed)?;
        f.read_exact(&mut b).map_err(|_| RawError::Changed)?;
        if b[0] != b'\n' {
            return Err(RawError::Changed);
        }
    }
    let keep = (len as usize).min(MAX_LINE + MASK_MARGIN);
    let mut buf = vec![0u8; keep];
    f.seek(SeekFrom::Start(start)).map_err(|_| RawError::Changed)?;
    f.read_exact(&mut buf).map_err(|_| RawError::Changed)?;
    let rec = render(start, &buf, len as usize, reveal);
    // A span the timeline produced was JSON; if it no longer is, the file was rewritten.
    if !rec.json && !rec.truncated {
        return Err(RawError::Changed);
    }
    Ok(rec)
}

/// Records from `from` until [`MAX_RECORDS`], the output limit, `until` or the end of the file.
fn read_page(mut f: std::fs::File, file_len: u64, from: u64, until: Option<u64>, reveal: bool) -> Result<RawPage, RawError> {
    if from > file_len || !at_boundary(&mut f, from)? {
        return Err(RawError::Changed);
    }
    let stop_at = until.unwrap_or(file_len).min(file_len);
    f.seek(SeekFrom::Start(from)).map_err(|_| RawError::Changed)?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    let started = Instant::now();
    let mut pos = from;
    let mut scanned = 0u64;
    let mut out_bytes = 0usize;
    let mut records = Vec::new();
    let mut line: Vec<u8> = Vec::new();
    let page = |records, next: u64, stopped| RawPage { records, start: from, next: (next < file_len).then_some(next), file_len, stopped };
    loop {
        if pos >= stop_at || records.len() >= MAX_RECORDS || out_bytes >= PAGE_TEXT {
            return Ok(page(records, pos, false));
        }
        // Between records the page can stop and still be continued from `pos`.
        if scanned > SCAN_BYTES || started.elapsed() > SCAN_TIME {
            return Ok(page(records, pos, true));
        }
        // One line: keep up to the cut (plus margin), skip the rest.
        line.clear();
        let mut full = 0usize;
        let mut ended = false;
        while !ended {
            // Inside one enormous line there is no boundary to continue from.
            if scanned > 2 * SCAN_BYTES || started.elapsed() > 2 * SCAN_TIME {
                return Ok(RawPage { records, start: from, next: None, file_len, stopped: true });
            }
            let chunk = r.fill_buf().map_err(|_| RawError::Changed)?;
            if chunk.is_empty() {
                break;
            }
            let (take, found) = match chunk.iter().position(|b| *b == b'\n') {
                Some(i) => (i, true),
                None => (chunk.len(), false),
            };
            let room = (MAX_LINE + MASK_MARGIN).saturating_sub(line.len());
            line.extend_from_slice(&chunk[..take.min(room)]);
            full += take;
            let consumed = if found { take + 1 } else { take };
            r.consume(consumed);
            scanned += consumed as u64;
            ended = found;
        }
        let start = pos;
        pos += full as u64 + u64::from(ended);
        if full == 0 {
            if !ended {
                return Ok(page(records, pos, false));
            }
            continue;
        }
        let rec = render(start, &line, full, reveal);
        out_bytes += rec.text.len();
        records.push(rec);
    }
}

/// Header facts of a parsed record: label, timestamp, and whether it is bookkeeping without prose.
fn header(v: &Value) -> (Option<String>, Option<String>, bool) {
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    let blocks = v.pointer("/message/content").and_then(Value::as_array);
    let has = |t: &str| blocks.is_some_and(|b| b.iter().any(|x| x.get("type").and_then(Value::as_str) == Some(t)));
    let ts = v.get("timestamp").and_then(Value::as_str).map(|t| crate::text::display_safe(t, 40));
    let (kind, minor) = match ty {
        "user" if has("tool_result") => (Some(crate::i18n::Fail::ToolResult.str().to_string()), true),
        "user" => (Some("user".to_string()), false),
        "assistant" => (Some("assistant".to_string()), !has("text")),
        "" => (None, true),
        other => (Some(crate::text::display_safe(other, 40)), true),
    };
    (kind, ts, minor)
}

/// `bytes` holds the first part of a record whose full length is `full`.
fn render(offset: u64, bytes: &[u8], full: usize, reveal: bool) -> RawRecord {
    if full <= MAX_LINE {
        if let Ok(mut v) = serde_json::from_slice::<Value>(bytes) {
            scrub(&mut v, None, reveal);
            let (kind, ts, minor) = header(&v);
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            return RawRecord { offset, kind, ts, minor, text: visible(&text), json: true, truncated: false };
        }
    }
    // Too long (or not JSON): mask first, then fold base64, then cut.
    let text = String::from_utf8_lossy(bytes);
    let text = if reveal { text.into_owned() } else { SECRET_FIELD_TEXT_RE.replace_all(&timeline::redact(&text), crate::i18n::Fail::RawRedact.str()).into_owned() };
    let text = B64_TEXT_RE
        .replace_all(&text, |c: &regex::Captures<'_>| format!("\"{}\":\"{}\"", &c[1], fold_label(c[2].len(), None)))
        .into_owned();
    let truncated = full > MAX_LINE;
    let text = if truncated { cut(&text, MAX_LINE) } else { text };
    RawRecord { offset, kind: None, ts: None, minor: false, text: visible(&text), json: false, truncated }
}

fn cut(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Control, bidi and zero-width characters shown as `\uXXXX` (valid inside JSON strings) instead of acting on the text.
fn visible(s: &str) -> String {
    if !s.chars().any(|c| is_unsafe_char(c) && c != '\n') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        if is_unsafe_char(c) && c != '\n' {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

fn fold_label(b64_len: usize, media_type: Option<&str>) -> String {
    let kb = (b64_len * 3 / 4).div_ceil(1024);
    match media_type {
        Some(m) if !m.starts_with("image/") => crate::tr_fmt!(crate::i18n::Fail::RawDoc, kb = kb),
        _ => crate::tr_fmt!(crate::i18n::Fail::RawImage, kb = kb),
    }
}

fn looks_base64(s: &str) -> bool {
    s.len() >= FOLD_MIN && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'\n' | b'\r'))
}

/// Field names whose value is a secret by name (every string under it is masked).
fn secret_key(key: &str) -> bool {
    let k: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    ["apikey", "accesskey", "secret", "password", "passwd", "credential", "authorization", "authtoken", "accesstoken", "refreshtoken", "token"]
        .iter()
        .any(|w| if *w == "token" { k == "token" || k.ends_with("token") } else { k.contains(w) })
}

/// Every non-empty string under a secret-named key.
fn mask_all(v: &mut Value) {
    match v {
        Value::String(s) if !s.is_empty() => *s = crate::i18n::Fail::Hidden.str().to_string(),
        Value::Array(items) => items.iter_mut().for_each(mask_all),
        Value::Object(map) => map.values_mut().for_each(mask_all),
        _ => {}
    }
}

/// Masks string values (unless `reveal`) and folds base64 payloads, in place.
fn scrub(v: &mut Value, key: Option<&str>, reveal: bool) {
    if !reveal && key.is_some_and(secret_key) {
        mask_all(v);
        return;
    }
    match v {
        Value::String(s) => {
            if !reveal {
                *s = timeline::redact(s);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|x| scrub(x, None, reveal)),
        Value::Object(map) => {
            let media = map.get("media_type").and_then(Value::as_str).map(str::to_string);
            for (k, x) in map.iter_mut() {
                if matches!(k.as_str(), "data" | "base64") {
                    if let Value::String(s) = x {
                        if looks_base64(s) {
                            *s = fold_label(s.len(), media.as_deref());
                            continue;
                        }
                    }
                }
                scrub(x, Some(k), reveal);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SID: &str = "bbbbbbbb-2222-4222-8222-222222222222";

    fn setup(lines: &[String]) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects").join("-w");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(format!("{SID}.jsonl"));
        std::fs::write(&file, lines.join("\n") + "\n").unwrap();
        let root = tmp.path().join("projects").canonicalize().unwrap();
        (tmp, root, file)
    }

    #[test]
    fn item_spans_return_full_records_masked_by_default() {
        let png = "A".repeat(4096);
        let lines = vec![
            r#"{"type":"user","timestamp":"2026-11-08T10:00:00Z","message":{"content":"배포 스크립트 고쳐 줘 sk-ant-api03-CANARYAAAAAAAA"}}"#.to_string(),
            r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"thinking","thinking":"생각"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"deploy","api_key":"CANARYplain"}}]}}"#.to_string(),
            format!(r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"t1","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"{png}"}}}}]}}]}}}}"#),
            "{\"type\":\"user\",\"message\":{\"content\":\"a\u{202e}b\"}}".to_string(),
        ];
        let (_t, root, _) = setup(&lines);
        let tl = timeline::read(&root, SID).unwrap();
        let spans: Vec<Span> = tl
            .items
            .iter()
            .flat_map(|i| match i {
                timeline::Item::Prompt { raw, .. } | timeline::Item::Tool { raw, .. } => raw.clone(),
                _ => vec![],
            })
            .collect();
        assert_eq!(spans.len(), 4, "prompt, tool_use record and its result, bidi prompt");
        let page = read(&root, SID, None, &Request::Spans(spans.clone()), false).unwrap();
        let all: String = page.records.iter().map(|r| r.text.clone()).collect();
        assert!(all.contains("배포 스크립트"), "prompt text is shown in full");
        assert!(all.contains("생각"), "thinking is part of the raw record");
        assert!(!all.contains("CANARY"), "masked by default: {all}");
        assert!(all.contains("[이미지 3 KB]") && !all.contains(&png[..100]), "base64 folded");
        assert!(all.contains("\\u202e") && !all.contains('\u{202e}'), "bidi made visible");
        assert!(page.records[0].text.starts_with("{\n  "), "indented JSON");
        assert_eq!(page.records[0].kind.as_deref(), Some("user"));
        let shown = read(&root, SID, None, &Request::Spans(spans), true).unwrap();
        let all: String = shown.records.iter().map(|r| r.text.clone()).collect();
        assert!(all.contains("CANARYAAAAAAAA") && all.contains("CANARYplain"), "reveal shows values");
        assert!(all.contains("[이미지 3 KB]"), "base64 stays folded when revealed");
    }

    #[test]
    fn changed_file_and_bad_positions_are_detected() {
        let lines = vec![r#"{"type":"user","message":{"content":"하나"}}"#.to_string(), r#"{"type":"user","message":{"content":"둘"}}"#.to_string()];
        let (_t, root, file) = setup(&lines);
        let l0 = lines[0].len() as u64;
        assert!(read(&root, SID, None, &Request::Spans(vec![[0, l0]]), false).is_ok());
        assert_eq!(read(&root, SID, None, &Request::Spans(vec![[1, l0 - 1]]), false).err(), Some(RawError::Changed), "mid-line start");
        assert_eq!(read(&root, SID, None, &Request::Spans(vec![[0, l0 + 50]]), false).err(), Some(RawError::Changed), "past end");
        assert_eq!(read(&root, SID, None, &Request::Page { from: 3 }, false).err(), Some(RawError::Changed));
        assert_eq!(read(&root, SID, None, &Request::Spans(vec![[0, 1]; 101]), false).err(), Some(RawError::Invalid));
        std::fs::write(&file, "{}\n").unwrap();
        assert_eq!(read(&root, SID, None, &Request::Spans(vec![[0, l0]]), false).err(), Some(RawError::Changed), "file shrank");
        assert_eq!(read(&root, "../x", None, &Request::Page { from: 0 }, false).err(), Some(RawError::NotFound));
        assert_eq!(read(&root, SID, Some("../../x"), &Request::Page { from: 0 }, false).err(), Some(RawError::NotFound));
    }

    #[test]
    fn pages_hold_100_records_and_long_lines_are_masked_then_cut() {
        let mut lines: Vec<String> = (0..150).map(|i| format!(r#"{{"type":"user","n":{i}}}"#)).collect();
        // A secret straddling the 1 MB cut: masking happens before cutting.
        let pre = r#"{"type":"user","message":{"content":""#;
        let fill = MAX_LINE - 10 - pre.len();
        let long = format!("{pre}{}{}token=CANARYlongvalue {}\"}}}}", "a ".repeat(fill / 2), "a".repeat(fill % 2), "b ".repeat(100));
        assert_eq!(long.find("token=").unwrap(), MAX_LINE - 10);
        lines.insert(3, long);
        lines.insert(5, String::new());
        let (_t, root, _) = setup(&lines);
        let p1 = read(&root, SID, None, &Request::Page { from: 0 }, false).unwrap();
        assert_eq!((p1.records.len(), p1.start), (100, 0));
        let long = &p1.records[3];
        assert!(long.truncated && !long.json && long.text.len() <= MAX_LINE);
        let p2 = read(&root, SID, None, &Request::Page { from: p1.next.unwrap() }, false).unwrap();
        assert_eq!(p2.records.len(), 51);
        assert_eq!(p2.next, None);
        assert!(p1.records.iter().chain(&p2.records).all(|r| !r.text.contains("CANA")));
        let revealed = read(&root, SID, None, &Request::Page { from: 0 }, true).unwrap();
        assert!(revealed.records[3].truncated && revealed.records[3].text.ends_with("token=CANA"));
    }

    #[test]
    fn subagent_files_only_inside_the_session_folder() {
        let lines = vec![r#"{"type":"user","message":{"content":"x"}}"#.to_string()];
        let (t, root, _) = setup(&lines);
        let sub = t.path().join("projects/-w").join(SID).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("agent-a1.jsonl"), "{\"type\":\"user\",\"message\":{\"content\":\"하위 작업\"}}\n").unwrap();
        let other = t.path().join("projects/-other").join(SID).join("subagents");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("agent-a2.jsonl"), "{}\n").unwrap();
        let p = read(&root, SID, Some("a1"), &Request::Page { from: 0 }, false).unwrap();
        assert!(p.records[0].text.contains("하위 작업"));
        assert_eq!(read(&root, SID, Some("a2"), &Request::Page { from: 0 }, false).err(), Some(RawError::NotFound), "other project folder");
    }

    #[test]
    fn secret_field_names() {
        for k in ["api_key", "apiKey", "ANTHROPIC_API_KEY", "password", "token", "access_token", "client_secret"] {
            assert!(secret_key(k), "{k}");
        }
        for k in ["input_tokens", "tokens", "content", "keyword", "tool_use_id"] {
            assert!(!secret_key(k), "{k}");
        }
    }

    #[test]
    fn secret_named_keys_mask_everything_under_them() {
        for raw in [r#"{"api_key":{"value":"CANARY1"}}"#, r#"{"secrets":["CANARY2",{"x":"CANARY3"}]}"#, r#"{"password":["CANARY4"]}"#] {
            let mut v: Value = serde_json::from_str(raw).unwrap();
            scrub(&mut v, None, false);
            assert!(!v.to_string().contains("CANARY"), "{raw} → {v}");
            let mut shown: Value = serde_json::from_str(raw).unwrap();
            scrub(&mut shown, None, true);
            assert!(shown.to_string().contains("CANARY"), "reveal keeps {raw}");
        }
    }

    #[test]
    fn long_text_masks_secret_named_fields() {
        let filler = "a ".repeat(MAX_LINE / 2 + 10);
        let line = format!(r#"{{"type":"user","password":"CANARYpw1","input":{{"content":"{{\"api_key\": \"CANARYesc\"}}"}},"x":"{filler}"}}"#);
        let rec = render(0, line.as_bytes(), line.len(), false);
        assert!(rec.truncated && !rec.json);
        assert!(!rec.text.contains("CANARY"), "{}", &rec.text[..200]);
        let shown = render(0, line.as_bytes(), line.len(), true);
        assert!(shown.text.contains("CANARYpw1"));
    }

    #[test]
    fn pages_backward_from_the_end_and_headers() {
        let mut lines: Vec<String> = (0..250).map(|i| format!(r#"{{"type":"user","timestamp":"2026-11-08T10:00:00Z","n":{i}}}"#)).collect();
        lines.push(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t","content":"ok"}]}}"#.to_string());
        lines.push(r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t","name":"Bash","input":{}}]}}"#.to_string());
        let (_t, root, _) = setup(&lines);
        let last = read(&root, SID, None, &Request::Before { end: None }, false).unwrap();
        assert_eq!(last.records.len(), 100);
        assert_eq!(last.next, None, "last page reaches the end");
        assert!(last.records[99].text.contains("tool_use"));
        assert_eq!((last.records[98].kind.as_deref(), last.records[98].minor), (Some(crate::i18n::Fail::ToolResult.str()), true));
        assert_eq!((last.records[99].kind.as_deref(), last.records[99].minor), (Some("assistant"), true));
        assert_eq!((last.records[0].ts.as_deref(), last.records[0].minor), (Some("2026-11-08T10:00:00Z"), false));
        let mid = read(&root, SID, None, &Request::Before { end: Some(last.start) }, false).unwrap();
        assert_eq!((mid.records.len(), mid.next), (100, Some(last.start)));
        let first = read(&root, SID, None, &Request::Before { end: Some(mid.start) }, false).unwrap();
        assert_eq!((first.records.len(), first.start), (52, 0));
        assert!(first.records[0].text.contains(r#""n": 0"#));
        assert_eq!(read(&root, SID, None, &Request::Before { end: Some(last.start + 1) }, false).err(), Some(RawError::Changed));
    }
}
