//! "링크로 설치" (install-from-link, D-085, D-087). GitHub only: a link is parsed here and rebuilt from validated
//! parts, previews are read over `http` (two allow-listed hosts), and a skill is downloaded once, pinned to the
//! commit SHA seen at preview time, into an app-data staging folder that the shell keeps behind an opaque token.
//! Installing moves exactly those bytes into `<config>/skills/<name>` with an exclusive rename — never overwriting.
//! Nothing here runs downloaded code or sets an executable bit.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::agents_catalog::{parse_frontmatter, split};
use crate::http::{self, HttpError, Transport};
use crate::text::{display_safe, is_unsafe_char};

/// Plugin origin codes in a marketplace preview (the webview words them).
pub const ORIGIN_IN_REPO: &str = "in_repo";
pub const ORIGIN_UNKNOWN: &str = "unknown";

pub const MAX_FILES: usize = 200;
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_DEPTH: usize = 4;
const MAX_INPUT: usize = 2048;
const MAX_TREE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MARKETPLACE_BYTES: u64 = 1024 * 1024;

fn re(s: &str) -> Regex {
    Regex::new(s).unwrap_or_else(|_| unreachable!())
}
static OWNER_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9][A-Za-z0-9-]{0,38}\z"));
static REPO_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9._-]{1,100}\z"));
static SHORT_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A([A-Za-z0-9][A-Za-z0-9-]{0,38})/([A-Za-z0-9._-]{1,100})\z"));
static REF_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9._-]{1,200}\z"));
static PART_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9_][A-Za-z0-9._-]{0,99}\z"));
static MARKET_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9][A-Za-z0-9._-]{0,99}\z"));
static SKILL_NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9][A-Za-z0-9_-]{0,63}\z"));
static SHA_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[0-9a-f]{40}\z"));

pub const UNSUPPORTED: crate::i18n::Fail = crate::i18n::Fail::LinkUnsupported;

/// A name usable as `<config>/skills/<name>` (same rule as skill-authoring).
pub fn valid_skill_name(n: &str) -> bool {
    SKILL_NAME_RE.is_match(n)
}

/// One file or folder name inside a skill: ASCII letters, digits, `. _ -`, not starting with a dot.
pub fn valid_part(p: &str) -> bool {
    PART_RE.is_match(p) && p != ".."
}

pub fn valid_market(n: &str) -> bool {
    MARKET_RE.is_match(n) && n != "." && n != ".."
}

// ---------- parsing ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// `name@marketplace` — the marketplace must already be added.
    Plugin { id: String },
    /// A GitHub repository, optionally a ref and a folder inside it (`""` = repository root).
    GitHub { owner: String, repo: String, git_ref: Option<String>, path: String },
}

impl Link {
    /// Display form rebuilt from validated parts.
    pub fn source(&self) -> String {
        match self {
            Link::Plugin { id } => id.clone(),
            Link::GitHub { owner, repo, git_ref, path } => {
                let mut s = format!("github.com/{owner}/{repo}");
                if let Some(r) = git_ref {
                    s.push_str(&format!(" @ {r}"));
                }
                if !path.is_empty() {
                    s.push_str(&format!(" / {path}"));
                }
                s
            }
        }
    }
}

fn repo_ok(owner: &str, repo: &str) -> bool {
    OWNER_RE.is_match(owner) && REPO_RE.is_match(repo) && repo != "." && repo != ".."
}

fn folder(parts: &[String]) -> Result<String, &'static str> {
    if parts.len() > 10 {
        return Err(crate::i18n::Fail::DirTooDeep.str());
    }
    if parts.iter().any(|p| !PART_RE.is_match(p) || p == "..") {
        return Err(crate::i18n::Fail::DirNameChars.str());
    }
    Ok(parts.join("/"))
}

/// A link to a file: only `…/SKILL.md` (its folder) and the root `.claude-plugin/marketplace.json` (the repository).
fn file_target(parts: &[String]) -> Result<String, &'static str> {
    match parts {
        [rest @ .., last] if last == "SKILL.md" => folder(rest),
        [rest @ .., a, b] if a == ".claude-plugin" && b == "marketplace.json" => {
            if rest.is_empty() { Ok(String::new()) } else { Err(crate::i18n::Fail::MarketSubdir.str()) }
        }
        _ => Err(crate::i18n::Fail::FileLinkKinds.str()),
    }
}

