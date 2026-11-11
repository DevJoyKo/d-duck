//! Installed Claude Code plugins (plugins-tab, D-075). Parses `claude plugin list --json` through allow-listed structs
//! only: MCP server URLs, headers and env never survive deserialization, and `installPath` stays in the core (it is
//! used to find components, never sent to the webview). Components are read from the install folder by name and
//! description only (64 KB per file, no symlinks); hook and MCP/LSP config files are checked for presence, not read.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::agents_catalog::parse_frontmatter;
use crate::skills_catalog::{Skill, SkillSource, commands_in, read_head, real_dir, skill_at, skills_in};
use crate::text::display_safe;

const MAX_AGENTS: usize = 500;

static PLUGIN_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,99}@[A-Za-z0-9][A-Za-z0-9._-]{0,99}$").unwrap_or_else(|_| unreachable!())
});

/// `name@marketplace`, safe to pass as one CLI argument (security review: argument injection).
pub fn valid_id(id: &str) -> bool {
    PLUGIN_ID_RE.is_match(id)
}

// ---------- CLI shapes (allow-listed fields only) ----------

/// An installed plugin as listed by the CLI (before components are read).
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub id: String,
    pub version: Option<String>,
    pub scope: Scope,
    pub enabled: bool,
    pub project_enabled: bool,
    pub install_path: Option<PathBuf>,
    pub installed_at: Option<String>,
    pub last_updated: Option<String>,
    pub mcp: Vec<McpServer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    User,
    Project,
    Local,
    Managed,
    Other,
}

impl Scope {
    fn parse(s: Option<&str>) -> Self {
        match s {
            Some("user") => Scope::User,
            Some("project") => Scope::Project,
            Some("local") => Scope::Local,
            Some("managed") => Scope::Managed,
            _ => Scope::Other,
        }
    }

    pub fn as_cli(self) -> Option<&'static str> {
        match self {
            Scope::User => Some("user"),
            Scope::Project => Some("project"),
            Scope::Local => Some("local"),
            Scope::Managed | Scope::Other => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    /// `http`, `sse`, `stdio`… — the URL and any headers/env are dropped while parsing.
    pub kind: Option<String>,
}

fn str_of(v: &serde_json::Value, key: &str, max: usize) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|x| display_safe(x, max))
}

/// One entry, read field by field (strings/bools only); `None` drops just that entry.
fn parse_entry(v: &serde_json::Value) -> Option<Installed> {
    let id = v.get("id")?.as_str()?;
    if !valid_id(id) {
        return None;
    }
    let mcp = v
        .get("mcpServers")
        .and_then(|m| m.as_object())
        .map(|m| {
            m.iter()
                .take(50)
                // Only the name and the `type` string: url, headers, command, args and env are never copied.
                .map(|(name, s)| McpServer { name: display_safe(name, 100), kind: str_of(s, "type", 20) })
                .collect()
        })
        .unwrap_or_default();
    Some(Installed {
        id: id.to_string(),
        version: str_of(v, "version", 40),
        scope: Scope::parse(v.get("scope").and_then(|x| x.as_str())),
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false),
        project_enabled: v.get("projectEnabled").and_then(|x| x.as_bool()).unwrap_or(false),
        install_path: v.get("installPath").and_then(|x| x.as_str()).map(PathBuf::from),
        installed_at: str_of(v, "installedAt", 40),
        last_updated: str_of(v, "lastUpdated", 40),
        mcp,
    })
}

/// `claude plugin list --json`. The top level must be an array; malformed entries are dropped one by one.
pub fn parse_list(stdout: &[u8]) -> Option<Vec<Installed>> {
    let raw: Vec<serde_json::Value> = serde_json::from_slice(stdout).ok()?;
    Some(raw.iter().take(500).filter_map(parse_entry).collect())
}

// ---------- versions (local marketplace clone, read-only) ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateState {
    /// The marketplace lists a newer version (or a different commit); `latest` is a version or a short sha.
    Available { latest: String },
    Latest,
    /// Nothing comparable (no version or sha on one side, plugin missing from the marketplace file).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    #[serde(flatten)]
    pub state: UpdateState,
    /// When Claude Code last refreshed the local marketplace clone (it may be stale).
    pub marketplace_updated: Option<String>,
}

fn numeric(v: &str) -> Option<Vec<u64>> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// Version first, then commit sha. Non-numeric versions compare by equality.
pub fn compare(installed_version: Option<&str>, installed_sha: Option<&str>, latest_version: Option<&str>, latest_sha: Option<&str>) -> UpdateState {
    if let (Some(i), Some(l)) = (installed_version, latest_version) {
        return match (numeric(i), numeric(l)) {
            (Some(a), Some(b)) => {
                let len = a.len().max(b.len());
                let pad = |v: &[u64]| (0..len).map(|k| v.get(k).copied().unwrap_or(0)).collect::<Vec<_>>();
                if pad(&b) > pad(&a) { UpdateState::Available { latest: l.to_string() } } else { UpdateState::Latest }
            }
            _ if i == l => UpdateState::Latest,
            _ => UpdateState::Available { latest: l.to_string() },
        };
    }
    match (installed_sha, latest_sha) {
        (Some(i), Some(l)) if i.eq_ignore_ascii_case(l) => UpdateState::Latest,
        (Some(_), Some(l)) => UpdateState::Available { latest: l.chars().take(7).collect() },
        _ => UpdateState::Unknown,
    }
}

const MAX_JSON: u64 = 4 * 1024 * 1024;

