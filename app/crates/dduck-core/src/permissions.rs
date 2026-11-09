//! Claude Code tool permissions (D-076, D-079): read and write only the `permissions` key of settings files,
//! validate rules, and explain which rule decides a tool call.
//!
//! Settings files can hold secrets (`env`, `apiKeyHelper`, hooks). Nothing outside `permissions` leaves this module:
//! the file is parsed to check it is valid JSON, then only the `permissions` member is extracted. Saving splices the
//! new `permissions` text into the original bytes, so every other key keeps its exact bytes, order and numbers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::text::{display_safe, is_unsafe_char};

pub const MAX_SETTINGS_BYTES: u64 = 256 * 1024;
pub const MAX_RULES_PER_LIST: usize = 500;
const MAX_RULE_CHARS: usize = 600;
const MAX_BACKUPS: usize = 10;
/// Modes the app lets the user pick. `bypassPermissions` is never offered (security review).
pub const MODES: [&str; 5] = ["default", "acceptEdits", "plan", "auto", "dontAsk"];
const RISKY_MODES: [&str; 3] = ["acceptEdits", "auto", "dontAsk"];

// ---------- catalog ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogGroup {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub group: String,
    pub rule: String,
    pub risk: String,
    pub recommended: String,
    /// Recommendation for the user-wide scope when it differs (edits: ask globally, allow per project).
    #[serde(default)]
    pub recommended_global: Option<String>,
    pub description: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub groups: Vec<CatalogGroup>,
    pub items: Vec<CatalogItem>,
}

fn load_catalog(lang: crate::i18n::Lang) -> Catalog {
    serde_json::from_str::<serde_json::Value>(include_str!("../data/permission_catalog.json"))
        .ok()
        .and_then(|v| serde_json::from_value(crate::i18n::pick_lang(v, lang)).ok())
        .unwrap_or(Catalog { groups: Vec::new(), items: Vec::new() })
}

/// Rules and risks are the same in both languages; only labels, descriptions and reasons differ.
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| load_catalog(crate::i18n::Lang::Ko));
static CATALOG_EN: LazyLock<Catalog> = LazyLock::new(|| load_catalog(crate::i18n::Lang::En));

/// The catalog in the current app language.
pub fn catalog() -> &'static Catalog {
    catalog_in(crate::i18n::current())
}

pub fn catalog_in(lang: crate::i18n::Lang) -> &'static Catalog {
    if lang == crate::i18n::Lang::En { &CATALOG_EN } else { &CATALOG }
}

/// Risk of a rule: the catalog's level for an exact match, else a conservative guess for a few obvious shapes.
pub fn risk_of_rule(rule: &str) -> &'static str {
    if let Some(i) = CATALOG.items.iter().find(|i| i.rule == rule) {
        return match i.risk.as_str() {
            "high" => "high",
            "medium" => "medium",
            _ => "low",
        };
    }
    let Ok(r) = parse_rule(rule) else { return "high" };
    let spec = r.spec.as_deref().unwrap_or("");
    match r.tool.as_str() {
        "*" => "high",
        "Bash" if spec.is_empty() || spec == "*" => "high",
        "Bash" if ["rm", "sudo", "curl", "wget", "git push", "kubectl", "terraform", "ssh", "scp", "dd"].iter().any(|p| spec.starts_with(p)) => "high",
        "Bash" | "Edit" | "Write" | "NotebookEdit" | "MultiEdit" | "WebFetch" => "medium",
        "Read" if [".env", ".ssh", ".aws", "secret", "credential"].iter().any(|p| spec.contains(p)) => "high",
        t if t.starts_with("mcp__") => "medium",
        _ => "low",
    }
}

// ---------- rules ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub tool: String,
    pub spec: Option<String>,
}

fn tool_char_ok(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '*')
}

/// Parses `Tool`, `Tool(spec)`, `mcp__server[__tool]` (globs allowed) and `*`. Rejects control/bidi characters.
pub fn parse_rule(s: &str) -> Result<Rule, &'static str> {
    if s.is_empty() || s.chars().count() > MAX_RULE_CHARS {
        return Err(crate::i18n::Fail::PermEmptyLong.str());
    }
    if s.chars().any(is_unsafe_char) {
        return Err(crate::i18n::Fail::PermInvisible.str());
    }
    if s.trim() != s {
        return Err(crate::i18n::Fail::PermSpaces.str());
    }
    let (tool, spec) = match s.find('(') {
        Some(i) => {
            if !s.ends_with(')') || i == 0 {
                return Err(crate::i18n::Fail::PermParens.str());
            }
            let inner = &s[i + 1..s.len() - 1];
            if inner.is_empty() {
                return Err(crate::i18n::Fail::PermParensEmpty.str());
            }
            (&s[..i], Some(inner.to_string()))
        }
        None => (s, None),
    };
    if !tool.chars().all(tool_char_ok) || !tool.starts_with(|c: char| c.is_ascii_alphabetic() || c == '*') {
        return Err(crate::i18n::Fail::PermToolName.str());
    }
    if tool.contains('*') && !tool.starts_with("mcp__") && tool != "*" {
        return Err(crate::i18n::Fail::PermWildcard.str());
    }
    Ok(Rule { tool: tool.to_string(), spec })
}

/// Forms Claude Code accepts syntactically but ignores or treats unexpectedly.
pub fn rule_warnings(s: &str, list: &str) -> Vec<&'static str> {
    let Ok(r) = parse_rule(s) else { return Vec::new() };
    let mut w = Vec::new();
    if r.tool.starts_with("mcp__") && r.spec.is_some() {
        w.push(crate::i18n::Fail::PermMcpParens.str());
    }
    let unanchored = r.tool == "*" || r.tool.starts_with("mcp__*");
    if list == "allow" && unanchored {
        w.push(crate::i18n::Fail::PermLooseWild.str());
    }
    if r.tool == "Bash" && r.spec.as_deref().is_some_and(|sp| sp.contains(":*") && !sp.ends_with(":*")) {
        w.push(crate::i18n::Fail::PermColonStar.str());
    }
    w
}

// ---------- JSON member scanning (input already validated by serde_json) ----------

struct Member {
    key: String,
    key_start: usize,
    val_start: usize,
    val_end: usize,
}

struct Obj {
    open: usize,
    close: usize,
    members: Vec<Member>,
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

fn skip_string(b: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    i
}

fn skip_value(b: &[u8], i: usize) -> usize {
    match b.get(i) {
        Some(b'"') => skip_string(b, i),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j = skip_string(b, j);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return j + 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            j
        }
        _ => {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            j
        }
    }
}

fn scan_object(text: &str, start: usize) -> Option<Obj> {
    let b = text.as_bytes();
    let open = skip_ws(b, start);
    if b.get(open) != Some(&b'{') {
        return None;
    }
    let mut i = skip_ws(b, open + 1);
    let mut members = Vec::new();
    if b.get(i) == Some(&b'}') {
        return Some(Obj { open, close: i, members });
    }
    loop {
        if b.get(i) != Some(&b'"') {
            return None;
        }
        let key_start = i;
        let key_end = skip_string(b, i);
        let key: String = serde_json::from_str(text.get(key_start..key_end)?).ok()?;
        i = skip_ws(b, key_end);
        if b.get(i) != Some(&b':') {
            return None;
        }
        let val_start = skip_ws(b, i + 1);
        let val_end = skip_value(b, val_start);
        members.push(Member { key, key_start, val_start, val_end });
        i = skip_ws(b, val_end);
        match b.get(i) {
            Some(b',') => i = skip_ws(b, i + 1),
            Some(b'}') => return Some(Obj { open, close: i, members }),
            _ => return None,
        }
    }
}

/// Leading whitespace of the line holding `pos`.
fn indent_at(text: &str, pos: usize) -> String {
    let line_start = text[..pos].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..pos].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

// ---------- reading a scope ----------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeView {
    pub path: String,
    pub exists: bool,
    /// sha256 of the file bytes, "" when the file does not exist.
    pub token: String,
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    pub default_mode: Option<String>,
    /// Other members of `permissions` (additionalDirectories, disableBypassPermissionsMode…), shown read-only.
    pub other: Vec<(String, Value)>,
    /// Member order of `permissions` in the file (the preview mirrors it and omits lists the file omits).
    pub keys: Vec<String>,
    pub readonly_reason: Option<String>,
    /// `allowManagedPermissionRulesOnly` (top level or under permissions) is true in this file.
    pub managed_only: bool,
    pub error: Option<String>,
}

pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(unix)]
fn nlink(m: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::nlink(m)
}
#[cfg(not(unix))]
fn nlink(_m: &std::fs::Metadata) -> u64 {
    1
}

fn str_list(v: Option<&Value>) -> Result<Vec<String>, ()> {
    match v {
        None => Ok(Vec::new()),
        // Raw text: rules go back to the file unchanged, so nothing is stripped or cut here (display code sanitizes).
        Some(Value::Array(a)) => a.iter().map(|x| x.as_str().map(str::to_string).ok_or(())).collect(),
        Some(_) => Err(()),
    }
}

/// Reads one settings file. Only the `permissions` member (and the `allowManagedPermissionRulesOnly` flag) is kept.
pub fn read_scope(path: &Path) -> ScopeView {
    let mut view = ScopeView { path: display_safe(&path.display().to_string(), 400), ..Default::default() };
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return view,
    };
    view.exists = true;
    if meta.file_type().is_symlink() {
        view.readonly_reason = Some(crate::i18n::Fail::LinkedNoEdit.str().into());
    } else if !meta.file_type().is_file() {
        view.readonly_reason = Some(crate::i18n::Fail::NotRegularFile.str().into());
        return view;
    } else if nlink(&meta) > 1 {
        view.readonly_reason = Some(crate::i18n::Fail::HardlinkNoEditHere.str().into());
    }
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() <= MAX_SETTINGS_BYTES => std::fs::read(path).ok(),
        _ => None,
    };
    let Some(bytes) = bytes else {
        view.readonly_reason = Some(crate::i18n::Fail::ReadOr256.str().into());
        return view;
    };
    view.token = sha(&bytes);
    let Ok(text) = String::from_utf8(bytes) else {
        view.error = Some(crate::i18n::Fail::NotUtf8File.str().into());
        view.readonly_reason = Some(crate::i18n::Fail::FileFormatBad.str().into());
        return view;
    };
    if text.trim().is_empty() {
        return view;
    }
    let root: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        // Position only: the error text could quote file content.
        Err(e) => {
            view.error = Some(crate::tr_fmt!(crate::i18n::Fail::JsonAt, a0 = e.line(), a1 = e.column()));
            view.readonly_reason = Some(crate::i18n::Fail::FileFormatBad.str().into());
            return view;
        }
    };
    let Value::Object(mut root) = root else {
        view.readonly_reason = Some(crate::i18n::Fail::TopNotObject.str().into());
        return view;
    };
    let top_flag = root.get("allowManagedPermissionRulesOnly").and_then(Value::as_bool).unwrap_or(false);
    let perms = root.remove("permissions");
    drop(root); // everything else is discarded here
    let mut perms = match perms {
        None => {
            view.managed_only = top_flag;
            return view;
        }
        Some(Value::Object(m)) => m,
        Some(_) => {
            view.readonly_reason = Some(crate::i18n::Fail::PermNotObject.str().into());
            return view;
        }
    };
    view.managed_only = top_flag || perms.get("allowManagedPermissionRulesOnly").and_then(Value::as_bool).unwrap_or(false);
    let lists = (str_list(perms.get("allow")), str_list(perms.get("ask")), str_list(perms.get("deny")));
    match lists {
        (Ok(a), Ok(k), Ok(d)) => (view.allow, view.ask, view.deny) = (a, k, d),
        _ => view.readonly_reason = Some(crate::i18n::Fail::PermNonString.str().into()),
    }
    match perms.get("defaultMode") {
        None => {}
        Some(Value::String(s)) => view.default_mode = Some(display_safe(s, 40)),
        Some(_) => view.readonly_reason = Some(crate::i18n::Fail::ModeFormat.str().into()),
    }
    // Remaining keys, in file order (serde_json sorts maps, so take the order from the text).
    let order: Vec<String> = scan_object(&text, 0)
        .and_then(|o| o.members.into_iter().find(|m| m.key == "permissions"))
        .and_then(|m| scan_object(&text, m.val_start))
        .map(|o| o.members.into_iter().map(|m| m.key).collect())
        .unwrap_or_default();
    view.keys = order.clone();
    for k in order {
        if matches!(k.as_str(), "allow" | "ask" | "deny" | "defaultMode") {
            continue;
        }
        if let Some(v) = perms.remove(&k) {
            view.other.push((k, v));
        }
    }
    view
}

// ---------- managed sources ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Managed {
    pub sources: Vec<ScopeView>,
    /// A configuration profile exists; its contents are not read.
    pub mdm: Option<String>,
    pub managed_only: bool,
}

pub fn default_managed_roots() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Library/Application Support/ClaudeCode")]
    } else {
        vec![PathBuf::from("/etc/claude-code")]
    }
}

pub fn default_mdm_files() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Library/Managed Preferences/com.anthropic.claudecode.plist")]
    } else {
        Vec::new()
    }
}

pub fn managed(roots: &[PathBuf], mdm_files: &[PathBuf]) -> Managed {
    let mut sources = Vec::new();
    for root in roots {
        let main = root.join("managed-settings.json");
        if main.exists() {
            sources.push(read_scope(&main));
        }
        if let Ok(rd) = std::fs::read_dir(root.join("managed-settings.d")) {
            let mut files: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
            files.sort();
            for f in files.into_iter().take(50) {
                sources.push(read_scope(&f));
            }
        }
    }
    for s in &mut sources {
        s.readonly_reason = Some(crate::i18n::Fail::ManagedReadOnly.str().into());
    }
    let managed_only = sources.iter().any(|s| s.managed_only);
    let mdm = mdm_files.iter().find(|p| p.exists()).map(|p| display_safe(&p.display().to_string(), 300));
    Managed { sources, mdm, managed_only }
}