pub fn parse_link(input: &str) -> Result<Link, &'static str> {
    let s = input.trim();
    if s.is_empty() || s.len() > MAX_INPUT || s.chars().any(|c| c.is_whitespace() || is_unsafe_char(c)) {
        return Err(UNSUPPORTED.str());
    }
    if crate::plugins::valid_id(s) {
        // `git@github.com`, `user@10.0.0.1`: an SSH target, not `plugin@marketplace`.
        if s.split_once('@').is_some_and(|(_, m)| m.contains('.')) {
            return Err(UNSUPPORTED.str());
        }
        return Ok(Link::Plugin { id: s.to_string() });
    }
    if let Some(c) = SHORT_RE.captures(s) {
        let (owner, repo) = (c[1].to_string(), c[2].trim_end_matches(".git").to_string());
        if !repo_ok(&owner, &repo) {
            return Err(UNSUPPORTED.str());
        }
        return Ok(Link::GitHub { owner, repo, git_ref: None, path: String::new() });
    }
    // `..`/`.` segments would be normalized away by the URL parser; refuse them outright.
    if !s.starts_with("https://") || s.contains("/../") || s.contains("/./") || s.ends_with("/..") || s.ends_with("/.") {
        return Err(UNSUPPORTED.str());
    }
    let url = Url::parse(s).map_err(|_| UNSUPPORTED.str())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err(UNSUPPORTED.str());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let parts: Vec<String> = url.path_segments().map(|p| p.filter(|x| !x.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    let (owner, repo, git_ref, path) = match host.as_str() {
        "github.com" | "www.github.com" => match parts.as_slice() {
            [o, r] => (o.clone(), r.trim_end_matches(".git").to_string(), None, String::new()),
            [o, r, kind, gref, rest @ ..] if kind == "tree" => (o.clone(), r.clone(), Some(gref.clone()), folder(rest)?),
            [o, r, kind, gref, rest @ ..] if kind == "blob" => (o.clone(), r.clone(), Some(gref.clone()), file_target(rest)?),
            _ => return Err(crate::i18n::Fail::LinkKinds.str()),
        },
        "raw.githubusercontent.com" => match parts.as_slice() {
            [o, r, gref, rest @ ..] if !rest.is_empty() => (o.clone(), r.clone(), Some(gref.clone()), file_target(rest)?),
            _ => return Err(UNSUPPORTED.str()),
        },
        _ => return Err(crate::i18n::Fail::GithubOnly.str()),
    };
    if !repo_ok(&owner, &repo) || git_ref.as_deref().is_some_and(|r| !REF_RE.is_match(r) || r == "." || r == "..") {
        return Err(UNSUPPORTED.str());
    }
    Ok(Link::GitHub { owner, repo, git_ref, path })
}

// ---------- GitHub reads ----------

fn api(path_parts: &[&str], query: Option<&str>) -> Url {
    let mut u = Url::parse("https://api.github.com/").unwrap_or_else(|_| unreachable!());
    if let Ok(mut seg) = u.path_segments_mut() {
        seg.clear().extend(path_parts);
    }
    u.set_query(query);
    u
}

fn raw(owner: &str, repo: &str, sha: &str, rel: &str) -> Url {
    let mut u = Url::parse("https://raw.githubusercontent.com/").unwrap_or_else(|_| unreachable!());
    if let Ok(mut seg) = u.path_segments_mut() {
        seg.clear().extend([owner, repo, sha]).extend(rel.split('/'));
    }
    u
}

fn net(e: HttpError) -> String {
    e.to_string()
}

fn status_error(status: u16) -> String {
    match status {
        403 | 429 => crate::i18n::Fail::GithubRate.str().into(),
        404 => crate::i18n::Fail::RepoNotFound.str().into(),
        422 => crate::i18n::Fail::BranchUnknown.str().into(),
        s => crate::tr_fmt!(crate::i18n::Fail::GithubError, s = s),
    }
}

/// Commit SHA for a ref (`HEAD` = default branch): one API call.
pub fn resolve_sha(t: &dyn Transport, owner: &str, repo: &str, git_ref: &str) -> Result<String, String> {
    let r = http::get(t, &api(&["repos", owner, repo, "commits", git_ref], None), Some("application/vnd.github.sha"), 200).map_err(net)?;
    if r.status != 200 {
        return Err(status_error(r.status));
    }
    let sha = String::from_utf8_lossy(&r.body).trim().to_ascii_lowercase();
    if SHA_RE.is_match(&sha) { Ok(sha) } else { Err(crate::i18n::Fail::GithubShape.str().into()) }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize)]
struct Tree {
    tree: Vec<TreeEntry>,
    #[serde(default)]
    truncated: bool,
}

/// The whole file list at `sha`: one API call.
pub fn tree(t: &dyn Transport, owner: &str, repo: &str, sha: &str) -> Result<Vec<TreeEntry>, String> {
    let r = http::get(t, &api(&["repos", owner, repo, "git", "trees", sha], Some("recursive=1")), Some("application/vnd.github+json"), MAX_TREE_BYTES).map_err(net)?;
    if r.status != 200 {
        return Err(status_error(r.status));
    }
    let tree: Tree = serde_json::from_slice(&r.body).map_err(|_| crate::i18n::Fail::GithubShape.str().to_string())?;
    if tree.truncated {
        return Err(crate::i18n::Fail::RepoTooBig.str().into());
    }
    Ok(tree.tree)
}

// ---------- marketplace preview ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketPlugin {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    /// Where the plugin's code comes from: `in_repo`, `unknown`, or `github`, `url`, `npm`…
    pub origin: String,
    /// Declared inline in the catalog entry (hooks, MCP, LSP).
    pub declares: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketPreview {
    pub market: String,
    pub description: Option<String>,
    pub plugins: Vec<MarketPlugin>,
    /// Entries dropped because their name is not a valid plugin id part.
    pub skipped: usize,
}

pub fn parse_marketplace(bytes: &[u8]) -> Result<MarketPreview, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| crate::i18n::Fail::MarketUnreadable.str().to_string())?;
    let market = v.get("name").and_then(|n| n.as_str()).filter(|n| valid_market(n)).ok_or(crate::i18n::Fail::MarketNameBad.str())?;
    let desc = v.get("metadata").and_then(|m| m.get("description")).or_else(|| v.get("description")).and_then(|d| d.as_str()).map(|d| display_safe(d, 300));
    let list = v.get("plugins").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    let mut plugins = Vec::new();
    let mut skipped = 0;
    for e in list.iter().take(300) {
        let Some(name) = e.get("name").and_then(|n| n.as_str()).filter(|n| valid_market(n)) else {
            skipped += 1;
            continue;
        };
        let origin = match e.get("source") {
            // Fixed codes (app-language): `in_repo`, `unknown`, else the source kind as written (`github`, `url`…).
            Some(serde_json::Value::String(_)) => ORIGIN_IN_REPO.to_string(),
            Some(o) => o.get("source").and_then(|s| s.as_str()).map(|s| display_safe(s, 20)).unwrap_or_else(|| ORIGIN_UNKNOWN.into()),
            None => ORIGIN_UNKNOWN.into(),
        };
        let declares = [("hooks", crate::i18n::Fail::Hooks.str()), ("mcpServers", "MCP"), ("lspServers", "LSP")].into_iter().filter(|(k, _)| e.get(*k).is_some()).map(|(_, l)| l).collect();
        plugins.push(MarketPlugin {
            name: name.to_string(),
            description: e.get("description").and_then(|d| d.as_str()).map(|d| display_safe(d, 300)),
            version: e.get("version").and_then(|d| d.as_str()).map(|d| display_safe(d, 40)),
            origin,
            declares,
        });
    }
    skipped += list.len().saturating_sub(300);
    Ok(MarketPreview { market: market.to_string(), description: desc, plugins, skipped })
}

