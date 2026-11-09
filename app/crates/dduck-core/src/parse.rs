//! Turns one transcript line into normalized events.
//!
//! Only whitelisted fields are read (D4, D-019). Free text — prompts, responses, thinking, tool input/output,
//! command/skill arguments, subagent descriptions — is never copied into an event, and parse errors carry only
//! a category, never line content (D-021).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// Which transcript file a line came from. The session id always comes from the path, not the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContext {
    pub session_id: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiCall {
    pub message_id: String,
    pub request_id: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub ts: i64,
    pub model: String,
    pub input: i64,
    pub output: i64,
    pub thinking: i64,
    pub cache_write_5m: i64,
    pub cache_write_1h: i64,
    pub cache_read: i64,
    pub web_search: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub tool_use_id: String,
    pub message_id: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub ts: i64,
    pub tool_name: String,
    pub mcp_server: Option<String>,
    pub skill_name: Option<String>,
    pub subagent_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Tool,
    Slash,
}

impl Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Via::Tool => "tool",
            Via::Slash => "slash",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandInvocation {
    pub record_uuid: String,
    pub session_id: String,
    pub ts: i64,
    /// Command or skill name without the leading `/`.
    pub name: String,
    pub via: Via,
}

/// Metadata D.Duck may store but must never print (D-023): `Debug` shows `<redacted>`.
#[derive(Clone, PartialEq, Eq)]
pub struct Redacted(pub String);

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Redacted {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionMeta {
    pub session_id: String,
    pub ts: i64,
    pub project_path: Option<Redacted>,
    pub git_branch: Option<Redacted>,
    pub version: Option<String>,
    pub entrypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Session(SessionMeta),
    Api(ApiCall),
    Tool(ToolCall),
    Command(CommandInvocation),
    CostSegment { session_id: String, start_time: i64, total_cost_usd: f64 },
    Title { session_id: String, title: Redacted },
    Artifact(ArtifactRecord),
    /// `claude --agent <name>` for the main session (record type `agent-setting`, D-055). Name only.
    AgentSetting { session_id: String, name: String },
    /// A tool result marked `is_error` (usage-insights). Id and time only, never the error text.
    ToolError { tool_use_id: String, session_id: String, ts: i64 },
    /// A `compact_boundary` system record (usage-insights). Id and time only.
    Compaction { record_uuid: String, session_id: String, ts: i64 },
    /// A usage-limit stop (usage-limit-resume): structured fields only, never the message text.
    LimitHit(LimitHit),
}

/// What D.Duck keeps of a usage-limit record: ids, times and a kind from an allowlist.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitHit {
    pub record_uuid: String,
    pub session_id: String,
    pub is_subagent: bool,
    pub ts: i64,
    /// Reset time in UTC ms; `None` when absent or implausible ("리셋 시각 모름").
    pub resets_at: Option<i64>,
    pub kind: &'static str,
}

/// Limit kinds stored as-is; anything else becomes `other`.
const LIMIT_KINDS: &[&str] = &["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"];
/// A reset more than this far after the record is treated as unknown.
const MAX_RESET_AHEAD_MS: i64 = 8 * 24 * 3600 * 1000;

/// `isApiErrorMessage` + `error: rate_limit` + `quotaLimits.status: rejected`. Reads `quotaLimits` or
/// `apiErrorParams.rate_limit_info` only for `resetsAt`/`rateLimitType`; the message body is never touched.
fn limit_hit(rec: &serde_json::Map<String, Value>, ctx: &FileContext, ts: i64) -> Option<LimitHit> {
    if rec.get("isApiErrorMessage").and_then(Value::as_bool) != Some(true) || rec.get("error").and_then(Value::as_str) != Some("rate_limit") {
        return None;
    }
    let quota = rec.get("quotaLimits").and_then(Value::as_object);
    let info = rec.get("apiErrorParams").and_then(|p| p.get("rate_limit_info")).and_then(Value::as_object);
    if let Some(q) = quota {
        if q.get("status").and_then(Value::as_str) != Some("rejected") {
            return None;
        }
    } else if info.and_then(|i| i.get("status")).and_then(Value::as_str).is_some_and(|s| s != "rejected") {
        return None;
    }
    let record_uuid = id(rec.get("uuid"))?.to_string();
    let pick = |k: &str| quota.and_then(|q| q.get(k)).or_else(|| info.and_then(|i| i.get(k)));
    let resets_at = pick("resetsAt")
        .and_then(Value::as_i64)
        .and_then(|s| s.checked_mul(1000))
        .filter(|ms| ts > 0 && *ms >= ts && *ms - ts <= MAX_RESET_AHEAD_MS);
    let kind = pick("rateLimitType").and_then(Value::as_str).and_then(|k| LIMIT_KINDS.iter().find(|x| **x == k).copied()).unwrap_or("other");
    Some(LimitHit { record_uuid, session_id: ctx.session_id.clone(), is_subagent: ctx.agent_id.is_some(), ts, resets_at, kind })
}