// ---------- saving ----------

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PermError {
    #[error("{0}")]
    ReadOnly(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{}", crate::i18n::ReasonCode::FileChanged.now())]
    Conflict,
    #[error("{}", crate::i18n::Fail::SaveFileFailed.str())]
    Io,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermEdit {
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    pub default_mode: Option<String>,
}

#[derive(Debug)]
pub struct PermPlan {
    pub path: PathBuf,
    pub exists: bool,
    pub content: String,
    pub old_token: String,
    /// (list, rule)
    pub added: Vec<(&'static str, String)>,
    pub removed: Vec<(&'static str, String)>,
    pub high_allows: Vec<String>,
    pub high_denies_removed: Vec<String>,
    pub mode_change: Option<(Option<String>, Option<String>)>,
    pub risky_mode: bool,
    old_permissions: Value,
    new_file_mode: u32,
    #[cfg(unix)]
    ino: u64,
    #[cfg(unix)]
    mode: u32,
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Element spans of a JSON array starting at `start`.
fn scan_array(text: &str, start: usize) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    if b.get(start) != Some(&b'[') {
        return out;
    }
    let mut i = skip_ws(b, start + 1);
    while i < b.len() && b[i] != b']' {
        let end = skip_value(b, i);
        out.push((i, end));
        i = skip_ws(b, end);
        if b.get(i) == Some(&b',') {
            i = skip_ws(b, i + 1);
        } else {
            break;
        }
    }
    out
}

/// Original element text by decoded value, so unchanged rules keep their exact bytes (escapes included).
type RawMap = std::collections::HashMap<String, String>;

fn render_list(items: &[String], ind: &str, unit: &str, raw: &RawMap) -> String {
    if items.is_empty() {
        return "[]".into();
    }
    let inner: Vec<String> = items.iter().map(|s| format!("{ind}{unit}{}", raw.get(s).cloned().unwrap_or_else(|| json_str(s)))).collect();
    format!("[\n{}\n{ind}]", inner.join(",\n"))
}

/// New `permissions` object text. Keeps the original member order and the raw text of keys this app does not edit.
fn render_permissions(text: &str, original: Option<&Obj>, edit: &PermEdit, ind: &str, unit: &str) -> String {
    let inner = format!("{ind}{unit}");
    let mut parts: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut raw = RawMap::new();
    let mut original_lists: std::collections::HashMap<String, (Vec<String>, String)> = Default::default();
    for m in original.map(|o| o.members.as_slice()).unwrap_or_default() {
        if matches!(m.key.as_str(), "allow" | "ask" | "deny") {
            let mut decoded = Vec::new();
            for (a, b) in scan_array(text, m.val_start) {
                if let Ok(v) = serde_json::from_str::<String>(&text[a..b]) {
                    raw.entry(v.clone()).or_insert_with(|| text[a..b].to_string());
                    decoded.push(v);
                }
            }
            original_lists.insert(m.key.clone(), (decoded, text[m.val_start..m.val_end].to_string()));
        }
    }
    let list_text = |k: &str, items: &[String]| match original_lists.get(k) {
        // Unchanged list: keep the original array text byte for byte.
        Some((old, txt)) if old == items => txt.clone(),
        _ => render_list(items, &inner, unit, &raw),
    };
    let ours = |k: &str| -> Option<Option<String>> {
        match k {
            "allow" => Some(Some(list_text("allow", &edit.allow))),
            "ask" => Some(Some(list_text("ask", &edit.ask))),
            "deny" => Some(Some(list_text("deny", &edit.deny))),
            "defaultMode" => Some(edit.default_mode.as_deref().map(json_str)),
            _ => None,
        }
    };
    if let Some(o) = original {
        for m in &o.members {
            seen.insert(m.key.clone());
            let key_raw = &text[m.key_start..skip_string(text.as_bytes(), m.key_start)];
            match ours(&m.key) {
                Some(Some(v)) => parts.push(format!("{inner}{key_raw}: {v}")),
                Some(None) => {}
                None => parts.push(format!("{inner}{key_raw}: {}", &text[m.val_start..m.val_end])),
            }
        }
    }
    for (k, list) in [("allow", &edit.allow), ("ask", &edit.ask), ("deny", &edit.deny)] {
        if !seen.contains(k) && !list.is_empty() {
            parts.push(format!("{inner}\"{k}\": {}", render_list(list, &inner, unit, &raw)));
        }
    }
    if !seen.contains("defaultMode") {
        if let Some(m) = &edit.default_mode {
            parts.push(format!("{inner}\"defaultMode\": {}", json_str(m)));
        }
    }
    if parts.is_empty() {
        return "{}".into();
    }
    format!("{{\n{}\n{ind}}}", parts.join(",\n"))
}

/// Files with CRLF line ends keep them in the rewritten block.
fn crlf(original: &str, block: String) -> String {
    if original.contains("\r\n") {
        block.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        block
    }
}

/// Splices `permissions` into the original file text. `None` text = new file.
pub fn splice(text: Option<&str>, edit: &PermEdit) -> Result<String, PermError> {
    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => return Ok(format!("{{\n  \"permissions\": {}\n}}\n", render_permissions("", None, edit, "  ", "  "))),
    };
    let bad = || PermError::Invalid(crate::i18n::Fail::SettingsShape.str().into());
    let root = scan_object(text, 0).ok_or_else(bad)?;
    let unit = root.members.first().map(|m| indent_at(text, m.key_start)).filter(|s| !s.is_empty()).unwrap_or_else(|| "  ".into());
    let perms: Vec<&Member> = root.members.iter().filter(|m| m.key == "permissions").collect();
    if perms.len() > 1 {
        return Err(PermError::ReadOnly(crate::i18n::Fail::PermKeyDup.str().into()));
    }
    match perms.first() {
        Some(m) => {
            let orig = scan_object(text, m.val_start).ok_or_else(|| PermError::ReadOnly(crate::i18n::Fail::PermNotObject.str().into()))?;
            let ind = indent_at(text, m.key_start);
            let inner_unit = orig.members.first().map(|x| indent_at(text, x.key_start)).and_then(|s| s.strip_prefix(&ind).map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| unit.clone());
            let new = crlf(text, render_permissions(text, Some(&orig), edit, &ind, &inner_unit));
            Ok(format!("{}{}{}", &text[..m.val_start], new, &text[m.val_end..]))
        }
        None => {
            let new = render_permissions("", None, edit, &unit, &unit);
            let out = match root.members.last() {
                Some(last) => format!("{},\n{unit}\"permissions\": {new}", &text[..last.val_end]),
                None => format!("{}\n{unit}\"permissions\": {new}\n", &text[..=root.open]),
            };
            let tail = match root.members.last() {
                Some(last) => &text[last.val_end..],
                None => &text[root.close..],
            };
            let head_len = match root.members.last() {
                Some(last) => last.val_end,
                None => root.open + 1,
            };
            Ok(format!("{}{}{}", &text[..head_len], crlf(text, out[head_len..].to_string()), tail))
        }
    }
}

/// Sets one top-level member to `value_json` (already JSON text), keeping every other byte (memory-tab:
/// `autoMemoryEnabled`). Refuses a key that appears twice.
pub fn splice_top_level(text: &str, key: &str, value_json: &str) -> Result<String, PermError> {
    if text.trim().is_empty() {
        return Ok(format!("{{\n  {}: {value_json}\n}}\n", json_str(key)));
    }
    let root = scan_object(text, 0).ok_or_else(|| PermError::Invalid(crate::i18n::Fail::SettingsShape.str().into()))?;
    let hits: Vec<&Member> = root.members.iter().filter(|m| m.key == key).collect();
    match hits.as_slice() {
        [m] => Ok(format!("{}{value_json}{}", &text[..m.val_start], &text[m.val_end..])),
        [] => {
            let unit = root.members.first().map(|m| indent_at(text, m.key_start)).filter(|s| !s.is_empty()).unwrap_or_else(|| "  ".into());
            let (head, tail) = match root.members.last() {
                Some(last) => (last.val_end, crlf(text, format!(",\n{unit}{}: {value_json}", json_str(key)))),
                None => (root.open + 1, crlf(text, format!("\n{unit}{}: {value_json}\n", json_str(key)))),
            };
            Ok(format!("{}{tail}{}", &text[..head], &text[head..]))
        }
        _ => Err(PermError::ReadOnly(crate::tr_fmt!(crate::i18n::Fail::KeyDup, key = key))),
    }
}

fn check_new_rules(edit: &PermEdit, old: &ScopeView) -> Result<(), PermError> {
    let known: std::collections::BTreeSet<&String> = old.allow.iter().chain(&old.ask).chain(&old.deny).collect();
    for (name, list) in [("allow", &edit.allow), ("ask", &edit.ask), ("deny", &edit.deny)] {
        if list.len() > MAX_RULES_PER_LIST {
            return Err(PermError::Invalid(crate::tr_fmt!(crate::i18n::Fail::PermListMax, name = name, MAX_RULES_PER_LIST = MAX_RULES_PER_LIST)));
        }
        for r in list {
            // Existing rules are kept even when this parser does not understand them (security review).
            if known.contains(r) {
                continue;
            }
            if let Err(e) = parse_rule(r) {
                return Err(PermError::Invalid(crate::tr_fmt!(crate::i18n::Fail::PermRuleErr, name = name, a0 = display_safe(r, 80), e = e)));
            }
        }
    }
    Ok(())
}

/// (list, rule) pairs.
type Changes = Vec<(&'static str, String)>;

fn diff(old: &ScopeView, edit: &PermEdit) -> (Changes, Changes) {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for (name, o, n) in [("allow", &old.allow, &edit.allow), ("ask", &old.ask, &edit.ask), ("deny", &old.deny, &edit.deny)] {
        added.extend(n.iter().filter(|r| !o.contains(r)).map(|r| (name, r.clone())));
        removed.extend(o.iter().filter(|r| !n.contains(r)).map(|r| (name, r.clone())));
    }
    (added, removed)
}

/// Resolves the real target: the parent directory is canonicalized (a missing `.claude` folder is allowed, it is
/// created on commit), the file itself must not be a link or hard-linked.
fn real_target(path: &Path) -> Result<PathBuf, PermError> {
    let name = path.file_name().ok_or_else(|| PermError::Invalid(crate::i18n::Fail::BadPathValue.str().into()))?;
    let parent = path.parent().ok_or_else(|| PermError::Invalid(crate::i18n::Fail::BadPathValue.str().into()))?;
    let canon = match std::fs::symlink_metadata(parent) {
        Ok(_) => parent.canonicalize().map_err(|_| PermError::Io)?,
        Err(_) => {
            let gp = parent.parent().ok_or(PermError::Io)?.canonicalize().map_err(|_| PermError::ReadOnly(crate::i18n::Fail::FolderMissing.str().into()))?;
            gp.join(parent.file_name().ok_or(PermError::Io)?)
        }
    };
    Ok(canon.join(name))
}

/// Plans a save of `edit` into the settings file at `path`. `shared` = a file usually committed to git (keeps 0644
/// when created). `token` is what the UI read; a mismatch means the file changed.
pub fn plan_save(path: &Path, edit: PermEdit, token: &str, shared: bool) -> Result<PermPlan, PermError> {
    let path = real_target(path)?;
    let old = read_scope(&path);
    if let Some(r) = &old.readonly_reason {
        return Err(PermError::ReadOnly(r.clone()));
    }
    if old.token != token {
        return Err(PermError::Conflict);
    }
    check_new_rules(&edit, &old)?;
    if let Some(m) = &edit.default_mode {
        if old.default_mode.as_ref() != Some(m) && !MODES.contains(&m.as_str()) {
            return Err(PermError::Invalid(if m == "bypassPermissions" { crate::i18n::Fail::BypassNotHere.str().into() } else { crate::i18n::Fail::ModeUnknown.str().into() }));
        }
    }
    let (text, old_permissions, meta) = if old.exists {
        let bytes = std::fs::read(&path).map_err(|_| PermError::Io)?;
        if sha(&bytes) != token {
            return Err(PermError::Conflict);
        }
        let text = String::from_utf8(bytes).map_err(|_| PermError::Io)?;
        let perms = serde_json::from_str::<Value>(&text).ok().and_then(|mut v| v.get_mut("permissions").map(Value::take)).unwrap_or(Value::Null);
        let meta = std::fs::symlink_metadata(&path).map_err(|_| PermError::Io)?;
        (Some(text), perms, Some(meta))
    } else {
        (None, Value::Null, None)
    };
    let content = splice(text.as_deref(), &edit)?;
    if serde_json::from_str::<Value>(&content).is_err() {
        return Err(PermError::Invalid(crate::i18n::Fail::SaveContentJson.str().into()));
    }
    let (added, removed) = diff(&old, &edit);
    let high_allows = added
        .iter()
        .filter(|(l, _)| *l == "allow")
        .filter_map(|(_, r)| {
            if risk_of_rule(r) == "high" {
                return Some(r.clone());
            }
            let covered = covers_high_risk(r);
            (!covered.is_empty()).then(|| crate::tr_fmt!(crate::i18n::Fail::PermHighCount, r = r, a0 = covered.join(", ")))
        })
        .collect();
    let high_denies_removed = old.deny.iter().filter(|r| !edit.deny.contains(r) && risk_of_rule(r) == "high").cloned().collect();
    let mode_change = (old.default_mode != edit.default_mode).then(|| (old.default_mode.clone(), edit.default_mode.clone()));
    let risky_mode = mode_change.as_ref().and_then(|(_, n)| n.as_deref()).is_some_and(|m| RISKY_MODES.contains(&m));
    Ok(PermPlan {
        exists: old.exists,
        content,
        old_token: token.to_string(),
        added,
        removed,
        high_allows,
        high_denies_removed,
        mode_change,
        risky_mode,
        old_permissions,
        new_file_mode: if shared { 0o644 } else { 0o600 },
        #[cfg(unix)]
        ino: meta.as_ref().map_or(0, std::os::unix::fs::MetadataExt::ino),
        #[cfg(unix)]
        mode: meta.as_ref().map_or(0, std::os::unix::fs::MetadataExt::mode),
        path,
    })
}

fn check_unchanged(plan: &PermPlan) -> Result<(), PermError> {
    match std::fs::symlink_metadata(&plan.path) {
        Err(_) if !plan.exists => Ok(()),
        Err(_) => Err(PermError::Conflict),
        Ok(_) if !plan.exists => Err(PermError::Conflict),
        Ok(m) => {
            if !m.file_type().is_file() || nlink(&m) != 1 {
                return Err(PermError::Conflict);
            }
            #[cfg(unix)]
            if std::os::unix::fs::MetadataExt::ino(&m) != plan.ino {
                return Err(PermError::Conflict);
            }
            let bytes = std::fs::read(&plan.path).map_err(|_| PermError::Conflict)?;
            if sha(&bytes) != plan.old_token {
                return Err(PermError::Conflict);
            }
            Ok(())
        }
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Backup = the old `permissions` subtree + file hash, never the whole file (it may hold secrets). Keeps 10.
fn backup(plan: &PermPlan, dir: &Path, label: &str, now_ms: i64) -> Result<(), PermError> {
    std::fs::create_dir_all(dir).map_err(|_| PermError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let body = serde_json::json!({
        "path": plan.path.display().to_string(),
        "sha256": plan.old_token,
        "savedAtMs": now_ms,
        "permissions": plan.old_permissions,
    });
    let safe_label: String = label.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(24).collect();
    let text = serde_json::to_string_pretty(&body).map_err(|_| PermError::Io)?;
    // Unique name (two saves in one millisecond must not collide); create_new never overwrites.
    let mut written = false;
    for attempt in 0..5u32 {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0) ^ std::process::id().rotate_left(attempt * 7);
        let file = dir.join(format!("{now_ms:015}-{safe_label}-{nonce:08x}.json"));
        if write_private(&file, text.as_bytes()).is_ok() {
            written = true;
            break;
        }
    }
    if !written {
        return Err(PermError::Io);
    }
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|_| PermError::Io)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    all.sort();
    let excess = all.len().saturating_sub(MAX_BACKUPS);
    for old in all.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

/// Writes an approved plan: re-checks, backs up, writes a temp file with the old file's mode, re-checks the hash
/// right before the rename.
pub fn commit_save(plan: &PermPlan, backup_dir: &Path, label: &str, now_ms: i64) -> Result<(), PermError> {
    check_unchanged(plan)?;
    let dir = plan.path.parent().ok_or(PermError::Io)?;
    if std::fs::symlink_metadata(dir).is_err() {
        std::fs::create_dir(dir).map_err(|_| PermError::Io)?;
    }
    if !std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_dir()) {
        return Err(PermError::Conflict);
    }
    backup(plan, backup_dir, label, now_ms)?;
    let file_name = plan.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ u128::from(std::process::id());
    let tmp = dir.join(format!(".{}.dduck-{:x}.tmp", file_name.trim_start_matches('.'), nonce & 0xffff_ffff_ffff));
    let result = (|| -> Result<(), PermError> {
        write_private(&tmp, plan.content.as_bytes()).map_err(|_| PermError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if plan.exists { plan.mode & 0o777 } else { plan.new_file_mode };
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)).map_err(|_| PermError::Io)?;
        }
        check_unchanged(plan)?;
        std::fs::rename(&tmp, &plan.path).map_err(|_| PermError::Io)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

// ---------- explaining a call ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
    /// No rule matched: the permission mode decides.
    None,
    /// The agent does not have this tool at all.
    Unavailable,
}

pub struct RuleSet {
    pub label: String,
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    pub managed: bool,
    /// Root for `/path` patterns (the project for project files, home for user files).
    pub root: PathBuf,
}

pub struct CallContext {
    pub cwd: PathBuf,
    pub home: PathBuf,
    /// Agent `tools` allow-list (`None` = inherits everything) and `disallowedTools`.
    pub agent: Option<(String, Option<Vec<String>>, Vec<String>)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub decision: Decision,
    pub rule: Option<String>,
    pub scope: Option<String>,
    /// [`scope_code`] of `scope`, the webview's wording key (app-language).
    pub scope_code: Option<&'static str>,
    pub list: Option<String>,
    pub notes: Vec<String>,
}

/// A rule-set label (the shell's fixed Korean labels, or "에이전트 <name>") as a code: `managed`, `local`, `shared`,
/// `user`, `agent`; anything else is `other`.
pub fn scope_code(label: &str) -> &'static str {
    match label {
        "관리자" => "managed",
        "프로젝트(개인)" => "local",
        "프로젝트(공유)" => "shared",
        "전역" => "user",
        l if l.starts_with("에이전트 ") => "agent",
        _ => "other",
    }
}

const READ_TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];
const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "NotebookEdit", "MultiEdit"];
const READ_ONLY_CMDS: [&str; 12] = ["ls", "pwd", "cat", "head", "tail", "wc", "echo", "which", "whoami", "git status", "git diff", "git log"];

enum Tok {
    Lit(char),
    One,
    Star,
    DStar,
    DStarSlash,
}

/// Glob match in O(pattern × text). In `path` mode `*` and `?` stop at `/`, `**` crosses it and `**/` may be empty.
fn glob(pat: &str, s: &str, path: bool) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            '*' if path && p.get(i + 1) == Some(&'*') => {
                if p.get(i + 2) == Some(&'/') {
                    toks.push(Tok::DStarSlash);
                    i += 3;
                } else {
                    toks.push(Tok::DStar);
                    i += 2;
                }
                continue;
            }
            '*' => toks.push(if path { Tok::Star } else { Tok::DStar }),
            '?' if path => toks.push(Tok::One),
            c => toks.push(Tok::Lit(c)),
        }
        i += 1;
    }
    let s: Vec<char> = s.chars().collect();
    let m = s.len();
    let mut next = vec![false; m + 1];
    next[m] = true;
    for t in toks.iter().rev() {
        let mut cur = vec![false; m + 1];
        let mut slash_ahead = false; // some k >= j with s[k] == '/' && next[k + 1]
        for j in (0..=m).rev() {
            if j < m && s[j] == '/' && next[j + 1] {
                slash_ahead = true;
            }
            cur[j] = match t {
                Tok::Lit(c) => j < m && s[j] == *c && next[j + 1],
                Tok::One => j < m && s[j] != '/' && next[j + 1],
                Tok::Star => next[j] || (j < m && s[j] != '/' && cur[j + 1]),
                Tok::DStar => next[j] || (j < m && cur[j + 1]),
                Tok::DStarSlash => next[j] || slash_ahead,
            };
        }
        next = cur;
    }
    next[0]
}

