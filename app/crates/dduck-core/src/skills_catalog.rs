//! Local skills and slash commands with their usage (sessions-skills-connectors S4). Only the frontmatter `name` and
//! `description` of each file are read (64 KB cap, no symlinks); usage comes from `command_invocations` alone so a
//! Skill tool call is not counted twice (design review MAJOR).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::Error;
use crate::agents_catalog::parse_frontmatter;
use crate::text::display_safe;

const MAX_FILE: u64 = 64 * 1024;
const MAX_ENTRIES: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    Project,
    User,
    Plugin,
    /// Only seen in usage: built into Claude Code or removed since.
    Builtin,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub source: SkillSource,
    pub scope: Option<String>,
    pub kind: &'static str, // "skill" | "command" | "workflow"
    pub description: Option<String>,
    pub uses: i64,
    pub last_used: Option<i64>,
    pub defined: bool,
    /// Opaque id of the definition file for the editor (skill-authoring); `None` for usage-only rows.
    pub id: Option<String>,
    /// Why the editor would be read-only (plugin, claude.ai sync, plugin folder), decided from the path.
    pub readonly: Option<&'static str>,
    /// `<plugin>:<name>` from the `<!-- copied-from: … -->` line "복사해서 내 것으로" leaves (nav-ia, D-117).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    /// Workflows only: whether Claude Code would load the file (workflow-meta-check, D-183).
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub cc: Option<CcCheck>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

/// How Claude Code reads one workflow file: `ccReadable`, the `reason` code when it would skip the file, and the
/// `registeredName` (display-cleaned `meta.name`) when that differs from the file name, since Claude Code
/// registers the workflow under `meta.name`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcCheck {
    pub cc_readable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<crate::authoring::MetaReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_name: Option<String>,
}

/// The origin a copy records at its end; only the safe characters `retitle` writes are accepted.
fn copied_from(text: &str) -> Option<String> {
    let tail = text.trim_end();
    let inner = tail.strip_suffix("-->")?.rsplit_once("<!-- copied-from: ")?.1.trim();
    (!inner.is_empty() && inner.len() <= 120 && inner.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '@' | '/' | '-'))).then(|| inner.to_string())
}

pub const RO_PLUGIN: crate::i18n::Fail = crate::i18n::Fail::PluginItemReadOnly;
pub const RO_SYNCED: crate::i18n::Fail = crate::i18n::Fail::SyncedReadOnly;
pub const RO_PLUGIN_DIR: crate::i18n::Fail = crate::i18n::Fail::PluginSkillReadOnly;
/// Same wording as `authoring::read_item` (extension-edit-delete review: no "수정"·"삭제" for such files).
pub const RO_HARDLINK: crate::i18n::Fail = crate::i18n::Fail::HardlinkNoEdit;

#[cfg(unix)]
fn hard_linked(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).is_ok_and(|m| m.nlink() > 1)
}

#[cfg(not(unix))]
fn hard_linked(_path: &Path) -> bool {
    false
}

fn file_id(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(path.as_os_str().as_encoded_bytes());
    format!("s-{}", h.iter().take(8).map(|b| format!("{b:02x}")).collect::<String>())
}

pub(crate) fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_dir())
}

pub(crate) fn read_head(path: &Path) -> Option<String> {
    let m = std::fs::symlink_metadata(path).ok()?;
    if !m.file_type().is_file() {
        return None;
    }
    let mut s = String::new();
    std::fs::File::open(path).ok()?.take(MAX_FILE).read_to_string(&mut s).ok()?;
    Some(s)
}

fn entry(name: String, file: &Path, source: SkillSource, scope: Option<&Path>, kind: &'static str) -> Option<Skill> {
    let text = read_head(file)?;
    let fm = parse_frontmatter(&text);
    let name = fm.as_ref().and_then(|f| f.get("name")).filter(|n| !n.is_empty() && kind == "skill").map(|n| n.to_string()).unwrap_or(name);
    let readonly = match source {
        SkillSource::Plugin => Some(RO_PLUGIN.str()),
        // A skill folder that is itself a plugin (`claude plugin init` layout) is not a plain skill.
        _ if kind == "skill" && file.parent().is_some_and(|d| d.join(".claude-plugin").exists()) => Some(RO_PLUGIN_DIR.str()),
        _ if hard_linked(file) => Some(RO_HARDLINK.str()),
        _ => None,
    };
    Some(Skill {
        name: display_safe(&name, 100),
        source,
        scope: scope.map(|s| s.display().to_string()),
        kind,
        description: fm.as_ref().and_then(|f| f.get("description")).map(|d| display_safe(d, 300)),
        uses: 0,
        last_used: None,
        defined: true,
        id: Some(file_id(file)),
        readonly,
        copied_from: if source == SkillSource::Plugin { None } else { copied_from(&text) },
        cc: None,
        path: Some(file.to_path_buf()),
    })
}

