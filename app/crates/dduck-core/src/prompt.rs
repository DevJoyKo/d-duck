//! Reads the first instruction of a session from its transcript, on demand, for display only (D-048).
//!
//! The text is returned to the UI and never stored, logged or cached on disk — the same rule as `claude logs`.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::discover;

/// Stop scanning after this many bytes; the first prompt is near the top of the file.
const SCAN_LIMIT: u64 = 4 * 1024 * 1024;
pub const MAX_PROMPT_CHARS: usize = 4000;

/// First user-typed instruction of `session_id` (not a slash command, not a meta/tool message).
pub fn first_prompt(projects_root: &Path, session_id: &str) -> Option<String> {
    let file = discover::discover(projects_root)
        .into_iter()
        .find(|f| f.kind == discover::TranscriptKind::Main && f.context.session_id == session_id)?;
    let reader = BufReader::new(File::open(&file.path).ok()?.take(SCAN_LIMIT));
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if let Some(text) = prompt_from_line(&line) {
            return Some(text);
        }
    }
    None
}

fn prompt_from_line(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "user" || v.get("isMeta").and_then(|m| m.as_bool()).unwrap_or(false) {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    let text = match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => {
            // A prompt is text blocks only; arrays holding tool results are not instructions.
            if blocks.iter().any(|b| b.get("type").and_then(|t| t.as_str()) != Some("text")) {
                return None;
            }
            blocks.iter().filter_map(|b| b.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")
        }
        _ => return None,
    };
    let stripped = strip_wrappers(&text);
    let trimmed = stripped.trim();
    if trimmed.is_empty() || trimmed.starts_with("<command-") || trimmed.starts_with("<local-command") {
        return None;
    }
    let clean: String = trimmed.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).take(MAX_PROMPT_CHARS).collect();
    Some(clean)
}

/// Context the IDE or harness adds around what the user typed (opened file, selection, notifications…).
const WRAPPERS: [&str; 7] = ["ide_opened_file", "ide_selection", "ide_diagnostics", "system-reminder", "task-notification", "pasted_content", "local-command-caveat"];

/// Drops leading `<tag>…</tag>` wrapper blocks so the instruction itself remains.
pub fn strip_wrappers(text: &str) -> String {
    let mut rest = text.trim_start();
    'outer: loop {
        for tag in WRAPPERS {
            let open = format!("<{tag}");
            if rest.starts_with(&open) {
                let close = format!("</{tag}>");
                match rest.find(&close) {
                    Some(i) => {
                        rest = rest[i + close.len()..].trim_start();
                        continue 'outer;
                    }
                    None => return String::new(),
                }
            }
        }
        return rest.to_string();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const S: &str = "11111111-1111-4111-8111-111111111111";

    #[test]
    fn finds_the_first_typed_instruction() {
        // The basic fixture starts with a typed prompt that mentions a command tag.
        let root = discover::projects_root(&crate::testkit::ensure_basic()).unwrap();
        let p = first_prompt(&root, S).unwrap();
        assert!(p.starts_with("Please explain what"), "{p}");
        assert_eq!(first_prompt(&root, "22222222-2222-4222-8222-222222222222"), None);
    }

    #[test]
    fn skips_commands_meta_and_tool_results() {
        assert_eq!(prompt_from_line(r#"{"type":"user","message":{"content":"<command-name>/x</command-name>"}}"#), None);
        assert_eq!(prompt_from_line(r#"{"type":"user","isMeta":true,"message":{"content":"hidden"}}"#), None);
        assert_eq!(prompt_from_line(r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}"#), None);
        assert_eq!(prompt_from_line(r#"{"type":"assistant","message":{"content":"no"}}"#), None);
        assert_eq!(
            prompt_from_line(r#"{"type":"user","message":{"content":"<ide_opened_file>The user opened x.md</ide_opened_file>\n결제 버그 고쳐 줘"}}"#).as_deref(),
            Some("결제 버그 고쳐 줘"),
            "IDE wrapper removed"
        );
        assert_eq!(prompt_from_line(r#"{"type":"user","message":{"content":"<ide_opened_file>only context</ide_opened_file>"}}"#), None);
        assert_eq!(
            prompt_from_line(r#"{"type":"user","message":{"content":[{"type":"text","text":"fix the tests"}]}}"#).as_deref(),
            Some("fix the tests")
        );
        let long = format!(r#"{{"type":"user","message":{{"content":"{}"}}}}"#, "a".repeat(9000));
        assert_eq!(prompt_from_line(&long).unwrap().chars().count(), MAX_PROMPT_CHARS);
    }
}