/// `<config>/plugins/known_marketplaces.json`: name → GitHub `owner/repo` (or the git URL), read-only.
pub fn known_marketplaces(config_dir: &Path) -> Vec<(String, Option<String>)> {
    use std::io::Read;
    let path = config_dir.join("plugins").join("known_marketplaces.json");
    let ok = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
    let mut buf = Vec::new();
    if !ok || std::fs::File::open(&path).and_then(|f| f.take(4 * 1024 * 1024).read_to_end(&mut buf)).is_err() {
        return Vec::new();
    }
    let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(&buf) else { return Vec::new() };
    map.into_iter()
        .map(|(name, v)| {
            let src = v.get("source");
            let repo = src.and_then(|s| s.get("repo")).and_then(|r| r.as_str()).map(str::to_string).or_else(|| {
                let url = src.and_then(|s| s.get("url")).and_then(|u| u.as_str())?;
                let rest = url.strip_prefix("https://github.com/")?;
                Some(rest.trim_end_matches('/').trim_end_matches(".git").to_string())
            });
            (name, repo)
        })
        .collect()
}

/// What a marketplace name already maps to: `None` = not added, `Some(true)` = this repository, `Some(false)` =
/// a different source under the same name.
pub fn market_state(config_dir: &Path, market: &str, owner: &str, repo: &str) -> Option<bool> {
    known_marketplaces(config_dir)
        .into_iter()
        .find(|(n, _)| n == market)
        .map(|(_, r)| r.is_some_and(|r| r.eq_ignore_ascii_case(&format!("{owner}/{repo}"))))
}

/// The plugin's catalog entry from Claude Code's local marketplace clone (read-only), for `name@market` input.
pub fn local_plugin(config_dir: &Path, id: &str) -> Option<MarketPlugin> {
    use std::io::Read;
    let (name, market) = id.split_once('@')?;
    let path = config_dir.join("plugins").join("marketplaces").join(market).join(".claude-plugin").join("marketplace.json");
    if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
        return None;
    }
    let mut buf = Vec::new();
    std::fs::File::open(&path).ok()?.take(MAX_MARKETPLACE_BYTES * 4).read_to_end(&mut buf).ok()?;
    parse_marketplace(&buf).ok()?.plugins.into_iter().find(|p| p.name == name)
}

// ---------- skill preview, download and install ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFile {
    pub path: String,
    pub size: u64,
    /// Marked only (mode 100755, script extension or no extension); the bit is never set on disk.
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPreview {
    pub name: String,
    pub description: Option<String>,
    pub files: Vec<SkillFile>,
    pub executables: usize,
    pub total_bytes: u64,
    /// Dot-named files and folders that are left out (never downloaded).
    pub skipped: Vec<String>,
    /// SKILL.md in full, invisible and bidi characters shown as `⟦U+XXXX⟧`.
    pub skill_md: String,
    pub risk: SkillRisk,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRisk {
    /// `allowed-tools` exactly as written (display-safe), if present.
    pub allowed_tools: Option<String>,
    pub hooks: bool,
    /// Lines that run shell commands when the skill loads (`` !`cmd` ``).
    pub shell_lines: usize,
    /// Bash or a wildcard in allowed-tools, or hooks: shown in red.
    pub danger: bool,
}

/// Text for review: keeps newlines and tabs, marks every other unsafe character so nothing hides.
pub fn display_marked(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for c in text.chars().take(max_chars) {
        if c == '\n' || c == '\t' || !is_unsafe_char(c) {
            out.push(c);
        } else if c != '\r' {
            out.push_str(&format!("⟦U+{:04X}⟧", c as u32));
        }
    }
    out
}

/// Permission-relevant parts of a SKILL.md (or command .md): `allowed-tools`, `hooks`, `` !` `` shell lines.
pub fn skill_risk(content: &str) -> SkillRisk {
    let (fm_lines, body) = split(content).unwrap_or_default();
    // Raw line scan, not the flat reader: quoted keys (`'allowed-tools':`), folded/block scalars (`>`, `|`), flow and
    // block lists all count, and every continuation line of the value is read (QA MEDIUM).
    let key_of = |l: &str| -> Option<String> {
        if l.starts_with([' ', '\t', '-']) {
            return None;
        }
        let (k, _) = l.split_once(':')?;
        Some(k.trim().trim_matches(|c| c == '\'' || c == '"').to_string())
    };
    let mut allowed: Option<String> = None;
    let mut hooks = false;
    let mut idx = 0;
    while idx < fm_lines.len() {
        let line = &fm_lines[idx];
        match key_of(line).as_deref() {
            Some("allowed-tools" | "allowed_tools") => {
                let mut parts = vec![line.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or_default()];
                while idx + 1 < fm_lines.len() && (fm_lines[idx + 1].starts_with([' ', '\t', '-']) || fm_lines[idx + 1].trim().is_empty()) {
                    idx += 1;
                    parts.push(fm_lines[idx].trim().trim_start_matches('-').trim().to_string());
                }
                let text = parts.into_iter().filter(|p| !p.is_empty() && p != ">" && p != "|" && p != ">-" && p != "|-").collect::<Vec<_>>().join(", ");
                allowed = Some(display_safe(&text, 300)).filter(|t| !t.is_empty()).or(allowed);
            }
            Some("hooks") => hooks = true,
            _ => {}
        }
        idx += 1;
    }
    // Belt and braces: the word anywhere in the frontmatter still counts as hooks.
    hooks = hooks || fm_lines.iter().any(|l| l.contains("hooks"));
    let shell_lines = body.lines().filter(|l| l.contains("!`") || l.trim_start().starts_with("```!")).count();
    // Red is kept for what runs on its own (hooks); tool grants and `!` lines are amber (design review).
    SkillRisk { allowed_tools: allowed, hooks, shell_lines, danger: hooks }
}

fn executable(path: &str, mode: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    let ext = file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    mode == "100755"
        || match ext.as_deref() {
            None => true,
            Some(e) => ["sh", "bash", "zsh", "fish", "py", "js", "mjs", "cjs", "ts", "rb", "pl", "php", "ps1", "bat", "cmd", "exe", "bin", "command", "app", "jar"].contains(&e),
        }
}

#[derive(Debug, Clone)]
struct Planned {
    rel: String,
    full: String,
    executable: bool,
}

/// Picks the files of the skill folder at `path` from the tree and applies every name and size rule.
fn plan_files(entries: &[TreeEntry], path: &str) -> Result<(Vec<Planned>, Vec<String>), String> {
    let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
    let mut planned = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = HashSet::new();
    let mut reported: u64 = 0;
    for e in entries.iter().filter(|e| e.path.starts_with(&prefix)) {
        let rel = &e.path[prefix.len()..];
        if rel.is_empty() {
            continue;
        }
        let parts: Vec<&str> = rel.split('/').collect();
        if let Some(dot) = parts.iter().position(|p| p.starts_with('.')) {
            let shown = parts[..=dot].join("/");
            if !skipped.contains(&shown) {
                skipped.push(shown);
            }
            continue;
        }
        if e.kind == "commit" || e.mode == "160000" {
            return Err(crate::tr_fmt!(crate::i18n::Fail::HasSubmodule, a0 = display_safe(rel, 100)));
        }
        if e.mode == "120000" {
            return Err(crate::tr_fmt!(crate::i18n::Fail::HasSymlink, a0 = display_safe(rel, 100)));
        }
        if let Some(bad) = parts.iter().find(|p| !PART_RE.is_match(p)) {
            return Err(crate::tr_fmt!(crate::i18n::Fail::NameChars, a0 = display_safe(bad, 100)));
        }
        if parts.len() > MAX_DEPTH {
            return Err(crate::tr_fmt!(crate::i18n::Fail::DirDepthMax, MAX_DEPTH = MAX_DEPTH));
        }
        if !seen.insert(rel.to_ascii_lowercase()) {
            return Err(crate::tr_fmt!(crate::i18n::Fail::CaseClash, a0 = display_safe(rel, 100)));
        }
        if e.kind != "blob" {
            continue;
        }
        if e.mode != "100644" && e.mode != "100755" {
            return Err(crate::tr_fmt!(crate::i18n::Fail::FileKind, a0 = display_safe(rel, 100)));
        }
        let size = e.size.unwrap_or(0);
        if size > MAX_FILE_BYTES {
            return Err(crate::tr_fmt!(crate::i18n::Fail::FileOver1Mb, a0 = display_safe(rel, 100)));
        }
        reported += size;
        planned.push(Planned { rel: rel.to_string(), full: e.path.clone(), executable: executable(rel, &e.mode) });
    }
    if planned.len() > MAX_FILES {
        return Err(crate::tr_fmt!(crate::i18n::Fail::FilesMax, MAX_FILES = MAX_FILES));
    }
    if reported > MAX_TOTAL_BYTES {
        return Err(crate::i18n::Fail::FilesOver5Mb.str().into());
    }
    if !planned.iter().any(|p| p.rel == "SKILL.md") {
        return Err(crate::i18n::Fail::NoSkillMd.str().into());
    }
    Ok((planned, skipped))
}

fn mkdir_private(p: &Path) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(p)
}