fn normalize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn resolve_call_path(arg: &str, ctx: &CallContext) -> PathBuf {
    let p = if let Some(rest) = arg.strip_prefix("~/") {
        ctx.home.join(rest)
    } else if arg.starts_with('/') {
        PathBuf::from(arg)
    } else {
        ctx.cwd.join(arg)
    };
    normalize_path(&p)
}

fn path_matches(spec: &str, arg: &str, root: &Path, ctx: &CallContext) -> bool {
    let target = resolve_call_path(arg, ctx);
    let target_s = target.to_string_lossy();
    let pattern = if let Some(rest) = spec.strip_prefix("//") {
        format!("/{rest}")
    } else if let Some(rest) = spec.strip_prefix("~/") {
        ctx.home.join(rest).to_string_lossy().to_string()
    } else if let Some(rest) = spec.strip_prefix('/') {
        root.join(rest).to_string_lossy().to_string()
    } else if let Some(rest) = spec.strip_prefix("./") {
        ctx.cwd.join(rest).to_string_lossy().to_string()
    } else if !spec.contains('/') {
        // gitignore: a bare name matches at any depth.
        let name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return glob(spec, &name, true);
    } else {
        ctx.cwd.join(spec).to_string_lossy().to_string()
    };
    let pattern = normalize_path(Path::new(&pattern)).to_string_lossy().to_string();
    glob(&pattern, &target_s, true) || (spec.ends_with('/') && target_s.starts_with(&pattern))
}