/// An Artifact tool publish/create result (D-052). Only these values are ever extracted.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactRecord {
    /// The id in the link (`…/artifact/<id>`), which is the merge key.
    pub artifact_id: String,
    pub url: String,
    pub title: Option<Redacted>,
    pub typed: bool,
    pub seq: i64,
    pub session_id: String,
    pub ts: i64,
}

/// Why a line could not be used. Deliberately carries no line content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidJson,
    NotAnObject,
}

impl ParseError {
    pub fn category(self) -> &'static str {
        match self {
            ParseError::InvalidJson => "invalid_json",
            ParseError::NotAnObject => "not_an_object",
        }
    }
}

const MAX_NAME_LEN: usize = 64;
const MAX_TITLE_CHARS: usize = 200;
const MAX_META_LEN: usize = 1024;

static NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Literal pattern; cannot fail to compile.
    Regex::new(r"^[A-Za-z0-9:_.\-]{1,64}$").unwrap_or_else(|_| unreachable!())
});
static ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9_\-:.]{1,128}$").unwrap_or_else(|_| unreachable!())
});
static ARTIFACT_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ahttps://claude\.ai/(?:code/)?artifact/([A-Za-z0-9-]{1,64})\z").unwrap_or_else(|_| unreachable!())
});
static ARTIFACT_VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[0-9]{1,12}-[0-9a-f]{4,64}\z").unwrap_or_else(|_| unreachable!()));
static COMMAND_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<command-name>([^<]{0,80})</command-name>").unwrap_or_else(|_| unreachable!())
});

/// Identifier fields (message/request/tool ids, tool and model names) must look like identifiers, so no free text
/// can ride along in an id column.
fn id(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| ID_RE.is_match(s))
}

use crate::text::is_unsafe_char;

/// Validates a command/skill/subagent identifier and strips a leading `/`.
pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim().strip_prefix('/').unwrap_or(raw.trim());
    (name.len() <= MAX_NAME_LEN && NAME_RE.is_match(name)).then(|| name.to_string())
}

pub fn parse_line(line: &str, ctx: &FileContext) -> Result<Vec<Event>, ParseError> {
    let value: Value = serde_json::from_str(line).map_err(|_| ParseError::InvalidJson)?;
    let rec = value.as_object().ok_or(ParseError::NotAnObject)?;
    let ts = rec.get("timestamp").and_then(Value::as_str).map(parse_ts).unwrap_or(0);

    let mut events = Vec::new();
    match rec.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            push_session_meta(&mut events, rec, ctx, ts);
            if let Some(hit) = limit_hit(rec, ctx, ts) {
                events.push(Event::LimitHit(hit));
            }
            parse_assistant(&mut events, rec, ctx, ts);
        }
        Some("user") => {
            push_session_meta(&mut events, rec, ctx, ts);
            parse_user(&mut events, rec, ctx, ts);
        }
        Some("cost-state") => {
            let start = rec.get("startTime").and_then(Value::as_i64);
            let total = rec.get("totalCostUSD").and_then(Value::as_f64);
            if let (Some(start_time), Some(total_cost_usd)) = (start, total) {
                events.push(Event::CostSegment { session_id: ctx.session_id.clone(), start_time, total_cost_usd });
            }
        }
        Some("agent-setting") if ctx.agent_id.is_none() => {
            if let Some(name) = rec.get("agentSetting").and_then(Value::as_str).filter(|n| crate::agents_catalog::valid_name(n)) {
                events.push(Event::AgentSetting { session_id: ctx.session_id.clone(), name: name.to_string() });
            }
        }
        Some("ai-title") if ctx.agent_id.is_none() => {
            if let Some(title) = rec.get("aiTitle").and_then(Value::as_str).and_then(clean_title) {
                events.push(Event::Title { session_id: ctx.session_id.clone(), title: Redacted(title) });
            }
        }
        Some("system") if rec.get("subtype").and_then(Value::as_str) == Some("compact_boundary") => {
            if let Some(uuid) = id(rec.get("uuid")) {
                events.push(Event::Compaction { record_uuid: uuid.to_string(), session_id: ctx.session_id.clone(), ts });
            }
        }
        // attachment, api-request-*, other system records, file-history-*, … are ignored on purpose.
        _ => {}
    }
    Ok(events)
}

