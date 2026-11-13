//! One session's flow — prompts, replies, tool calls and their outcome — read from its transcript on demand and
//! returned for display only (D-048 extended, D-071). Nothing here is stored, logged or cached; strings pass through
//! [`redact`] (secret-looking values) and `display_safe` (control/bidi) before they leave this module.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::discover;
use crate::text::display_safe;

const WINDOW_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LINE: usize = 1024 * 1024;
const MAX_ITEMS: usize = 2000;
const PROMPT_CHARS: usize = 2000;
const REPLY_CHARS: usize = 400;
const SUMMARY_CHARS: usize = 160;
const ERROR_CHARS: usize = 160;

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z").unwrap_or_else(|_| unreachable!())
});

/// Secret-looking values (security review HIGH). Order matters: specific token shapes first.
static SECRETS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(-----END [A-Z ]*PRIVATE KEY-----|$)", crate::i18n::Fail::RedactPrivateKey.str()),
        (r"sk-ant-[A-Za-z0-9_\-]{8,}", crate::i18n::Fail::RedactKey.str()),
        (r"\bAIza[0-9A-Za-z_\-]{35}", crate::i18n::Fail::RedactKey.str()),
        (r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})", crate::i18n::Fail::RedactToken.str()),
        (r"\bxox[abprs]-[A-Za-z0-9\-]{10,}", crate::i18n::Fail::RedactToken.str()),
        (r"\bAKIA[0-9A-Z]{16}\b", crate::i18n::Fail::RedactKey.str()),
        (r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{5,}", crate::i18n::Fail::RedactToken.str()),
        (r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=\-]{8,}", crate::i18n::Fail::RedactSpace.str()),
        (r"(?i)(authorization:\s*basic)\s+[A-Za-z0-9+/=]{8,}", crate::i18n::Fail::RedactSpace.str()),
        // `--token X`, `--password X`, `--api-key X` (value after a space).
        (r"(?i)(--?[A-Za-z0-9_-]*(?:token|secret|passw(?:or)?d|api[-_]?key|access[-_]?key)[A-Za-z0-9_-]*)\s+[^\s-]\S*", crate::i18n::Fail::RedactSpace.str()),
        // JSON-ish `"api_key": "…"`.
        (r#"(?i)("[A-Za-z0-9_-]*(?:key|token|secret|passw(?:or)?d|credential)[A-Za-z0-9_-]*")(\s*:\s*)"[^"]*""#, crate::i18n::Fail::RedactQuoted.str()),
        // `NAME=value` / `name: value` where the name says secret. `auth` only as a whole word or auth_token/authorization.
        (r#"(?i)\b([A-Za-z0-9_]*(?:api[_-]?key|access[_-]?key|secret|token|passw(?:or)?d|credential)[A-Za-z0-9_]*|auth(?:orization|[_-]?token)?)(\s*[=:]\s*)("[^"]*"|'[^']*'|\S+)"#, crate::i18n::Fail::RedactPlain.str()),
        (r"(?i)(https?://)[^/\s:@]+:[^/\s@]+@", crate::i18n::Fail::RedactUserinfo.str()),
        (r"(?i)(https?://[^\s?#]+)\?[^\s#]*", "$1?…"),
        // Long opaque blobs, but not plain 40/64-char hex (git SHAs, digests stay readable).
        (r"\b(?:[0-9a-f]{40}|[0-9a-f]{64})\b|\b([A-Za-z0-9_+]{40,}={0,2})", "$0"),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).unwrap_or_else(|_| unreachable!()), r))
    .collect()
});

/// Masks secret-looking substrings. Applied before any truncation so a cut can never expose half a token.
pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    for (re, rep) in SECRETS.iter() {
        out = if *rep == "$0" {
            // Long-blob rule: hex digests pass, other long blobs are hidden.
            re.replace_all(&out, |c: &regex::Captures<'_>| if c.get(1).is_some() { crate::i18n::Fail::RedactLong.str().to_string() } else { c[0].to_string() }).into_owned()
        } else {
            re.replace_all(&out, *rep).into_owned()
        };
    }
    out
}