/// `<dir>/<name>/SKILL.md` for every visible, non-link sub-folder.
pub(crate) fn skills_in(dir: &Path, prefix: Option<&str>, source: SkillSource, scope: Option<&Path>, out: &mut Vec<Skill>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.filter_map(Result::ok).take(MAX_ENTRIES) {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !real_dir(&e.path()) {
            continue;
        }
        skill_at(&e.path(), prefix, source, scope, out);
    }
}

/// One skill folder (`<dir>/SKILL.md`), named like [`skills_in`] names its children. A plugin manifest may list
/// such a folder directly (`"skills": ["./skills/engineering/tdd"]`), not only folders that hold skills.
pub(crate) fn skill_at(dir: &Path, prefix: Option<&str>, source: SkillSource, scope: Option<&Path>, out: &mut Vec<Skill>) {
    let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()).filter(|n| !n.starts_with('.')) else { return };
    let full = match prefix {
        Some(p) => format!("{p}:{name}"),
        None => name.clone(),
    };
    if let Some(mut s) = entry(full, &dir.join("SKILL.md"), source, scope, "skill") {
        if prefix == Some("anthropic-skills") && source != SkillSource::Plugin {
            s.readonly = Some(RO_SYNCED.str());
        }
        if let Some(p) = prefix {
            if !s.name.contains(':') {
                s.name = format!("{p}:{}", s.name);
            }
        }
        out.push(s);
    }
}

/// `.claude/commands/**.md`; a sub-folder becomes a namespace (`opsx/propose.md` → `opsx:propose`).
pub(crate) fn commands_in(dir: &Path, prefix: Option<&str>, source: SkillSource, scope: Option<&Path>, out: &mut Vec<Skill>) {
    for e in walkdir::WalkDir::new(dir).follow_links(false).max_depth(3).into_iter().filter_map(Result::ok).take(MAX_ENTRIES) {
        if !e.file_type().is_file() || e.path().extension().is_none_or(|x| x != "md") {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(dir) else { continue };
        let parts: Vec<String> = rel.with_extension("").components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        if parts.iter().any(|p| p.starts_with('.')) {
            continue;
        }
        let mut name = parts.join(":");
        if let Some(p) = prefix {
            name = format!("{p}:{name}");
        }
        if let Some(s) = entry(name, e.path(), source, scope, "command") {
            out.push(s);
        }
    }
}

/// `<dir>/*.js` workflows: `meta` read by the literal parser (never executed, D-183). Name = file name; `cc` says
/// whether Claude Code would load it and under which name.
pub(crate) fn workflows_in(dir: &Path, source: SkillSource, scope: Option<&Path>, out: &mut Vec<Skill>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.filter_map(Result::ok).take(MAX_ENTRIES) {
        let path = e.path();
        let fname = e.file_name().to_string_lossy().to_string();
        if fname.starts_with('.') || path.extension().is_none_or(|x| x != "js") {
            continue;
        }
        let Some(text) = read_head(&path) else { continue };
        let stem = fname.strip_suffix(".js").unwrap_or(&fname);
        let (description, cc) = match crate::authoring::workflow_meta(&text) {
            Ok(m) => {
                let registered_name = (m.name != stem).then(|| display_safe(&m.name, 100));
                (Some(display_safe(&m.description, 300)), CcCheck { cc_readable: true, reason: None, registered_name })
            }
            Err(issue) => (None, CcCheck { cc_readable: false, reason: Some(issue.reason), registered_name: None }),
        };
        out.push(Skill {
            name: display_safe(stem, 100),
            source,
            scope: scope.map(|s| s.display().to_string()),
            kind: "workflow",
            description,
            uses: 0,
            last_used: None,
            defined: true,
            id: Some(file_id(&path)),
            readonly: hard_linked(&path).then_some(RO_HARDLINK.str()),
            copied_from: None,
            cc: Some(cc),
            path: Some(path),
        });
    }
}

/// Project folders plus the parent folders whose `.claude` Claude Code also loads (D-169): a session in
/// `~/work/repo` sees `~/work/.claude` skills, commands and workflows. Canonical, deduplicated, at most 40
/// given folders and 8 parents each. Stops before the home folder and the root, and never returns a parent
/// whose `.claude` is the user config folder.
pub fn with_parents(config_dir: &Path, project_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).and_then(|h| h.canonicalize().ok());
    let cfg = config_dir.canonicalize().ok();
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for p in project_dirs.iter().take(40) {
        let Ok(c) = p.canonicalize() else { continue };
        if seen.insert(c.clone()) {
            out.push(c.clone());
        }
        for a in c.ancestors().skip(1).take(8) {
            if a.parent().is_none() || home.as_deref() == Some(a) {
                break;
            }
            let claude = a.join(".claude");
            if !real_dir(&claude) || cfg.is_some() && claude.canonicalize().ok() == cfg {
                continue;
            }
            if seen.insert(a.to_path_buf()) {
                out.push(a.to_path_buf());
            }
        }
    }
    out
}