fn bash_matches(spec: &str, cmd: &str) -> bool {
    let prefix = spec.strip_suffix(":*").or_else(|| spec.strip_suffix(" *"));
    if let Some(prefix) = prefix {
        if !prefix.contains('*') {
            return cmd == prefix || cmd.starts_with(&format!("{prefix} "));
        }
    }
    if spec.contains('*') {
        return glob(spec, cmd, false);
    }
    cmd == spec
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', ':', '?', '#']).next().unwrap_or("").to_lowercase()
}

fn tool_matches(rule_tool: &str, call_tool: &str, list: &str) -> bool {
    let alias = |t: &str| if t == "Task" { "Agent".to_string() } else { t.to_string() };
    let (rt, ct) = (alias(rule_tool), alias(call_tool));
    if rt == "*" {
        return true;
    }
    if rt.contains('*') {
        return glob(&rt, &ct, false);
    }
    if rt.starts_with("mcp__") {
        return ct == rt || ct.starts_with(&format!("{rt}__"));
    }
    if rt == ct {
        return true;
    }
    // Edit rules cover every editing tool; Read rules cover the read-only file tools; a Read deny also blocks edits.
    (rt == "Edit" && EDIT_TOOLS.contains(&ct.as_str()))
        || (rt == "Read" && READ_TOOLS.contains(&ct.as_str()))
        || (rt == "Read" && list == "deny" && EDIT_TOOLS.contains(&ct.as_str()))
}

fn rule_matches(rule: &Rule, list: &str, tool: &str, arg: Option<&str>, root: &Path, ctx: &CallContext) -> bool {
    if !tool_matches(&rule.tool, tool, list) {
        return false;
    }
    let Some(spec) = rule.spec.as_deref() else { return true };
    let Some(arg) = arg else { return false };
    match rule.tool.as_str() {
        "Bash" => bash_matches(spec, arg.trim()),
        "Read" | "Edit" | "Write" | "Glob" | "Grep" | "NotebookEdit" | "MultiEdit" => path_matches(spec, arg, root, ctx),
        "WebFetch" => match spec.strip_prefix("domain:") {
            Some(d) => {
                let host = host_of(arg.strip_prefix("domain:").unwrap_or(arg));
                let d = d.to_lowercase();
                match d.strip_prefix("*.") {
                    Some(base) => host.ends_with(&format!(".{base}")),
                    None => host == d,
                }
            }
            None => glob(spec, arg, false),
        },
        _ => glob(spec, arg, false),
    }
}

/// Splits `a && b; c | d` into simple commands (deny/ask apply when any part matches; allow needs every part).
fn split_compound(cmd: &str) -> Vec<String> {
    let mut parts = vec![cmd.to_string()];
    for sep in ["&&", "||", ";", "|", "\n"] {
        parts = parts.iter().flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>()).collect();
    }
    parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

struct Hit {
    decision: Decision,
    rule: String,
    scope: String,
    list: &'static str,
}

fn decide_one(sets: &[&RuleSet], tool: &str, arg: Option<&str>, ctx: &CallContext) -> Option<Hit> {
    for (list, decision) in [("deny", Decision::Deny), ("ask", Decision::Ask), ("allow", Decision::Allow)] {
        for set in sets {
            let rules = match list {
                "deny" => &set.deny,
                "ask" => &set.ask,
                _ => &set.allow,
            };
            let parsed: Vec<(String, Rule)> = rules.iter().filter_map(|r| parse_rule(r).ok().map(|p| (r.clone(), p))).collect();
            // `Tool(!pattern)` takes matching paths out of the same file's list.
            let negated = parsed.iter().any(|(_, p)| {
                p.spec.as_deref().and_then(|s| s.strip_prefix('!')).is_some_and(|neg| {
                    let r = Rule { tool: p.tool.clone(), spec: Some(neg.to_string()) };
                    rule_matches(&r, list, tool, arg, &set.root, ctx)
                })
            });
            if negated {
                continue;
            }
            for (raw, p) in &parsed {
                if p.spec.as_deref().is_some_and(|s| s.starts_with('!')) {
                    continue;
                }
                if rule_matches(p, list, tool, arg, &set.root, ctx) {
                    return Some(Hit { decision, rule: raw.clone(), scope: set.label.clone(), list });
                }
            }
        }
    }
    None
}

fn parse_call(call: &str) -> Option<(String, Option<String>)> {
    let call = call.trim();
    match call.find('(') {
        Some(i) if call.ends_with(')') => Some((call[..i].trim().to_string(), Some(call[i + 1..call.len() - 1].to_string()))),
        Some(_) => None,
        None if !call.is_empty() => Some((call.to_string(), None)),
        None => None,
    }
}

