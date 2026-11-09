//! Local agent definitions: discovery, a small frontmatter reader, risk detection and the guarded save (D-054, D-057).
//!
//! The webview only ever sees ids. Every write re-discovers, re-checks the file and is confirmed natively by the shell.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::text::display_safe;

pub const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_PROJECT_DIRS: usize = 42;
const MAX_PLUGIN_ENTRIES: usize = 5000;
const PLUGIN_BUDGET: Duration = Duration::from_millis(500);

static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9][A-Za-z0-9_.:-]{0,63}\z").unwrap_or_else(|_| unreachable!()));
static KEY_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A([A-Za-z_][A-Za-z0-9_-]*):(?:[ \t]+(.*))?\z").unwrap_or_else(|_| unreachable!()));

/// Agent names usable as `--agent=<name>`: start alphanumeric, so nothing parses as an option.
pub fn valid_name(n: &str) -> bool {
    NAME_RE.is_match(n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Project,
    User,
    Plugin,
    Builtin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Risk {
    pub hooks: bool,
    pub mcp_servers: bool,
    pub danger_mode: bool,
    /// Frontmatter that this reader cannot vouch for (broken, nested, anchors, duplicate keys).
    pub unreadable: bool,
}

impl Risk {
    pub fn any(&self) -> bool {
        self.hooks || self.mcp_servers || self.danger_mode || self.unreadable
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    pub source: Source,
    /// Project folder for project agents.
    pub scope: Option<String>,
    pub path: Option<String>,
    pub description: Option<String>,
    pub model: Option<String>,
    pub tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    /// Frontmatter `skills` preloaded at start (agent-loadout).
    pub skills: Vec<String>,
    pub malformed: bool,
    pub editable: bool,
    pub readonly_reason: Option<&'static str>,
    /// First definition of this name in precedence order (project > user > plugin > builtin).
    pub primary: bool,
    /// Usable as a persona in the folder the catalog was asked for (canonical-path check, D-057).
    pub usable: bool,
    pub risk: Risk,
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

const BUILTINS: [(&str, crate::i18n::Fail); 5] = [
    ("general-purpose", crate::i18n::Fail::BuiltinGeneral),
    ("Explore", crate::i18n::Fail::BuiltinExplore),
    ("Plan", crate::i18n::Fail::BuiltinPlan),
    ("claude-code-guide", crate::i18n::Fail::BuiltinGuide),
    ("statusline-setup", crate::i18n::Fail::BuiltinStatusline),
];

// ---------- frontmatter ----------

#[derive(Debug, Default, PartialEq)]
pub struct Frontmatter {
    /// Top-level `key: value` pairs in order (value is the raw text after the colon).
    pub fields: Vec<(String, String)>,
    /// Anything beyond flat `key: value` lines: indented lines, lists, flow maps, anchors, block scalars,
    /// quoted keys, duplicate keys.
    pub complex: bool,
    pub lines: Vec<String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| unquote(v))
    }
}

fn unquote(v: &str) -> &str {
    let v = v.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return &v[1..v.len() - 1];
        }
    }
    v
}

fn normalize(content: &str) -> String {
    content.strip_prefix('\u{feff}').unwrap_or(content).replace("\r\n", "\n").replace('\r', "\n")
}

/// Splits `---\n…\n---` off the top. `None` when there is no well-formed block.
pub fn split(content: &str) -> Option<(Vec<String>, String)> {
    let text = normalize(content);
    let mut lines = text.split('\n');
    if lines.next()? != "---" {
        return None;
    }
    let mut fm = Vec::new();
    for line in lines.by_ref() {
        if line == "---" {
            let body: Vec<&str> = lines.collect();
            return Some((fm, body.join("\n")));
        }
        fm.push(line.to_string());
    }
    None
}

pub fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let (lines, _) = split(content)?;
    let mut fm = Frontmatter { lines: lines.clone(), ..Default::default() };
    for line in &lines {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t', '-']) {
            fm.complex = true;
            continue;
        }
        match KEY_LINE_RE.captures(line) {
            Some(c) => {
                let key = c.get(1).map_or("", |m| m.as_str()).to_string();
                let value = c.get(2).map_or("", |m| m.as_str()).trim().to_string();
                if value.starts_with('"') && value.contains('\\') {
                    fm.complex = true; // YAML escapes ("\x61uto") would hide the real value
                }
                if value.is_empty() || value.starts_with(['{', '[', '&', '*', '|', '>', '!']) || value.contains(" #") {
                    // `[a, b]` lists are fine for tools; everything else is structure we do not interpret.
                    if !(value.starts_with('[') && value.ends_with(']') && !value[1..].contains('[')) {
                        fm.complex = true;
                    }
                }
                if fm.fields.iter().any(|(k, _)| *k == key) {
                    fm.complex = true;
                }
                fm.fields.push((key, value));
            }
            None => fm.complex = true,
        }
    }
    Some(fm)
}

fn list_value(v: &str) -> Vec<String> {
    let v = v.trim().trim_start_matches('[').trim_end_matches(']');
    v.split(',').map(|t| unquote(t).trim().to_string()).filter(|t| !t.is_empty()).take(40).collect()
}

/// Frontmatter keys whose values cannot make an agent run commands on their own. Anything else is treated as
/// unreadable so new or unknown keys fail closed (D-057).
const SAFE_KEYS: [&str; 14] = [
    "name", "description", "tools", "disallowedTools", "model", "effort", "color", "maxTurns", "isolation", "memory",
    "skills", "background", "initialPrompt", "permissionMode",
];
/// Modes no stronger than asking the user. Any other value (including escaped or quoted tricks) counts as dangerous.
const CALM_MODES: [&str; 3] = ["default", "plan", "manual"];