fn push_session_meta(events: &mut Vec<Event>, rec: &serde_json::Map<String, Value>, ctx: &FileContext, ts: i64) {
    // Values with control/bidi characters are dropped rather than cleaned: they are paths and names, and a
    // "cleaned" path would point somewhere else.
    let field = |k: &str| {
        rec.get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= MAX_META_LEN && !s.chars().any(is_unsafe_char))
            .map(str::to_string)
    };
    events.push(Event::Session(SessionMeta {
        session_id: ctx.session_id.clone(),
        ts,
        project_path: field("cwd").map(Redacted),
        git_branch: field("gitBranch").map(Redacted),
        version: field("version").filter(|v| ID_RE.is_match(v)),
        entrypoint: field("entrypoint").filter(|v| ID_RE.is_match(v)),
    }));
}

fn parse_assistant(events: &mut Vec<Event>, rec: &serde_json::Map<String, Value>, ctx: &FileContext, ts: i64) {
    let Some(msg) = rec.get("message").and_then(Value::as_object) else { return };
    let model = msg.get("model").and_then(Value::as_str).unwrap_or("");
    let model_ok = ID_RE.is_match(model);
    let api_error = rec.get("isApiErrorMessage").and_then(Value::as_bool).unwrap_or(false);
    // `<synthetic>` fails the id pattern too; the explicit check documents the intent.
    if model == "<synthetic>" || !model_ok || api_error {
        return;
    }
    let Some(message_id) = id(msg.get("id")) else { return };
    let request_id = match rec.get("requestId") {
        None | Some(Value::Null) => String::new(),
        Some(v) => match id(Some(v)) {
            Some(r) => r.to_string(),
            None => return,
        },
    };

    if let Some(usage) = msg.get("usage").and_then(Value::as_object) {
        let n = |v: Option<&Value>| v.and_then(Value::as_i64).unwrap_or(0).max(0);
        let cache = usage.get("cache_creation").and_then(Value::as_object);
        let total_cw = n(usage.get("cache_creation_input_tokens"));
        let (cw5, cw1) = match cache {
            Some(c) => (n(c.get("ephemeral_5m_input_tokens")), n(c.get("ephemeral_1h_input_tokens"))),
            // Older records only carry the total; treat it as 5-minute cache writes.
            None => (total_cw, 0),
        };
        events.push(Event::Api(ApiCall {
            message_id: message_id.to_string(),
            request_id,
            session_id: ctx.session_id.clone(),
            agent_id: ctx.agent_id.clone(),
            ts,
            model: model.to_string(),
            input: n(usage.get("input_tokens")),
            output: n(usage.get("output_tokens")),
            thinking: n(usage.get("output_tokens_details").and_then(|d| d.get("thinking_tokens"))),
            cache_write_5m: cw5,
            cache_write_1h: cw1,
            cache_read: n(usage.get("cache_read_input_tokens")),
            web_search: n(usage.get("server_tool_use").and_then(|d| d.get("web_search_requests"))),
        }));
    }

    let Some(content) = msg.get("content").and_then(Value::as_array) else { return };
    for block in content {
        if block.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let (Some(id), Some(name)) = (id(block.get("id")), id(block.get("name"))) else {
            continue;
        };
        // Only these two input keys are ever read; everything else in `input` is free text.
        let input = block.get("input");
        let skill_name = (name == "Skill")
            .then(|| input.and_then(|i| i.get("skill")).and_then(Value::as_str).and_then(clean_name))
            .flatten();
        let subagent_type = (name == "Agent" || name == "Task")
            .then(|| input.and_then(|i| i.get("subagent_type")).and_then(Value::as_str).and_then(clean_name))
            .flatten();
        let mcp_server = name
            .strip_prefix("mcp__")
            .and_then(|rest| rest.split("__").next())
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        if let Some(skill) = &skill_name {
            events.push(Event::Command(CommandInvocation {
                record_uuid: id.to_string(),
                session_id: ctx.session_id.clone(),
                ts,
                name: skill.clone(),
                via: Via::Tool,
            }));
        }
        events.push(Event::Tool(ToolCall {
            tool_use_id: id.to_string(),
            message_id: message_id.to_string(),
            session_id: ctx.session_id.clone(),
            agent_id: ctx.agent_id.clone(),
            ts,
            tool_name: name.to_string(),
            mcp_server,
            skill_name,
            subagent_type,
        }));
    }
}