/// A JSON file under `root` (already canonical): relative path of plain components, canonical target still inside,
/// regular file, at most 4 MB.
fn read_json_under(root: &Path, rel: &Path) -> Option<serde_json::Value> {
    use std::io::Read;
    if !rel.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) {
        return None;
    }
    let real = root.join(rel).canonicalize().ok()?;
    if !real.starts_with(root) || !std::fs::symlink_metadata(&real).is_ok_and(|m| m.file_type().is_file()) {
        return None;
    }
    let mut buf = Vec::new();
    std::fs::File::open(&real).ok()?.take(MAX_JSON).read_to_end(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn sha_ok(s: &str) -> bool {
    (7..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Update state per installed plugin from Claude Code's own files under `<config>/plugins` (read-only):
/// `installed_plugins.json` (version, gitCommitSha), `marketplaces/<m>/.claude-plugin/marketplace.json` (version,
/// `source.sha`, or a relative source's `plugin.json` version) and `known_marketplaces.json` (lastUpdated).
pub fn local_updates(config_dir: &Path, installed: &[Installed]) -> HashMap<String, UpdateInfo> {
    let unknown = || UpdateInfo { state: UpdateState::Unknown, marketplace_updated: None };
    let Ok(root) = config_dir.join("plugins").canonicalize() else {
        return installed.iter().map(|p| (p.id.clone(), unknown())).collect();
    };
    let recorded = read_json_under(&root, Path::new("installed_plugins.json"));
    let known = read_json_under(&root, Path::new("known_marketplaces.json"));
    let mut markets: HashMap<String, Option<serde_json::Value>> = HashMap::new();
    installed
        .iter()
        .map(|p| {
            let Some((name, market)) = p.id.split_once('@') else { return (p.id.clone(), unknown()) };
            // installed_plugins.json: { plugins: { id: [ { scope, version, gitCommitSha } ] } }
            let entries = recorded.as_ref().and_then(|r| r.get("plugins")?.get(&p.id)?.as_array().cloned()).unwrap_or_default();
            let entry = entries.iter().find(|e| e.get("scope").and_then(|s| s.as_str()) == p.scope.as_cli()).or(entries.first());
            let inst_version = p.version.clone().or_else(|| entry.and_then(|e| str_of(e, "version", 40)));
            let inst_sha = entry.and_then(|e| e.get("gitCommitSha")?.as_str()).filter(|s| sha_ok(s)).map(str::to_string);
            let base = Path::new("marketplaces").join(market);
            let catalog = markets.entry(market.to_string()).or_insert_with(|| read_json_under(&root, &base.join(".claude-plugin").join("marketplace.json")));
            let listed = catalog.as_ref().and_then(|c| c.get("plugins")?.as_array()?.iter().find(|e| e.get("name").and_then(|n| n.as_str()) == Some(name)).cloned());
            let (mut latest_version, mut latest_sha) = (None, None);
            if let Some(e) = &listed {
                latest_version = str_of(e, "version", 40);
                match e.get("source") {
                    Some(serde_json::Value::String(src)) if latest_version.is_none() => {
                        latest_version = read_json_under(&root, &base.join(src).join(".claude-plugin").join("plugin.json")).and_then(|m| str_of(&m, "version", 40));
                    }
                    Some(src @ serde_json::Value::Object(_)) => latest_sha = src.get("sha").and_then(|s| s.as_str()).filter(|s| sha_ok(s)).map(str::to_string),
                    _ => {}
                }
            }
            let state = if listed.is_some() { compare(inst_version.as_deref(), inst_sha.as_deref(), latest_version.as_deref(), latest_sha.as_deref()) } else { UpdateState::Unknown };
            let marketplace_updated = known.as_ref().and_then(|k| str_of(k.get(market)?, "lastUpdated", 40));
            (p.id.clone(), UpdateInfo { state, marketplace_updated })
        })
        .collect()
}

// ---------- components ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub name: String,
    pub description: Option<String>,
    pub uses: i64,
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    pub skills: Vec<Item>,
    pub commands: Vec<Item>,
    pub agents: Vec<Item>,
    pub hooks: bool,
    pub lsp: bool,
    /// The install folder was missing, outside the plugin cache, or a link: nothing was read.
    pub unreadable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: Option<String>,
    pub scope: Scope,
    pub enabled: bool,
    pub project_enabled: bool,
    pub installed_at: Option<String>,
    pub last_updated: Option<String>,
    pub mcp: Vec<McpServer>,
    pub components: Components,
    pub update: UpdateInfo,
    /// Why the toggle and update button are locked (managed or project scope), if they are.
    pub locked: Option<&'static str>,
}

/// Install folder inside `<config>/plugins/cache`, canonical, with no link anywhere below the cache root.
fn checked_root(config_dir: &Path, install: &Path) -> Option<PathBuf> {
    let literal = config_dir.join("plugins").join("cache");
    let cache = literal.canonicalize().ok()?;
    let rel = install.strip_prefix(&literal).or_else(|_| install.strip_prefix(&cache)).ok()?;
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) || rel.as_os_str().is_empty() {
        return None;
    }
    let mut p = cache.clone();
    for c in rel.components() {
        p.push(c);
        if !real_dir(&p) {
            return None;
        }
    }
    Some(p)
}

/// Relative paths from plugin.json (`"./x"` or `["./a", "./b"]`), kept only when they stay inside the plugin.
/// Paths read from one manifest field. Real plugins list a folder per skill (27 in mattpocock-skills), so 20 cut them.
const MAX_MANIFEST_PATHS: usize = 200;

/// `.claude-plugin/plugin.json` of an install folder; `Null` when missing or unreadable.
pub(crate) fn read_manifest(root: &Path) -> serde_json::Value {
    read_head(&root.join(".claude-plugin").join("plugin.json")).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
}

/// A plugin's skills: the default `skills/` folder plus every manifest `skills` path, where a path is either a folder
/// of skills or one skill folder (it holds `SKILL.md`). Paths outside the install folder are dropped.
pub(crate) fn plugin_skills(root: &Path, manifest: &serde_json::Value, plugin: &str, out: &mut Vec<Skill>) {
    let Ok(root) = root.canonicalize() else { return };
    for dir in dedupe([vec![root.join("skills")], manifest_paths(manifest.get("skills"), &root)].concat()) {
        if !real_dir(&dir) {
            continue;
        }
        if dir.join("SKILL.md").is_file() {
            skill_at(&dir, Some(plugin), SkillSource::Plugin, None, out);
        } else {
            skills_in(&dir, Some(plugin), SkillSource::Plugin, None, out);
        }
    }
}

/// A plugin's agent files as Claude Code reads them: `*.md` in the default `agents/` folder plus every manifest `agents`
/// entry, which names one `.md` file (Claude Code rejects a folder there: "must end with .md", so a folder entry is
/// ignored). Never any other folder that happens to be named `agents` (e.g. `docs/agents/`). Each file is a regular
/// file (not a link) whose canonical path stays inside the canonical install folder. The plugins tab and the agents
/// tab both use this list.
pub(crate) fn plugin_agent_files(root: &Path) -> Vec<PathBuf> {
    let Ok(root) = root.canonicalize() else { return Vec::new() };
    let mut files = Vec::new();
    let default = root.join("agents");
    if real_dir(&default) {
        if let Ok(entries) = std::fs::read_dir(&default) {
            let mut found: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md") && file_exists(p))
                .collect();
            found.sort();
            files.extend(found.into_iter().take(MAX_AGENTS));
        }
    }
    let manifest = read_manifest(&root);
    let listed = manifest_paths(manifest.get("agents"), &root).into_iter().filter(|p| p.extension().is_some_and(|x| x == "md") && file_exists(p));
    files.extend(listed.take(MAX_AGENTS));
    dedupe(files)
}

/// Install folders recorded for a plugin id in `<config>/plugins/installed_plugins.json` (one per scope/project;
/// project installs may pin different versions). Each must pass `checked_root` and sit in that plugin's own cache
/// folder (`cache/<marketplace>/<plugin>/…`).
fn recorded_roots(config_dir: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut out: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let Ok(base) = config_dir.join("plugins").canonicalize() else { return out };
    let Ok(cache) = config_dir.join("plugins").join("cache").canonicalize() else { return out };
    let Some(recorded) = read_json_under(&base, Path::new("installed_plugins.json")) else { return out };
    let Some(plugins) = recorded.get("plugins").and_then(|p| p.as_object()) else { return out };
    for (id, entries) in plugins.iter().take(500) {
        let Some((name, market)) = id.split_once('@').filter(|_| valid_id(id)) else { continue };
        let own = cache.join(market).join(name);
        let roots: Vec<PathBuf> = entries
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .take(50)
            .filter_map(|e| e.get("installPath")?.as_str())
            .filter_map(|p| checked_root(config_dir, Path::new(p)))
            .filter(|r| r.starts_with(&own) && r != &own)
            .collect();
        if !roots.is_empty() {
            out.insert(id.clone(), dedupe(roots));
        }
    }
    out
}

/// Version folders scanned per run, across all plugins.
const MAX_VERSION_DIRS: usize = 5000;

/// The install folders whose components count, as `(plugin name, canonical folder)`: the `installPath`s recorded in
/// `installed_plugins.json` (what Claude Code loads), or, for a cached plugin with no usable record, its most recently
/// modified version folder. Only real folders (no links) under a real `<config>/plugins/cache`.
pub(crate) fn plugin_roots(config_dir: &Path) -> Vec<(String, PathBuf)> {
    let cache = config_dir.join("plugins").join("cache");
    if !real_dir(&config_dir.join("plugins")) || !real_dir(&cache) {
        return Vec::new();
    }
    let recorded = recorded_roots(config_dir);
    let dirs = |p: &Path| -> Vec<std::fs::DirEntry> {
        let mut v: Vec<std::fs::DirEntry> = std::fs::read_dir(p).map(|r| r.filter_map(Result::ok).filter(|e| real_dir(&e.path())).collect()).unwrap_or_default();
        v.sort_by_key(|e| e.file_name());
        v
    };
    let mut out = Vec::new();
    let mut seen = 0usize;
    'scan: for m in dirs(&cache) {
        for p in dirs(&m.path()) {
            let (market, name) = (m.file_name().to_string_lossy().to_string(), p.file_name().to_string_lossy().to_string());
            if let Some(roots) = recorded.get(&format!("{name}@{market}")) {
                out.extend(roots.iter().map(|r| (name.clone(), r.clone())));
                continue;
            }
            let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
            for v in dirs(&p.path()) {
                seen += 1;
                if seen > MAX_VERSION_DIRS {
                    break 'scan;
                }
                let when = std::fs::symlink_metadata(v.path()).and_then(|x| x.modified()).unwrap_or(std::time::UNIX_EPOCH);
                if newest.as_ref().is_none_or(|(t, _)| when > *t) {
                    newest = Some((when, v.path()));
                }
            }
            if let Some(root) = newest.and_then(|(_, v)| v.canonicalize().ok()) {
                out.push((name, root));
            }
        }
    }
    out
}