/// Labels for [`SECRETS`], same order. Empty = not reported by [`secret_kind`] (URL query strings are too common).
fn secret_kinds() -> [&'static str; 15] {
    [
    crate::i18n::Fail::KindPrivateKey.str(), crate::i18n::Fail::KindApiKey.str(), crate::i18n::Fail::KindApiKey.str(), crate::i18n::Fail::KindGithub.str(), crate::i18n::Fail::KindSlack.str(), crate::i18n::Fail::KindAws.str(), "JWT", crate::i18n::Fail::KindBearer.str(), crate::i18n::Fail::KindBasic.str(),
    crate::i18n::Fail::KindSecretFlag.str(), crate::i18n::Fail::KindSecretJson.str(), crate::i18n::Fail::KindSecretSetting.str(), crate::i18n::Fail::KindUrlCreds.str(), "", crate::i18n::Fail::KindOpaque.str(),
    ]
}

/// Detect-only (memory-tab): the kind of the first secret-looking value in `s`, never the value itself.
pub fn secret_kind(s: &str) -> Option<&'static str> {
    for (i, (re, rep)) in SECRETS.iter().enumerate() {
        let kind = secret_kinds().get(i).copied().unwrap_or(crate::i18n::Fail::KindSecret.str());
        if kind.is_empty() {
            continue;
        }
        let hit = if *rep == "$0" { re.captures_iter(s).any(|c| c.get(1).is_some()) } else { re.is_match(s) };
        if hit {
            return Some(kind);
        }
    }
    None
}

fn clean(s: &str, max: usize) -> String {
    display_safe(&redact(s), max)
}

/// Where a record sits in its transcript: `[start byte, length]`, the newline excluded (D-074). Lets `raw::read`
/// return exactly the records an item came from without storing them.
pub type Span = [u64; 2];
/// An item rarely spans more records than this (a reply split across fragments, a tool call and its result).
const MAX_SPANS: usize = 16;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Item {
    Prompt { ts: i64, text: String, raw: Vec<Span> },
    Reply { ts: i64, text: String, raw: Vec<Span> },
    Tool { ts: i64, id: String, name: String, summary: String, ok: Option<bool>, error: Option<String>, raw: Vec<Span> },
    /// A Skill tool call: the skill's name only.
    Skill { ts: i64, id: String, name: String, ok: Option<bool>, error: Option<String>, raw: Vec<Span> },
    /// An Agent/Task call. `agent_id` names the matching `subagents/agent-<id>.jsonl` in the same session folder.
    Agent {
        ts: i64,
        id: String,
        subagent_type: String,
        description: String,
        ok: Option<bool>,
        error: Option<String>,
        agent_id: Option<String>,
        raw: Vec<Span>,
    },
    /// A typed slash command: the name only, never its arguments.
    Command { ts: i64, name: String, raw: Vec<Span> },
    Compact { ts: i64, raw: Vec<Span> },
}

impl Item {
    fn raw_mut(&mut self) -> &mut Vec<Span> {
        match self {
            Item::Prompt { raw, .. }
            | Item::Reply { raw, .. }
            | Item::Tool { raw, .. }
            | Item::Skill { raw, .. }
            | Item::Agent { raw, .. }
            | Item::Command { raw, .. }
            | Item::Compact { raw, .. } => raw,
        }
    }

    fn outcome_mut(&mut self) -> Option<(&mut Option<bool>, &mut Option<String>)> {
        match self {
            Item::Tool { ok, error, .. } | Item::Skill { ok, error, .. } | Item::Agent { ok, error, .. } => Some((ok, error)),
            _ => None,
        }
    }

