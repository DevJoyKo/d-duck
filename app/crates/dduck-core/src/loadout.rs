//! Role loadout ("기본 장비", agent-loadout, D-120): skills preloaded at start and the MCP servers a role may use.
//!
//! Skills are pinned by name, source (`user` / `project` / `plugin:<name>`) and the hash of their SKILL.md, and checked
//! again on every start: a pin whose file changed or disappeared is dropped (strict types refuse to start). MCP is
//! "only the chosen servers": every server of the static list (D-114) that was not chosen goes into
//! `disallowedTools: mcp__<server>__*`. The app never writes `mcpServers` (AG-09), so no server is defined here.

use crate::platform::CanonExt;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::skills_catalog::{self, SkillSource};

pub const MAX_SKILLS: usize = 12;
pub const MAX_MCP: usize = 8;
/// Skill files above this are not offered (and not hashed): they would flood the context anyway.
const MAX_SKILL_BYTES: u64 = 256 * 1024;

static SKILL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}\z").unwrap_or_else(|_| unreachable!()));
static SERVER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9][A-Za-z0-9_.-]{0,63}\z").unwrap_or_else(|_| unreachable!()));
static HASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[0-9a-f]{32}\z").unwrap_or_else(|_| unreachable!()));

/// `plugin:skill` is allowed; the first character is alphanumeric so nothing reads as an option.
pub fn valid_skill_name(s: &str) -> bool {
    SKILL_RE.is_match(s)
}

pub fn valid_server_name(s: &str) -> bool {
    SERVER_RE.is_match(s)
}

/// A server whose name has `__` cannot be withheld on its own: `mcp__a__*` would also match `a__b`'s tools.
pub fn separable(server: &str) -> bool {
    !server.contains("__")
}

fn valid_source(s: &str) -> bool {
    match s.strip_prefix("plugin:") {
        Some(p) => crate::agents_catalog::valid_name(p) && !p.contains(':'),
        None => s == "user" || s == "project",
    }
}

/// One pinned skill of a role. Only these three keys exist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPin {
    pub name: String,
    pub source: String,
    pub hash: String,
}

/// Checks the stored loadout of one type (names, counts, duplicates). The webview's values are never trusted.
pub fn validate(skills: &[SkillPin], mcp: &[String]) -> Result<(), String> {
    if skills.len() > MAX_SKILLS {
        return Err(crate::tr_fmt!(crate::i18n::Fail::LoadoutSkillsMax, MAX_SKILLS = MAX_SKILLS));
    }
    let mut seen = BTreeSet::new();
    for s in skills {
        if !valid_skill_name(&s.name) || !valid_source(&s.source) || !HASH_RE.is_match(&s.hash) {
            return Err(crate::i18n::Fail::LoadoutSkillBad.str().into());
        }
        if !seen.insert(s.name.as_str()) {
            return Err(crate::i18n::Fail::LoadoutSkillDup.str().into());
        }
    }
    if mcp.len() > MAX_MCP {
        return Err(crate::tr_fmt!(crate::i18n::Fail::LoadoutMcpMax, MAX_MCP = MAX_MCP));
    }
    let mut seen = BTreeSet::new();
    for m in mcp {
        if !valid_server_name(m) {
            return Err(crate::i18n::Fail::LoadoutMcpBad.str().into());
        }
        if !separable(m) {
            return Err(crate::i18n::Fail::LoadoutMcpUnderscore.str().into());
        }
        if !seen.insert(m.as_str()) {
            return Err(crate::i18n::Fail::LoadoutMcpDup.str().into());
        }
    }
    Ok(())
}

// ---------- skills on disk ----------

/// A skill the loadout picker can offer: preloadable, a regular file, at most 256 KB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOption {
    pub name: String,
    pub source: String,
    /// The plugin that ships it (for "플러그인 묶음").
    pub plugin: Option<String>,
    pub hash: String,
    /// SKILL.md size, for "약 N천 토큰".
    pub bytes: u64,
    pub description: Option<String>,
}

fn source_of(s: &skills_catalog::Skill) -> Option<(String, Option<String>)> {
    match s.source {
        SkillSource::User => Some(("user".into(), None)),
        SkillSource::Project => Some(("project".into(), None)),
        SkillSource::Plugin => {
            let p = s.name.split_once(':')?.0.to_string();
            Some((format!("plugin:{p}"), Some(p)))
        }
        SkillSource::Builtin => None,
    }
}

