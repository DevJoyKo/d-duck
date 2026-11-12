//! App-wide project list for the sidebar picker (project-switcher, D-111).
//!
//! Every project gets an opaque id: the first 16 hex digits of sha256 over its canonical folder, or `key:` plus the
//! same over a Claude Code memory key whose folder is unknown. Scope commands take only these ids and resolve them
//! again on every call; the webview never sends a path. Candidate folders that are links (canonical path differs
//! from the one given) are left out. Nothing here is logged.

use crate::platform::CanonExt;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory;
use crate::text::display_safe;

pub const HOME_REASON: &str = "홈 폴더는 전역과 같아요";
pub const NO_HOME_REASON: &str = "홈 폴더를 확인하지 못해 프로젝트 범위는 꺼 둬요";
pub const UNKNOWN_REASON: &str = "폴더를 몰라요";
pub const MISSING_REASON: &str = "폴더가 없어졌어요";

/// The fixed code of an off reason (app-language): `home`, `noHome`, `unknown`, `missing`. The webview words it;
/// `off_reason` stays as the Korean fallback.
pub fn off_code(reason: &str) -> &'static str {
    match reason {
        HOME_REASON => "home",
        NO_HOME_REASON => "noHome",
        MISSING_REASON => "missing",
        _ => "unknown",
    }
}
const MAX_LIST: usize = 300;
const WORKTREE_DIR: &str = "/.claude/worktrees/";
const WORKTREE_KEY: &str = "--claude-worktrees-";

/// A folder the app has seen (task, routine, agent catalog, usage history, home) and when it was last active.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub last_active: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Folder {
    Ok,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownProject {
    pub id: String,
    pub name: String,
    /// Folder that holds it, for the grey second line.
    pub parent_dir: Option<String>,
    pub path: Option<String>,
    pub folder: Folder,
    pub is_home: bool,
    /// Why folder-based screens (permissions, rules, instruction files, adding items) cannot use it.
    pub off_reason: Option<&'static str>,
    /// [`off_code`] of `off_reason`.
    pub off_code: Option<&'static str>,
    pub memory: bool,
    pub memories: usize,
    pub rules: usize,
    pub last_active: Option<i64>,
    pub worktree_of: Option<String>,
    pub empty: bool,
    /// In a "원격 push 막기" project (delivery-guard): the picker shows "로컬만". Display only; starts re-check.
    pub local_only: bool,
    #[serde(skip)]
    pub memory_key: Option<String>,
}

pub struct Env<'a> {
    pub config_dir: &'a Path,
    /// `autoMemoryDirectory`, when set.
    pub custom: Option<&'a Path>,
    pub home: Option<&'a Path>,
}

fn hex16(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn path_id(canon: &Path) -> String {
    hex16(canon.as_os_str().as_encoded_bytes())
}

pub fn key_id(key: &str) -> String {
    format!("key:{}", hex16(key.as_bytes()))
}

/// Shape check only (the id must still resolve): 16 hex digits, optionally after `key:`.
pub fn valid_id(id: &str) -> bool {
    let h = id.strip_prefix("key:").unwrap_or(id);
    id.len() <= 64 && h.len() == 16 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The canonical folder when `p` is a real directory that is not reached through a link.
fn real(p: &Path) -> Option<PathBuf> {
    let c = p.canon().ok()?;
    (crate::platform::same_path(&c, p) && std::fs::symlink_metadata(&c).is_ok_and(|m| m.file_type().is_dir())).then_some(c)
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canon(), b.canon()), (Ok(x), Ok(y)) if crate::platform::same_path(&x, &y))
}

/// `Some(reason)` when a project folder must not get a project scope: it is the home folder (same device and
/// inode), home is unknown, or its `.claude` is the Claude config folder itself.
pub fn home_like(canon: &Path, env: &Env) -> Option<&'static str> {
    let Some(home) = env.home.filter(|h| !h.as_os_str().is_empty()).and_then(|h| h.canon().ok()) else { return Some(NO_HOME_REASON) };
    if same_file(canon, &home) {
        return Some(HOME_REASON);
    }
    let claude = canon.join(".claude");
    if claude.exists() && same_file(&claude, env.config_dir) {
        return Some(HOME_REASON);
    }
    None
}