    fn add_span(&mut self, span: Span) {
        let raw = self.raw_mut();
        if raw.len() < MAX_SPANS && raw.last() != Some(&span) {
            raw.push(span);
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub items: Vec<Item>,
    /// The file was bigger than the window or had more items than the cap: older entries were left out.
    pub truncated_front: bool,
    pub skipped_lines: usize,
}

/// A typed slash command: the message starts with `<command-name>` or `<command-message>` (as in `parse`, D-019).
static COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A\s*<command-(?:name|message)>(?s:.{0,400}?)<command-name>([^<]{0,80})</command-name>|\A\s*<command-name>([^<]{0,80})</command-name>")
        .unwrap_or_else(|_| unreachable!())
});

/// The main transcript of a session and the subagent transcripts in the same session folder, found through
/// `discover` (no symlinks, no hard links, canonical paths). Paths inside records are never followed.
pub(crate) struct SessionFiles {
    pub main: discover::TranscriptFile,
    pub subagents: Vec<discover::TranscriptFile>,
}

pub(crate) fn session_files(projects_root: &Path, session_id: &str) -> Option<SessionFiles> {
    if !UUID_RE.is_match(session_id) {
        return None;
    }
    let found: Vec<_> = discover::discover(projects_root).into_iter().filter(|f| f.context.session_id == session_id).collect();
    let main = found.iter().find(|f| f.kind == discover::TranscriptKind::Main)?.clone();
    let subagents = found
        .into_iter()
        .filter(|f| f.kind == discover::TranscriptKind::Subagent && f.project_dir == main.project_dir)
        .collect();
    Some(SessionFiles { main, subagents })
}

/// `toolUseId` → agent id, from each subagent's small meta file (64 KB cap).
fn subagent_links(files: &[discover::TranscriptFile]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for f in files {
        let (Some(meta), Some(agent)) = (&f.meta_path, &f.context.agent_id) else { continue };
        let Ok(file) = std::fs::File::open(meta) else { continue };
        let mut buf = Vec::new();
        if file.take(64 * 1024).read_to_end(&mut buf).is_err() {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) else { continue };
        if let Some(id) = v.get("toolUseId").and_then(|t| t.as_str()).filter(|t| t.len() <= 128) {
            out.insert(id.to_string(), agent.clone());
        }
    }
    out
}

fn ts_of(v: &serde_json::Value) -> i64 {
    v.get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp_millis())
        .unwrap_or(0)
}

/// The informative line of a tool error: skips `Exit code N` and the `<tool_use_error>` wrapper.
fn error_line(text: &str) -> String {
    let t = text.replace("<tool_use_error>", "").replace("</tool_use_error>", "");
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !(l.starts_with("Exit code ") && l[10..].trim().chars().all(|c| c.is_ascii_digit())))
        .unwrap_or("")
        .to_string()
}

fn tool_summary(name: &str, input: &serde_json::Value) -> String {
    let field = |k: &str| input.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let raw = match name {
        "Bash" => field("command"),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => field("file_path"),
        "Grep" | "Glob" => format!("{} {}", field("pattern"), field("path")).trim().to_string(),
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        "Agent" | "Task" => format!("{} · {}", field("subagent_type"), field("description")),
        "Skill" => field("skill"),
        _ => input.as_object().map(|o| o.keys().take(6).cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default(),
    };
    clean(&raw.replace('\n', " ⏎ "), SUMMARY_CHARS)
}

fn user_text(content: &serde_json::Value) -> Option<String> {
    let text = match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) if blocks.iter().all(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")) => {
            blocks.iter().filter_map(|b| b.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")
        }
        _ => return None,
    };
    let text = crate::prompt::strip_wrappers(&text);
    let t = text.trim();
    let skip = ["<command-", "<local-command", "<task-notification", "<system-reminder", "<pasted_content"];
    (!t.is_empty() && !skip.iter().any(|p| t.starts_with(p))).then(|| t.to_string())
}