fn manifest_paths(v: Option<&serde_json::Value>, root: &Path) -> Vec<PathBuf> {
    let list: Vec<&str> = match v {
        Some(serde_json::Value::String(s)) => vec![s.as_str()],
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).take(MAX_MANIFEST_PATHS).collect(),
        _ => Vec::new(),
    };
    list.into_iter()
        .map(Path::new)
        .filter(|p| p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)))
        .map(|p| root.join(p))
        .filter(|p| p.canonicalize().is_ok_and(|c| c.starts_with(root)))
        .collect()
}

fn agent_item(path: &Path, prefix: &str) -> Option<Item> {
    let text = read_head(path)?;
    let fm = parse_frontmatter(&text);
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let name = fm.as_ref().and_then(|f| f.get("name")).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or(stem);
    Some(Item {
        name: display_safe(&format!("{prefix}:{name}"), 100),
        description: fm.as_ref().and_then(|f| f.get("description")).map(|d| display_safe(d, 300)),
        uses: 0,
        last_used: None,
    })
}

fn file_exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())
}

fn dedupe(mut v: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    v.retain(|p| seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())));
    v
}

fn to_item(s: Skill) -> Item {
    Item { name: s.name, description: s.description, uses: 0, last_used: None }
}

/// Skills, commands and agents (default folders plus plugin.json paths), and whether hooks/LSP are declared.
pub fn components(config_dir: &Path, install: Option<&Path>, plugin: &str) -> Components {
    let Some(root) = install.and_then(|p| checked_root(config_dir, p)) else {
        return Components { unreadable: true, ..Default::default() };
    };
    let manifest = read_manifest(&root);
    let mut c = Components::default();
    let mut skills = Vec::new();
    plugin_skills(&root, &manifest, plugin, &mut skills);
    let mut commands = Vec::new();
    for dir in dedupe([vec![root.join("commands")], manifest_paths(manifest.get("commands"), &root)].concat()) {
        if real_dir(&dir) {
            commands_in(&dir, Some(plugin), SkillSource::Plugin, None, &mut commands);
        }
    }
    c.agents = plugin_agent_files(&root).iter().filter_map(|f| agent_item(f, plugin)).collect();
    let uniq = |v: Vec<Skill>| {
        let mut seen = HashSet::new();
        let mut items: Vec<Item> = v.into_iter().filter(|s| seen.insert(s.name.clone())).map(to_item).collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    };
    c.skills = uniq(skills);
    c.commands = uniq(commands);
    c.agents.sort_by(|a, b| a.name.cmp(&b.name));
    c.agents.dedup_by(|a, b| a.name == b.name);
    c.hooks = manifest.get("hooks").is_some() || file_exists(&root.join("hooks").join("hooks.json"));
    c.lsp = manifest.get("lspServers").is_some() || file_exists(&root.join(".lsp.json"));
    c
}