/// Artifact results ride on tool_result records (array content), so this runs before the string-content check.
/// Only `url`, `title`, `version`, `seq` and the presence of `type` are read (D-052).
fn parse_artifact(rec: &serde_json::Map<String, Value>, ctx: &FileContext, ts: i64) -> Option<ArtifactRecord> {
    let r = rec.get("toolUseResult")?.as_object()?;
    let url = r.get("url")?.as_str()?;
    let artifact_id = ARTIFACT_URL_RE.captures(url)?.get(1)?.as_str().to_string();
    let title = r.get("title")?.as_str()?;
    if !ARTIFACT_VERSION_RE.is_match(r.get("version")?.as_str()?) {
        return None; // e.g. a web fetch of an artifact link: url, but no artifact version marker
    }
    Some(ArtifactRecord {
        artifact_id,
        url: url.to_string(),
        title: clean_title(title).map(Redacted),
        typed: r.get("type").is_some_and(Value::is_object),
        seq: r.get("seq").and_then(Value::as_i64).filter(|s| (1..=1_000_000).contains(s)).unwrap_or(1),
        session_id: ctx.session_id.clone(),
        ts,
    })
}

fn parse_user(events: &mut Vec<Event>, rec: &serde_json::Map<String, Value>, ctx: &FileContext, ts: i64) {
    if let Some(a) = parse_artifact(rec, ctx, ts) {
        events.push(Event::Artifact(a));
    }
    // Failed tool results: only the id of the call they answer, never their content.
    if let Some(blocks) = rec.get("message").and_then(|m| m.get("content")).and_then(Value::as_array) {
        for b in blocks {
            let is_err = b.get("type").and_then(Value::as_str) == Some("tool_result") && b.get("is_error").and_then(Value::as_bool) == Some(true);
            if let Some(tid) = id(b.get("tool_use_id")).filter(|_| is_err) {
                events.push(Event::ToolError { tool_use_id: tid.to_string(), session_id: ctx.session_id.clone(), ts });
            }
        }
    }
    if rec.get("isMeta").and_then(Value::as_bool).unwrap_or(false) {
        return;
    }
    let Some(uuid) = id(rec.get("uuid")) else { return };
    // Only a plain-string message that *starts* with a command tag is a typed slash command.
    // Tool results, attachments and prompts that merely mention a tag are ignored (D-019).
    let Some(text) = rec.get("message").and_then(|m| m.get("content")).and_then(Value::as_str) else { return };
    let trimmed = text.trim_start();
    if !(trimmed.starts_with("<command-name>") || trimmed.starts_with("<command-message>")) {
        return;
    }
    let Some(name) = COMMAND_NAME_RE.captures(trimmed).and_then(|c| c.get(1)).and_then(|m| clean_name(m.as_str()))
    else {
        return;
    };
    events.push(Event::Command(CommandInvocation {
        record_uuid: uuid.to_string(),
        session_id: ctx.session_id.clone(),
        ts,
        name,
        via: Via::Slash,
    }));
}