/// Builds the timeline of `session_id` from its main transcript. `None` if the id is malformed or not found.
pub fn read(projects_root: &Path, session_id: &str) -> Option<Timeline> {
    let files = session_files(projects_root, session_id)?;
    let mut f = std::fs::File::open(&files.main.path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut truncated_front = len > WINDOW_BYTES;
    // Most recent activity matters most: read the last window of a large file.
    let base = if truncated_front { len - WINDOW_BYTES } else { 0 };
    if truncated_front {
        f.seek(SeekFrom::Start(base)).ok()?;
    }
    let mut buf = Vec::with_capacity(len.min(WINDOW_BYTES) as usize);
    f.take(WINDOW_BYTES).read_to_end(&mut buf).ok()?;
    let mut cursor = 0usize;
    if truncated_front {
        // Partial first line.
        cursor = buf.iter().position(|b| *b == b'\n').map_or(buf.len(), |i| i + 1);
    }

    let mut items: Vec<Item> = Vec::new();
    let mut reply_index: HashMap<String, usize> = HashMap::new();
    let mut tool_index: HashMap<String, usize> = HashMap::new();
    let mut fallback_agent: HashMap<usize, String> = HashMap::new();
    let mut skipped = 0usize;
    while cursor < buf.len() {
        let end = buf[cursor..].iter().position(|b| *b == b'\n').map_or(buf.len(), |i| cursor + i);
        let line = &buf[cursor..end];
        let span: Span = [base + cursor as u64, (end - cursor) as u64];
        cursor = end + 1;
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_LINE {
            skipped += 1;
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
            skipped += 1;
            continue;
        };
        let ts = ts_of(&v);
        match v.get("type").and_then(|t| t.as_str()) {
            Some("user") => {
                if v.get("isCompactSummary").and_then(|b| b.as_bool()) == Some(true) {
                    items.push(Item::Compact { ts, raw: vec![span] });
                    continue;
                }
                if v.get("isMeta").and_then(|b| b.as_bool()) == Some(true) {
                    continue;
                }
                let Some(content) = v.pointer("/message/content") else { continue };
                if let Some(blocks) = content.as_array() {
                    for b in blocks.iter().filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result")) {
                        let Some(id) = b.get("tool_use_id").and_then(|i| i.as_str()) else { continue };
                        let Some(&i) = tool_index.get(id) else { continue };
                        let Some(item) = items.get_mut(i) else { continue };
                        item.add_span(span);
                        // Older meta files lack `toolUseId`; the result record names the agent instead.
                        if let Some(a) = v.pointer("/toolUseResult/agentId").and_then(|a| a.as_str()).filter(|a| discover::is_agent_id(a)) {
                            fallback_agent.insert(i, a.to_string());
                        }
                        let is_error = b.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                        if let Some((ok, error)) = item.outcome_mut() {
                            *ok = Some(!is_error);
                            if is_error {
                                // First line only, masked: error text can echo a command with secrets.
                                let text = match b.get("content") {
                                    Some(serde_json::Value::String(s)) => s.clone(),
                                    Some(serde_json::Value::Array(p)) => p.iter().filter_map(|x| x.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join(" "),
                                    _ => String::new(),
                                };
                                *error = Some(clean(&error_line(&text), ERROR_CHARS));
                            }
                        }
                    }
                }
                if let Some(name) = content
                    .as_str()
                    .and_then(|t| COMMAND_RE.captures(t))
                    .and_then(|c| c.get(1).or_else(|| c.get(2)))
                    .and_then(|m| crate::parse::clean_name(m.as_str()))
                {
                    items.push(Item::Command { ts, name, raw: vec![span] });
                } else if let Some(text) = user_text(content) {
                    items.push(Item::Prompt { ts, text: clean(&text, PROMPT_CHARS), raw: vec![span] });
                }
            }
            Some("assistant") => {
                let msg_id = v.pointer("/message/id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                let Some(blocks) = v.pointer("/message/content").and_then(|c| c.as_array()) else { continue };
                for b in blocks {
                    match b.get("type").and_then(|t| t.as_str()) {
                        // One reply per message id: Claude Code splits a message across several records.
                        Some("text") => {
                            let t = b.get("text").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
                            if t.is_empty() {
                                continue;
                            }
                            match reply_index.get(&msg_id).copied() {
                                Some(i) if !msg_id.is_empty() => {
                                    if let Some(item) = items.get_mut(i) {
                                        item.add_span(span);
                                        if let Item::Reply { text, .. } = item {
                                            if text.chars().count() < REPLY_CHARS {
                                                let joined = format!("{text} {t}");
                                                *text = clean(&joined, REPLY_CHARS);
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    items.push(Item::Reply { ts, text: clean(&t, REPLY_CHARS), raw: vec![span] });
                                    reply_index.insert(msg_id.clone(), items.len() - 1);
                                }
                            }
                        }
                        Some("tool_use") => {
                            let id = b.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                            let name = b.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                            let name = display_safe(name, 60);
                            let input = b.get("input").unwrap_or(&serde_json::Value::Null);
                            let field = |k: &str| input.get(k).and_then(|x| x.as_str()).unwrap_or("");
                            let short_id: String = id.chars().take(64).collect();
                            let raw = vec![span];
                            let item = match name.as_str() {
                                "Skill" if !field("skill").is_empty() => {
                                    Item::Skill { ts, id: short_id, name: display_safe(field("skill"), 80), ok: None, error: None, raw }
                                }
                                "Agent" | "Task" => Item::Agent {
                                    ts,
                                    id: short_id,
                                    subagent_type: display_safe(field("subagent_type"), 60),
                                    description: clean(field("description"), SUMMARY_CHARS),
                                    ok: None,
                                    error: None,
                                    agent_id: None,
                                    raw,
                                },
                                _ => {
                                    let summary = tool_summary(&name, input);
                                    Item::Tool { ts, id: short_id, name, summary, ok: None, error: None, raw }
                                }
                            };
                            items.push(item);
                            if !id.is_empty() {
                                tool_index.insert(id, items.len() - 1);
                            }
                        }
                        _ => {} // thinking and anything else are never shown
                    }
                }
            }
            _ => {}
        }
    }
    // Link Agent items to their subagent transcript: meta `toolUseId` first, then the result's `agentId`.
    let links = subagent_links(&files.subagents);
    let known: std::collections::HashSet<&str> = files.subagents.iter().filter_map(|f| f.context.agent_id.as_deref()).collect();
    for (i, item) in items.iter_mut().enumerate() {
        if let Item::Agent { id, agent_id, .. } = item {
            *agent_id = links
                .get(id.as_str())
                .cloned()
                .or_else(|| fallback_agent.get(&i).filter(|a| known.contains(a.as_str())).cloned());
        }
    }
    if items.len() > MAX_ITEMS {
        items.drain(..items.len() - MAX_ITEMS);
        truncated_front = true;
    }
    Some(Timeline { items, truncated_front, skipped_lines: skipped })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::platform::CanonExt;
    use super::*;

    #[test]
    fn redacts_secret_shapes() {
        let cases = [
            "export ANTHROPIC_API_KEY=sk-ant-api03-CANARYAAAAAAAAAAAA",
            "curl -H \"Authorization: Bearer CANARYabcdefghijklmnop\" https://x",
            "token = 'CANARYvalue123'",
            "ghp_CANARYAAAAAAAAAAAAAAAAAAAAAA",
            "https://user:CANARYpass@example.com/p",
            "https://hooks.slack.com/services/T0/B0/x?token=CANARY",
            "AKIAABCDEFGHIJKLMNOP CANARYAKIA",
        ];
        for c in cases {
            let r = redact(c);
            assert!(!r.contains("CANARY") || r.contains("CANARYAKIA"), "{c} → {r}");
        }
        assert!(!redact("AKIAABCDEFGHIJKLMNOP").contains("AKIA"));
        assert_eq!(redact("cargo test -q"), "cargo test -q", "ordinary commands stay readable");
        for c in [
            "gh auth login --token CANARYtok1",
            "mysql --password CANARYpw",
            r#"{"api_key": "CANARYjson"}"#,
            "AIzaCANARYAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "-----BEGIN RSA PRIVATE KEY-----\nCANARYpem+/abc\n-----END RSA PRIVATE KEY-----",
            "authorization: CANARYhdr",
        ] {
            assert!(!redact(c).contains("CANARY"), "{c} → {}", redact(c));
        }
        // No over-masking of ordinary text.
        for keep in [
            "git commit --author=me@x.com -m msg",
            "keyboard: us",
            "authService: init",
            "git show 3f2a1b9c4d5e6f708192a3b4c5d6e7f8091a2b3c",
            "/Users/me/Agents/dev_team/app/src/components/SessionBits.tsx",
            "Basic functionality works",
            "2026-11-07-sessions-skills-connectors-and-labels-v2-final-review",
            "monkey: 1",
        ] {
            assert_eq!(redact(keep), keep, "over-masked {keep}");
        }
        assert_eq!(error_line("Exit code 1\n\nerror[E0425]: cannot find value"), "error[E0425]: cannot find value");
        assert_eq!(error_line("<tool_use_error>File has not been read yet</tool_use_error>"), "File has not been read yet");
    }

    #[test]
    fn builds_flow_without_thinking_or_result_bodies() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("projects");
        let dir = root.join("-w");
        std::fs::create_dir_all(&dir).unwrap();
        let sid = "aaaaaaaa-1111-4111-8111-111111111111";
        let lines = [
            r#"{"type":"user","timestamp":"2026-11-07T10:00:00Z","message":{"content":"<ide_opened_file>x</ide_opened_file> 결제 버그 고쳐 줘"}}"#,
            r#"{"type":"assistant","timestamp":"2026-11-07T10:00:01Z","message":{"id":"m1","content":[{"type":"thinking","thinking":"CANARY_THINK"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-11-07T10:00:02Z","message":{"id":"m1","content":[{"type":"text","text":"먼저 테스트를"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-11-07T10:00:02Z","message":{"id":"m1","content":[{"type":"text","text":"돌려 볼게요"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"npm test --token=CANARY_TOK"}}]}}"#,
            r#"{"type":"user","timestamp":"2026-11-07T10:00:03Z","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"Error: boom token=CANARY_TOK2\nmore CANARY_BODY"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-11-07T10:00:04Z","message":{"id":"m2","content":[{"type":"tool_use","id":"t2","name":"Read","input":{"file_path":"/w/a.rs"}}]}}"#,
            r#"{"type":"user","timestamp":"2026-11-07T10:00:05Z","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"CANARY_FILE_BODY"}]}}"#,
            r#"{"type":"user","isCompactSummary":true,"timestamp":"2026-11-07T10:00:06Z","message":{"content":"CANARY_SUMMARY"}}"#,
            r#"{"type":"user","timestamp":"2026-11-07T10:00:07Z","message":{"content":"<task-notification>x</task-notification>"}}"#,
        ];
        std::fs::write(dir.join(format!("{sid}.jsonl")), lines.join("\n")).unwrap();
        let root = root.canon().unwrap();
        let t = read(&root, sid).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        for c in ["CANARY_THINK", "CANARY_TOK", "CANARY_BODY", "CANARY_FILE_BODY", "CANARY_SUMMARY"] {
            assert!(!json.contains(c), "{c} leaked: {json}");
        }
        let kinds: Vec<&str> = t.items.iter().map(|i| match i { Item::Prompt { .. } => "p", Item::Reply { .. } => "r", Item::Tool { .. } => "t", Item::Compact { .. } => "c", _ => "?" }).collect();
        assert_eq!(kinds, ["p", "r", "t", "t", "c"], "one reply per message id, wrappers dropped");
        match &t.items[0] { Item::Prompt { text, .. } => assert_eq!(text, "결제 버그 고쳐 줘"), _ => panic!() }
        match &t.items[1] { Item::Reply { text, .. } => assert_eq!(text, "먼저 테스트를 돌려 볼게요"), _ => panic!() }
        match &t.items[2] {
            Item::Tool { ok, error, .. } => {
                assert_eq!(*ok, Some(false));
                assert!(error.as_deref().unwrap().starts_with("Error: boom"));
            }
            _ => panic!(),
        }
        match &t.items[3] { Item::Tool { ok, summary, .. } => assert_eq!((*ok, summary.as_str()), (Some(true), "/w/a.rs")), _ => panic!() }
        assert!(read(&root, "../etc").is_none());
    }

    #[test]
    fn skills_agents_commands_spans_and_subagent_links() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("projects");
        let dir = root.join("-w");
        let sid = "cccccccc-3333-4333-8333-333333333333";
        let sub = dir.join(sid).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let lines = [
            r#"{"type":"user","message":{"content":"<command-message>opsx:propose</command-message>\n<command-name>/opsx:propose</command-name><command-args>sk-ant-api03-CANARYARGS</command-args>"}}"#,
            r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"tu_skill","name":"Skill","input":{"skill":"opsx:propose"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_skill","content":"ok"}]}}"#,
            r#"{"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","id":"tu_a1","name":"Agent","input":{"subagent_type":"Explore","description":"파서 위치 찾기","prompt":"CANARY_PROMPT"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_a1","content":"done"}]}}"#,
            r#"{"type":"assistant","message":{"id":"m3","content":[{"type":"tool_use","id":"tu_a2","name":"Agent","input":{"subagent_type":"Explore","description":"둘째"}}]}}"#,
            r#"{"type":"user","toolUseResult":{"agentId":"old2"},"message":{"content":[{"type":"tool_result","tool_use_id":"tu_a2","content":"done"}]}}"#,
            r#"{"type":"assistant","message":{"id":"m4","content":[{"type":"tool_use","id":"tu_a3","name":"Agent","input":{"subagent_type":"Explore","description":"셋째"}}]}}"#,
            r#"{"type":"user","toolUseResult":{"agentId":"missing"},"message":{"content":[{"type":"tool_result","tool_use_id":"tu_a3","content":"done"}]}}"#,
        ];
        let body = lines.join("\n");
        std::fs::write(dir.join(format!("{sid}.jsonl")), &body).unwrap();
        std::fs::write(sub.join("agent-new1.jsonl"), "{}\n").unwrap();
        std::fs::write(sub.join("agent-new1.meta.json"), r#"{"agentType":"Explore","toolUseId":"tu_a1"}"#).unwrap();
        std::fs::write(sub.join("agent-old2.jsonl"), "{}\n").unwrap();
        std::fs::write(sub.join("agent-old2.meta.json"), r#"{"agentType":"Explore"}"#).unwrap();
        let root = root.canon().unwrap();
        let t = read(&root, sid).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("CANARY"), "no args or prompts in the flow: {json}");
        assert!(json.contains(r#""kind":"command","ts":0,"name":"opsx:propose""#), "{json}");
        assert!(json.contains(r#""subagentType":"Explore""#) && json.contains(r#""agentId":"new1""#), "{json}");
        match &t.items[1] {
            Item::Skill { name, ok, raw, .. } => {
                assert_eq!((name.as_str(), *ok), ("opsx:propose", Some(true)));
                // Spans point at the tool_use record and its result, exactly.
                let starts: Vec<usize> = raw.iter().map(|r| r[0] as usize).collect();
                assert_eq!(&body[starts[0]..starts[0] + raw[0][1] as usize], lines[1]);
                assert_eq!(&body[starts[1]..starts[1] + raw[1][1] as usize], lines[2]);
            }
            _ => panic!("skill item"),
        }
        let agents: Vec<Option<&str>> = t.items.iter().filter_map(|i| match i { Item::Agent { agent_id, .. } => Some(agent_id.as_deref()), _ => None }).collect();
        assert_eq!(agents, [Some("new1"), Some("old2"), None], "meta toolUseId, then result agentId if the file exists");
    }
}