/// Conservative: unknown keys, non-literal modes and anything the flat reader cannot vouch for are risky. The
/// `hooks`/`mcpServers` flags also look at raw bytes so nesting or quoting cannot hide them (D-057).
pub fn risk_of(content: &str) -> Risk {
    let Some(fm) = parse_frontmatter(content) else {
        return Risk { unreadable: true, ..Default::default() };
    };
    let raw = fm.lines.join("\n");
    let mode_bad = fm.fields.iter().filter(|(k, _)| k == "permissionMode").any(|(_, v)| !CALM_MODES.contains(&unquote(v)));
    let unknown_key = fm.fields.iter().any(|(k, _)| !SAFE_KEYS.contains(&k.as_str()) && k != "hooks" && k != "mcpServers");
    Risk {
        hooks: raw.contains("hooks"),
        mcp_servers: raw.contains("mcpServers"),
        danger_mode: mode_bad || raw.contains("bypassPermissions") || raw.contains("dangerously"),
        unreadable: fm.complex || unknown_key || raw.contains("<<"),
    }
}

fn merge(a: &mut Risk, b: &Risk) {
    a.hooks |= b.hooks;
    a.mcp_servers |= b.mcp_servers;
    a.danger_mode |= b.danger_mode;
    a.unreadable |= b.unreadable;
}

/// Raw scan of one agents directory for `name`, without discovery's filters: anything skipped (links, oversize,
/// non-UTF-8, unreadable, too many entries) counts as an unreadable candidate, because the CLI may still use it.
/// Returns `None` when the directory has no candidate for `name`.
fn scan_candidates(dir: &Path, name: &str) -> Option<Risk> {
    let mut found: Option<Risk> = None;
    let add = |r: Risk, found: &mut Option<Risk>| match found {
        Some(f) => merge(f, &r),
        None => *found = Some(r),
    };
    let unreadable = Risk { unreadable: true, ..Default::default() };
    let Ok(entries) = std::fs::read_dir(dir) else { return Some(unreadable) };
    for (i, e) in entries.filter_map(Result::ok).enumerate() {
        let path = e.path();
        if !path.extension().is_some_and(|x| x == "md") {
            continue;
        }
        if i >= 1000 {
            add(unreadable.clone(), &mut found);
            break;
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let meta = std::fs::symlink_metadata(&path);
        let text = match meta {
            Ok(m) if m.file_type().is_file() && m.len() <= MAX_FILE_BYTES => std::fs::read_to_string(&path).ok(),
            _ => None,
        };
        match text {
            Some(t) => {
                let fm_name = parse_frontmatter(&t).and_then(|f| f.get("name").map(str::to_string));
                if fm_name.as_deref() == Some(name) || (fm_name.is_none() && stem == name) {
                    add(risk_of(&t), &mut found);
                }
            }
            // Can't read it, so can't rule it out: whatever its name, it might define `name`.
            None => add(unreadable.clone(), &mut found),
        }
    }
    found
}

/// Risk of `claude --agent=<name>` started in `cwd`, scanning `cwd` and every ancestor's `.claude/agents` directly
/// (not the cached catalog), then the user directory. Project candidates win over user/plugin/builtin; several
/// candidates at one level are merged (D-057). `None` = no definition found anywhere and not a known builtin/plugin.
pub fn persona_risk(defs: &[AgentDef], config_dir: &Path, name: &str, cwd: &Path) -> Option<(String, Risk)> {
    if !valid_name(name) {
        return None;
    }
    let cwd = cwd.canonicalize().ok()?;
    let unreadable = Risk { unreadable: true, ..Default::default() };
    for dir in cwd.ancestors() {
        let claude = dir.join(".claude");
        match std::fs::symlink_metadata(&claude) {
            Ok(m) if m.file_type().is_symlink() => return Some((crate::tr_fmt!(crate::i18n::Fail::LinkedSuffix, a0 = claude.display()), unreadable)),
            Ok(m) if m.file_type().is_dir() => {}
            _ => continue,
        }
        let agents = claude.join("agents");
        match std::fs::symlink_metadata(&agents) {
            Ok(m) if m.file_type().is_symlink() => return Some((crate::tr_fmt!(crate::i18n::Fail::LinkedSuffix, a0 = agents.display()), unreadable)),
            Ok(m) if m.file_type().is_dir() => {
                if let Some(r) = scan_candidates(&agents, name) {
                    return Some((agents.display().to_string(), r));
                }
            }
            _ => {}
        }
    }
    let user = config_dir.join("agents");
    if std::fs::symlink_metadata(&user).is_ok_and(|m| m.file_type().is_dir()) {
        if let Some(r) = scan_candidates(&user, name) {
            return Some((user.display().to_string(), r));
        }
    }
    let mut others = defs.iter().filter(|d| d.name == name && matches!(d.source, Source::Plugin | Source::Builtin));
    let first = others.next()?;
    let mut risk = first.risk.clone();
    for d in others {
        merge(&mut risk, &d.risk);
    }
    Some((first.path.clone().unwrap_or_else(|| crate::i18n::Fail::Builtin.str().into()), risk))
}

// ---------- discovery ----------

pub struct Roots {
    pub config_dir: PathBuf,
    pub project_dirs: Vec<PathBuf>,
}

fn id_for(path: &Path) -> String {
    let h = Sha256::digest(path.as_os_str().as_encoded_bytes());
    let hex: String = h.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("f-{hex}")
}

/// `dir` must be a real directory: no component from `base` down may be a symlink.
fn real_dir_under(base: &Path, rel: &[&str]) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for part in rel {
        p.push(part);
        let m = std::fs::symlink_metadata(&p).ok()?;
        if !m.file_type().is_dir() {
            return None;
        }
    }
    Some(p)
}

#[cfg(unix)]
fn owned_private(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let me = unsafe { libc::geteuid() };
    m.uid() == me && m.mode() & 0o022 == 0
}

#[cfg(not(unix))]
fn owned_private(_m: &std::fs::Metadata) -> bool {
    true
}