/// Reads a SKILL.md like D-130's guarded reads: opened with O_NOFOLLOW|O_NONBLOCK, then fstat on the open file
/// (regular, one link, at most 256 KB) and a capped read, so a swap between check and read cannot slip in.
fn read_skill(path: &Path) -> Option<Vec<u8>> {
    let f = crate::artifact_preview::open_flags(path).ok()?;
    let m = crate::artifact_preview::single_link_regular(&f)?;
    if m.len() > MAX_SKILL_BYTES {
        return None;
    }
    let mut buf = Vec::new();
    f.take(MAX_SKILL_BYTES + 1).read_to_end(&mut buf).ok()?;
    (buf.len() as u64 <= MAX_SKILL_BYTES).then_some(buf)
}

pub fn hash_of(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// `disable-model-invocation: true` skills cannot be preloaded, so they are not offered.
fn preloadable(text: &str) -> bool {
    let fm = crate::agents_catalog::parse_frontmatter(text);
    !fm.as_ref().and_then(|f| f.get("disable-model-invocation")).is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

/// The preloadable skills found, plus the names of definitions that could not be vouched for (unreadable, a link,
/// too big, not UTF-8, manual-only, or a second copy with other content). A pin with such a name counts as changed.
#[derive(Debug, Clone, Default)]
pub struct SkillScan {
    pub options: Vec<SkillOption>,
    pub unreadable: BTreeSet<String>,
}

/// Skills under the config folder and the given project folders. `only` limits hashing to these names (start checks).
pub fn scan_skills(config_dir: &Path, project_dirs: &[PathBuf], only: Option<&BTreeSet<String>>) -> SkillScan {
    let mut scan = SkillScan::default();
    let found = skills_catalog::discover(config_dir, project_dirs);
    // Skill folders the catalog skipped (linked folder or SKILL.md, not UTF-8): Claude Code may still load them.
    let listed: BTreeSet<PathBuf> = found.iter().filter_map(|s| s.path.clone()).collect();
    let dirs = std::iter::once(config_dir.join("skills")).chain(project_dirs.iter().take(40).filter_map(|p| p.canon().ok()).map(|c| c.join(".claude/skills")));
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.filter_map(Result::ok).take(500) {
            let name = e.file_name().to_string_lossy().to_string();
            let file = e.path().join("SKILL.md");
            let exists = e.file_type().is_ok_and(|t| t.is_symlink()) || std::fs::symlink_metadata(&file).is_ok();
            if name.starts_with('.') || !exists || listed.contains(&file) || only.is_some_and(|o| !o.contains(&name)) {
                continue;
            }
            scan.unreadable.insert(name);
        }
    }
    for s in found {
        if s.kind != "skill" || !valid_skill_name(&s.name) || only.is_some_and(|o| !o.contains(&s.name)) {
            continue;
        }
        let Some((source, plugin)) = source_of(&s) else { continue };
        let read = s.path.as_ref().and_then(|p| read_skill(p)).and_then(|b| String::from_utf8(b).ok()).filter(|t| preloadable(t));
        let Some(text) = read else {
            scan.unreadable.insert(s.name);
            continue;
        };
        let hash = hash_of(text.as_bytes());
        if let Some(o) = scan.options.iter().find(|o| o.name == s.name && o.source == source) {
            if o.hash != hash {
                scan.unreadable.insert(s.name);
            }
            continue;
        }
        scan.options.push(SkillOption { name: s.name, source, plugin, hash, bytes: text.len() as u64, description: s.description });
    }
    scan.options.sort_by(|a, b| (&a.plugin, &a.name, &a.source).cmp(&(&b.plugin, &b.name, &b.source)));
    scan
}

pub fn skill_options(config_dir: &Path, project_dirs: &[PathBuf], only: Option<&BTreeSet<String>>) -> Vec<SkillOption> {
    scan_skills(config_dir, project_dirs, only).options
}

/// What a start may use: the skills found for the task's folder and the static MCP server names.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub skills: Vec<SkillOption>,
    /// Same-name definitions that could not be read: such a pin is dropped.
    pub unreadable_skills: BTreeSet<String>,
    pub mcp_servers: Vec<String>,
    /// A settings file with MCP servers exists but could not be read: "only the chosen servers" cannot be built.
    pub mcp_unreadable: bool,
    /// "먼저 살펴볼 스킬·에이전트" installed for the task's folder (preferred-skills). Filled only when some are chosen.
    pub preferred: Vec<crate::preferred::Available>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verified {
    pub skills: Vec<String>,
    /// Pins left out because the file changed, moved or disappeared.
    pub dropped: usize,
}

/// A pin counts only when a skill with the same name, source and hash exists and no other definition of that name
/// (which Claude Code might pick instead) has different content or could not be read.
pub fn verify(pins: &[SkillPin], options: &[SkillOption]) -> Verified {
    verify_in(pins, &Env { skills: options.to_vec(), ..Env::default() }, false)
}

/// `worktree`: the task runs in a fresh worktree checked out from git, so project skills there may differ from the
/// files hashed here. A pin is dropped whenever a project definition of its name exists.
pub fn verify_in(pins: &[SkillPin], env: &Env, worktree: bool) -> Verified {
    let mut v = Verified::default();
    for p in pins {
        let same_name: Vec<&SkillOption> = env.skills.iter().filter(|o| o.name == p.name).collect();
        let ok = valid_skill_name(&p.name)
            && !env.unreadable_skills.contains(&p.name)
            && same_name.iter().any(|o| o.source == p.source && o.hash == p.hash)
            && same_name.iter().all(|o| o.hash == p.hash)
            && !(worktree && same_name.iter().any(|o| o.source == "project"));
        if ok {
            v.skills.push(p.name.clone());
        } else {
            v.dropped += 1;
        }
    }
    v
}

// ---------- MCP ----------

/// `disallowedTools` entries for "only the chosen servers". Nothing chosen = no entries (every server allowed).
pub fn mcp_disallow(chosen: &[String], servers: &[String]) -> Vec<String> {
    if chosen.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    for s in servers {
        if chosen.contains(s) || !valid_server_name(s) {
            continue;
        }
        let entry = format!("mcp__{s}__*");
        if !out.contains(&entry) {
            out.push(entry);
        }
    }
    out
}

/// `tools` entries for a role with a tool list: without them the chosen servers would not be usable at all.
pub fn mcp_allow_tools(chosen: &[String]) -> Vec<String> {
    chosen.iter().filter(|c| valid_server_name(c) && separable(c)).map(|c| format!("mcp__{c}__*")).collect()
}

/// Servers a file agent's `disallowedTools` withholds as a whole (`mcp__x` or `mcp__x__*`).
pub fn withheld_servers(disallowed: &[String]) -> Vec<String> {
    disallowed
        .iter()
        .filter_map(|d| {
            let rest = d.strip_prefix("mcp__")?;
            let name = rest.strip_suffix("__*").unwrap_or(rest);
            (valid_server_name(name) && separable(name)).then(|| name.to_string())
        })
        .collect()
}

/// `tools` lets a whole server through (`mcp__x` or `mcp__x__*`).
fn tools_allow(tools: &[String], server: &str) -> bool {
    tools.iter().any(|t| t.strip_prefix("mcp__").is_some_and(|r| r.strip_suffix("__*").unwrap_or(r) == server))
}

/// The servers of the static list a file agent may use. `None`: every server (inherits tools, nothing withheld).
/// With a `tools` line only whole servers listed there count; servers outside the static list are not covered.
pub fn agent_servers(tools: &[String], disallowed: &[String], servers: &[String]) -> Option<Vec<String>> {
    let off = withheld_servers(disallowed);
    if tools.is_empty() {
        if off.is_empty() {
            return None;
        }
        return Some(servers.iter().filter(|s| !off.contains(s)).cloned().collect());
    }
    Some(servers.iter().filter(|s| separable(s) && tools_allow(tools, s) && !off.contains(s)).cloned().collect())
}

/// What a file agent's loadout save changes in its tool lists, for the confirm.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListChange {
    /// `disallowedTools` MCP entries that go away (a block is lifted).
    pub removed_blocks: Vec<String>,
    /// `tools` MCP entries that are new.
    pub added_tools: Vec<String>,
}

