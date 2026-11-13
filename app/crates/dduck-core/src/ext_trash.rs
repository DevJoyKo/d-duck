//! Moving the user's own skills, slash commands and workflows to the Trash (extension-edit-delete, D-147).
//!
//! The shell finds the item by catalog id (never a webview path) and calls [`plan_trash`]; after the native
//! confirmation it calls [`recheck`] and only then moves [`ExtTrash::path`] with NSFileManager. Same rules as
//! `agent_controls::plan_trash`: only the folders the catalog scans, no link on the way, regular single-link files,
//! and a fingerprint (device, inode, content, and for a skill folder every entry's size and times) re-checked.
//! Paths are compared as configured; only the project list and the home check are canonical (security review).

use crate::platform::CanonExt;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::agents_catalog::{AgentDef, Source};
use crate::skills_catalog::{RO_HARDLINK, Skill, SkillSource};
use crate::team_config::TypeDef;
use crate::text::display_safe;

/// A skill folder holds at most this many entries, this many levels deep (one more is walked to see overflow).
pub const MAX_ENTRIES: usize = 2000;
pub const MAX_DEPTH: usize = 8;

pub const LINK: crate::i18n::Fail = crate::i18n::Fail::LinkNoTrash;
pub const HARD_LINK: crate::i18n::Fail = crate::i18n::Fail::LinkedOtherName;
pub const DDUCK_NEW: crate::i18n::Fail = crate::i18n::Fail::TrashViaToggle;
pub const LINKED_PROJECT: crate::i18n::Fail = crate::i18n::Fail::LinkedProjectKeep;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExtTrashError {
    #[error("{}", crate::i18n::Fail::PluginSyncKeep.str())]
    NotDeletable,
    #[error("{0}")]
    Unsafe(&'static str),
    #[error("{}", crate::i18n::Fail::ChangedNoDelete.str())]
    Changed,
}

use ExtTrashError::{Changed, NotDeletable, Unsafe};

/// Where deletable items may live. `config_dir` as configured (it may itself be a link, like for creating);
/// `projects` and `home` canonical.
#[derive(Debug, Clone)]
pub struct TrashRoots {
    pub config_dir: PathBuf,
    pub projects: Vec<PathBuf>,
    pub home: Option<PathBuf>,
    /// Canonical targets of project folders configured through a link: refused (fail closed, review 6).
    pub linked: Vec<PathBuf>,
}

impl TrashRoots {
    /// From the configured folders: a project whose path resolves somewhere else (a link on the way) goes to
    /// `linked`, the rest to `projects`. Missing folders are dropped.
    pub fn from_configured(config_dir: PathBuf, project_dirs: &[PathBuf], home: &Path) -> Self {
        let (mut projects, mut linked) = (Vec::new(), Vec::new());
        for p in project_dirs {
            let Ok(c) = p.canon() else { continue };
            if c == *p { projects.push(c) } else { linked.push(c) }
        }
        for v in [&mut projects, &mut linked] {
            v.sort();
            v.dedup();
        }
        TrashRoots { config_dir, projects, home: home.canon().ok(), linked }
    }
}

/// What the dialog shows; re-checked right before the move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtTrash {
    /// The skill folder, or the command/workflow file.
    pub path: PathBuf,
    pub kind: &'static str,
    pub name: String,
    /// The folder or file name on disk (shown first in the dialog).
    pub disk_name: String,
    pub project: Option<PathBuf>,
    /// Files moved with a skill folder (1 for a file).
    pub files: usize,
    /// sha256 of SKILL.md (skills only), for matching loadout pins.
    pub skill_token: Option<String>,
    /// Folders from the base down to the target's parent; each must stay a real folder.
    guard: Vec<PathBuf>,
    dev: u64,
    ino: u64,
    fingerprint: String,
}

/// (device, inode, links) of `p`, whose metadata is `m` (Windows reads them from a handle; see platform::ids).
fn ids(p: &Path, m: &std::fs::Metadata) -> (u64, u64, u64) {
    crate::platform::ids(p, m)
}

#[cfg(unix)]
fn times(m: &std::fs::Metadata) -> (i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())
}

#[cfg(not(unix))]
fn times(m: &std::fs::Metadata) -> (i64, i64, i64, i64) {
    let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as i64).unwrap_or(0);
    (t, 0, 0, 0)
}

fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_dir())
}

/// sha256 of a regular single-link file, read in chunks through a no-follow handle (never the whole file at once).
fn stream_token(path: &Path) -> Result<(std::fs::Metadata, String), ExtTrashError> {
    let mut f = crate::artifact_preview::open_flags(path).map_err(|_| Unsafe(LINK.str()))?;
    let m = f.metadata().map_err(|_| Unsafe(crate::i18n::Fail::ReadFileFailed.str()))?;
    if !m.is_file() {
        return Err(Unsafe(crate::i18n::Fail::NotRegularFile.str()));
    }
    if crate::platform::handle_ids(&f, &m).2 != 1 {
        return Err(Unsafe(HARD_LINK.str()));
    }
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|_| Unsafe(crate::i18n::Fail::ReadFileFailed.str()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok((m, h.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

struct Snap {
    dev: u64,
    ino: u64,
    files: usize,
    fingerprint: String,
    skill_token: Option<String>,
}

fn check_guard(guard: &[PathBuf]) -> Result<(), ExtTrashError> {
    if guard.iter().all(|d| real_dir(d)) { Ok(()) } else { Err(Unsafe(LINK.str())) }
}

/// Device, inode and fingerprint of the target, refusing links, hard links, special files and oversized folders.
fn snapshot(path: &Path, kind: &str, guard: &[PathBuf]) -> Result<Snap, ExtTrashError> {
    check_guard(guard)?;
    let m = std::fs::symlink_metadata(path).map_err(|_| Unsafe(crate::i18n::Fail::FileNotFound.str()))?;
    if m.file_type().is_symlink() {
        return Err(Unsafe(LINK.str()));
    }
    let (dev, ino, _) = ids(path, &m);
    if kind != "skill" {
        let (fm, token) = stream_token(path)?;
        if ids(path, &fm).0 != dev || ids(path, &fm).1 != ino {
            return Err(Changed);
        }
        return Ok(Snap { dev, ino, files: 1, fingerprint: token, skill_token: None });
    }
    if !m.is_dir() {
        return Err(Unsafe(crate::i18n::Fail::NotSkillDir.str()));
    }
    if std::fs::symlink_metadata(path.join(".claude-plugin")).is_ok() {
        return Err(NotDeletable);
    }
    let (_, skill_token) = stream_token(&path.join("SKILL.md"))?;
    let mut lines = Vec::new();
    let mut files = 0;
    let mut seen = 0;
    for e in walkdir::WalkDir::new(path).follow_links(false).min_depth(1).max_depth(MAX_DEPTH + 1).into_iter().take(MAX_ENTRIES + 1) {
        let e = e.map_err(|_| Unsafe(crate::i18n::Fail::SkillDirPartial.str()))?;
        seen += 1;
        if seen > MAX_ENTRIES || e.depth() > MAX_DEPTH {
            return Err(Unsafe(crate::i18n::Fail::SkillDirBig.str()));
        }
        let ft = e.file_type();
        if ft.is_symlink() {
            return Err(Unsafe(crate::i18n::Fail::SkillDirLink.str()));
        }
        let em = e.metadata().map_err(|_| Unsafe(crate::i18n::Fail::SkillDirPartial.str()))?;
        let (d, i, n) = ids(e.path(), &em);
        if ft.is_file() {
            if n != 1 {
                return Err(Unsafe(crate::i18n::Fail::HasHardlink.str()));
            }
            files += 1;
        } else if !ft.is_dir() {
            return Err(Unsafe(crate::i18n::Fail::HasSpecial.str()));
        }
        let rel = e.path().strip_prefix(path).map_err(|_| Changed)?;
        let (ms, mn, cs, cn) = times(&em);
        lines.push(format!("{}\0{d}\0{i}\0{}\0{ms}.{mn}\0{cs}.{cn}", rel.to_string_lossy(), em.len()));
    }
    lines.sort();
    let mut h = Sha256::new();
    for l in &lines {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    h.update(skill_token.as_bytes());
    Ok(Snap { dev, ino, files, fingerprint: h.finalize().iter().map(|b| format!("{b:02x}")).collect(), skill_token: Some(skill_token) })
}

fn names(rel: &Path) -> Option<Vec<String>> {
    rel.components().map(|c| if let Component::Normal(s) = c { s.to_str().map(str::to_string) } else { None }).collect()
}

/// Checks one catalog entry (found by id). See the module comment for the rules.
pub fn plan_trash(item: &Skill, roots: &TrashRoots) -> Result<ExtTrash, ExtTrashError> {
    match item.readonly {
        Some(r) if r == RO_HARDLINK.str() => return Err(Unsafe(HARD_LINK.str())),
        Some(_) => return Err(NotDeletable),
        None => {}
    }
    let (base, mut guard, project) = match item.source {
        SkillSource::User => (roots.config_dir.clone(), Vec::new(), None),
        SkillSource::Project => {
            let scope = PathBuf::from(item.scope.as_deref().ok_or(Unsafe(crate::i18n::Fail::ProjectFolderUnknown.str()))?);
            if roots.linked.contains(&scope) {
                return Err(Unsafe(LINKED_PROJECT.str()));
            }
            if !roots.projects.contains(&scope) {
                return Err(Unsafe(crate::i18n::Fail::UnknownProject.str()));
            }
            if roots.home.as_ref() == Some(&scope) {
                return Err(Unsafe(crate::i18n::Fail::HomeNotProject.str()));
            }
            let claude = scope.join(".claude");
            (claude.clone(), vec![scope.clone(), claude], Some(scope))
        }
        SkillSource::Plugin | SkillSource::Builtin => return Err(NotDeletable),
    };
    let path = item.path.clone().ok_or(NotDeletable)?;
    let folder = match item.kind {
        "skill" => "skills",
        "command" => "commands",
        "workflow" => "workflows",
        _ => return Err(NotDeletable),
    };
    let kind_dir = base.join(folder);
    let rel = names(path.strip_prefix(&kind_dir).map_err(|_| Unsafe(crate::i18n::Fail::OutsideRoots.str()))?).ok_or(Unsafe(crate::i18n::Fail::BadPathValue.str()))?;
    if rel.iter().any(|p| p.starts_with('.')) {
        return Err(Unsafe(crate::i18n::Fail::InHiddenDir.str()));
    }
    guard.push(kind_dir.clone());
    let target = match (item.kind, rel.as_slice()) {
        ("skill", [dir, file]) if file == "SKILL.md" && dir != "synced" => kind_dir.join(dir),
        ("command", parts) if (1..=3).contains(&parts.len()) && parts.last().is_some_and(|f| f.ends_with(".md")) => {
            if item.source == SkillSource::User && parts == ["dduck-new.md"] {
                return Err(Unsafe(DDUCK_NEW.str()));
            }
            let mut d = kind_dir.clone();
            for p in &parts[..parts.len() - 1] {
                d.push(p);
                guard.push(d.clone());
            }
            path.clone()
        }
        ("workflow", [file]) if file.ends_with(".js") => path.clone(),
        _ => return Err(Unsafe(crate::i18n::Fail::NotDirectChild.str())),
    };
    let snap = snapshot(&target, item.kind, &guard)?;
    let disk_name = target.file_name().map(|n| display_safe(&n.to_string_lossy(), 100)).unwrap_or_default();
    Ok(ExtTrash {
        path: target,
        kind: item.kind,
        name: item.name.clone(),
        disk_name,
        project,
        files: snap.files,
        skill_token: snap.skill_token,
        guard,
        dev: snap.dev,
        ino: snap.ino,
        fingerprint: snap.fingerprint,
    })
}

/// Same target as when the dialog opened: folders, device, inode, link count and fingerprint.
pub fn recheck(plan: &ExtTrash) -> Result<(), ExtTrashError> {
    let snap = snapshot(&plan.path, plan.kind, &plan.guard).map_err(|_| Changed)?;
    if (snap.dev, snap.ino, &snap.fingerprint) != (plan.dev, plan.ino, &plan.fingerprint) {
        return Err(Changed);
    }
    Ok(())
}

/// Agents and task types that preload this skill as "기본 장비".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadoutUsers {
    pub names: Vec<String>,
    pub more: usize,
    pub strict: bool,
}

impl LoadoutUsers {
    /// One dialog line, or `None` when nobody uses the skill.
    pub fn line(&self) -> Option<String> {
        use crate::i18n::{Code, Lang, Params, current, msg};
        let last = self.names.last()?;
        let lang = current();
        let mut who = self.names.join(", ");
        if self.more > 0 {
            who.push_str(&msg(lang, Code::LoadoutUsersMore, &Params::new().num("count", self.more as i64)));
        }
        // Korean picks 이/가 by the last name's final consonant; English has no particle.
        let particle = match lang {
            Lang::Ko if self.more == 0 && has_batchim(last) => "이",
            Lang::Ko => "가",
            Lang::En => "",
        };
        let mut s = msg(lang, Code::LoadoutUsers, &Params::new().text("who", &who, 400).text("particle", particle, 2));
        if self.strict {
            s.push_str(crate::i18n::text(lang, Code::LoadoutUsersStrict));
        }
        Some(s)
    }
}

fn has_batchim(s: &str) -> bool {
    s.chars().last().is_some_and(|c| ('가'..='힣').contains(&c) && (c as u32 - 0xAC00) % 28 != 0)
}

/// Who preloads `item` (skills only). File agents match the exact name in their `skills` line (a `plugin:name`
/// entry is a plugin skill); a project skill counts for that project's agents and user agents. Task types match a
/// pin with the same name, source and SKILL.md hash (`loadout::hash_of` = first 16 bytes of `skill_token`).
pub fn loadout_users(item: &Skill, skill_token: Option<&str>, agents: &[AgentDef], types: &[TypeDef]) -> LoadoutUsers {
    if item.kind != "skill" {
        return LoadoutUsers::default();
    }
    let source = match item.source {
        SkillSource::User => "user",
        SkillSource::Project => "project",
        _ => return LoadoutUsers::default(),
    };
    let mut names: Vec<String> = agents
        .iter()
        .filter(|a| match item.source {
            SkillSource::Project => a.source == Source::User || (a.source == Source::Project && a.scope == item.scope),
            _ => true,
        })
        .filter(|a| a.skills.contains(&item.name))
        .map(|a| display_safe(&a.name, 64))
        .collect();
    let hash = skill_token.and_then(|t| t.get(..32));
    let mut strict = false;
    for t in types {
        if t.skills.iter().any(|p| p.name == item.name && p.source == source && Some(p.hash.as_str()) == hash) {
            names.push(display_safe(&t.label, 64));
            strict |= t.strict;
        }
    }
    names.sort();
    names.dedup();
    let more = names.len().saturating_sub(5);
    names.truncate(5);
    LoadoutUsers { names, more, strict }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::skills_catalog::discover;

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    struct Fx {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        cfg: PathBuf,
        proj: PathBuf,
    }

    fn fixture() -> Fx {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canon().unwrap();
        let cfg = root.join("cfg");
        let proj = root.join("proj");
        write(&cfg.join("skills/standup/SKILL.md"), "---\nname: standup\n---\n");
        write(&cfg.join("skills/standup/examples/a.md"), "ex");
        write(&cfg.join("commands/opsx/propose.md"), "---\ndescription: p\n---\n");
        write(&cfg.join("commands/dduck-new.md"), "---\ndescription: d\n---\n");
        write(&cfg.join("workflows/triage.js"), "export const meta = { name: 'triage' }\n");
        write(&cfg.join("skills/synced/abc/docx/SKILL.md"), "---\nname: docx\n---\n");
        write(&cfg.join("plugins/cache/mk/cf/1.0.0/skills/wrangler/SKILL.md"), "---\nname: wrangler\n---\n");
        write(&proj.join(".claude/skills/release/SKILL.md"), "---\nname: release\n---\n");
        write(&proj.join(".claude/workflows/ship.js"), "export const meta = { name: 'ship' }\n");
        Fx { _tmp: tmp, root, cfg, proj }
    }

    fn roots(f: &Fx) -> TrashRoots {
        TrashRoots { config_dir: f.cfg.clone(), projects: vec![f.proj.clone()], home: Some(f.root.join("home")), linked: vec![] }
    }

    fn find(f: &Fx, kind: &str, name: &str) -> Skill {
        discover(&f.cfg, std::slice::from_ref(&f.proj)).into_iter().find(|s| s.kind == kind && s.name == name).unwrap()
    }

    /// Windows CI diagnostic: two snapshots of an unchanged skill folder must agree part by part.
    #[test]
    fn snapshot_of_an_unchanged_folder_is_stable() {
        let f = fixture();
        let p = f.cfg.join("skills/standup");
        let (a, b) = (snapshot(&p, "skill", &[]).unwrap(), snapshot(&p, "skill", &[]).unwrap());
        assert_eq!((a.dev, a.ino), (b.dev, b.ino), "folder identity");
        assert_eq!(a.files, b.files);
        assert_eq!(a.skill_token, b.skill_token, "SKILL.md token");
        assert_eq!(a.fingerprint, b.fingerprint, "entry fingerprint");
    }

    #[test]
    fn plans_own_skills_commands_and_workflows() {
        let f = fixture();
        let r = roots(&f);
        let s = plan_trash(&find(&f, "skill", "standup"), &r).unwrap();
        assert_eq!((s.path.clone(), s.files, s.disk_name.as_str()), (f.cfg.join("skills/standup"), 2, "standup"));
        assert!(s.skill_token.as_deref().is_some_and(|t| t.len() == 64));
        recheck(&s).unwrap();
        let c = plan_trash(&find(&f, "command", "opsx:propose"), &r).unwrap();
        assert_eq!(c.path, f.cfg.join("commands/opsx/propose.md"));
        recheck(&c).unwrap();
        assert_eq!(plan_trash(&find(&f, "workflow", "triage"), &r).unwrap().path, f.cfg.join("workflows/triage.js"));
        let p = plan_trash(&find(&f, "skill", "release"), &r).unwrap();
        assert_eq!((p.path.clone(), p.project.clone()), (f.proj.join(".claude/skills/release"), Some(f.proj.clone())));
        assert_eq!(plan_trash(&find(&f, "workflow", "ship"), &r).unwrap().path, f.proj.join(".claude/workflows/ship.js"));
    }

    #[test]
    fn refuses_plugins_synced_and_the_dduck_new_command() {
        let f = fixture();
        let r = roots(&f);
        assert_eq!(plan_trash(&find(&f, "skill", "cf:wrangler"), &r).unwrap_err(), NotDeletable);
        assert_eq!(plan_trash(&find(&f, "skill", "anthropic-skills:docx"), &r).unwrap_err(), NotDeletable);
        assert_eq!(plan_trash(&find(&f, "command", "dduck-new"), &r).unwrap_err(), Unsafe(DDUCK_NEW.str()));
        // A skill folder that is a plugin.
        write(&f.cfg.join("skills/kit/SKILL.md"), "---\nname: kit\n---\n");
        write(&f.cfg.join("skills/kit/.claude-plugin/plugin.json"), "{}");
        assert_eq!(plan_trash(&find(&f, "skill", "kit"), &r).unwrap_err(), NotDeletable);
        // Hand-made entries the catalog would not produce: a skill named `synced`, a builtin row, a path elsewhere.
        let mut fake = find(&f, "skill", "standup");
        fake.path = Some(f.cfg.join("skills/synced/SKILL.md"));
        assert!(plan_trash(&fake, &r).is_err());
        fake.path = Some(f.root.join("elsewhere/SKILL.md"));
        assert!(plan_trash(&fake, &r).is_err());
        fake.source = SkillSource::Builtin;
        assert_eq!(plan_trash(&fake, &r).unwrap_err(), NotDeletable);
    }

    #[test]
    fn refuses_unknown_and_home_projects() {
        let f = fixture();
        let item = find(&f, "skill", "release");
        let mut r = roots(&f);
        r.projects.clear();
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("알려진 프로젝트가 아니에요"));
        let mut r = roots(&f);
        r.home = Some(f.proj.clone());
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("홈 폴더는 프로젝트로 쓰지 않아요"));
    }

    #[cfg(unix)]
    #[test]
    fn a_project_configured_through_a_link_is_refused_with_its_own_message() {
        let f = fixture();
        let link = f.root.join("proj-link");
        std::os::unix::fs::symlink(&f.proj, &link).unwrap();
        // The catalog lists the project by its canonical path; the configured path was the link.
        let item = discover(&f.cfg, std::slice::from_ref(&link)).into_iter().find(|s| s.name == "release").unwrap();
        let r = TrashRoots::from_configured(f.cfg.clone(), std::slice::from_ref(&link), &f.root.join("home"));
        assert_eq!((r.projects.len(), r.linked.clone()), (0, vec![f.proj.clone()]));
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe(LINKED_PROJECT.str()));
        let plain = TrashRoots::from_configured(f.cfg.clone(), &[f.proj.clone(), f.root.join("missing")], &f.root.join("home"));
        assert_eq!((plain.projects.clone(), plain.linked.len()), (vec![f.proj.clone()], 0));
        plan_trash(&item, &plain).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links_on_the_way_and_inside() {
        use std::os::unix::fs::symlink;
        let f = fixture();
        let r = roots(&f);
        let item = find(&f, "skill", "standup");
        // The kind folder swapped for a link after the catalog saw it.
        let real = f.root.join("real-skills");
        std::fs::rename(f.cfg.join("skills"), &real).unwrap();
        symlink(&real, f.cfg.join("skills")).unwrap();
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe(LINK.str()));
        std::fs::remove_file(f.cfg.join("skills")).unwrap();
        std::fs::rename(&real, f.cfg.join("skills")).unwrap();
        // The skill folder itself a link.
        std::fs::rename(f.cfg.join("skills/standup"), f.root.join("standup-real")).unwrap();
        symlink(f.root.join("standup-real"), f.cfg.join("skills/standup")).unwrap();
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe(LINK.str()));
        std::fs::remove_file(f.cfg.join("skills/standup")).unwrap();
        std::fs::rename(f.root.join("standup-real"), f.cfg.join("skills/standup")).unwrap();
        // A link inside the folder.
        symlink("/etc/hosts", f.cfg.join("skills/standup/hosts")).unwrap();
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("스킬 폴더 안에 링크가 있어 지우지 않아요"));
        std::fs::remove_file(f.cfg.join("skills/standup/hosts")).unwrap();
        // A hard link inside the folder, and a hard-linked command (read-only in the catalog too).
        std::fs::hard_link(f.cfg.join("skills/standup/examples/a.md"), f.root.join("a2.md")).unwrap();
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("다른 이름으로도 연결된 파일이 들어 있어요"));
        std::fs::hard_link(f.cfg.join("workflows/triage.js"), f.root.join("t2.js")).unwrap();
        assert_eq!(plan_trash(&find(&f, "workflow", "triage"), &r).unwrap_err(), Unsafe(HARD_LINK.str()));
        let mut raw = find(&f, "workflow", "triage");
        raw.readonly = None;
        assert_eq!(plan_trash(&raw, &r).unwrap_err(), Unsafe(HARD_LINK.str()), "checked again on disk");
        // A namespace folder swapped for a link after the catalog saw it.
        let cmd = find(&f, "command", "opsx:propose");
        std::fs::rename(f.cfg.join("commands/opsx"), f.root.join("opsx-real")).unwrap();
        symlink(f.root.join("opsx-real"), f.cfg.join("commands/opsx")).unwrap();
        assert_eq!(plan_trash(&cmd, &r).unwrap_err(), Unsafe(LINK.str()));
    }

    #[test]
    fn refuses_oversized_skill_folders() {
        let f = fixture();
        let r = roots(&f);
        let item = find(&f, "skill", "standup");
        let deep = (0..MAX_DEPTH).fold(f.cfg.join("skills/standup"), |p, i| p.join(format!("d{i}")));
        write(&deep.join("x.md"), "x");
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("스킬 폴더가 너무 커요(2,000개·8단계까지)"));
        std::fs::remove_dir_all(f.cfg.join("skills/standup/d0")).unwrap();
        plan_trash(&item, &r).unwrap();
        for i in 0..MAX_ENTRIES {
            std::fs::write(f.cfg.join(format!("skills/standup/f{i}")), "").unwrap();
        }
        assert_eq!(plan_trash(&item, &r).unwrap_err(), Unsafe("스킬 폴더가 너무 커요(2,000개·8단계까지)"));
    }

    #[test]
    fn changes_after_the_dialog_are_caught() {
        let f = fixture();
        let r = roots(&f);
        let item = find(&f, "skill", "standup");
        let plan = plan_trash(&item, &r).unwrap();
        write(&f.cfg.join("skills/standup/SKILL.md"), "---\nname: standup\ndescription: other\n---\n");
        assert_eq!(recheck(&plan).unwrap_err(), Changed);
        let plan = plan_trash(&item, &r).unwrap();
        write(&f.cfg.join("skills/standup/new.md"), "n");
        assert_eq!(recheck(&plan).unwrap_err(), Changed, "an added file");
        let wf = find(&f, "workflow", "triage");
        let plan = plan_trash(&wf, &r).unwrap();
        let text = std::fs::read(f.cfg.join("workflows/triage.js")).unwrap();
        std::fs::remove_file(f.cfg.join("workflows/triage.js")).unwrap();
        std::fs::write(f.root.join("swap.js"), &text).unwrap();
        std::fs::rename(f.root.join("swap.js"), f.cfg.join("workflows/triage.js")).unwrap();
        assert_eq!(recheck(&plan).unwrap_err(), Changed, "same bytes, another inode");
    }

    fn agent(name: &str, source: Source, scope: Option<&str>, skills: &[&str]) -> AgentDef {
        AgentDef {
            id: format!("f-{name}"),
            name: name.into(),
            source,
            scope: scope.map(str::to_string),
            path: None,
            description: None,
            model: None,
            tools: vec![],
            disallowed_tools: vec![],
            skills: skills.iter().map(|s| s.to_string()).collect(),
            malformed: false,
            editable: true,
            readonly_reason: None,
            primary: true,
            usable: true,
            risk: Default::default(),
            file: None,
        }
    }

    #[test]
    fn finds_loadout_users() {
        let f = fixture();
        let user = find(&f, "skill", "standup");
        let token = "ab".repeat(32);
        let proj = f.proj.display().to_string();
        let agents = [
            agent("code-reviewer", Source::User, None, &["standup"]),
            agent("cf-agent", Source::User, None, &["cf:standup"]),
            agent("other-proj", Source::Project, Some("/elsewhere"), &["release"]),
            agent("here", Source::Project, Some(&proj), &["release"]),
        ];
        let mut ty = crate::team_config::defaults().types.into_iter().map(|t| t.def).find(|t| t.strict).unwrap();
        ty.skills = vec![crate::loadout::SkillPin { name: "standup".into(), source: "user".into(), hash: "ab".repeat(16) }];
        let mut stale = ty.clone();
        stale.label = "옛 고정".into();
        stale.skills[0].hash = "cd".repeat(16);
        let u = loadout_users(&user, Some(&token), &agents, &[ty.clone(), stale]);
        assert_eq!(u.names, vec!["code-reviewer".to_string(), ty.label.clone()], "plugin entry and stale hash ignored");
        assert!(u.strict);
        assert!(u.line().unwrap().contains("기본 장비로 써요") && u.line().unwrap().contains("엄격 유형은 시작하지 못해요"));
        let release = find(&f, "skill", "release");
        assert_eq!(loadout_users(&release, None, &agents, &[]).names, vec!["here".to_string()], "other projects excluded");
        assert_eq!(loadout_users(&find(&f, "workflow", "triage"), None, &agents, &[]).line(), None);
        let many: Vec<AgentDef> = (0..7).map(|i| agent(&format!("a{i}"), Source::User, None, &["standup"])).collect();
        let m = loadout_users(&user, None, &many, &[]);
        assert_eq!((m.names.len(), m.more), (5, 2));
        assert_eq!(m.line().unwrap(), "a0, a1, a2, a3, a4 외 2개가 기본 장비로 써요. 지우면 다음 시작 때 빠져요.");
        let one = LoadoutUsers { names: vec!["code-reviewer".into()], more: 0, strict: false };
        assert!(one.line().unwrap().starts_with("code-reviewer가 기본 장비로 써요"));
        let kr = LoadoutUsers { names: vec!["설계".into()], more: 0, strict: false };
        assert!(kr.line().unwrap().starts_with("설계가"));
        let kr2 = LoadoutUsers { names: vec!["리뷰 담당".into()], more: 0, strict: false };
        assert!(kr2.line().unwrap().starts_with("리뷰 담당이"));
    }
}