/// New file, never replacing anything; 0644 — the executable bit is never set.
fn write_new(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o644);
    let mut f = o.open(p)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Downloads the skill folder `path` at `sha` into `stage` (created here, 0700; `stage/payload` holds the files).
/// Limits apply to the bytes actually received. On any error the caller removes `stage`.
pub fn stage_skill(t: &dyn Transport, owner: &str, repo: &str, sha: &str, path: &str, entries: &[TreeEntry], stage: &Path) -> Result<SkillPreview, String> {
    let (planned, skipped) = plan_files(entries, path)?;
    mkdir_private(stage).map_err(|_| crate::i18n::Fail::TempDirFailed.str().to_string())?;
    let payload = stage.join("payload");
    mkdir_private(&payload).map_err(|_| crate::i18n::Fail::TempDirFailed.str().to_string())?;
    let mut total: u64 = 0;
    let mut files = Vec::new();
    let mut skill_md = Vec::new();
    for p in &planned {
        let r = http::get(t, &raw(owner, repo, sha, &p.full), None, MAX_FILE_BYTES).map_err(|e| match e {
            HttpError::TooLarge => crate::tr_fmt!(crate::i18n::Fail::FileOver1Mb, a0 = display_safe(&p.rel, 100)),
            other => other.to_string(),
        })?;
        if r.status != 200 {
            return Err(crate::tr_fmt!(crate::i18n::Fail::FetchFailed, a0 = r.status, a1 = display_safe(&p.rel, 100)));
        }
        total += r.body.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(crate::i18n::Fail::FilesOver5Mb.str().into());
        }
        let mut dest = payload.clone();
        let parts: Vec<&str> = p.rel.split('/').collect();
        for d in &parts[..parts.len() - 1] {
            dest.push(d);
            if !std::fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_dir()) {
                mkdir_private(&dest).map_err(|_| crate::i18n::Fail::TempDirFailed.str().to_string())?;
            }
        }
        dest.push(parts[parts.len() - 1]);
        write_new(&dest, &r.body).map_err(|_| crate::i18n::Fail::SaveFileFailed.str().to_string())?;
        if p.rel == "SKILL.md" {
            skill_md = r.body.clone();
        }
        files.push(SkillFile { path: p.rel.clone(), size: r.body.len() as u64, executable: p.executable });
    }
    let text = String::from_utf8(skill_md).map_err(|_| crate::i18n::Fail::SkillNotUtf8.str().to_string())?;
    let fm = parse_frontmatter(&text);
    let folder_name = if path.is_empty() { repo } else { path.rsplit('/').next().unwrap_or(path) };
    let name = fm
        .as_ref()
        .and_then(|f| f.get("name"))
        .filter(|n| valid_skill_name(n))
        .or(Some(folder_name).filter(|n| valid_skill_name(n)))
        .ok_or(crate::i18n::Fail::SkillNameFormat.str())?
        .to_string();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(SkillPreview {
        name,
        description: fm.as_ref().and_then(|f| f.get("description")).map(|d| display_safe(d, 500)),
        executables: files.iter().filter(|f| f.executable).count(),
        total_bytes: total,
        files,
        skipped: skipped.into_iter().map(|s| display_safe(&s, 100)).collect(),
        skill_md: display_marked(&text, 200_000),
        risk: skill_risk(&text),
    })
}