fn is_mcp(t: &str) -> bool {
    t.starts_with("mcp__")
}

fn whole_server_entry(t: &str) -> bool {
    t.strip_prefix("mcp__").and_then(|r| r.strip_suffix("__*")).is_some_and(|s| valid_server_name(s) && separable(s))
}

/// The loadout may only touch MCP entries: everything else in `tools` and `disallowedTools` must stay exactly as
/// stored, and an agent that inherits every tool keeps doing so (a `tools` line would narrow it). New entries are
/// whole servers only (`mcp__<server>__*`). The webview's lists are never trusted.
pub fn check_agent_lists(old_tools: &[String], old_dis: &[String], new_tools: &[String], new_dis: &[String]) -> Result<ListChange, &'static str> {
    let rest = |v: &[String]| v.iter().filter(|t| !is_mcp(t)).cloned().collect::<Vec<_>>();
    if rest(old_dis) != rest(new_dis) {
        return Err(crate::i18n::Fail::LoadoutDisallowed.str());
    }
    if rest(old_tools) != rest(new_tools) || (old_tools.is_empty() && !new_tools.is_empty()) {
        return Err(crate::i18n::Fail::LoadoutTools.str());
    }
    // An emptied `tools` line disappears and the agent inherits every tool (re-review HIGH): refuse.
    if !old_tools.is_empty() && new_tools.is_empty() {
        return Err(crate::i18n::Fail::LoadoutEmptyTools.str());
    }
    let added = |old: &[String], new: &[String]| new.iter().filter(|t| is_mcp(t) && !old.contains(t)).cloned().collect::<Vec<_>>();
    let added_tools = added(old_tools, new_tools);
    if added_tools.iter().chain(added(old_dis, new_dis).iter()).any(|t| !whole_server_entry(t)) {
        return Err(crate::i18n::Fail::LoadoutMcpShape.str());
    }
    let removed_blocks = old_dis.iter().filter(|t| is_mcp(t) && !new_dis.contains(t)).cloned().collect();
    Ok(ListChange { removed_blocks, added_tools })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pin(name: &str, source: &str, hash: &str) -> SkillPin {
        SkillPin { name: name.into(), source: source.into(), hash: hash.into() }
    }

    const H: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn emptying_tools_is_refused() {
        let old = vec!["mcp__figma__*".to_string()];
        assert!(check_agent_lists(&old, &[], &[], &[]).is_err(), "removing the last MCP entry must not drop the tools line");
        assert!(check_agent_lists(&old, &[], &old, &[]).is_ok());
    }

    #[test]
    fn names_and_validation() {
        assert!(valid_skill_name("frontend-design") && valid_skill_name("cloudflare:wrangler") && valid_skill_name("a.b_c"));
        for bad in ["", "-x", ":x", "a b", "a/b", "a\u{202e}", &"a".repeat(129)] {
            assert!(!valid_skill_name(bad), "{bad:?}");
        }
        assert!(valid_server_name("figma") && !valid_server_name("a:b") && !valid_server_name("-x"));
        assert!(validate(&[pin("frontend-design", "user", H), pin("expo:expo-ui", "plugin:expo", H)], &["figma".into()]).is_ok());
        assert!(validate(&[pin("x", "plugin:a:b", H)], &[]).is_err(), "nested plugin source");
        assert!(validate(&[pin("x", "managed", H)], &[]).is_err());
        assert!(validate(&[pin("x", "user", "nothex")], &[]).is_err());
        assert!(validate(&[pin("x", "user", H), pin("x", "project", H)], &[]).is_err(), "duplicate name");
        assert!(validate(&(0..13).map(|i| pin(&format!("s{i}"), "user", H)).collect::<Vec<_>>(), &[]).is_err());
        assert!(validate(&[], &(0..9).map(|i| format!("m{i}")).collect::<Vec<_>>()).is_err());
        assert!(validate(&[], &["reports__v2".into()]).is_err(), "cannot be withheld separately");
        assert!(validate(&[], &["a".into(), "a".into()]).is_err());
        // Object-shaped input is refused (only name/source/hash strings).
        assert!(serde_json::from_str::<SkillPin>(r#"{"name":"x","source":"user","hash":"h","command":"rm"}"#).is_err());
        assert!(serde_json::from_str::<Vec<SkillPin>>(r#"[{"name":{"a":1},"source":"user","hash":"h"}]"#).is_err());
    }

    #[test]
    fn mcp_only_chosen_servers() {
        let servers: Vec<String> = ["figma", "notion", "sentry", "reports__v2", "bad name"].iter().map(|s| s.to_string()).collect();
        assert!(mcp_disallow(&[], &servers).is_empty(), "nothing chosen = every server");
        assert_eq!(mcp_disallow(&["figma".into()], &servers), ["mcp__notion__*", "mcp__sentry__*", "mcp__reports__v2__*"]);
        assert_eq!(mcp_allow_tools(&["figma".into(), "x__y".into()]), ["mcp__figma__*"]);
        assert_eq!(withheld_servers(&["Edit".into(), "mcp__notion".into(), "mcp__sentry__*".into(), "mcp__a__tool".into()]), ["notion", "sentry"]);
    }

    #[test]
    fn skills_are_pinned_by_source_and_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let proj = tmp.path().join("proj");
        let write = |p: &Path, text: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write(&cfg.join("skills/frontend-design/SKILL.md"), "---\nname: frontend-design\ndescription: d\n---\n본문");
        write(&cfg.join("skills/manual-only/SKILL.md"), "---\nname: manual-only\ndisable-model-invocation: true\n---\nx");
        write(&cfg.join("plugins/cache/m/expo/1.0.0/skills/expo-ui/SKILL.md"), "---\nname: expo-ui\n---\nx");
        write(&proj.join(".claude/skills/frontend-design/SKILL.md"), "---\nname: frontend-design\n---\n다른 내용");
        let opts = skill_options(&cfg, &[], None);
        let names: Vec<(&str, &str)> = opts.iter().map(|o| (o.name.as_str(), o.source.as_str())).collect();
        assert_eq!(names, [("frontend-design", "user"), ("expo:expo-ui", "plugin:expo")], "manual-only is not preloadable");
        assert_eq!(opts[1].plugin.as_deref(), Some("expo"));
        let fd = pin("frontend-design", "user", &opts[0].hash);
        assert_eq!(verify(std::slice::from_ref(&fd), &opts), Verified { skills: vec!["frontend-design".into()], dropped: 0 });
        // Same name in the task's project with other content: Claude Code might load that one, so it is dropped.
        let with_proj = skill_options(&cfg, std::slice::from_ref(&proj), None);
        assert_eq!(verify(std::slice::from_ref(&fd), &with_proj).dropped, 1);
        // Changed file, wrong source, missing skill.
        write(&cfg.join("skills/frontend-design/SKILL.md"), "---\nname: frontend-design\n---\n바뀜");
        assert_eq!(verify(std::slice::from_ref(&fd), &skill_options(&cfg, &[], None)).dropped, 1);
        assert_eq!(verify(&[pin("expo:expo-ui", "user", &opts[1].hash), pin("gone", "user", H)], &opts).dropped, 2);
        // `only` limits the work to pinned names.
        let only: BTreeSet<String> = ["expo:expo-ui".to_string()].into();
        assert_eq!(skill_options(&cfg, &[], Some(&only)).len(), 1);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(cfg.join("skills/frontend-design/SKILL.md"), cfg.join("skills/linked.md")).unwrap();
            std::fs::create_dir_all(cfg.join("skills/linked")).unwrap();
            std::os::unix::fs::symlink(cfg.join("skills/frontend-design/SKILL.md"), cfg.join("skills/linked/SKILL.md")).unwrap();
            assert!(!skill_options(&cfg, &[], None).iter().any(|o| o.name == "linked"), "links are not read");
        }
    }

    #[test]
    fn unreadable_or_worktree_skills_count_as_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let proj = tmp.path().join("proj");
        let write = |p: &Path, text: &[u8]| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write(&cfg.join("skills/frontend-design/SKILL.md"), b"---\nname: frontend-design\n---\nx");
        let scan = scan_skills(&cfg, &[], None);
        let fd = pin("frontend-design", "user", &scan.options[0].hash);
        // A same-name project copy that is not UTF-8: Claude Code may load it, D.Duck cannot vouch for it.
        write(&proj.join(".claude/skills/frontend-design/SKILL.md"), b"---\nname: frontend-design\n---\n\xff\xfe");
        let with_proj = scan_skills(&cfg, std::slice::from_ref(&proj), None);
        assert!(with_proj.unreadable.contains("frontend-design"));
        let env = Env { skills: with_proj.options.clone(), unreadable_skills: with_proj.unreadable.clone(), ..Env::default() };
        assert_eq!(verify_in(std::slice::from_ref(&fd), &env, false).dropped, 1);
        // Hard-linked SKILL.md is not read (one link only, D-130).
        #[cfg(unix)]
        {
            std::fs::remove_file(proj.join(".claude/skills/frontend-design/SKILL.md")).unwrap();
            std::fs::hard_link(cfg.join("skills/frontend-design/SKILL.md"), proj.join("hard.md")).unwrap();
            assert!(scan_skills(&cfg, &[], None).unreadable.contains("frontend-design"), "nlink 2");
            std::fs::remove_file(proj.join("hard.md")).unwrap();
        }
        // Worktree: a project definition of the name exists, so the pin is dropped; a user-only skill stays.
        write(&proj.join(".claude/skills/frontend-design/SKILL.md"), b"---\nname: frontend-design\n---\nx");
        let same = scan_skills(&cfg, std::slice::from_ref(&proj), None);
        let env = Env { skills: same.options.clone(), unreadable_skills: same.unreadable.clone(), ..Env::default() };
        assert_eq!(verify_in(std::slice::from_ref(&fd), &env, false).dropped, 0, "same content in the project");
        assert_eq!(verify_in(std::slice::from_ref(&fd), &env, true).dropped, 1, "worktree checks out its own copy");
        let user_only = Env { skills: scan_skills(&cfg, &[], None).options, ..Env::default() };
        assert_eq!(verify_in(std::slice::from_ref(&fd), &user_only, true).dropped, 0);
    }

    #[test]
    fn servers_outside_the_static_list_are_not_withheld() {
        // Pinned behaviour (security review): only the static list is withheld, so claude.ai connectors and plugin MCP
        // stay usable for roles that inherit tools. The screen says so ("목록 밖 서버 … 막지 못해요").
        let servers: Vec<String> = vec!["figma".into(), "notion".into()];
        let d = mcp_disallow(&["figma".into()], &servers);
        assert_eq!(d, ["mcp__notion__*"]);
        assert!(!d.iter().any(|e| e.contains("claude_ai") || e.contains("plugin")), "connectors are not in the list");
    }

    #[test]
    fn file_agent_servers_and_list_checks() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let servers = v(&["figma", "notion", "sentry", "reports__v2"]);
        assert_eq!(agent_servers(&[], &[], &servers), None, "inherits: every server");
        assert_eq!(agent_servers(&[], &v(&["mcp__notion__*"]), &servers), Some(v(&["figma", "sentry", "reports__v2"])));
        assert_eq!(agent_servers(&v(&["Read"]), &[], &servers), Some(vec![]), "a tools line without MCP: none");
        assert_eq!(agent_servers(&v(&["Read", "mcp__figma__*", "mcp__notion"]), &v(&["mcp__notion__*"]), &servers), Some(v(&["figma"])));

        let ok = check_agent_lists(&v(&["Read"]), &v(&["Edit", "mcp__notion__*"]), &v(&["Read", "mcp__figma__*"]), &v(&["Edit"])).unwrap();
        assert_eq!(ok, ListChange { removed_blocks: v(&["mcp__notion__*"]), added_tools: v(&["mcp__figma__*"]) });
        assert!(check_agent_lists(&[], &v(&["Edit"]), &[], &[]).is_err(), "a non-MCP block cannot go");
        assert!(check_agent_lists(&[], &v(&["Edit"]), &[], &v(&["Edit", "Bash"])).is_err(), "nor a new one appear");
        assert!(check_agent_lists(&v(&["Read"]), &[], &v(&["Read", "Bash"]), &[]).is_err());
        assert!(check_agent_lists(&[], &[], &v(&["mcp__figma__*"]), &[]).is_err(), "inherit stays inherit");
        assert!(check_agent_lists(&v(&["Read"]), &[], &v(&["Read", "mcp__figma__get"]), &[]).is_err(), "whole servers only");
        assert!(check_agent_lists(&[], &[], &[], &v(&["mcp__a__b__*"])).is_err());
        assert_eq!(check_agent_lists(&[], &[], &[], &v(&["mcp__notion__*"])).unwrap(), ListChange::default());
    }
}