fn clean_title(raw: &str) -> Option<String> {
    let title: String = raw.chars().filter(|c| !is_unsafe_char(*c)).take(MAX_TITLE_CHARS).collect();
    let title = title.trim().to_string();
    (!title.is_empty()).then_some(title)
}

fn parse_ts(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp_millis()).unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ctx() -> FileContext {
        FileContext { session_id: "s1".into(), agent_id: None }
    }

    #[test]
    fn clean_name_rules() {
        assert_eq!(clean_name("/opsx:propose").as_deref(), Some("opsx:propose"));
        assert_eq!(clean_name("upgrade").as_deref(), Some("upgrade"));
        assert_eq!(clean_name("/bad name;rm"), None);
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name(&"a".repeat(65)), None);
    }

    #[test]
    fn invalid_json_has_category_only() {
        let err = parse_line("{oops SECRET", &ctx()).unwrap_err();
        assert_eq!(err.category(), "invalid_json");
        assert!(!format!("{err:?}").contains("SECRET"));
    }

    #[test]
    fn unsafe_metadata_and_ids_are_rejected() {
        let rec = r#"{"type":"user","uuid":"u1","cwd":"/tmp/a\nINJECT","gitBranch":"main\u202e","message":{"content":"hi"}}"#;
        let evs = parse_line(rec, &ctx()).unwrap();
        let Some(Event::Session(m)) = evs.first() else { panic!() };
        assert_eq!((m.project_path.as_ref(), m.git_branch.as_ref()), (None, None));

        let free = r#"{"type":"assistant","requestId":"REQ free text","message":{"model":"claude-x","id":"msg_1","usage":{"output_tokens":1}}}"#;
        assert!(parse_line(free, &ctx()).unwrap().iter().all(|e| !matches!(e, Event::Api(_))));

        let title = r#"{"type":"ai-title","aiTitle":"Fix\u202eevil\u200b title"}"#;
        let evs = parse_line(title, &ctx()).unwrap();
        assert!(matches!(&evs[0], Event::Title { title, .. } if title.as_str() == "Fixevil title"));
    }

    #[test]
    fn debug_output_redacts_metadata() {
        let rec = r#"{"type":"user","uuid":"u1","cwd":"/Users/me/SECRETPROJ","gitBranch":"SECRETBRANCH","message":{"content":"hi"}}"#;
        let evs = parse_line(rec, &ctx()).unwrap();
        let dbg = format!("{evs:?}");
        assert!(!dbg.contains("SECRET"), "{dbg}");
    }

    #[test]
    fn tool_errors_and_compactions_keep_ids_only() {
        let err = r#"{"type":"user","uuid":"u9","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":true,"content":"CANARY_ERR"},{"type":"tool_result","tool_use_id":"toolu_2","content":"ok"}]}}"#;
        let evs = parse_line(err, &ctx()).unwrap();
        let errs: Vec<_> = evs.iter().filter(|e| matches!(e, Event::ToolError { .. })).collect();
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], Event::ToolError { tool_use_id, .. } if tool_use_id == "toolu_1"));
        assert!(!format!("{evs:?}").contains("CANARY"));
        let compact = r#"{"type":"system","subtype":"compact_boundary","uuid":"c1","timestamp":"2026-10-13T17:01:08.456Z","content":"CANARY"}"#;
        let evs = parse_line(compact, &ctx()).unwrap();
        assert!(matches!(&evs[..], [Event::Compaction { record_uuid, ts, .. }] if record_uuid == "c1" && *ts > 0));
        let other = r#"{"type":"system","subtype":"api_error","uuid":"c2"}"#;
        assert!(parse_line(other, &ctx()).unwrap().is_empty());
    }

    fn limit_rec(extra: &str) -> String {
        format!(
            r#"{{"type":"assistant","uuid":"L1","timestamp":"2026-11-09T03:00:00.000Z","isApiErrorMessage":true,"error":"rate_limit","apiErrorStatus":429{extra},"message":{{"model":"<synthetic>","content":[{{"type":"text","text":"CANARY_LIMIT_TEXT"}}]}}}}"#
        )
    }

    #[test]
    fn limit_hits_keep_structured_fields_only() {
        let ts = parse_ts("2026-11-09T03:00:00.000Z");
        let reset = ts / 1000 + 3 * 3600;
        let five = limit_rec(&format!(r#","quotaLimits":{{"status":"rejected","resetsAt":{reset},"rateLimitType":"five_hour","upgradePaths":["CANARY_UP"]}}"#));
        let evs = parse_line(&five, &ctx()).unwrap();
        let hits: Vec<_> = evs.iter().filter_map(|e| if let Event::LimitHit(h) = e { Some(h) } else { None }).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].resets_at, hits[0].kind, hits[0].is_subagent), (Some(reset * 1000), "five_hour", false));
        assert!(!format!("{evs:?}").contains("CANARY"), "no message text or upgrade paths");
        assert!(evs.iter().all(|e| !matches!(e, Event::Api(_))), "still not an API call");

        // Fallback field, sub-agent file, unknown kind.
        let sub = FileContext { session_id: "s1".into(), agent_id: Some("a1".into()) };
        let alt = limit_rec(&format!(r#","apiErrorParams":{{"rate_limit_info":{{"status":"rejected","resetsAt":{reset},"rateLimitType":"weird"}}}}"#));
        let Some(Event::LimitHit(h)) = parse_line(&alt, &sub).unwrap().into_iter().find(|e| matches!(e, Event::LimitHit(_))) else { panic!() };
        assert_eq!((h.resets_at, h.kind, h.is_subagent), (Some(reset * 1000), "other", true));

        // No reset fields, or a reset in the past / too far ahead: "리셋 시각 모름".
        for extra in [String::new(), format!(r#","quotaLimits":{{"status":"rejected","resetsAt":{}}}"#, ts / 1000 - 10), format!(r#","quotaLimits":{{"status":"rejected","resetsAt":{}}}"#, ts / 1000 + 9 * 86400)] {
            let Some(Event::LimitHit(h)) = parse_line(&limit_rec(&extra), &ctx()).unwrap().into_iter().find(|e| matches!(e, Event::LimitHit(_))) else { panic!("{extra}") };
            assert_eq!(h.resets_at, None, "{extra}");
        }
    }

    #[test]
    fn other_api_errors_are_not_limit_hits() {
        let allowed = limit_rec(r#","quotaLimits":{"status":"allowed_warning","resetsAt":1794113400}"#);
        assert!(parse_line(&allowed, &ctx()).unwrap().iter().all(|e| !matches!(e, Event::LimitHit(_))));
        let auth = r#"{"type":"assistant","uuid":"L2","isApiErrorMessage":true,"error":"authentication_failed","message":{"model":"<synthetic>"}}"#;
        assert!(parse_line(auth, &ctx()).unwrap().iter().all(|e| !matches!(e, Event::LimitHit(_))));
        let plain = r#"{"type":"assistant","uuid":"L3","error":"rate_limit","message":{"model":"claude-x","id":"m1"}}"#;
        assert!(parse_line(plain, &ctx()).unwrap().iter().all(|e| !matches!(e, Event::LimitHit(_))));
    }

    #[test]
    fn slash_command_must_start_message() {
        let mention = r#"{"type":"user","uuid":"u1","message":{"content":"see <command-name>/x</command-name>"}}"#;
        assert!(parse_line(mention, &ctx()).unwrap().iter().all(|e| !matches!(e, Event::Command(_))));
        let typed = r#"{"type":"user","uuid":"u2","message":{"content":"<command-name>/x</command-name>"}}"#;
        let evs = parse_line(typed, &ctx()).unwrap();
        assert!(evs.iter().any(|e| matches!(e, Event::Command(c) if c.name == "x" && c.via == Via::Slash)));
    }
}