fn read_def(file: &Path, dir_meta: &std::fs::Metadata, source: Source, scope: Option<&Path>, prefix: Option<&str>) -> Option<AgentDef> {
    let m = std::fs::symlink_metadata(file).ok()?;
    if !m.file_type().is_file() || m.len() > MAX_FILE_BYTES {
        return None;
    }
    let content = std::fs::read_to_string(file).ok()?;
    let stem = file.file_stem()?.to_string_lossy().to_string();
    let fm = parse_frontmatter(&content);
    let fm_name = fm.as_ref().and_then(|f| f.get("name")).filter(|n| valid_name(n)).map(str::to_string);
    let base = fm_name.clone().unwrap_or_else(|| display_safe(&stem, 64));
    // Plugin agents are namespaced by their plugin, as Claude Code reports them (`plugin:name`).
    let name = match prefix {
        Some(p) if !base.contains(':') => format!("{p}:{base}"),
        _ => base,
    };
    let editable_kind = matches!(source, Source::User | Source::Project);
    let private = owned_private(&m) && owned_private(dir_meta);
    let readonly_reason = if !editable_kind {
        Some(crate::i18n::Fail::PluginAgentReadOnly.str())
    } else if !private {
        Some(crate::i18n::Fail::OtherOwnerReadOnly.str())
    } else {
        None
    };
    Some(AgentDef {
        id: id_for(file),
        name,
        source,
        scope: scope.map(|s| s.display().to_string()),
        path: Some(display_safe(&file.display().to_string(), 300)),
        description: fm.as_ref().and_then(|f| f.get("description")).map(|d| display_safe(d, 300)),
        model: fm.as_ref().and_then(|f| f.get("model")).map(|d| display_safe(d, 40)),
        tools: fm.as_ref().and_then(|f| f.get("tools")).map(|t| list_value(t).iter().map(|x| display_safe(x, 40)).collect()).unwrap_or_default(),
        disallowed_tools: fm.as_ref().and_then(|f| f.get("disallowedTools")).map(|t| list_value(t).iter().map(|x| display_safe(x, 80)).collect()).unwrap_or_default(),
        skills: fm.as_ref().and_then(|f| f.get("skills")).map(|t| list_value(t).iter().map(|x| display_safe(x, 128)).collect()).unwrap_or_default(),
        malformed: fm.is_none() || fm_name.is_none(),
        editable: readonly_reason.is_none(),
        readonly_reason,
        primary: false,
        usable: false,
        risk: risk_of(&content),
        file: Some(file.to_path_buf()),
    })
}

fn scan_dir(dir: &Path, source: Source, scope: Option<&Path>, prefix: Option<&str>, out: &mut Vec<AgentDef>) {
    let Ok(dir_meta) = std::fs::symlink_metadata(dir) else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
    files.sort();
    for f in files.into_iter().take(200) {
        if let Some(d) = read_def(&f, &dir_meta, source, scope, prefix) {
            out.push(d);
        }
    }
}

/// Catalog for one folder: also scans `cwd` and its ancestors, and marks which definitions `cwd` can use.
pub fn discover_for(roots: &Roots, cwd: Option<&Path>) -> Vec<AgentDef> {
    let mut roots = Roots { config_dir: roots.config_dir.clone(), project_dirs: roots.project_dirs.clone() };
    let canon = cwd.and_then(|c| c.canonicalize().ok());
    if let Some(c) = &canon {
        roots.project_dirs.splice(0..0, c.ancestors().take(12).map(Path::to_path_buf));
    }
    let mut defs = discover(&roots);
    for d in &mut defs {
        d.usable = match (d.source, &canon) {
            (Source::Project, Some(c)) => d.scope.as_deref().is_some_and(|s| c.starts_with(s)),
            (Source::Project, None) => false,
            _ => true,
        };
    }
    defs
}

pub fn discover(roots: &Roots) -> Vec<AgentDef> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in roots.project_dirs.iter().take(MAX_PROJECT_DIRS) {
        let Ok(canon) = p.canonicalize() else { continue };
        if !seen.insert(canon.clone()) {
            continue;
        }
        if let Some(dir) = real_dir_under(&canon, &[".claude", "agents"]) {
            scan_dir(&dir, Source::Project, Some(&canon), None, &mut out);
        }
    }
    if let Some(dir) = real_dir_under(&roots.config_dir, &["agents"]) {
        scan_dir(&dir, Source::User, None, None, &mut out);
    }
    if let Some(cache) = real_dir_under(&roots.config_dir, &["plugins", "cache"]) {
        let start = Instant::now();
        // Layout: cache/<marketplace>/<plugin>/<version>/…/agents. Only the newest version of each plugin counts.
        let mut latest: std::collections::BTreeMap<(String, String), (std::time::SystemTime, PathBuf)> = Default::default();
        for e in walkdir::WalkDir::new(&cache).follow_links(false).max_depth(6).into_iter().filter_map(Result::ok).take(MAX_PLUGIN_ENTRIES) {
            if start.elapsed() > PLUGIN_BUDGET {
                break;
            }
            if !(e.file_type().is_dir() && e.file_name() == "agents") {
                continue;
            }
            let Ok(rel) = e.path().strip_prefix(&cache) else { continue };
            let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
            if parts.len() < 3 {
                continue;
            }
            let version_dir = cache.join(&parts[0]).join(&parts[1]).join(&parts[2]);
            let when = std::fs::symlink_metadata(&version_dir).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            let key = (parts[0].clone(), parts[1].clone());
            if latest.get(&key).is_none_or(|(t, _)| when > *t) {
                latest.insert(key, (when, e.path().to_path_buf()));
            }
        }
        for ((_, plugin), (_, dir)) in latest {
            if valid_name(&plugin) {
                scan_dir(&dir, Source::Plugin, None, Some(&plugin), &mut out);
            }
        }
    }
    for (name, desc) in BUILTINS {
        out.push(AgentDef {
            id: format!("b-{name}"),
            name: name.to_string(),
            source: Source::Builtin,
            scope: None,
            path: None,
            description: Some(desc.str().to_string()),
            model: None,
            tools: Vec::new(),
            disallowed_tools: Vec::new(),
            skills: Vec::new(),
            malformed: false,
            editable: false,
            readonly_reason: Some(crate::i18n::Fail::BuiltinAgent.str()),
            primary: false,
            usable: true,
            risk: Risk::default(),
            file: None,
        });
    }
    out.sort_by(|a, b| (a.name.to_lowercase(), a.source, &a.scope).cmp(&(b.name.to_lowercase(), b.source, &b.scope)));
    let mut last: Option<String> = None;
    for d in &mut out {
        d.primary = last.as_deref() != Some(&d.name);
        last = Some(d.name.clone());
    }
    out
}