/// Folder of a path id, re-resolved from the candidates (links excluded).
pub fn resolve_path(id: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
    if !valid_id(id) || id.starts_with("key:") {
        return None;
    }
    candidates.iter().filter_map(|c| real(c)).find(|c| path_id(c) == id)
}

/// Memory keys that have a memory folder (plus the `autoMemoryDirectory` one).
fn memory_keys(env: &Env) -> Vec<String> {
    let mut out = Vec::new();
    if env.custom.is_some() && memory::memory_dir(env.config_dir, env.custom, memory::CUSTOM_ID).is_ok() {
        out.push(memory::CUSTOM_ID.to_string());
    }
    if let Ok(rd) = std::fs::read_dir(env.config_dir.join("projects")) {
        for e in rd.filter_map(Result::ok).take(2000) {
            if let Ok(n) = e.file_name().into_string() {
                if memory::memory_dir(env.config_dir, env.custom, &n).is_ok() {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// Memory key of the folder, when exactly one known folder has it.
pub fn memory_key_for(canon: &Path, candidates: &[PathBuf]) -> Option<String> {
    let key = memory::project_key(canon);
    (memory::resolve_keys(candidates).get(&key) == Some(&Some(canon.to_path_buf()))).then_some(key)
}

/// Memory key for any id: a path id's own key, or the key behind a `key:` id.
pub fn resolve_memory_key(id: &str, env: &Env, candidates: &[PathBuf]) -> Option<String> {
    if !valid_id(id) {
        return None;
    }
    if id.starts_with("key:") {
        return memory_keys(env).into_iter().find(|k| key_id(k) == id);
    }
    let canon = resolve_path(id, candidates)?;
    memory_key_for(&canon, candidates)
}

fn name_of(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

fn shown(p: &Path) -> String {
    display_safe(&p.display().to_string(), 300)
}

/// Every known project, most recently active first. `rule_count` counts `<folder>/.claude/rules` (0 for home).
pub fn known(env: &Env, candidates: &[Candidate], rule_count: impl Fn(&Path) -> usize) -> Vec<KnownProject> {
    let mut folders: Vec<(PathBuf, Option<i64>)> = Vec::new();
    let mut index: HashMap<PathBuf, usize> = HashMap::new();
    let mut missing: Vec<PathBuf> = Vec::new();
    for c in candidates {
        match c.path.canon() {
            Err(_) if c.path.is_absolute() => missing.push(c.path.clone()),
            Err(_) => {}
            Ok(_) => {
                let Some(canon) = real(&c.path) else { continue };
                if let Some(&i) = index.get(&canon) {
                    folders[i].1 = folders[i].1.max(c.last_active);
                } else {
                    index.insert(canon.clone(), folders.len());
                    folders.push((canon, c.last_active));
                }
            }
        }
    }
    let canon_list: Vec<PathBuf> = folders.iter().map(|(p, _)| p.clone()).collect();
    let rows = memory::projects(env.config_dir, env.custom, &canon_list);
    let by_key: HashMap<&str, &memory::ProjectRow> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let keys = memory::resolve_keys(&canon_list);
    let mut used: HashSet<String> = HashSet::new();
    let mut out: Vec<KnownProject> = Vec::new();
    for (canon, last) in &folders {
        let key = memory::project_key(canon);
        let row = (keys.get(&key) == Some(&Some(canon.clone()))).then(|| by_key.get(key.as_str()).copied()).flatten();
        if row.is_some() {
            used.insert(key.clone());
        }
        let off = home_like(canon, env);
        let rules = if off.is_some() { 0 } else { rule_count(canon) };
        let memories = row.map_or(0, |r| r.on + r.off);
        out.push(KnownProject {
            id: path_id(canon),
            name: name_of(canon),
            parent_dir: canon.parent().map(shown),
            path: Some(shown(canon)),
            folder: Folder::Ok,
            is_home: off == Some(HOME_REASON),
            off_reason: off,
            off_code: off.map(off_code),
            memory: row.is_some(),
            memories,
            rules,
            last_active: *last,
            worktree_of: None,
            empty: memories == 0 && rules == 0 && last.is_none() && !canon.join(".claude").is_dir(),
            local_only: false,
            memory_key: row.map(|r| r.id.clone()),
        });
    }
    for r in rows.iter().filter(|r| !used.contains(&r.id)) {
        let raw = missing.iter().find(|m| memory::project_key(m) == r.id);
        let (name, folder, reason) = if r.id == memory::CUSTOM_ID {
            ("autoMemoryDirectory".to_string(), Folder::Unknown, UNKNOWN_REASON)
        } else if let Some(m) = raw {
            (name_of(m), Folder::Missing, MISSING_REASON)
        } else {
            (r.label.clone(), Folder::Unknown, UNKNOWN_REASON)
        };
        out.push(KnownProject {
            id: key_id(&r.id),
            name,
            parent_dir: raw.and_then(|m| m.parent()).map(shown),
            path: None,
            folder,
            is_home: false,
            off_reason: Some(reason),
            off_code: Some(off_code(reason)),
            memory: true,
            memories: r.on + r.off,
            rules: 0,
            last_active: None,
            worktree_of: None,
            empty: r.empty,
            local_only: false,
            memory_key: Some(r.id.clone()),
        });
    }
    // Worktrees fold under their parent project.
    let by_path: HashMap<String, String> = out.iter().filter_map(|p| Some((p.path.clone()?, p.id.clone()))).collect();
    let by_mem: HashMap<String, String> = out.iter().filter_map(|p| Some((p.memory_key.clone()?, p.id.clone()))).collect();
    for p in &mut out {
        let parent = match (&p.path, &p.memory_key) {
            (Some(path), _) if path.contains(WORKTREE_DIR) => path.find(WORKTREE_DIR).and_then(|i| by_path.get(&path[..i])),
            (None, Some(k)) if k.contains(WORKTREE_KEY) => k.find(WORKTREE_KEY).and_then(|i| by_mem.get(&k[..i])),
            _ => None,
        };
        p.worktree_of = parent.filter(|id| **id != p.id).cloned();
    }
    out.sort_by(|a, b| b.last_active.cmp(&a.last_active).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out.truncate(MAX_LIST);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(p: &Path) -> PathBuf {
        std::fs::create_dir_all(p).ok();
        p.canon().unwrap_or_else(|_| p.to_path_buf())
    }

    #[test]
    fn ids_are_opaque_and_checked() {
        let id = path_id(Path::new("/Users/me/dev_team"));
        assert_eq!(id.len(), 16);
        assert!(valid_id(&id));
        assert!(valid_id(&key_id("-Users-me")));
        assert!(!id.contains("dev_team"));
        assert!(!valid_id("/Users/me"));
        assert!(!valid_id("key:-Users-me"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn home_and_config_inode_are_refused() {
        let t = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let home = dir(&t.path().join("home"));
        let config = dir(&home.join(".claude"));
        let proj = dir(&t.path().join("proj"));
        let env = Env { config_dir: &config, custom: None, home: Some(&home) };
        assert_eq!(home_like(&home, &env), Some(HOME_REASON));
        assert_eq!(home_like(&proj, &env), None);
        let none = Env { config_dir: &config, custom: None, home: None };
        assert_eq!(home_like(&proj, &none), Some(NO_HOME_REASON));
        // A project whose .claude is the config folder (custom config dir case).
        let other = dir(&t.path().join("other"));
        let env2 = Env { config_dir: &other.join(".claude"), custom: None, home: Some(&home) };
        dir(&other.join(".claude"));
        assert_eq!(home_like(&other, &env2), Some(HOME_REASON));
    }

    #[cfg(unix)]
    #[test]
    fn links_are_left_out_and_ids_resolve() {
        let t = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let home = dir(&t.path().join("home"));
        let config = dir(&home.join(".claude"));
        let proj = dir(&t.path().join("work").join("dev_team"));
        let link = t.path().join("linked");
        std::os::unix::fs::symlink(&proj, &link).unwrap_or_else(|_| unreachable!());
        let env = Env { config_dir: &config, custom: None, home: Some(&home) };
        let cands = vec![
            Candidate { path: link.clone(), last_active: Some(5) },
            Candidate { path: proj.clone(), last_active: Some(3) },
            Candidate { path: home.clone(), last_active: None },
            Candidate { path: t.path().join("gone"), last_active: None },
        ];
        let list = known(&env, &cands, |_| 2);
        assert_eq!(list.len(), 2, "link and missing folder without memory are not listed");
        let p = list.iter().find(|p| p.name == "dev_team").unwrap_or_else(|| unreachable!());
        assert_eq!(p.last_active, Some(3), "the link's activity is not merged in");
        assert_eq!(p.rules, 2);
        let h = list.iter().find(|p| p.is_home).unwrap_or_else(|| unreachable!());
        assert_eq!(h.rules, 0);
        let raw: Vec<PathBuf> = cands.iter().map(|c| c.path.clone()).collect();
        assert_eq!(resolve_path(&p.id, &raw), Some(proj.clone()));
        assert_eq!(resolve_path(&path_id(&link), &raw), None);
        assert_eq!(resolve_path(&p.id, &[link]), None, "only reachable through a link: refused");
        let json = serde_json::to_string(&list).unwrap_or_default();
        assert!(!json.contains("memoryKey"));
    }

    #[test]
    fn memory_only_keys_and_worktrees() {
        let t = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let home = dir(&t.path().join("home"));
        let config = dir(&home.join(".claude"));
        let proj = dir(&t.path().join("dev_team"));
        let key = memory::project_key(&proj);
        let wt = format!("{key}--claude-worktrees-lead");
        for k in [key.as_str(), wt.as_str(), "-Somewhere-else"] {
            dir(&config.join("projects").join(k).join("memory"));
            std::fs::write(config.join("projects").join(k).join("memory").join("a.md"), "---\nname: a\n---\nx").ok();
        }
        let gone = t.path().join("deleted_one");
        dir(&config.join("projects").join(memory::project_key(&gone)).join("memory"));
        let env = Env { config_dir: &config, custom: None, home: Some(&home) };
        let cands = vec![Candidate { path: proj.clone(), last_active: Some(1) }, Candidate { path: gone.clone(), last_active: None }];
        let list = known(&env, &cands, |_| 0);
        let main = list.iter().find(|p| p.path.is_some()).unwrap_or_else(|| unreachable!());
        assert!(main.memory);
        let w = list.iter().find(|p| p.memory_key.as_deref() == Some(wt.as_str())).unwrap_or_else(|| unreachable!());
        assert_eq!(w.worktree_of.as_deref(), Some(main.id.as_str()));
        assert!(w.id.starts_with("key:") && w.off_reason == Some(UNKNOWN_REASON) && w.off_code == Some("unknown"));
        assert_eq!((off_code(HOME_REASON), off_code(NO_HOME_REASON), off_code(MISSING_REASON)), ("home", "noHome", "missing"));
        let g = list.iter().find(|p| p.name == "deleted_one").unwrap_or_else(|| unreachable!());
        assert_eq!(g.folder, Folder::Missing);
        let raw: Vec<PathBuf> = cands.iter().map(|c| c.path.clone()).collect();
        assert_eq!(resolve_memory_key(&w.id, &env, &raw).as_deref(), Some(wt.as_str()));
        assert_eq!(resolve_memory_key(&main.id, &env, &raw), Some(key));
        assert_eq!(resolve_memory_key("key:0000000000000000", &env, &raw), None);
    }
}