pub fn explain(sets: &[RuleSet], managed_only: bool, call: &str, ctx: &CallContext) -> Explanation {
    let mut notes = vec![crate::i18n::Fail::NoteReference.str().to_string()];
    let out = |decision, rule: Option<String>, scope: Option<String>, list: Option<&str>, notes: Vec<String>| Explanation { decision, rule, scope_code: scope.as_deref().map(scope_code), scope, list: list.map(str::to_string), notes };
    let Some((tool, arg)) = parse_call(&display_safe(call, 2000)) else {
        notes.insert(0, crate::i18n::Fail::NoteFormat.str().into());
        return out(Decision::None, None, None, None, notes);
    };
    if let Some((name, tools, disallowed)) = &ctx.agent {
        let base = |t: &str| t.split('(').next().unwrap_or(t).trim().to_string();
        if disallowed.iter().any(|d| base(d) == tool && (d == &tool || parse_rule(d).is_ok_and(|r| rule_matches(&r, "deny", &tool, arg.as_deref(), &ctx.cwd, ctx)))) {
            return out(Decision::Deny, disallowed.iter().find(|d| base(d) == tool).cloned(), Some(format!("에이전트 {name}")), Some("disallowedTools"), notes);
        }
        if let Some(list) = tools {
            if !list.is_empty() && !list.iter().any(|t| base(t) == tool || (t.starts_with("mcp__") && tool_matches(t, &tool, "allow"))) {
                notes.insert(0, crate::tr_fmt!(crate::i18n::Fail::NoteAgentTools, name = name, tool = tool));
                return out(Decision::Unavailable, None, Some(format!("에이전트 {name}")), Some("tools"), notes);
            }
        }
    }
    let active: Vec<&RuleSet> = sets.iter().filter(|s| !managed_only || s.managed).collect();
    if managed_only {
        notes.push(crate::i18n::Fail::NoteManagedOnly.str().into());
    }
    let subs: Vec<Option<String>> = if tool == "Bash" {
        match arg.as_deref().map(split_compound) {
            Some(v) if !v.is_empty() => v.into_iter().map(Some).collect(),
            _ => vec![arg.clone()],
        }
    } else {
        vec![arg.clone()]
    };
    let hits: Vec<Option<Hit>> = subs.iter().map(|a| decide_one(&active, &tool, a.as_deref(), ctx)).collect();
    if subs.len() > 1 {
        notes.push(crate::tr_fmt!(crate::i18n::Fail::NoteCompound, a0 = subs.len()));
    }
    for d in [Decision::Deny, Decision::Ask] {
        if let Some(h) = hits.iter().flatten().find(|h| h.decision == d) {
            if d == Decision::Deny && tool == "Bash" {
                notes.push(crate::i18n::Fail::NoteBashBypass.str().into());
            }
            return out(d, Some(display_safe(&h.rule, 300)), Some(h.scope.clone()), Some(h.list), notes);
        }
    }
    // Redirection, substitution or chaining makes a "read-only" command able to write or run anything.
    // `--output` (git diff --output=FILE) writes a file; `\r` can hide the rest of a command line.
    let plain = |c: &str| !c.contains(['$', '`', '>', '<', '&', '|', ';', '\n', '\r', '(', ')']) && !c.contains("--output");
    let read_only = |a: &Option<String>| tool == "Bash" && a.as_deref().is_some_and(|c| plain(c) && READ_ONLY_CMDS.iter().any(|p| c == *p || c.starts_with(&format!("{p} "))));
    if hits.iter().zip(&subs).all(|(h, a)| h.is_some() || read_only(a)) {
        if let Some(h) = hits.iter().flatten().next() {
            return out(Decision::Allow, Some(display_safe(&h.rule, 300)), Some(h.scope.clone()), Some(h.list), notes);
        }
        notes.insert(0, crate::i18n::Fail::NoteReadOnly.str().into());
        return out(Decision::Allow, None, None, None, notes);
    }
    notes.insert(0, crate::i18n::Fail::NoteNoRule.str().into());
    out(Decision::None, None, None, None, notes)
}

// ---------- catalog rows: effective result ----------

/// A representative call for a catalog rule, e.g. `Bash(git push:*)` → `Bash(git push)`, `Read(**/.env)` → `Read(.env)`.
pub fn sample_call(rule: &str) -> Option<String> {
    let r = parse_rule(rule).ok()?;
    let tool = if r.tool.contains('*') { r.tool.replace("mcp__*", "mcp__x__y").replace('*', "x") } else { r.tool.clone() };
    let Some(spec) = r.spec else { return Some(tool) };
    let arg = match r.tool.as_str() {
        "Bash" => spec.strip_suffix(":*").or_else(|| spec.strip_suffix(" *")).unwrap_or(&spec).replace('*', "x"),
        "WebFetch" => match spec.strip_prefix("domain:") {
            Some(d) => format!("https://{}/", d.trim_start_matches("*.")),
            None => spec.replace('*', "x"),
        },
        "Read" | "Edit" | "Write" | "Glob" | "Grep" | "NotebookEdit" => {
            let s = spec.trim_start_matches('!');
            let s = s.strip_prefix("**/").unwrap_or(s);
            let s = s.strip_prefix("//").map(|x| format!("/{x}")).unwrap_or_else(|| s.to_string());
            s.replace("**", "x").replace('*', "x")
        }
        _ => spec.replace('*', "x"),
    };
    Some(format!("{tool}({arg})"))
}