/// The definition `claude --agent=<name>` would use in `cwd`: project agents only from `cwd` or an ancestor
/// (deepest wins), then user, plugin, builtin (D-057). `None` = not usable there.
pub fn resolve_for_cwd<'a>(defs: &'a [AgentDef], name: &str, cwd: &Path) -> Option<&'a AgentDef> {
    if !valid_name(name) {
        return None;
    }
    let cwd = cwd.canonicalize().ok()?;
    let mut best: Option<&AgentDef> = None;
    for d in defs.iter().filter(|d| d.name == name) {
        let usable = match d.source {
            Source::Project => d.scope.as_deref().is_some_and(|s| cwd.starts_with(s)),
            _ => true,
        };
        if !usable {
            continue;
        }
        best = match best {
            None => Some(d),
            Some(b) if (d.source, std::cmp::Reverse(d.scope.as_ref().map_or(0, String::len))) < (b.source, std::cmp::Reverse(b.scope.as_ref().map_or(0, String::len))) => Some(d),
            keep => keep,
        };
    }
    best
}

// ---------- reading and saving ----------

pub fn token_of(content: &[u8]) -> String {
    Sha256::digest(content).iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SaveError {
    #[error("{}", crate::i18n::Fail::AgentNotListed.str())]
    NotFound,
    #[error("{0}")]
    ReadOnly(&'static str),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{}", crate::i18n::ReasonCode::ChangedOutside.now())]
    Conflict,
    #[error("{}", crate::i18n::Fail::SaveFileFailed.str())]
    Io,
}

#[cfg(unix)]
fn nlink(m: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::nlink(m)
}

#[cfg(not(unix))]
fn nlink(_m: &std::fs::Metadata) -> u64 {
    1
}

/// Full text for the editor plus its token.
pub fn load(defs: &[AgentDef], id: &str) -> Result<(String, String), SaveError> {
    let d = defs.iter().find(|d| d.id == id).ok_or(SaveError::NotFound)?;
    let file = d.file.as_ref().ok_or(SaveError::ReadOnly(crate::i18n::Fail::BuiltinNoFile.str()))?;
    let m = std::fs::symlink_metadata(file).map_err(|_| SaveError::Io)?;
    if !m.file_type().is_file() || nlink(&m) != 1 {
        return Err(SaveError::ReadOnly(crate::i18n::Fail::NotRegularOrLinked.str()));
    }
    let bytes = std::fs::read(file).map_err(|_| SaveError::Io)?;
    let text = String::from_utf8(bytes).map_err(|_| SaveError::Invalid(crate::i18n::Fail::NotUtf8File.str()))?;
    let token = token_of(text.as_bytes());
    Ok((text, token))
}

#[derive(Debug)]
pub struct SavePlan {
    pub path: PathBuf,
    pub file_name: String,
    pub content: String,
    pub old_token: String,
    pub changed_lines: usize,
    /// Anything other than body text or a one-line `name`/`description` changed.
    pub permission_warning: bool,
    pub risk: Risk,
    pub frontmatter_preview: String,
    #[cfg(unix)]
    ino: u64,
    #[cfg(unix)]
    mode: u32,
}

fn changed_lines(a: &str, b: &str) -> usize {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let common = a.iter().zip(&b).filter(|(x, y)| x == y).count();
    a.len().max(b.len()) - common
}

/// Allow-list (D-054): only body edits or a plain one-line `name`/`description` change skip the warning.
pub fn needs_permission_warning(old: &str, new: &str) -> bool {
    let (Some(o), Some(n)) = (parse_frontmatter(old), parse_frontmatter(new)) else { return true };
    if o.complex || n.complex || o.lines.len() != n.lines.len() {
        return true;
    }
    o.lines.iter().zip(&n.lines).filter(|(a, b)| a != b).any(|(a, b)| {
        let key = |l: &str| KEY_LINE_RE.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
        let (ka, kb) = (key(a), key(b));
        !(ka == kb && matches!(ka.as_deref(), Some("name" | "description")))
    })
}

pub fn plan_save(defs: &[AgentDef], id: &str, content: String, token: &str) -> Result<SavePlan, SaveError> {
    let d = defs.iter().find(|d| d.id == id).ok_or(SaveError::NotFound)?;
    if let Some(r) = d.readonly_reason {
        return Err(SaveError::ReadOnly(r));
    }
    let path = d.file.clone().ok_or(SaveError::NotFound)?;
    plan_save_path(path, content, token)
}

/// The guarded save for any one text file the caller already resolved (skill-authoring reuses it, D-086).
pub fn plan_save_path(path: PathBuf, content: String, token: &str) -> Result<SavePlan, SaveError> {
    if content.len() as u64 > MAX_FILE_BYTES || content.contains('\0') {
        return Err(SaveError::Invalid(crate::i18n::Fail::Text256Only.str()));
    }
    let m = std::fs::symlink_metadata(&path).map_err(|_| SaveError::NotFound)?;
    if !m.file_type().is_file() || nlink(&m) != 1 {
        return Err(SaveError::ReadOnly(crate::i18n::Fail::NotRegularOrLinked.str()));
    }
    let current = std::fs::read(&path).map_err(|_| SaveError::Io)?;
    if token_of(&current) != token {
        return Err(SaveError::Conflict);
    }
    let old = String::from_utf8(current).map_err(|_| SaveError::Invalid(crate::i18n::Fail::NotUtf8File.str()))?;
    // Risky and changed lines first, so a long description cannot push them past the cut-off.
    let fm_raw = match (split(&content), split(&old)) {
        (Some((new_lines, _)), old_fm) => {
            let old_lines = old_fm.map(|(l, _)| l).unwrap_or_default();
            let key = |l: &String| KEY_LINE_RE.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
            let urgent = |l: &String| !SAFE_KEYS.contains(&key(l).as_str()) || key(l) == "permissionMode" || key(l) == "tools" || !old_lines.contains(l);
            let (first, rest): (Vec<&String>, Vec<&String>) = new_lines.iter().partition(|l| urgent(l));
            first.into_iter().chain(rest).cloned().collect::<Vec<_>>().join("\n")
        }
        (None, _) => crate::i18n::Fail::NoFrontmatter.str().into(),
    };
    let mut preview = display_safe(&fm_raw.replace('\n', " ⏎ "), 600);
    if fm_raw.chars().count() > 600 {
        preview.push('…');
    }
    Ok(SavePlan {
        file_name: display_safe(&path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(), 80),
        changed_lines: changed_lines(&old, &content),
        permission_warning: needs_permission_warning(&old, &content),
        risk: risk_of(&content),
        frontmatter_preview: preview,
        old_token: token.to_string(),
        content,
        #[cfg(unix)]
        ino: std::os::unix::fs::MetadataExt::ino(&m),
        #[cfg(unix)]
        mode: std::os::unix::fs::MetadataExt::mode(&m),
        path,
    })
}

/// Writes an approved plan. Re-checks the file first: it may have changed while the dialog was open.
pub fn commit_save(plan: &SavePlan) -> Result<(), SaveError> {
    let m = std::fs::symlink_metadata(&plan.path).map_err(|_| SaveError::Conflict)?;
    if !m.file_type().is_file() || nlink(&m) != 1 {
        return Err(SaveError::Conflict);
    }
    let parent_ok = plan.path.parent().and_then(|d| std::fs::symlink_metadata(d).ok()).is_some_and(|pm| pm.file_type().is_dir() && owned_private(&pm));
    if !parent_ok {
        return Err(SaveError::Conflict);
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::ino(&m) != plan.ino || !owned_private(&m) {
        return Err(SaveError::Conflict);
    }
    let current = std::fs::read(&plan.path).map_err(|_| SaveError::Conflict)?;
    if token_of(&current) != plan.old_token {
        return Err(SaveError::Conflict);
    }
    let dir = plan.path.parent().ok_or(SaveError::Io)?;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ u128::from(std::process::id());
    let tmp = dir.join(format!(".{}.dduck-{:x}.tmp", plan.file_name.trim_start_matches('.'), nonce & 0xffff_ffff_ffff));
    let result = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&tmp)?;
        f.write_all(plan.content.as_bytes())?;
        f.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(plan.mode & 0o777 & !0o022))?;
        }
        std::fs::rename(&tmp, &plan.path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(SaveError::Io);
    }
    Ok(())
}