pub fn discover(config_dir: &Path, project_dirs: &[PathBuf]) -> Vec<Skill> {
    let mut out = Vec::new();
    for c in with_parents(config_dir, project_dirs) {
        let claude = c.join(".claude");
        if real_dir(&claude) {
            skills_in(&claude.join("skills"), None, SkillSource::Project, Some(&c), &mut out);
            if real_dir(&claude.join("commands")) {
                commands_in(&claude.join("commands"), None, SkillSource::Project, Some(&c), &mut out);
            }
            if real_dir(&claude.join("workflows")) {
                workflows_in(&claude.join("workflows"), SkillSource::Project, Some(&c), &mut out);
            }
        }
    }
    let user = config_dir.join("skills");
    if real_dir(&user) {
        skills_in(&user, None, SkillSource::User, None, &mut out);
        // claude.ai-synced skills: skills/synced/<id>/<name>/SKILL.md, used as `anthropic-skills:<name>`.
        if let Ok(ids) = std::fs::read_dir(user.join("synced")) {
            for id in ids.filter_map(Result::ok).take(50).filter(|e| real_dir(&e.path())) {
                skills_in(&id.path(), Some("anthropic-skills"), SkillSource::User, None, &mut out);
            }
        }
    }
    if real_dir(&config_dir.join("commands")) {
        commands_in(&config_dir.join("commands"), None, SkillSource::User, None, &mut out);
    }
    if real_dir(&config_dir.join("workflows")) {
        workflows_in(&config_dir.join("workflows"), SkillSource::User, None, &mut out);
    }
    // Plugins: the install folders Claude Code loads (installed_plugins.json, else the newest cached version).
    for (plugin, dir) in crate::plugins::plugin_roots(config_dir) {
        // Default `skills/` plus the folders the plugin's manifest lists (the same rule as the plugins tab).
        crate::plugins::plugin_skills(&dir, &crate::plugins::read_manifest(&dir), &plugin, &mut out);
        if real_dir(&dir.join("commands")) {
            commands_in(&dir.join("commands"), Some(&plugin), SkillSource::Plugin, None, &mut out);
        }
    }
    out
}