/// Usage from the skills catalog (same numbers the skills tab shows), matched by the full `plugin:name`.
pub fn apply_usage(plugins: &mut [Plugin], catalog: &[Skill]) {
    let used: HashMap<&str, (i64, Option<i64>)> = catalog
        .iter()
        .filter(|s| s.source == SkillSource::Plugin && s.uses > 0)
        .map(|s| (s.name.as_str(), (s.uses, s.last_used)))
        .collect();
    for p in plugins {
        for item in p.components.skills.iter_mut().chain(p.components.commands.iter_mut()) {
            if let Some((n, last)) = used.get(item.name.as_str()) {
                item.uses = *n;
                item.last_used = *last;
            }
        }
    }
}

pub fn build(config_dir: &Path, installed: Vec<Installed>) -> Vec<Plugin> {
    let mut updates = local_updates(config_dir, &installed);
    let mut out: Vec<Plugin> = installed
        .into_iter()
        .map(|p| {
            let (name, marketplace) = p.id.split_once('@').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
            let locked = match p.scope {
                Scope::User => None,
                Scope::Managed => Some(crate::i18n::Fail::PluginManaged.str()),
                Scope::Project | Scope::Local => Some(crate::i18n::Fail::PluginProjectScope.str()),
                Scope::Other => Some(crate::i18n::Fail::PluginScopeUnknown.str()),
            };
            Plugin {
                components: components(config_dir, p.install_path.as_deref(), &name),
                update: updates.remove(&p.id).unwrap_or(UpdateInfo { state: UpdateState::Unknown, marketplace_updated: None }),
                id: p.id,
                name: display_safe(&name, 100),
                marketplace: display_safe(&marketplace, 100),
                version: p.version,
                scope: p.scope,
                enabled: p.enabled,
                project_enabled: p.project_enabled,
                installed_at: p.installed_at,
                last_updated: p.last_updated,
                mcp: p.mcp,
                locked,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    out
}

// ---------- action results ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Enable,
    Disable,
    Update,
}

/// Outcome of enable/disable/update. CLI stderr never reaches the webview; it is mapped to one of these.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    Done,
    /// The CLI wants a marketplace-declared command approved (`shownCommand`): nothing was run.
    NeedsApproval,
    /// Enforced by managed settings.
    Locked,
    /// Exit 0 but the re-read list shows no change.
    Unchanged,
    Cancelled,
    Timeout,
    Failed,
}

/// Classifies a finished `plugin enable|disable|update --json` run from its exit status and output.
pub fn classify(success: bool, stdout: &[u8], stderr: &[u8]) -> Outcome {
    let out = String::from_utf8_lossy(stdout);
    let shown = out.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok()).any(|v| v.get("shownCommand").is_some_and(|s| !s.is_null()))
        || out.contains("\"shownCommand\"");
    if shown {
        return Outcome::NeedsApproval;
    }
    if success {
        return Outcome::Done;
    }
    let all = format!("{out}\n{}", String::from_utf8_lossy(stderr)).to_lowercase();
    // Only the CLI's managed-settings wording; a plugin or marketplace merely named "policy…" is not a lock.
    if ["managed settings", "managed setting", "managed policy", "enterprise policy", "managed-settings"].iter().any(|k| all.contains(k)) {
        return Outcome::Locked;
    }
    Outcome::Failed
}