/// Rewrites only the `tools` and `disallowedTools` lines of the frontmatter (tool-permissions, D-076). An empty
/// `tools` list removes the line (the agent then inherits every tool). Body and other lines are kept as they are.
/// `tools` and `disallowedTools` exactly as written (no count or length cut, unlike the catalog's display copy).
pub fn tool_lists_raw(content: &str) -> (Vec<String>, Vec<String>) {
    let fm = parse_frontmatter(content);
    let raw = |k: &str| -> Vec<String> {
        fm.as_ref()
            .and_then(|f| f.get(k))
            .map(|v| v.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(|t| unquote(t).trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default()
    };
    (raw("tools"), raw("disallowedTools"))
}

/// The frontmatter lines of a file as written, for line edits that keep everything else byte for byte (QA review:
/// a BOM and CRLF endings survive). Each line keeps its own ending; added lines use the opening line's ending.
struct RawFm<'a> {
    bom: &'a str,
    open: &'a str,
    eol: &'a str,
    lines: Vec<(String, &'a str)>,
    /// From the closing `---` to the end of the file, untouched.
    tail: &'a str,
}

impl<'a> RawFm<'a> {
    fn parse(content: &'a str) -> Option<Self> {
        let (bom, text) = match content.strip_prefix('\u{feff}') {
            Some(t) => (&content[..content.len() - t.len()], t),
            None => ("", content),
        };
        let open = if text.starts_with("---\r\n") { &text[..5] } else if text.starts_with("---\n") { &text[..4] } else { return None };
        let eol = &open[3..];
        let mut pos = open.len();
        let mut lines = Vec::new();
        while pos < text.len() {
            let rest = &text[pos..];
            let end = rest.find('\n').map_or(rest.len(), |i| i + 1);
            let with_end = &rest[..end];
            let ending_len = if with_end.ends_with("\r\n") { 2 } else { usize::from(with_end.ends_with('\n')) };
            let line = &with_end[..with_end.len() - ending_len];
            if line == "---" {
                return Some(RawFm { bom, open, eol, lines, tail: rest });
            }
            if line.contains('\r') {
                return None; // a lone CR: `split` would read other lines than we would write
            }
            lines.push((line.to_string(), &with_end[with_end.len() - ending_len..]));
            pos += end;
        }
        None
    }

    fn texts(&self) -> Vec<String> {
        self.lines.iter().map(|(l, _)| l.clone()).collect()
    }

    fn set(&mut self, i: usize, l: String) {
        self.lines[i].0 = l;
    }

    fn insert(&mut self, i: usize, l: String) {
        let eol = self.eol;
        self.lines.insert(i, (l, eol));
    }

    fn render(&self) -> String {
        let mut out = String::with_capacity(self.tail.len() + 256);
        out.push_str(self.bom);
        out.push_str(self.open);
        for (l, e) in &self.lines {
            out.push_str(l);
            out.push_str(if e.is_empty() { self.eol } else { e });
        }
        out.push_str(self.tail);
        out
    }
}

pub fn with_tool_lists(content: &str, tools: &[String], disallowed: &[String]) -> Result<String, SaveError> {
    let mut fm = RawFm::parse(content).ok_or(SaveError::Invalid(crate::i18n::Fail::ToolsNoFront.str()))?;
    let (cur_tools, cur_dis) = tool_lists_raw(content);
    for (list, cur) in [(tools, &cur_tools), (disallowed, &cur_dis)] {
        if list.len() > 60 && list != cur.as_slice() {
            return Err(SaveError::Invalid(crate::i18n::Fail::ToolsMax60.str()));
        }
        // Values already in the file are kept as they are; only new ones are validated.
        for t in list.iter().filter(|t| !cur_tools.contains(t) && !cur_dis.contains(t)) {
            let bad_chars = t.contains([',', '#', '"', '\'', '[', ']']) || t.contains(": ");
            if bad_chars || crate::permissions::parse_rule(t).is_err() {
                return Err(SaveError::Invalid(crate::i18n::Fail::ToolNameFormat.str()));
            }
        }
    }
    let key_of = |l: &str| KEY_LINE_RE.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    for (key, list, cur) in [("tools", tools, &cur_tools), ("disallowedTools", disallowed, &cur_dis)] {
        if list == cur.as_slice() {
            continue; // unchanged: the line stays byte for byte
        }
        let lines = fm.texts();
        let idx: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| key_of(l).as_deref() == Some(key)).map(|(i, _)| i).collect();
        if idx.len() > 1 {
            return Err(SaveError::Invalid(crate::i18n::Fail::ToolsLineDup.str()));
        }
        if let Some(&i) = idx.first() {
            let value = KEY_LINE_RE.captures(&lines[i]).and_then(|c| c.get(2)).map_or("", |m| m.as_str()).trim().to_string();
            let next_indented = lines.get(i + 1).is_some_and(|n| n.starts_with([' ', '\t', '-']));
            if value.is_empty() || next_indented {
                return Err(SaveError::Invalid(crate::i18n::Fail::ToolsMultiline.str()));
            }
        }
        let new_line = (!list.is_empty()).then(|| format!("{key}: {}", list.join(", ")));
        match (idx.first(), new_line) {
            (Some(&i), Some(l)) => fm.set(i, l),
            (Some(&i), None) => {
                fm.lines.remove(i);
            }
            (None, Some(l)) => {
                let after = lines.iter().rposition(|x| matches!(key_of(x).as_deref(), Some("tools" | "description" | "name"))).map_or(lines.len(), |i| i + 1);
                fm.insert(after, l);
            }
            (None, None) => {}
        }
    }
    Ok(fm.render())
}

/// The `skills` line exactly as written.
pub fn skills_raw(content: &str) -> Vec<String> {
    parse_frontmatter(content).as_ref().and_then(|f| f.get("skills")).map(|v| v.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(|t| unquote(t).trim().to_string()).filter(|t| !t.is_empty()).collect()).unwrap_or_default()
}

/// "기본 장비" of a file agent (agent-loadout): rewrites the `skills` line (`skills: [a, b]`) and the `tools` /
/// `disallowedTools` lines (the caller checks with `loadout::check_agent_lists` that only MCP entries differ).
/// `mcpServers` and every other line stay as they are, and so do the BOM and line endings.
pub fn with_loadout(content: &str, skills: &[String], tools: &[String], disallowed: &[String]) -> Result<String, SaveError> {
    let (cur_tools, cur_dis) = tool_lists_raw(content);
    if tools == cur_tools.as_slice() && disallowed == cur_dis.as_slice() && skills == skills_raw(content).as_slice() {
        return Ok(content.to_string());
    }
    let text = with_tool_lists(content, tools, disallowed)?;
    let cur = skills_raw(&text);
    if skills == cur.as_slice() {
        return Ok(text);
    }
    if skills.len() > crate::loadout::MAX_SKILLS {
        return Err(SaveError::Invalid(crate::i18n::Fail::LoadoutMax12.str()));
    }
    if skills.iter().any(|s| !cur.contains(s) && !crate::loadout::valid_skill_name(s)) {
        return Err(SaveError::Invalid(crate::i18n::Fail::SkillNameBad2.str()));
    }
    let mut fm = RawFm::parse(&text).ok_or(SaveError::Invalid(crate::i18n::Fail::LoadoutNoFront.str()))?;
    let lines = fm.texts();
    let key_of = |l: &str| KEY_LINE_RE.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    let idx: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| key_of(l).as_deref() == Some("skills")).map(|(i, _)| i).collect();
    if idx.len() > 1 {
        return Err(SaveError::Invalid(crate::i18n::Fail::SkillsLineDup.str()));
    }
    if let Some(&i) = idx.first() {
        let value = KEY_LINE_RE.captures(&lines[i]).and_then(|c| c.get(2)).map_or("", |m| m.as_str()).trim().to_string();
        if value.is_empty() || lines.get(i + 1).is_some_and(|n| n.starts_with([' ', '\t', '-'])) {
            return Err(SaveError::Invalid(crate::i18n::Fail::SkillsMultiline.str()));
        }
    }
    let new_line = (!skills.is_empty()).then(|| format!("skills: [{}]", skills.join(", ")));
    match (idx.first(), new_line) {
        (Some(&i), Some(l)) => fm.set(i, l),
        (Some(&i), None) => {
            fm.lines.remove(i);
        }
        (None, Some(l)) => {
            let after = lines.iter().rposition(|x| matches!(key_of(x).as_deref(), Some("tools" | "disallowedTools" | "description" | "name"))).map_or(lines.len(), |i| i + 1);
            fm.insert(after, l);
        }
        (None, None) => {}
    }
    let out = fm.render();
    // Re-read what will be written: it must still parse and carry exactly the new list.
    if skills_raw(&out) != skills || parse_frontmatter(&out).is_none() {
        return Err(SaveError::Invalid(crate::i18n::Fail::LoadoutReread.str()));
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const OK: &str = "---\nname: design-critic\ndescription: 리뷰어\ntools: Read, Grep\n---\n\n본문\n";

    fn roots(tmp: &Path) -> (Roots, PathBuf) {
        let proj = tmp.join("proj");
        std::fs::create_dir_all(proj.join(".claude/agents")).unwrap();
        std::fs::create_dir_all(tmp.join("cfg/agents")).unwrap();
        (Roots { config_dir: tmp.join("cfg"), project_dirs: vec![proj.clone()] }, proj)
    }

    #[test]
    fn frontmatter_reader() {
        let fm = parse_frontmatter(OK).unwrap();
        assert_eq!(fm.get("name"), Some("design-critic"));
        assert_eq!(list_value(fm.get("tools").unwrap()), ["Read", "Grep"]);
        assert!(!fm.complex);
        assert!(parse_frontmatter("no frontmatter").is_none());
        assert!(parse_frontmatter("\u{feff}---\r\nname: a\r\n---\r\nx").is_some(), "BOM and CRLF normalized");
        assert!(parse_frontmatter("---\nname: a\n--- \nx").is_none(), "`--- ` is not a boundary");
        for tricky in ["---\nname: a\nname: b\n---\n", "---\nhooks:\n  x: y\n---\n", "---\n\"tools\": Bash\n---\n", "---\nx: {a: 1}\n---\n", "---\nx: &a v\n---\n", "---\nx: |\n  y\n---\n"] {
            assert!(parse_frontmatter(tricky).unwrap().complex, "{tricky:?} is complex");
        }
    }

    #[test]
    fn risk_and_warning_rules() {
        assert!(!risk_of(OK).any());
        assert!(risk_of("---\nname: a\nhooks:\n  PreToolUse: x\n---\n").hooks);
        assert!(risk_of("---\nname: a\n\"mcpServers\": {}\n---\n").mcp_servers);
        assert!(risk_of("---\nname: a\npermissionMode: bypassPermissions\n---\n").danger_mode);
        assert!(risk_of("---\nname: a\npermissionMode: auto\n---\n").danger_mode);
        assert!(risk_of("broken").unreadable);

        let desc = OK.replace("description: 리뷰어", "description: 더 엄격한 리뷰어");
        assert!(!needs_permission_warning(OK, &desc));
        assert!(!needs_permission_warning(OK, &OK.replace("본문", "새 본문")));
        assert!(needs_permission_warning(OK, &OK.replace("tools: Read, Grep", "tools: Bash")));
        assert!(needs_permission_warning(OK, &OK.replace("description: 리뷰어\n", "description: 리뷰어\ndescription: x\n")), "duplicate key");
        assert!(needs_permission_warning(OK, &OK.replace("tools: Read, Grep\n", "tools: Read, Grep\nhooks:\n  a: b\n")));
        assert!(needs_permission_warning(OK, &OK.replace("name: design-critic", "\"tools\": Bash")));
        assert!(needs_permission_warning(OK, "본문만"));
    }

    #[test]
    fn discovery_and_resolution() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, proj) = roots(tmp.path());
        std::fs::write(proj.join(".claude/agents/design-critic.md"), OK).unwrap();
        std::fs::write(r.config_dir.join("agents/broken.md"), "그냥 텍스트\u{202e}").unwrap();
        std::fs::write(r.config_dir.join("agents/design-critic.md"), OK).unwrap();
        std::fs::write(r.config_dir.join("agents/big.md"), vec![b'a'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(proj.join(".claude/agents/design-critic.md"), r.config_dir.join("agents/link.md")).unwrap();
        let defs = discover(&r);
        let names: Vec<(&str, Source)> = defs.iter().filter(|d| d.source != Source::Builtin).map(|d| (d.name.as_str(), d.source)).collect();
        assert_eq!(names, [("broken", Source::User), ("design-critic", Source::Project), ("design-critic", Source::User)]);
        let broken = defs.iter().find(|d| d.name == "broken").unwrap();
        assert!(broken.malformed && broken.editable);
        assert!(defs.iter().find(|d| d.source == Source::Project).unwrap().primary);
        assert!(defs.iter().any(|d| d.id == "b-Explore" && !d.editable));

        let project = resolve_for_cwd(&defs, "design-critic", &proj).unwrap();
        assert_eq!(project.source, Source::Project);
        let elsewhere = tmp.path().join("other");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(resolve_for_cwd(&defs, "design-critic", &elsewhere).unwrap().source, Source::User, "project agent not usable elsewhere");
        assert!(resolve_for_cwd(&defs, "--dangerously-skip-permissions", &proj).is_none());
        assert!(resolve_for_cwd(&defs, "nope", &proj).is_none());
    }

    #[test]
    fn hardened_risk_rules() {
        assert!(risk_of("---\nname: a\npermissionMode: \"\\x61uto\"\n---\n").any(), "escaped value");
        assert!(risk_of("---\nname: a\npermissionMode: acceptEdits\n---\n").danger_mode, "stronger than asking");
        assert!(!risk_of("---\nname: a\npermissionMode: plan\n---\n").any());
        assert!(risk_of("---\nname: a\nfutureKey: x\n---\n").unreadable, "unknown key fails closed");
    }

    #[test]
    fn persona_risk_scans_what_discovery_skips() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, proj) = roots(tmp.path());
        let agents = proj.join(".claude/agents");
        let defs = discover(&r);
        // Builtin name shadowed by an oversized project file: discovery drops it, the CLI might not.
        std::fs::write(agents.join("Explore.md"), vec![b'a'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        let (_, risk) = persona_risk(&defs, &r.config_dir, "Explore", &proj).unwrap();
        assert!(risk.unreadable);
        std::fs::remove_file(agents.join("Explore.md")).unwrap();
        // Two files with the same name: risks merge.
        std::fs::write(agents.join("a.md"), "---\nname: dup\n---\n").unwrap();
        std::fs::write(agents.join("b.md"), "---\nname: dup\npermissionMode: bypassPermissions\n---\n").unwrap();
        assert!(persona_risk(&defs, &r.config_dir, "dup", &proj).unwrap().1.danger_mode);
        // A cwd that discovery never saw, and a subfolder of the project.
        let sub = proj.join("src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(persona_risk(&defs, &r.config_dir, "dup", &sub).unwrap().1.danger_mode);
        // Linked agents directory fails closed.
        #[cfg(unix)]
        {
            let other = tmp.path().join("other");
            std::fs::create_dir_all(other.join(".claude")).unwrap();
            std::os::unix::fs::symlink(&agents, other.join(".claude/agents")).unwrap();
            assert!(persona_risk(&defs, &r.config_dir, "anything", &other).unwrap().1.unreadable);
        }
        assert!(persona_risk(&defs, &r.config_dir, "nope", &tmp.path().join("cfg")).is_none());
        assert!(!persona_risk(&defs, &r.config_dir, "Plan", &tmp.path().join("cfg")).unwrap().1.any());
    }

    #[test]
    fn loadout_edits_only_skills_and_disallowed_lines() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let out = with_loadout(OK, &v(&["frontend-design", "expo:expo-ui"]), &v(&["Read", "Grep"]), &v(&["mcp__notion__*"])).unwrap();
        assert_eq!(out, "---\nname: design-critic\ndescription: 리뷰어\ntools: Read, Grep\ndisallowedTools: mcp__notion__*\nskills: [frontend-design, expo:expo-ui]\n---\n\n본문\n");
        assert_eq!(skills_raw(&out), ["frontend-design", "expo:expo-ui"]);
        let back = with_loadout(&out, &[], &v(&["Read", "Grep"]), &[]).unwrap();
        assert_eq!(back, OK, "removing both restores the file");
        assert_eq!(with_loadout(OK, &[], &v(&["Read", "Grep"]), &[]).unwrap(), OK, "nothing to change");
        assert!(with_loadout(OK, &v(&["-evil"]), &v(&["Read", "Grep"]), &[]).is_err());
        assert!(with_loadout(OK, &v(&["a, b"]), &v(&["Read", "Grep"]), &[]).is_err());
        assert!(with_loadout(OK, &(0..13).map(|i| format!("s{i}")).collect::<Vec<_>>(), &v(&["Read", "Grep"]), &[]).is_err());
        assert!(with_loadout("---\nname: a\nskills:\n  - x\n---\n", &v(&["y"]), &[], &[]).is_err(), "block lists left alone");
        // mcpServers lines are never touched.
        let with_mcp = "---\nname: a\nmcpServers: [figma]\n---\nb";
        assert!(with_loadout(with_mcp, &v(&["x"]), &[], &[]).unwrap().contains("mcpServers: [figma]\n"));
        // A tools line gets the chosen server so it can be used at all (design review MAJOR 1).
        let out = with_loadout(OK, &[], &v(&["Read", "Grep", "mcp__figma__*"]), &[]).unwrap();
        assert!(out.contains("tools: Read, Grep, mcp__figma__*\n"), "{out}");
    }

    #[test]
    fn line_edits_keep_bom_and_crlf() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let crlf = "\u{feff}---\r\nname: a\r\ndescription: d\r\ntools: Read\r\n---\r\n\r\n본문\r\n둘째 줄\n";
        let out = with_loadout(crlf, &v(&["frontend-design"]), &v(&["Read", "mcp__figma__*"]), &v(&["mcp__notion__*"])).unwrap();
        assert_eq!(out, "\u{feff}---\r\nname: a\r\ndescription: d\r\ntools: Read, mcp__figma__*\r\ndisallowedTools: mcp__notion__*\r\nskills: [frontend-design]\r\n---\r\n\r\n본문\r\n둘째 줄\n");
        assert_eq!(skills_raw(&out), ["frontend-design"]);
        let back = with_loadout(&out, &[], &v(&["Read"]), &[]).unwrap();
        assert_eq!(back, crlf, "round trip restores every byte");
        let tools = with_tool_lists(crlf, &v(&["Read", "Grep"]), &[]).unwrap();
        assert_eq!(tools, crlf.replace("tools: Read\r\n", "tools: Read, Grep\r\n"));
        // Closing line at the end of the file without a newline, LF file.
        assert_eq!(with_tool_lists("---\nname: a\n---", &v(&["Read"]), &[]).unwrap(), "---\nname: a\ntools: Read\n---");
        assert!(with_tool_lists("---\nname: a\rtools: x\n---\n", &v(&["Read"]), &[]).is_err(), "lone CR refused");
    }

    #[test]
    fn tool_lists_edit_only_their_lines() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let out = with_tool_lists(OK, &v(&["Read", "Grep"]), &v(&["Edit", "Write"])).unwrap();
        assert_eq!(out, "---\nname: design-critic\ndescription: 리뷰어\ntools: Read, Grep\ndisallowedTools: Edit, Write\n---\n\n본문\n");
        let out = with_tool_lists(&out, &[], &v(&["Bash(git push:*)"])).unwrap();
        assert_eq!(out, "---\nname: design-critic\ndescription: 리뷰어\ndisallowedTools: Bash(git push:*)\n---\n\n본문\n");
        assert!(with_tool_lists("본문만", &[], &[]).is_err());
        assert!(with_tool_lists("---\nname: a\ntools:\n  - Read\n---\n", &v(&["Bash"]), &[]).is_err(), "block lists left alone");
        assert!(with_tool_lists(OK, &v(&["Read, Bash"]), &[]).is_err());
        assert!(with_tool_lists(OK, &v(&["Read\nhooks: x"]), &[]).is_err());
        let fm = parse_frontmatter(&with_tool_lists(OK, &v(&["Read"]), &v(&["Edit"])).unwrap()).unwrap();
        assert_eq!(list_value(fm.get("disallowedTools").unwrap()), ["Edit"]);
        // Long names and long lists survive a round trip through the raw reader (no 40-char/40-item cut).
        let long = format!("mcp__{}__{}", "s".repeat(30), "t".repeat(40));
        let many: Vec<String> = (0..50).map(|i| format!("mcp__srv__tool_{i}")).collect();
        let src = with_tool_lists(OK, &many, std::slice::from_ref(&long)).unwrap();
        let (t, d) = tool_lists_raw(&src);
        assert_eq!((t.len(), d), (50, vec![long.clone()]));
        let again = with_tool_lists(&src, &t, &[long, "Write".into()]).unwrap();
        assert_eq!(tool_lists_raw(&again).0, many, "unchanged tools line kept");
    }

    #[test]
    fn guarded_save() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, proj) = roots(tmp.path());
        let file = proj.join(".claude/agents/design-critic.md");
        std::fs::write(&file, OK).unwrap();
        let defs = discover(&r);
        let id = defs.iter().find(|d| d.source == Source::Project).unwrap().id.clone();
        let (text, token) = load(&defs, &id).unwrap();
        assert_eq!(text, OK);

        assert_eq!(plan_save(&defs, "f-unknown", "x".into(), &token).unwrap_err(), SaveError::NotFound);
        assert!(matches!(plan_save(&defs, "b-Explore", "x".into(), &token), Err(SaveError::ReadOnly(_))));
        assert_eq!(plan_save(&defs, &id, "x".into(), "stale").unwrap_err(), SaveError::Conflict);

        let new = OK.replace("tools: Read, Grep", "tools: Bash");
        let plan = plan_save(&defs, &id, new.clone(), &token).unwrap();
        assert!(plan.permission_warning && plan.changed_lines == 1 && plan.frontmatter_preview.contains("tools: Bash"));
        std::fs::write(&file, "밖에서 고침").unwrap();
        assert_eq!(commit_save(&plan).unwrap_err(), SaveError::Conflict, "changed while the dialog was open");

        std::fs::write(&file, OK).unwrap();
        let plan = plan_save(&defs, &id, new.clone(), &token).unwrap();
        commit_save(&plan).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), new);
        let leftovers = std::fs::read_dir(file.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1, "no temp files left");
    }
}