/// Usage per name from slash commands and Skill tool calls (each invocation once).
pub fn usage(conn: &Connection) -> Result<HashMap<String, (i64, i64)>, Error> {
    let mut stmt = conn.prepare("SELECT name, COUNT(*), MAX(ts) FROM command_invocations WHERE kind = 'skill' GROUP BY name")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn short(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Joins definitions with usage: exact name first, then the unprefixed name when exactly one definition matches.
/// Usage without a definition is listed as built-in/unknown.
pub fn catalog(mut defs: Vec<Skill>, used: HashMap<String, (i64, i64)>) -> Vec<Skill> {
    let mut leftover = Vec::new();
    for (name, (count, last)) in used {
        let target = defs.iter().position(|d| d.name == name && d.kind != "workflow").or_else(|| {
            let cands: Vec<usize> = defs.iter().enumerate().filter(|(_, d)| d.kind != "workflow" && short(&d.name) == short(&name)).map(|(i, _)| i).collect();
            (cands.len() == 1).then(|| cands[0])
        });
        match target {
            Some(i) => {
                defs[i].uses += count;
                defs[i].last_used = defs[i].last_used.max(Some(last));
            }
            None => leftover.push(Skill {
                name: display_safe(&name, 100),
                source: SkillSource::Builtin,
                scope: None,
                kind: "skill",
                description: None,
                uses: count,
                last_used: Some(last),
                defined: false,
                id: None,
                readonly: None,
                copied_from: None,
                cc: None,
                path: None,
            }),
        }
    }
    defs.extend(leftover);
    defs.sort_by(|a, b| b.uses.cmp(&a.uses).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    defs
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn parent_claude_folders_are_found_like_claude_code() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let ws = tmp.path().join("ws");
        let proj = ws.join("trials");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&cfg).unwrap();
        write(&ws.join(".claude/workflows/pin-audit.js"), "export const meta = { name: 'pin-audit', description: '핀 대조' }\n");
        write(&ws.join(".claude/commands/np-do.md"), "---\ndescription: do\n---\n");
        let defs = discover(&cfg, std::slice::from_ref(&proj));
        let wf = defs.iter().find(|d| d.name == "pin-audit").expect("parent workflow listed");
        assert_eq!((wf.kind, wf.source), ("workflow", SkillSource::Project));
        assert_eq!(wf.scope.as_deref(), Some(ws.canonicalize().unwrap().to_str().unwrap()));
        assert!(defs.iter().any(|d| d.name == "np-do"));
        // Listed once even when the parent is also a project folder itself.
        let both = discover(&cfg, &[proj.clone(), ws.clone()]);
        assert_eq!(both.iter().filter(|d| d.name == "pin-audit").count(), 1);
    }

    #[test]
    fn finds_all_sources_and_counts_once() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let proj = tmp.path().join("proj");
        write(&proj.join(".claude/skills/openspec-propose/SKILL.md"), "---\nname: openspec-propose\ndescription: 제안 만들기\n---\n");
        write(&proj.join(".claude/commands/opsx/propose.md"), "---\ndescription: propose\n---\n");
        write(&cfg.join("skills/synced/abc/docx/SKILL.md"), "---\nname: docx\ndescription: 문서\n---\n");
        write(&cfg.join("skills/.trash/old/SKILL.md"), "---\nname: old\n---\n");
        write(&cfg.join("plugins/cache/mk/cloudflare/1.0.2/skills/wrangler/SKILL.md"), "---\nname: wrangler\n---\n");
        write(&cfg.join("workflows/triage.js"), "export const meta = { name: 'triage', description: '이슈 분류' }\n");
        write(&proj.join(".claude/workflows/release.js"), "// x\nexport const meta = { name: 'release', description: '릴리스' }\n");
        write(&proj.join(".claude/workflows/notes.md"), "not a workflow");
        let defs = discover(&cfg, std::slice::from_ref(&proj));
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        for n in ["openspec-propose", "opsx:propose", "anthropic-skills:docx", "cloudflare:wrangler"] {
            assert!(names.contains(&n), "{n} in {names:?}");
        }
        assert!(!names.iter().any(|n| n.contains("old")), "dot folders skipped");
        let wf: Vec<&Skill> = defs.iter().filter(|d| d.kind == "workflow").collect();
        assert_eq!(wf.len(), 2);
        assert!(wf.iter().any(|w| w.name == "triage" && w.description.as_deref() == Some("이슈 분류") && w.source == SkillSource::User));
        let ro = |n: &str| defs.iter().find(|d| d.name == n).unwrap().readonly;
        assert_eq!((ro("cloudflare:wrangler"), ro("anthropic-skills:docx"), ro("openspec-propose")), (Some(RO_PLUGIN.str()), Some(RO_SYNCED.str()), None));
        assert!(defs.iter().all(|d| d.id.as_deref().is_some_and(|i| i.starts_with("s-"))));

        let conn = crate::db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO command_invocations VALUES ('1','s',10,'opsx:propose','skill','slash'),('2','s',20,'opsx:propose','skill','tool'),
             ('3','s',30,'wrangler','skill','tool'),('4','s',40,'update-config','skill','tool'),('5','s',50,'upgrade','builtin','slash');
             INSERT INTO tool_calls(tool_use_id,message_id,session_id,ts,tool_name,skill_name) VALUES ('t','m','s',20,'Skill','opsx:propose');",
        )
        .unwrap();
        let cat = catalog(defs, usage(&conn).unwrap());
        let get = |n: &str| cat.iter().find(|s| s.name == n).unwrap();
        assert_eq!(get("opsx:propose").uses, 2, "a Skill tool call is not counted twice");
        assert_eq!(get("cloudflare:wrangler").uses, 1, "unique unprefixed match");
        let builtin = get("update-config");
        assert!(!builtin.defined && builtin.source == SkillSource::Builtin);
        assert!(cat.iter().all(|s| s.name != "upgrade"), "built-in slash commands are not skills");
        assert_eq!(cat[0].name, "opsx:propose", "most used first");
    }

    #[test]
    fn workflows_say_whether_claude_code_reads_them() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        write(&cfg.join("workflows/good.js"), "export const meta = { name: 'good', description: '좋아요' }\n");
        // Hand-made files the app never saved: a call in meta, a meta that is not first, a different meta.name.
        write(&cfg.join("workflows/x.js"), "export const meta = { name: 'x', description: 'd', whenToUse: String(1) }\n");
        write(&cfg.join("workflows/late.js"), "const a = 1\nexport const meta = { name: 'late', description: 'd' }\n");
        write(&cfg.join("workflows/file-name.js"), "export const meta = { name: 'meta-name\u{202e}', description: 'd' }\n");
        let defs = discover(&cfg, &[]);
        let get = |n: &str| defs.iter().find(|d| d.name == n).unwrap();
        let cc = |n: &str| get(n).cc.clone().unwrap();
        assert_eq!(cc("good"), CcCheck { cc_readable: true, reason: None, registered_name: None });
        assert_eq!(cc("x"), CcCheck { cc_readable: false, reason: Some(crate::authoring::MetaReason::NotLiteral), registered_name: None });
        assert_eq!(get("x").description, None, "no description from a file Claude Code skips");
        assert_eq!(cc("late").reason, Some(crate::authoring::MetaReason::NotFirstStatement));
        let reg = cc("file-name");
        assert!(reg.cc_readable && reg.registered_name.as_deref().is_some_and(|n| n.starts_with("meta-name") && !n.contains('\u{202e}')), "{reg:?}");
        let json = serde_json::to_value(get("x")).unwrap();
        assert_eq!((json["ccReadable"].clone(), json["reason"].clone()), (serde_json::json!(false), serde_json::json!("notLiteral")));
        assert!(json.get("registeredName").is_none() && json.get("cc").is_none());
        let skill = serde_json::to_value(Skill { kind: "skill", cc: None, ..get("good").clone() }).unwrap();
        assert!(skill.get("ccReadable").is_none(), "skills and commands carry no check");
    }

    #[test]
    fn hard_linked_definitions_are_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        write(&cfg.join("commands/standup.md"), "---\ndescription: d\n---\n");
        write(&cfg.join("workflows/triage.js"), "export const meta = { name: 'triage' }\n");
        write(&cfg.join("skills/solo/SKILL.md"), "---\nname: solo\n---\n");
        std::fs::hard_link(cfg.join("commands/standup.md"), tmp.path().join("other.md")).unwrap();
        std::fs::hard_link(cfg.join("workflows/triage.js"), tmp.path().join("other.js")).unwrap();
        let defs = discover(&cfg, &[]);
        let ro = |n: &str| defs.iter().find(|d| d.name == n).unwrap().readonly;
        assert_eq!((ro("standup"), ro("triage"), ro("solo")), (Some(RO_HARDLINK.str()), Some(RO_HARDLINK.str()), None));
    }

    #[test]
    fn a_deleted_skill_with_usage_stays_as_a_usage_only_row() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        write(&cfg.join("skills/standup/SKILL.md"), "---\nname: standup\n---\n");
        let used: HashMap<String, (i64, i64)> = [("standup".to_string(), (5, 99))].into();
        let before = catalog(discover(&cfg, &[]), used.clone());
        assert!(before.iter().any(|s| s.name == "standup" && s.defined && s.id.is_some() && s.uses == 5));
        std::fs::remove_dir_all(cfg.join("skills/standup")).unwrap();
        let after = catalog(discover(&cfg, &[]), used);
        let rows: Vec<&Skill> = after.iter().filter(|s| s.name == "standup").collect();
        assert_eq!(rows.len(), 1, "the definition row is gone");
        assert!(!rows[0].defined && rows[0].id.is_none() && rows[0].readonly.is_none() && rows[0].uses == 5, "only the usage row remains, with no editor id");
    }

    #[test]
    fn copies_name_their_origin() {
        assert_eq!(copied_from("body\n\n<!-- copied-from: example:frontend-design -->\n").as_deref(), Some("example:frontend-design"));
        assert_eq!(copied_from("<!-- copied-from: <script> -->"), None);
        assert_eq!(copied_from("no comment"), None);
    }
}