/// Toggle result judged by the re-read list, not the exit code.
pub fn judge_toggle(outcome: Outcome, after: Option<&Installed>, want: bool) -> Outcome {
    match outcome {
        Outcome::Done if after.is_some_and(|a| a.enabled == want) => Outcome::Done,
        Outcome::Done => Outcome::Unchanged,
        other => other,
    }
}

/// Update result judged by the re-read list: version or lastUpdated must have moved.
pub fn judge_update(outcome: Outcome, before: &Installed, after: Option<&Installed>) -> Outcome {
    match outcome {
        Outcome::Done if after.is_some_and(|a| a.version != before.version || a.last_updated != before.last_updated) => Outcome::Done,
        Outcome::Done => Outcome::Unchanged,
        other => other,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Shape observed on CLI 2.1.319 (values altered; CANARY fields must never survive).
    const LIST: &str = r#"[
      {"id":"agent-skills@addy-agent-skills","version":"0.6.15","scope":"user","enabled":true,
       "installPath":"INSTALL_A","installedAt":"2026-11-07T08:45:46.567Z","lastUpdated":"2026-11-07T08:45:46.567Z","projectEnabled":true},
      {"id":"cloudflare@cloudflare","version":"1.0.2","scope":"user","enabled":false,
       "installPath":"INSTALL_B","installedAt":"2026-10-26T08:01:30.460Z","lastUpdated":"2026-10-26T08:01:30.460Z",
       "mcpServers":{"cloudflare":{"type":"http","url":"https://u:CANARY_PW@mcp.cloudflare.com/mcp?k=CANARY_Q","headers":{"Authorization":"Bearer CANARY_TOKEN"}},
                     "local":{"command":"npx","args":["--token","CANARY_ARG"],"env":{"KEY":"CANARY_ENV"}}},
       "projectEnabled":false,"secretExtra":"CANARY_EXTRA"},
      {"id":"corp@internal","version":"2.0.0","scope":"managed","enabled":true,"installPath":"/etc/x"},
      {"id":"--evil@x","version":"1","scope":"user","enabled":true}
    ]"#;

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn id_validation() {
        for ok in ["cloudflare@cloudflare", "expo@claude-plugins-official", "a.b_c-1@m"] {
            assert!(valid_id(ok), "{ok}");
        }
        for bad in ["--evil@x", "x", "a@", "@b", "a@b@c", "a b@c", "a@-s", "a@b\n", &format!("{}@m", "a".repeat(101))] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn list_keeps_allow_listed_fields_only() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let a = cfg.join("plugins/cache/addy-agent-skills/agent-skills/0.6.15");
        let b = cfg.join("plugins/cache/cloudflare/cloudflare/1.0.2");
        std::fs::create_dir_all(&a).unwrap();
        write(&b.join(".mcp.json"), r#"{"mcpServers":{"cloudflare":{"url":"https://CANARY_FILE"}}}"#);
        let json = LIST.replace("INSTALL_A", &a.display().to_string()).replace("INSTALL_B", &b.display().to_string());
        let installed = parse_list(json.as_bytes()).unwrap();
        assert_eq!(installed.len(), 3, "invalid id dropped");
        let plugins = build(&cfg, installed);
        let out = serde_json::to_string(&plugins).unwrap();
        assert!(!out.contains("CANARY"), "{out}");
        assert!(!out.contains("installPath") && !out.contains(&cfg.display().to_string()), "install path stays in core");
        let cf = plugins.iter().find(|p| p.id == "cloudflare@cloudflare").unwrap();
        assert_eq!((cf.name.as_str(), cf.marketplace.as_str(), cf.enabled, cf.locked), ("cloudflare", "cloudflare", false, None));
        assert_eq!(cf.mcp, vec![McpServer { name: "cloudflare".into(), kind: Some("http".into()) }, McpServer { name: "local".into(), kind: None }]);
        let corp = plugins.iter().find(|p| p.id == "corp@internal").unwrap();
        assert!(corp.locked.is_some() && corp.components.unreadable, "managed is locked; path outside cache unread");
        let ag = plugins.iter().find(|p| p.name == "agent-skills").unwrap();
        assert!(ag.project_enabled && !ag.components.unreadable);
        assert!(parse_list(b"not json").is_none());
    }

    #[test]
    fn lenient_list_drops_bad_entries_only() {
        let json = br#"[
          {"id":"good@m","version":"1.0.0","scope":"user","enabled":true,"mcpServers":{"a":{"type":"http"},"b":{"type":7},"c":"x"}},
          {"id":"weird@m","version":12,"scope":["user"],"enabled":"yes","installPath":false,"mcpServers":[1]},
          {"id":42},
          "string entry",
          {"version":"1.0.0"}
        ]"#;
        let l = parse_list(json).unwrap();
        assert_eq!(l.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["good@m", "weird@m"]);
        assert_eq!(l[0].mcp, vec![McpServer { name: "a".into(), kind: Some("http".into()) }, McpServer { name: "b".into(), kind: None }, McpServer { name: "c".into(), kind: None }]);
        let w = &l[1];
        assert_eq!((w.version.clone(), w.scope, w.enabled, w.install_path.clone(), w.mcp.len()), (None, Scope::Other, false, None, 0));
        assert!(parse_list(br#"{"not":"array"}"#).is_none());
    }

    #[test]
    fn version_then_sha_comparison() {
        let c = |iv, is, lv, ls| compare(iv, is, lv, ls);
        assert_eq!(c(Some("1.0.2"), None, Some("1.0.3"), None), UpdateState::Available { latest: "1.0.3".into() });
        assert_eq!(c(Some("1.0.10"), None, Some("1.0.9"), None), UpdateState::Latest);
        assert_eq!(c(Some("1.2"), None, Some("1.2.0"), None), UpdateState::Latest);
        assert_eq!(c(Some("v1.2.0"), None, Some("1.3.0-beta"), None), UpdateState::Available { latest: "1.3.0-beta".into() });
        assert_eq!(c(Some("main"), None, Some("main"), None), UpdateState::Latest);
        assert_eq!(c(Some("1.0.0"), Some("aaaaaaa1"), Some("1.0.0"), Some("bbbbbbb2")), UpdateState::Latest, "version first");
        assert_eq!(c(Some("1.0.0"), Some("abcdef0123"), None, Some("ABCDEF0123")), UpdateState::Latest);
        assert_eq!(c(None, Some("abcdef0123"), None, Some("1234567890")), UpdateState::Available { latest: "1234567".into() });
        assert_eq!(c(Some("1.0.0"), None, None, Some("1234567890")), UpdateState::Unknown);
        assert_eq!(c(None, None, None, None), UpdateState::Unknown);
    }

    #[test]
    fn local_marketplace_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let pl = cfg.join("plugins");
        let sha = "d4f484024fec15196bfd3c272e953e3f983972cf";
        write(&pl.join("installed_plugins.json"), &format!(r#"{{"version":2,"plugins":{{
            "expo@official":[{{"scope":"user","version":"1.13.9","gitCommitSha":"{sha}"}}],
            "kit@official":[{{"scope":"user","version":"1.0.2","gitCommitSha":"1111111111"}}],
            "cf@cf":[{{"scope":"user","version":"1.0.2"}}],
            "pinned@official":[{{"scope":"user","gitCommitSha":"2222222222"}}]}}}}"#));
        write(&pl.join("known_marketplaces.json"), r#"{"official":{"lastUpdated":"2026-11-07T10:16:52.432Z","installLocation":"/x"},"cf":{"lastUpdated":"2026-10-26T08:01:29.687Z"}}"#);
        write(&pl.join("marketplaces/official/.claude-plugin/marketplace.json"), &format!(r#"{{"plugins":[
            {{"name":"expo","source":{{"source":"git-subdir","url":"https://CANARY","sha":"{sha}"}}}},
            {{"name":"kit","source":"./plugins/kit"}},
            {{"name":"pinned","source":{{"source":"url","sha":"3333333333"}}}},
            {{"name":"escape","source":"../../../outside"}}]}}"#));
        write(&pl.join("marketplaces/official/plugins/kit/.claude-plugin/plugin.json"), r#"{"name":"kit","version":"1.0.3"}"#);
        write(&pl.join("marketplaces/cf/.claude-plugin/marketplace.json"), r#"{"plugins":[{"name":"cf","source":"./"}]}"#);
        write(&pl.join("marketplaces/cf/.claude-plugin/plugin.json"), r#"{"version":"1.0.2"}"#);
        let inst = |id: &str, v: Option<&str>| Installed {
            id: id.into(), version: v.map(String::from), scope: Scope::User, enabled: true, project_enabled: false,
            install_path: None, installed_at: None, last_updated: None, mcp: vec![],
        };
        let u = local_updates(&cfg, &[inst("expo@official", Some("1.13.9")), inst("kit@official", Some("1.0.2")), inst("cf@cf", Some("1.0.2")), inst("pinned@official", None), inst("escape@official", Some("1.0.0")), inst("gone@nowhere", Some("1.0.0"))]);
        assert_eq!(u["expo@official"].state, UpdateState::Latest, "no version in marketplace → sha equal");
        assert_eq!(u["kit@official"].state, UpdateState::Available { latest: "1.0.3".into() }, "relative source → plugin.json");
        assert_eq!(u["cf@cf"].state, UpdateState::Latest);
        assert_eq!(u["pinned@official"].state, UpdateState::Available { latest: "3333333".into() });
        assert_eq!(u["escape@official"].state, UpdateState::Unknown, "source outside the clone is not read");
        assert_eq!(u["gone@nowhere"], UpdateInfo { state: UpdateState::Unknown, marketplace_updated: None });
        assert_eq!(u["kit@official"].marketplace_updated.as_deref(), Some("2026-11-07T10:16:52.432Z"));
        let json = serde_json::to_string(&u["kit@official"]).unwrap();
        assert!(json.contains(r#""state":"available""#) && json.contains("marketplaceUpdated") && !json.contains("CANARY"), "{json}");
    }

    #[test]
    fn results_are_judged_by_the_reread_list() {
        let mut before = Installed {
            id: "kit@m".into(), version: Some("1.0.2".into()), scope: Scope::User, enabled: true, project_enabled: false,
            install_path: None, installed_at: None, last_updated: Some("t1".into()), mcp: vec![],
        };
        assert_eq!(judge_update(Outcome::Done, &before, Some(&before)), Outcome::Unchanged, "exit 0 but nothing moved");
        assert_eq!(judge_update(Outcome::Done, &before, None), Outcome::Unchanged);
        assert_eq!(judge_update(Outcome::NeedsApproval, &before, Some(&before)), Outcome::NeedsApproval);
        let mut after = before.clone();
        after.version = Some("1.0.3".into());
        assert_eq!(judge_update(Outcome::Done, &before, Some(&after)), Outcome::Done);
        assert_eq!(judge_toggle(Outcome::Done, Some(&before), false), Outcome::Unchanged, "still enabled");
        before.enabled = false;
        assert_eq!(judge_toggle(Outcome::Done, Some(&before), false), Outcome::Done);
        assert_eq!(judge_toggle(Outcome::Failed, Some(&before), false), Outcome::Failed);
    }

    #[test]
    fn components_from_defaults_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let root = cfg.join("plugins/cache/mk/kit/1.0.0");
        write(&root.join(".claude-plugin/plugin.json"), r#"{"name":"kit","commands":["./.claude/commands","./commands","../../escape"],"skills":"./skills","hooks":"./h.json","agents":["./extra/helper.md","./custom/agents"]}"#);
        for s in ["alpha", "beta", "gamma"] {
            write(&root.join(format!("skills/{s}/SKILL.md")), &format!("---\nname: {s}\ndescription: {s} 설명\u{202E}\n---\nBODY_CANARY\n"));
        }
        write(&root.join(".claude/commands/build.md"), "---\ndescription: 빌드\n---\n");
        write(&root.join("commands/ship.md"), "---\ndescription: 배포\n---\n");
        write(&root.join("commands/ship.toml"), "not md");
        write(&root.join("agents/reviewer.md"), "---\nname: code-reviewer\ndescription: 리뷰\n---\n");
        write(&root.join("extra/helper.md"), "---\nname: helper\n---\n");
        write(&root.join("custom/agents/in-folder.md"), "---\nname: in-folder\n---\n");
        write(&root.join("docs/agents/triage-labels.md"), "# doc\n");
        write(&tmp.path().join("escape/x.md"), "---\ndescription: 밖\n---\n");
        #[cfg(unix)]
        {
            write(&tmp.path().join("outside/SKILL.md"), "---\nname: linked\n---\n");
            std::os::unix::fs::symlink(tmp.path().join("outside"), root.join("skills/linked")).unwrap();
        }
        let c = components(&cfg, Some(&root), "kit");
        let names = |v: &[Item]| v.iter().map(|i| i.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&c.skills), ["kit:alpha", "kit:beta", "kit:gamma"], "links skipped");
        assert_eq!(names(&c.commands), ["kit:build", "kit:ship"], "manifest paths join defaults; escapes dropped");
        assert_eq!(names(&c.agents), ["kit:code-reviewer", "kit:helper"], "manifest agents are .md files; a folder entry is ignored");
        assert!(c.hooks && !c.lsp && !c.unreadable);
        assert_eq!(c.skills[0].description.as_deref(), Some("alpha 설명"), "display_safe");
        assert!(!serde_json::to_string(&c).unwrap().contains("BODY_CANARY"));

        let mut plugins = build(&cfg, vec![Installed {
            id: "kit@mk".into(), version: Some("1.0.0".into()), scope: Scope::User, enabled: true, project_enabled: false,
            install_path: Some(root.clone()), installed_at: None, last_updated: None, mcp: vec![],
        }]);
        let used = Skill { name: "kit:beta".into(), source: SkillSource::Plugin, scope: None, kind: "skill", description: None, uses: 4, last_used: Some(9), defined: true, id: None, readonly: None, copied_from: None, cc: None, path: None };
        apply_usage(&mut plugins, &[used]);
        assert_eq!(plugins[0].components.skills[1].uses, 4);
    }

    /// mattpocock-skills 1.3.2 shape: skills grouped in category folders, each skill folder listed in the manifest
    /// (27 of them). Both the plugins tab and the skills tab must show them (D-162).
    #[test]
    fn manifest_may_list_skill_folders_directly_and_more_than_twenty() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let root = cfg.join("plugins/cache/mk/kit/1.0.0");
        let listed: Vec<String> = (0..27).map(|i| format!("s{i:02}")).collect();
        let paths: Vec<String> = listed.iter().map(|s| format!("\"./skills/engineering/{s}\"")).collect();
        write(&root.join(".claude-plugin/plugin.json"), &format!(r#"{{"name":"kit","skills":[{}]}}"#, paths.join(",")));
        for s in &listed {
            write(&root.join(format!("skills/engineering/{s}/SKILL.md")), &format!("---\nname: {s}\ndescription: d\n---\n"));
        }
        // Not listed in the manifest and not directly under skills/: Claude Code does not load it, neither do we.
        write(&root.join("skills/misc/unlisted/SKILL.md"), "---\nname: unlisted\n---\n");

        let c = components(&cfg, Some(&root), "kit");
        let names: Vec<String> = c.skills.iter().map(|i| i.name.clone()).collect();
        let want: Vec<String> = listed.iter().map(|s| format!("kit:{s}")).collect();
        assert_eq!(names, want);

        let catalog = crate::skills_catalog::discover(&cfg, &[]);
        let mut from_tab: Vec<String> = catalog.iter().filter(|s| s.source == SkillSource::Plugin).map(|s| s.name.clone()).collect();
        from_tab.sort();
        assert_eq!(from_tab, want, "the skills tab reads the manifest too");
    }

    #[test]
    fn skills_tab_reads_the_recorded_install_folder_not_the_newest() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let cache = cfg.join("plugins/cache/mk/kit");
        write(&cache.join("1.3.1/skills/old/SKILL.md"), "---\nname: old\n---\n");
        write(&cache.join("49dd158d1076/skills/new/SKILL.md"), "---\nname: new\n---\n");
        let at = |p: &Path, s: u64| std::fs::File::open(p).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s)).unwrap();
        at(&cache.join("1.3.1"), 1_000);
        at(&cache.join("49dd158d1076"), 2_000);
        let plugin_skills = || {
            crate::skills_catalog::discover(&cfg, &[]).into_iter().filter(|s| s.source == SkillSource::Plugin).map(|s| s.name).collect::<Vec<_>>()
        };
        assert_eq!(plugin_skills(), ["kit:new"], "no record: newest folder");
        write(&cfg.join("plugins/installed_plugins.json"), &format!(r#"{{"plugins":{{"kit@mk":[{{"installPath":"{}"}}]}}}}"#, cache.join("1.3.1").display()));
        assert_eq!(plugin_skills(), ["kit:old"], "the recorded install path wins");
    }

    #[cfg(unix)]
    #[test]
    fn install_path_must_be_a_real_folder_in_the_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(cfg.join("plugins/cache/mk")).unwrap();
        let outside = tmp.path().join("elsewhere/p/1.0");
        write(&outside.join("skills/s/SKILL.md"), "---\nname: s\n---\n");
        std::os::unix::fs::symlink(tmp.path().join("elsewhere/p"), cfg.join("plugins/cache/mk/p")).unwrap();
        assert!(components(&cfg, Some(&cfg.join("plugins/cache/mk/p/1.0")), "p").unreadable, "link into elsewhere");
        assert!(components(&cfg, Some(&outside), "p").unreadable, "outside the cache");
        assert!(components(&cfg, Some(&cfg.join("plugins/cache")), "p").unreadable, "the cache root itself");
        assert!(components(&cfg, None, "p").unreadable);
    }

    #[test]
    fn classifies_cli_results() {
        let shown = br#"{"ok":false,"shownCommand":{"command":"curl x","sha256":"ab12"}}"#;
        assert_eq!(classify(false, shown, b""), Outcome::NeedsApproval);
        assert_eq!(classify(true, shown, b""), Outcome::NeedsApproval, "approval wins even on exit 0");
        assert_eq!(classify(true, br#"{"ok":true}"#, b""), Outcome::Done);
        assert_eq!(classify(false, b"", b"Plugin is enforced by managed settings"), Outcome::Locked);
        assert_eq!(classify(false, b"", b"boom"), Outcome::Failed);
        assert_eq!(classify(false, b"", b"Plugin policy-kit@m not found"), Outcome::Failed, "a name with 'policy' is not a lock");
    }

    /// Fake CLI: records argv and stdin, answers like the real one would.
    #[cfg(unix)]
    #[test]
    fn cli_wrapper_passes_scope_and_never_auto_approves() {
        use crate::claude_cli::{ClaudeCli, CliError};
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("argv");
        let bin = tmp.path().join("claude");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{log}'\nif [ -t 0 ]; then echo TTY >> '{log}'; fi\ncat >/dev/null\n\
             case \"$2\" in\n\
               update) case \"$6\" in\n\
                   slow@m) sleep 20;;\n\
                   needs@m) echo '{{\"ok\":false,\"shownCommand\":{{\"command\":\"curl x\",\"sha256\":\"ab\"}}}}'; exit 1;;\n\
                   *) echo '{{\"ok\":true}}';;\n\
                 esac;;\n\
               enable) echo 'Error: plugin is enforced by managed settings' >&2; exit 1;;\n\
               disable) echo '{{\"ok\":true}}';;\n\
               list) echo '[]';;\n\
             esac\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cli = ClaudeCli::new(bin, None);
        let home = tmp.path();
        assert_eq!(cli.plugin_action(home, Action::Update, "kit@m", Scope::User).unwrap(), Outcome::Done);
        assert_eq!(cli.plugin_action(home, Action::Update, "needs@m", Scope::User).unwrap(), Outcome::NeedsApproval);
        assert_eq!(cli.plugin_action(home, Action::Enable, "kit@m", Scope::Project).unwrap(), Outcome::Locked);
        assert_eq!(cli.plugin_action(home, Action::Disable, "kit@m", Scope::Local).unwrap(), Outcome::Done);
        assert!(matches!(cli.plugin_action(home, Action::Disable, "--yes@m", Scope::User), Err(CliError::InvalidInput { .. })));
        assert!(matches!(cli.plugin_action(home, Action::Disable, "kit@m", Scope::Managed), Err(CliError::InvalidInput { .. })));
        let argv = std::fs::read_to_string(&log).unwrap();
        assert!(argv.contains("plugin\nupdate\n--json\n-s\nuser\nkit@m\n"), "{argv}");
        assert!(argv.contains("plugin\nenable\n--json\n-s\nproject\nkit@m\n"));
        assert!(argv.contains("plugin\ndisable\n--json\n-s\nlocal\nkit@m\n"));
        assert!(!argv.lines().any(|l| l == "-y" || l == "--yes" || l.starts_with("--accept-command")), "{argv}");
        assert!(!argv.contains("TTY") && !argv.contains("--yes@m"), "stdin closed; invalid id never spawned");
        assert_eq!(cli.plugin_list_raw(home).unwrap(), b"[]\n");
        let t = std::time::Instant::now();
        assert_eq!(cli.plugin_action_within(home, Action::Update, "slow@m", Scope::User, std::time::Duration::from_millis(500)).unwrap(), Outcome::Timeout);
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "process group killed on timeout");
    }
}