/// A real directory at `dir` (created 0700 if missing; its parent must exist). Links are refused.
pub fn ensure_real_dir(dir: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_dir() => Ok(()),
        Ok(_) => Err(crate::i18n::Fail::InstallDirBad.str().into()),
        Err(_) => mkdir_private(dir).map_err(|_| crate::i18n::Fail::InstallDirFailed.str().to_string()),
    }
}

/// A folder in the user's own config (`<config>/skills`): links are followed here, because dotfile managers often
/// link `~/.claude` or `~/.claude/skills` (QA MEDIUM). The resolved folder must be a directory; created 0700 under
/// the resolved config dir when missing. Project folders keep refusing links (see `authoring`).
pub fn user_dir(dir: &Path) -> Result<PathBuf, String> {
    if let Ok(real) = dir.canonicalize() {
        return if real.is_dir() { Ok(real) } else { Err(crate::i18n::Fail::InstallDirNotDir.str().into()) };
    }
    let parent = dir.parent().and_then(|p| p.canonicalize().ok()).filter(|p| p.is_dir()).ok_or(crate::i18n::Fail::ConfigDirMissing.str())?;
    let real = parent.join(dir.file_name().ok_or(crate::i18n::Fail::PathError.str())?);
    ensure_real_dir(&real)?;
    Ok(real)
}

/// Renames `from` to `to` only if `to` does not exist (macOS `renamex_np(RENAME_EXCL)`; same volume only).
pub fn rename_exclusive(from: &Path, to: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let a = std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(|_| crate::i18n::Fail::BadPath.str().to_string())?;
        let b = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(|_| crate::i18n::Fail::BadPath.str().to_string())?;
        // SAFETY: both pointers are valid NUL-terminated strings for the duration of the call.
        let rc = unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_EXCL) };
        if rc == 0 {
            return Ok(());
        }
        Err(match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EEXIST) | Some(libc::ENOTEMPTY) => crate::i18n::Fail::SkillExists.str().into(),
            Some(libc::EXDEV) => crate::i18n::Fail::CrossDisk.str().into(),
            _ => crate::i18n::Fail::InstallFailed.str().into(),
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(crate::i18n::Fail::SkillExists.str().into());
        }
        std::fs::rename(from, to).map_err(|_| crate::i18n::Fail::InstallFailed.str().to_string())
    }
}

/// Installs a staged skill as `<skills_root>/<name>`. Never overwrites.
pub fn install_staged(stage: &Path, skills_root: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_skill_name(name) {
        return Err(crate::i18n::Fail::SkillNameBad.str().into());
    }
    let payload = stage.join("payload");
    if !std::fs::symlink_metadata(&payload).is_ok_and(|m| m.file_type().is_dir()) {
        return Err(crate::i18n::Fail::PreviewExpired.str().into());
    }
    let skills_root = &user_dir(skills_root)?;
    let target = skills_root.join(name);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(crate::i18n::Fail::SkillExists.str().into());
    }
    rename_exclusive(&payload, &target)?;
    Ok(target)
}