/// High-risk catalog rows an allow rule would let through (warned when adding a broad rule like `Bash(git:*)`).
pub fn covers_high_risk(rule: &str) -> Vec<String> {
    if parse_rule(rule).is_err() {
        return Vec::new();
    }
    let set = [RuleSet { label: String::new(), allow: vec![rule.to_string()], ask: Vec::new(), deny: Vec::new(), managed: false, root: PathBuf::from("/p") }];
    let ctx = CallContext { cwd: PathBuf::from("/p"), home: PathBuf::from("/h"), agent: None };
    CATALOG
        .items
        .iter()
        .filter(|i| i.risk == "high" && i.rule != rule)
        .filter(|i| sample_call(&i.rule).is_some_and(|c| explain(&set, false, &c, &ctx).rule.as_deref() == Some(rule)))
        .map(|i| i.rule.clone())
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowEffect {
    pub rule: String,
    pub decision: Decision,
    pub by_rule: Option<String>,
    pub by_scope: Option<String>,
    pub by_scope_code: Option<&'static str>,
}

/// Effective decision for every catalog row with all scopes combined (the draft replaces its own scope's rules).
pub fn catalog_effective(sets: &[RuleSet], managed_only: bool, ctx: &CallContext) -> Vec<RowEffect> {
    CATALOG
        .items
        .iter()
        .filter_map(|i| {
            let call = sample_call(&i.rule)?;
            let e = explain(sets, managed_only, &call, ctx);
            Some(RowEffect { rule: i.rule.clone(), decision: e.decision, by_rule: e.rule, by_scope_code: e.scope_code, by_scope: e.scope })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const SETTINGS: &str = "{\n  \"env\": {\n    \"ANTHROPIC_API_KEY\": \"sk-secret\"\n  },\n  \"cleanupPeriodDays\": 100000000000000000000001,\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(git:*)\",\n      \"weird rule((\"\n    ],\n    \"additionalDirectories\": [\"../x\"],\n    \"deny\": []\n  },\n  \"hooks\": {\"PreToolUse\": []}\n}\n";

    fn edit(allow: &[&str], ask: &[&str], deny: &[&str]) -> PermEdit {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect();
        PermEdit { allow: v(allow), ask: v(ask), deny: v(deny), default_mode: None }
    }

    #[test]
    fn reads_only_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("settings.json");
        std::fs::write(&f, SETTINGS).unwrap();
        let v = read_scope(&f);
        assert_eq!(v.allow, ["Bash(git:*)", "weird rule(("]);
        assert_eq!(v.other.len(), 1);
        assert_eq!(v.other[0].0, "additionalDirectories");
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("sk-secret") && !json.contains("hooks") && !json.contains("cleanupPeriodDays"), "{json}");

        std::fs::write(&f, "{\"env\": {\"K\": \"sk-secret\"}, oops}").unwrap();
        let v = read_scope(&f);
        assert!(v.readonly_reason.is_some() && !serde_json::to_string(&v).unwrap().contains("sk-secret"));
        assert!(!read_scope(&tmp.path().join("missing.json")).exists);
    }

    #[test]
    fn rule_parser() {
        for ok in ["Read", "Bash(git push:*)", "mcp__notion", "mcp__notion__search", "mcp__x__get_*", "mcp__*", "*", "Read(!./secret/**)", "Agent(Explore)", "Skill(opsx:propose)", "WebFetch(domain:example.com)"] {
            assert!(parse_rule(ok).is_ok(), "{ok}");
        }
        for bad in ["", " Read", "Read(", "Read()", "(x)", "Ba sh", "Read\u{202e}", "Bash*", "1Read"] {
            assert!(parse_rule(bad).is_err(), "{bad:?}");
        }
        assert!(!rule_warnings("mcp__x(y)", "deny").is_empty());
        assert!(!rule_warnings("mcp__*", "allow").is_empty());
        assert!(rule_warnings("mcp__*", "deny").is_empty());
        assert!(rule_warnings("mcp__x__get_*", "allow").is_empty());
        assert!(!rule_warnings("Bash(git:* push)", "deny").is_empty());
    }

    #[test]
    fn splice_preserves_other_bytes() {
        let out = splice(Some(SETTINGS), &edit(&["Bash(git:*)", "weird rule(("], &[], &["Bash(rm -rf:*)"])).unwrap();
        assert!(out.contains("100000000000000000000001"), "big number kept verbatim");
        assert!(out.starts_with("{\n  \"env\": {\n    \"ANTHROPIC_API_KEY\": \"sk-secret\"\n  },"));
        assert!(out.ends_with("  \"hooks\": {\"PreToolUse\": []}\n}\n"));
        assert!(out.contains("\"additionalDirectories\": [\"../x\"]"));
        assert!(out.contains("    \"deny\": [\n      \"Bash(rm -rf:*)\"\n    ]"));
        assert!(!out.contains("\"ask\""), "absent empty list not added");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["permissions"]["allow"][1], "weird rule((");

        let fresh = splice(None, &PermEdit { default_mode: Some("plan".into()), ..edit(&["Read"], &[], &[]) }).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&fresh).unwrap()["permissions"]["defaultMode"], "plan");
        let added = splice(Some("{\n  \"model\": \"opus\"\n}\n"), &edit(&[], &["WebFetch"], &[])).unwrap();
        assert_eq!(added, "{\n  \"model\": \"opus\",\n  \"permissions\": {\n    \"ask\": [\n      \"WebFetch\"\n    ]\n  }\n}\n");
        assert!(serde_json::from_str::<Value>(&splice(Some("{}"), &edit(&["Read"], &[], &[])).unwrap()).is_ok());
        assert!(matches!(splice(Some("{\"permissions\": {}, \"permissions\": {}}"), &edit(&[], &[], &[])), Err(PermError::ReadOnly(_))));
    }

    #[test]
    fn guarded_save() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("proj/.claude");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("settings.json");
        let backups = tmp.path().join("backups");
        std::fs::write(&f, SETTINGS).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let token = read_scope(&f).token;
        let e = edit(&["Bash(git:*)", "weird rule((", "Bash(git push:*)"], &[], &["Read(./.env)"]);

        assert_eq!(plan_save(&f, e.clone(), "stale", true).unwrap_err(), PermError::Conflict);
        assert!(matches!(plan_save(&f, edit(&["bad rule("], &[], &[]), &token, true), Err(PermError::Invalid(_))), "new rules are validated");
        let bypass = PermEdit { default_mode: Some("bypassPermissions".into()), ..e.clone() };
        assert!(matches!(plan_save(&f, bypass, &token, true), Err(PermError::Invalid(_))));

        let plan = plan_save(&f, e.clone(), &token, true).unwrap();
        assert_eq!(plan.high_allows, ["Bash(git push:*)"]);
        assert_eq!(plan.added.len(), 2);
        std::fs::write(&f, SETTINGS.replace("100000", "200000")).unwrap();
        assert_eq!(commit_save(&plan, &backups, "project", 1).unwrap_err(), PermError::Conflict, "changed while the dialog was open");

        std::fs::write(&f, SETTINGS).unwrap();
        let plan = plan_save(&f, e, &token, true).unwrap();
        commit_save(&plan, &backups, "project", 2).unwrap();
        let saved = std::fs::read_to_string(&f).unwrap();
        assert!(saved.contains("sk-secret") && saved.contains("\"hooks\"") && saved.contains("Read(./.env)"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o644, "existing mode kept");
        }
        let b: Vec<_> = std::fs::read_dir(&backups).unwrap().map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap()).collect();
        assert_eq!(b.len(), 1);
        assert!(!b[0].contains("sk-secret") && b[0].contains("Bash(git:*)") && b[0].contains("sha256"), "backup holds permissions only");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temp files left");

        // Backups are capped.
        for i in 0..12 {
            let t = read_scope(&f).token;
            let p = plan_save(&f, edit(&["Read"], &[], &[if i % 2 == 0 { "Bash(sudo:*)" } else { "Bash(su:*)" }]), &t, true).unwrap();
            commit_save(&p, &backups, "project", 10 + i).unwrap();
        }
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), MAX_BACKUPS);

        // New file in a project without `.claude`.
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        let local = other.join(".claude/settings.local.json");
        let p = plan_save(&local, edit(&["Read"], &[], &[]), "", false).unwrap();
        commit_save(&p, &backups, "local", 99).unwrap();
        assert_eq!(read_scope(&local).allow, ["Read"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&local).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.json");
        std::fs::write(&real, "{}").unwrap();
        let link = tmp.path().join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let v = read_scope(&link);
        assert!(v.readonly_reason.is_some());
        assert!(matches!(plan_save(&link, edit(&["Read"], &[], &[]), &v.token, false), Err(PermError::ReadOnly(_))));
        let hard = tmp.path().join("hard.json");
        std::fs::hard_link(&real, &hard).unwrap();
        let v = read_scope(&hard);
        assert!(matches!(plan_save(&hard, edit(&["Read"], &[], &[]), &v.token, false), Err(PermError::ReadOnly(_))));
    }

    #[test]
    fn managed_sources() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("managed-settings.d")).unwrap();
        std::fs::write(tmp.path().join("managed-settings.json"), "{\"apiKeyHelper\": \"x\", \"allowManagedPermissionRulesOnly\": true, \"permissions\": {\"deny\": [\"WebFetch\"]}}").unwrap();
        std::fs::write(tmp.path().join("managed-settings.d/10-a.json"), "{\"permissions\": {\"ask\": [\"Bash\"]}}").unwrap();
        let m = managed(&[tmp.path().to_path_buf()], &[tmp.path().join("nope.plist")]);
        assert_eq!(m.sources.len(), 2);
        assert!(m.managed_only && m.mdm.is_none());
        assert!(m.sources.iter().all(|s| s.readonly_reason.is_some()));
        assert!(!serde_json::to_string(&m).unwrap().contains("apiKeyHelper"));
    }

    fn set(label: &str, allow: &[&str], ask: &[&str], deny: &[&str]) -> RuleSet {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect();
        RuleSet { label: label.into(), allow: v(allow), ask: v(ask), deny: v(deny), managed: label == "관리자", root: PathBuf::from("/p") }
    }

    fn ctx() -> CallContext {
        CallContext { cwd: PathBuf::from("/p"), home: PathBuf::from("/h"), agent: None }
    }

    #[test]
    fn explain_deny_wins_across_scopes() {
        let sets = [set("전역", &["Bash(git:*)"], &[], &[]), set("프로젝트(공유)", &[], &[], &["Bash(git push:*)"])];
        let e = explain(&sets, false, "Bash(git push origin main)", &ctx());
        assert_eq!((e.decision, e.rule.as_deref(), e.scope.as_deref()), (Decision::Deny, Some("Bash(git push:*)"), Some("프로젝트(공유)")));
        assert!(e.notes.iter().any(|n| n.contains("우회")));
        assert_eq!(explain(&sets, false, "Bash(git status)", &ctx()).decision, Decision::Allow);
        assert_eq!(explain(&sets, false, "Bash(git status && git push)", &ctx()).decision, Decision::Deny, "compound: any part denied");
        assert_eq!(explain(&sets, false, "Bash(git status && npm i)", &ctx()).decision, Decision::None, "compound: allow needs every part");
        assert_eq!(explain(&sets, false, "Bash(gitx)", &ctx()).decision, Decision::None, "prefix stops at word boundary");
        assert_eq!(explain(&sets, false, "Bash(ls -la)", &ctx()).decision, Decision::Allow, "read-only builtin");
        assert_eq!(explain(&[set("전역", &["Bash(npm run *)"], &[], &[])], false, "Bash(npm run build)", &ctx()).decision, Decision::Allow, "` *` same as `:*`");

        let managed = [set("관리자", &[], &[], &[]), set("전역", &[], &[], &["Read"])];
        assert_eq!(explain(&managed, true, "Read(/p/a)", &ctx()).decision, Decision::None, "managed-only ignores other scopes");
    }

    #[test]
    fn explain_paths_and_tools() {
        let sets = [set("전역", &["Edit(./src/**)", "mcp__notion", "WebFetch(domain:docs.rs)"], &["mcp__x__get_*"], &["Read(./.env)", "Read(~/.ssh/**)", "Read(*.pem)", "Read(./secret/**)", "Read(!./secret/ok.txt)"])];
        let c = ctx();
        assert_eq!(explain(&sets, false, "Read(.env)", &c).decision, Decision::Deny);
        assert_eq!(explain(&sets, false, "Read(/h/.ssh/id_rsa)", &c).decision, Decision::Deny);
        assert_eq!(explain(&sets, false, "Read(~/.ssh/id_rsa)", &c).decision, Decision::Deny);
        assert_eq!(explain(&sets, false, "Read(deep/dir/key.pem)", &c).decision, Decision::Deny, "bare name matches at any depth");
        assert_eq!(explain(&sets, false, "Edit(.env)", &c).decision, Decision::Deny, "Read deny blocks edits");
        assert_eq!(explain(&sets, false, "Read(secret/a.txt)", &c).decision, Decision::Deny);
        assert_eq!(explain(&sets, false, "Read(secret/ok.txt)", &c).decision, Decision::None, "negation removes the path");
        assert_eq!(explain(&sets, false, "Write(src/a/b.rs)", &c).decision, Decision::Allow, "Edit rule covers Write");
        assert_eq!(explain(&sets, false, "Edit(other/b.rs)", &c).decision, Decision::None);
        assert_eq!(explain(&sets, false, "mcp__notion__search", &c).decision, Decision::Allow, "server rule");
        assert_eq!(explain(&sets, false, "mcp__x__get_page", &c).decision, Decision::Ask);
        assert_eq!(explain(&sets, false, "WebFetch(https://docs.rs/serde)", &c).decision, Decision::Allow);
        assert_eq!(explain(&sets, false, "WebFetch(https://evil.docs.rs.example)", &c).decision, Decision::None);

        let mut agent = ctx();
        agent.agent = Some(("reviewer".into(), Some(vec!["Read".into(), "Grep".into()]), vec!["Edit".into()]));
        assert_eq!(explain(&sets, false, "Bash(ls)", &agent).decision, Decision::Unavailable);
        assert_eq!(explain(&sets, false, "Edit(src/a.rs)", &agent).decision, Decision::Deny);
        assert_eq!(explain(&sets, false, "Read(src/a.rs)", &agent).decision, Decision::None);
    }

    #[test]
    fn glob_rules() {
        assert!(glob("/a/**", "/a/b/c", true));
        assert!(glob("/a/**/c", "/a/c", true));
        assert!(!glob("/a/*", "/a/b/c", true));
        assert!(glob("npm run *", "npm run build", false));
        assert!(!glob(&"*a".repeat(200), &"b".repeat(2000), false), "no blow-up");
    }

    #[test]
    fn unchanged_rules_keep_exact_text() {
        let long = format!("Bash(echo {})", "a".repeat(700));
        let text = format!("{{\r\n  \"permissions\": {{\r\n    \"allow\": [\"Bash(cat <<EOF\\nhi\\nEOF)\", \"Re\\u200bad\", \"{long}\"],\r\n    \"deny\": [\"Bash(sudo:*)\"]\r\n  }}\r\n}}\r\n");
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("settings.json");
        std::fs::write(&f, &text).unwrap();
        let v = read_scope(&f);
        assert_eq!(v.allow[0], "Bash(cat <<EOF\nhi\nEOF)");
        assert_eq!(v.allow[1], "Re\u{200b}ad");
        assert_eq!(v.allow[2], long, "not truncated");
        // Only deny changes: allow array kept byte for byte, escapes included; CRLF kept.
        let e = PermEdit { allow: v.allow.clone(), ask: Vec::new(), deny: vec!["Bash(sudo:*)".into(), "Read(**/.env)".into()], default_mode: None };
        let out = splice(Some(&text), &e).unwrap();
        assert!(out.contains("[\"Bash(cat <<EOF\\nhi\\nEOF)\", \"Re\\u200bad\""), "{out}");
        assert!(!out.replace("\r\n", "").contains('\n'), "CRLF kept");
        // Reordered list: elements still use their original raw text.
        let e2 = PermEdit { allow: vec![v.allow[1].clone(), v.allow[0].clone()], ..e };
        assert!(splice(Some(&text), &e2).unwrap().contains("\"Re\\u200bad\""));
        let p = plan_save(&f, PermEdit { allow: v.allow.clone(), ask: vec![], deny: vec!["Bash(sudo:*)".into(), "Read(**/.env)".into()], default_mode: None }, &v.token, false).unwrap();
        commit_save(&p, &tmp.path().join("b"), "user", 1).unwrap();
        assert_eq!(read_scope(&f).allow, v.allow);
    }

    #[test]
    fn rows_show_effective_results() {
        assert_eq!(sample_call("Bash(git push:*)").as_deref(), Some("Bash(git push)"));
        assert_eq!(sample_call("Read(**/.env)").as_deref(), Some("Read(.env)"));
        assert_eq!(sample_call("Read(~/.ssh/**)").as_deref(), Some("Read(~/.ssh/x)"));
        assert_eq!(sample_call("WebFetch(domain:github.com)").as_deref(), Some("WebFetch(https://github.com/)"));
        let sets = [set("전역", &["Bash(git:*)"], &[], &[])];
        let rows = catalog_effective(&sets, false, &ctx());
        let push = rows.iter().find(|r| r.rule == "Bash(git push:*)").unwrap();
        assert_eq!((push.decision, push.by_rule.as_deref(), push.by_scope.as_deref()), (Decision::Allow, Some("Bash(git:*)"), Some("전역")));
        assert_eq!(push.by_scope_code, Some("user"));
        assert_eq!([scope_code("관리자"), scope_code("프로젝트(개인)"), scope_code("프로젝트(공유)"), scope_code("에이전트 x"), scope_code("?")], ["managed", "local", "shared", "agent", "other"]);
        assert!(covers_high_risk("Bash(git:*)").contains(&"Bash(git push:*)".to_string()));
        assert!(covers_high_risk("Bash(git status:*)").is_empty());
        // Every catalog rule resolves to itself when it is the only rule.
        for i in &catalog().items {
            let only = [set("전역", &[], &[], &[i.rule.as_str()])];
            let call = sample_call(&i.rule).unwrap();
            assert_eq!(explain(&only, false, &call, &ctx()).decision, Decision::Deny, "{} via {call}", i.rule);
        }
    }

    #[test]
    fn read_only_shortcut_needs_a_plain_command() {
        for c in ["cat x > y", "echo $(rm -rf /)", "ls `x`", "cat <(x)", "echo a |& tee b"] {
            assert_eq!(explain(&[], false, &format!("Bash({c})"), &ctx()).decision, Decision::None, "{c}");
        }
        assert_eq!(explain(&[], false, "Bash(ls -la)", &ctx()).decision, Decision::Allow);
    }

    #[test]
    fn catalog_loads_and_rates() {
        let c = catalog();
        assert!(c.items.len() > 30);
        assert!(c.items.iter().all(|i| c.groups.iter().any(|g| g.id == i.group) && parse_rule(&i.rule).is_ok()));
        assert!(c.items.iter().all(|i| ["low", "medium", "high"].contains(&i.risk.as_str()) && ["allow", "ask", "deny"].contains(&i.recommended.as_str())));
        assert_eq!(risk_of_rule("Bash(git push:*)"), "high");
        assert_eq!(risk_of_rule("Bash(rm -r x)"), "high");
        assert_eq!(risk_of_rule("Grep"), "low");
    }

    #[test]
    fn catalog_has_the_same_rules_in_english() {
        use crate::i18n::Lang;
        let (ko, en) = (catalog_in(Lang::Ko), catalog_in(Lang::En));
        assert_eq!(ko.items.iter().map(|i| (&i.rule, &i.risk)).collect::<Vec<_>>(), en.items.iter().map(|i| (&i.rule, &i.risk)).collect::<Vec<_>>());
        let hangul = |s: &str| s.chars().any(|c| ('가'..='힣').contains(&c));
        assert!(en.groups.iter().all(|g| !hangul(&g.label)) && en.items.iter().all(|i| !hangul(&i.description) && !hangul(&i.reason)));
        assert_eq!(ko.groups[0].label, "읽기·탐색");
    }
}