/// 32 hex characters from the OS random source.
pub fn random_token() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_ok();
    if !ok {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let h = crate::agents_catalog::token_of(format!("{n}-{}-{:p}", std::process::id(), &b).as_bytes());
        return h[..32].to_string();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn valid_token(t: &str) -> bool {
    t.len() == 32 && t.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

// ---------- inspect ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Preview {
    Marketplace {
        source: String,
        sha: String,
        #[serde(flatten)]
        market: MarketPreview,
        /// `None` = not added yet; `Some(true)` = already added from this repository.
        already_added: Option<bool>,
    },
    Plugin {
        id: String,
        plugin: Option<MarketPlugin>,
    },
    Skill {
        source: String,
        sha: String,
        #[serde(flatten)]
        skill: SkillPreview,
    },
}

/// Resolves a GitHub link: marketplace (repository root) or skill folder. `stage` is used only for a skill.
pub fn inspect_github(t: &dyn Transport, config_dir: &Path, owner: &str, repo: &str, git_ref: Option<&str>, path: &str, stage: &Path) -> Result<Preview, String> {
    let sha = resolve_sha(t, owner, repo, git_ref.unwrap_or("HEAD"))?;
    let source = Link::GitHub { owner: owner.into(), repo: repo.into(), git_ref: git_ref.map(str::to_string), path: path.into() }.source();
    if path.is_empty() {
        let r = http::get(t, &raw(owner, repo, &sha, ".claude-plugin/marketplace.json"), None, MAX_MARKETPLACE_BYTES).map_err(net)?;
        if r.status == 200 {
            let market = parse_marketplace(&r.body)?;
            let already_added = market_state(config_dir, &market.market, owner, repo);
            if already_added == Some(false) {
                return Err(crate::tr_fmt!(crate::i18n::Fail::MarketNameClash, a0 = market.market));
            }
            return Ok(Preview::Marketplace { source, sha, market, already_added });
        }
        if r.status != 404 {
            return Err(status_error(r.status));
        }
    }
    let entries = tree(t, owner, repo, &sha)?;
    let has = |p: &str| entries.iter().any(|e| e.path == p && e.kind == "blob");
    let at = |f: &str| if path.is_empty() { f.to_string() } else { format!("{path}/{f}") };
    if !has(&at("SKILL.md")) {
        if !path.is_empty() && has(&at(".claude-plugin/marketplace.json")) {
            return Err(crate::i18n::Fail::MarketSubdir.str().into());
        }
        return Err(crate::i18n::Fail::NothingFound.str().into());
    }
    let skill = stage_skill(t, owner, repo, &sha, path, &entries, stage)?;
    Ok(Preview::Skill { source, sha, skill })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::http::fake::Fake;

    const SHA: &str = "2c5958ecac7cee2b583967a2c4981d0f4a75c579";

    #[test]
    fn link_forms() {
        let gh = |o: &str, r: &str, g: Option<&str>, p: &str| Link::GitHub { owner: o.into(), repo: r.into(), git_ref: g.map(String::from), path: p.into() };
        assert_eq!(parse_link("anthropics/skills").unwrap(), gh("anthropics", "skills", None, ""));
        assert_eq!(parse_link(" https://github.com/anthropics/skills.git ").unwrap(), gh("anthropics", "skills", None, ""));
        assert_eq!(parse_link("https://github.com/anthropics/skills/").unwrap(), gh("anthropics", "skills", None, ""));
        assert_eq!(parse_link("https://github.com/anthropics/skills/tree/main/skills/pdf").unwrap(), gh("anthropics", "skills", Some("main"), "skills/pdf"));
        assert_eq!(parse_link("https://github.com/anthropics/skills/blob/main/skills/pdf/SKILL.md").unwrap(), gh("anthropics", "skills", Some("main"), "skills/pdf"));
        assert_eq!(parse_link("https://raw.githubusercontent.com/o/r/abc/.claude-plugin/marketplace.json").unwrap(), gh("o", "r", Some("abc"), ""));
        assert_eq!(parse_link("https://raw.githubusercontent.com/o/r/main/x/SKILL.md?token=1#f").unwrap(), gh("o", "r", Some("main"), "x"));
        assert_eq!(parse_link("expo@claude-plugins-official").unwrap(), Link::Plugin { id: "expo@claude-plugins-official".into() });
        for bad in [
            "curl -fsSL https://x.sh | sh", "http://github.com/o/r", "git@github.com:o/r.git", "ssh://github.com/o/r", "file:///etc", "ext::sh -c x",
            "/Users/me/skill", "./skill", "-y", "--accept-command", "https://user:tok@github.com/o/r", "https://github.com:8443/o/r",
            "https://gitlab.com/o/r", "https://127.0.0.1/o/r", "https://localhost/o/r", "https://github.com/o", "https://github.com/o/r/issues/1",
            "https://github.com/o/r/tree/main/../x", "https://github.com/o/r/tree/main/.git", "https://github.com/o/r/blob/main/x/README.md",
            "https://github.com/o/r/tree/main/sub/.claude-plugin", "https://github.com/o/r/blob/main/sub/.claude-plugin/marketplace.json",
            "o/r\u{202E}", "", "-o/r", "git@github.com", "user@10.0.0.1", "me@host.example.com", "https://github.com/-o/r", "https://github.com/o/r/tree/main/a%2F..%2Fb",
        ] {
            assert!(parse_link(bad).is_err(), "{bad}");
        }
        assert_eq!(gh("o", "r", Some("main"), "a/b").source(), "github.com/o/r @ main / a/b");
    }

    fn marketplace_json() -> &'static str {
        r#"{"name":"anthropic-agent-skills","metadata":{"description":"Example\u202e skills"},"plugins":[
            {"name":"document-skills","description":"Docs","source":"./","skills":["./skills/pdf"]},
            {"name":"remote","source":{"source":"github","repo":"x/y"},"hooks":{"PreToolUse":[]},"mcpServers":{}},
            {"name":"--evil"}]}"#
    }

    #[test]
    fn marketplace_preview_without_changing_anything() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = Fake::default();
        f.on("https://api.github.com/repos/anthropics/skills/commits/HEAD", 200, format!("{SHA}\n"))
            .on(&format!("https://raw.githubusercontent.com/anthropics/skills/{SHA}/.claude-plugin/marketplace.json"), 200, marketplace_json());
        let p = inspect_github(&f, tmp.path(), "anthropics", "skills", None, "", &tmp.path().join("stage")).unwrap();
        let Preview::Marketplace { market, sha, already_added, .. } = p else { panic!() };
        assert_eq!((market.market.as_str(), sha.as_str(), already_added), ("anthropic-agent-skills", SHA, None));
        assert_eq!(market.plugins.len(), 2);
        assert_eq!(market.skipped, 1);
        assert_eq!(market.description.as_deref(), Some("Example skills"));
        assert_eq!(market.plugins[1].declares, ["훅", "MCP"]);
        assert_eq!(market.plugins[1].origin, "github");
        assert_eq!(f.count("https://api.github.com/"), 1, "one API call");
        assert!(!tmp.path().join("stage").exists(), "nothing staged for a marketplace");

        // Same name, other repository → refused; same repository → already added.
        let pl = tmp.path().join("plugins");
        std::fs::create_dir_all(&pl).unwrap();
        std::fs::write(pl.join("known_marketplaces.json"), r#"{"anthropic-agent-skills":{"source":{"source":"github","repo":"someone/else"}}}"#).unwrap();
        assert!(inspect_github(&f, tmp.path(), "anthropics", "skills", None, "", &tmp.path().join("s2")).unwrap_err().contains("다른 마켓플레이스"));
        std::fs::write(pl.join("known_marketplaces.json"), r#"{"anthropic-agent-skills":{"source":{"source":"git","url":"https://github.com/Anthropics/skills.git"}}}"#).unwrap();
        let Preview::Marketplace { already_added, .. } = inspect_github(&f, tmp.path(), "anthropics", "skills", None, "", &tmp.path().join("s3")).unwrap() else { panic!() };
        assert_eq!(already_added, Some(true));
    }

    #[test]
    fn github_errors_are_explained() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = Fake::default();
        f.on("https://api.github.com/repos/o/r/commits/feature", 422, "{}").on("https://api.github.com/repos/o/limit/commits/HEAD", 403, "{}");
        let s = tmp.path().join("s");
        assert!(inspect_github(&f, tmp.path(), "o", "r", Some("feature"), "x", &s).unwrap_err().contains("브랜치"));
        assert!(inspect_github(&f, tmp.path(), "o", "limit", None, "", &s).unwrap_err().contains("한도"));
        assert!(inspect_github(&f, tmp.path(), "o", "missing", None, "", &s).unwrap_err().contains("찾을 수 없어요"));
    }

    fn entry(path: &str, mode: &str, kind: &str, size: u64) -> String {
        format!(r#"{{"path":"{path}","mode":"{mode}","type":"{kind}","size":{size},"sha":"x"}}"#)
    }

    fn skill_fake(entries: &[String], files: &[(&str, &str)]) -> Fake {
        let mut f = Fake::default();
        f.on("https://api.github.com/repos/o/r/commits/main", 200, SHA);
        f.on(&format!("https://api.github.com/repos/o/r/git/trees/{SHA}?recursive=1"), 200, format!(r#"{{"sha":"{SHA}","truncated":false,"tree":[{}]}}"#, entries.join(",")));
        for (p, body) in files {
            f.on(&format!("https://raw.githubusercontent.com/o/r/{SHA}/{p}"), 200, *body);
        }
        f
    }

    const SKILL: &str = "---\nname: pdf-tools\ndescription: PDF 다루기\u{202E}\nallowed-tools: Read, Bash(python:*)\n---\n\n# PDF\n\n현재 상태: !`git status`\n";

    #[test]
    fn skill_is_staged_pinned_and_installed_without_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let f = skill_fake(
            &[
                entry("skills/pdf", "040000", "tree", 0),
                entry("skills/pdf/SKILL.md", "100644", "blob", 10),
                entry("skills/pdf/scripts", "040000", "tree", 0),
                entry("skills/pdf/scripts/fill.py", "100755", "blob", 4),
                entry("skills/pdf/forms.md", "100644", "blob", 2),
                entry("skills/pdf/.DS_Store", "100644", "blob", 2),
                entry("skills/other/SKILL.md", "100644", "blob", 2),
            ],
            &[("skills/pdf/SKILL.md", SKILL), ("skills/pdf/scripts/fill.py", "print()"), ("skills/pdf/forms.md", "ok")],
        );
        let stage = tmp.path().join("stage");
        let Preview::Skill { skill, sha, source } = inspect_github(&f, tmp.path(), "o", "r", Some("main"), "skills/pdf", &stage).unwrap() else { panic!() };
        assert_eq!((skill.name.as_str(), sha.as_str()), ("pdf-tools", SHA));
        assert_eq!(source, "github.com/o/r @ main / skills/pdf");
        assert_eq!(skill.files.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(), ["SKILL.md", "forms.md", "scripts/fill.py"]);
        assert_eq!((skill.executables, skill.skipped.clone()), (1, vec![".DS_Store".to_string()]));
        assert!(!skill.risk.danger && skill.risk.shell_lines == 1 && skill.risk.allowed_tools.as_deref() == Some("Read, Bash(python:*)"));
        assert!(skill.skill_md.contains("⟦U+202E⟧"), "bidi marked, not hidden");
        assert_eq!(f.count("https://api.github.com/"), 2, "commit + tree");
        assert!(f.seen.lock().unwrap().iter().filter(|u| u.starts_with("https://raw")).all(|u| u.contains(SHA)), "raw pinned to the SHA");
        assert!(!f.seen.lock().unwrap().iter().any(|u| u.contains("DS_Store") || u.contains("other")));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(stage.join("payload/scripts/fill.py")).unwrap().permissions().mode();
            assert_eq!(m & 0o111, 0, "never executable");
            assert_eq!(std::fs::metadata(&stage).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let skills = tmp.path().join("cfg/skills");
        std::fs::create_dir_all(tmp.path().join("cfg")).unwrap();
        // Existing name: refused, nothing replaced.
        std::fs::create_dir_all(skills.join("pdf-tools")).unwrap();
        std::fs::write(skills.join("pdf-tools/SKILL.md"), "mine").unwrap();
        assert_eq!(install_staged(&stage, &skills, "pdf-tools").unwrap_err(), "이미 있는 스킬이에요");
        assert_eq!(std::fs::read_to_string(skills.join("pdf-tools/SKILL.md")).unwrap(), "mine");
        std::fs::remove_dir_all(skills.join("pdf-tools")).unwrap();
        let dest = install_staged(&stage, &skills, "pdf-tools").unwrap();
        assert_eq!(std::fs::read_to_string(dest.join("SKILL.md")).unwrap(), SKILL);
        assert!(install_staged(&stage, &skills, "pdf-tools").is_err(), "a token installs once");
        assert!(install_staged(&stage, &skills, "../x").is_err());
    }

    #[test]
    fn skill_rules_refuse_unsafe_trees() {
        let tmp = tempfile::tempdir().unwrap();
        let ok = entry("s/SKILL.md", "100644", "blob", 10);
        let cases: Vec<(Vec<String>, &str)> = vec![
            (vec![ok.clone(), entry("s/link", "120000", "blob", 3)], "심볼릭"),
            (vec![ok.clone(), entry("s/sub", "160000", "commit", 0)], "서브모듈"),
            (vec![ok.clone(), entry("s/a/b/c/d/e.md", "100644", "blob", 1)], "깊어요"),
            (vec![ok.clone(), entry("s/big.bin", "100644", "blob", MAX_FILE_BYTES + 1)], "1MB"),
            (vec![ok.clone(), entry("s/README.md", "100644", "blob", 1), entry("s/readme.md", "100644", "blob", 1)], "대소문자"),
            (vec![ok.clone(), entry("s/한글.md", "100644", "blob", 1)], "문자"),
            (vec![entry("s/notes.md", "100644", "blob", 1)], "찾지 못했어요"),
            ((0..=MAX_FILES).map(|i| entry(&format!("s/f{i}.md"), "100644", "blob", 1)).chain([ok.clone()]).collect(), "많아요"),
            ((0..6).map(|i| entry(&format!("s/f{i}.bin"), "100644", "blob", MAX_FILE_BYTES)).chain([ok.clone()]).collect(), "5MB"),
        ];
        for (entries, want) in cases {
            let f = skill_fake(&entries, &[("s/SKILL.md", SKILL)]);
            let err = inspect_github(&f, tmp.path(), "o", "r", Some("main"), "s", &tmp.path().join(format!("st-{want}"))).unwrap_err();
            assert!(err.contains(want), "{want}: {err}");
        }
        // The tree under-reports a size: the cap applies to bytes received.
        let big = "x".repeat(MAX_FILE_BYTES as usize + 10);
        let f = skill_fake(&[ok.clone(), entry("s/liar.md", "100644", "blob", 1)], &[("s/SKILL.md", SKILL), ("s/liar.md", &big)]);
        assert!(inspect_github(&f, tmp.path(), "o", "r", Some("main"), "s", &tmp.path().join("liar")).unwrap_err().contains("1MB"));
        // A subfolder marketplace is refused with its reason.
        let f = skill_fake(&[entry("sub/.claude-plugin/marketplace.json", "100644", "blob", 1)], &[]);
        assert!(inspect_github(&f, tmp.path(), "o", "r", Some("main"), "sub", &tmp.path().join("m")).unwrap_err().contains("하위 폴더"));
    }

    /// Fake CLI: records argv; `needs@m` asks for a marketplace command approval.
    #[cfg(unix)]
    #[test]
    fn cli_add_then_install_never_auto_approves() {
        use crate::claude_cli::{ClaudeCli, CliError};
        use crate::plugins::Outcome;
        use std::os::unix::fs::PermissionsExt;
        use std::time::Duration;
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("argv");
        let bin = tmp.path().join("claude");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{log}'\necho --- >> '{log}'\nif [ -t 0 ]; then echo TTY >> '{log}'; fi\n\
             case \"$2\" in\n\
               marketplace) echo '{{\"ok\":true,\"name\":\"m\"}}';;\n\
               install) case \"$6\" in\n\
                   needs@m) echo '{{\"ok\":false,\"shownCommand\":{{\"command\":\"curl x | sh\",\"sha256\":\"ab\"}}}}'; exit 1;;\n\
                   slow@m) sleep 20;;\n\
                   *) echo '{{\"ok\":true}}';;\n\
                 esac;;\n\
             esac\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cli = ClaudeCli::new(bin, None);
        let t = Duration::from_secs(5);
        assert_eq!(cli.marketplace_add(tmp.path(), "anthropics", "skills", t).unwrap(), Outcome::Done);
        assert_eq!(cli.plugin_install(tmp.path(), "kit@m", t).unwrap(), Outcome::Done);
        assert_eq!(cli.plugin_install(tmp.path(), "needs@m", t).unwrap(), Outcome::NeedsApproval);
        assert_eq!(cli.plugin_install(tmp.path(), "slow@m", Duration::from_millis(300)).unwrap(), Outcome::Timeout);
        assert!(matches!(cli.marketplace_add(tmp.path(), "-x", "r", t), Err(CliError::InvalidInput { .. })));
        assert!(matches!(cli.marketplace_add(tmp.path(), "o", "r/../x", t), Err(CliError::InvalidInput { .. })));
        assert!(matches!(cli.plugin_install(tmp.path(), "--yes@m", t), Err(CliError::InvalidInput { .. })));
        let argv = std::fs::read_to_string(&log).unwrap();
        assert!(argv.contains("plugin\nmarketplace\nadd\n--json\n--scope\nuser\nanthropics/skills\n---"), "{argv}");
        assert!(argv.contains("plugin\ninstall\n--json\n-s\nuser\nkit@m\n---"), "{argv}");
        assert!(!argv.lines().any(|l| l == "-y" || l == "--yes" || l.starts_with("--accept-command") || l.starts_with("--marketplace")), "{argv}");
        assert!(!argv.contains("TTY") && !argv.contains("-x"), "stdin closed; invalid input never spawned");
    }

    #[test]
    fn risk_markers_cannot_be_hidden() {
        for fm in [
            "allowed-tools: >\n  Bash(rm:*)",
            "'allowed-tools': Bash",
            "\"allowed-tools\": [Read, Bash]",
            "allowed-tools: |\n  Read\n  PowerShell",
            "allowed_tools:\n  - \"*\"",
        ] {
            let r = skill_risk(&format!("---\nname: a\n{fm}\ndescription: d\n---\nbody"));
            let t = r.allowed_tools.clone().unwrap_or_default();
            assert!(["Bash", "*", "PowerShell"].iter().any(|k| t.contains(k)), "{fm}: {r:?}");
            assert!(!r.danger, "tool grants are amber, not red");
        }
        let r = skill_risk("---\nname: a\n'hooks':\n  Stop: []\n---\n");
        assert!(r.hooks && r.danger);
    }

    #[cfg(unix)]
    #[test]
    fn linked_user_skills_folder_is_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("dotfiles/skills");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::create_dir_all(tmp.path().join("cfg")).unwrap();
        std::os::unix::fs::symlink(&real, tmp.path().join("cfg/skills")).unwrap();
        let stage = tmp.path().join("stage");
        std::fs::create_dir_all(stage.join("payload")).unwrap();
        std::fs::write(stage.join("payload/SKILL.md"), "x").unwrap();
        let dest = install_staged(&stage, &tmp.path().join("cfg/skills"), "s1").unwrap();
        assert!(real.join("s1/SKILL.md").is_file() && dest.starts_with(real.canonicalize().unwrap()));
        // A missing skills folder is created under a linked config dir.
        std::os::unix::fs::symlink(tmp.path().join("cfg"), tmp.path().join("cfglink")).unwrap();
        assert!(user_dir(&tmp.path().join("cfglink/commands")).unwrap().ends_with("cfg/commands"));
    }

    #[test]
    fn risk_markers() {
        let r = skill_risk("---\nname: a\nhooks:\n  PreToolUse: []\n---\nbody");
        assert!(r.hooks && r.danger);
        let r = skill_risk("---\nname: a\nallowed-tools: Read, Grep\n---\nbody");
        assert!(!r.danger && r.allowed_tools.as_deref() == Some("Read, Grep"));
        let r = skill_risk("---\nname: a\nallowed-tools:\n  - Bash\n---\n");
        assert_eq!(r.allowed_tools.as_deref(), Some("Bash"), "block list cannot hide Bash");
        assert_eq!(display_marked("a\u{200B}b\r\nc", 100), "a⟦U+200B⟧b\nc");
        assert!(valid_token(&random_token()));
    }
}
