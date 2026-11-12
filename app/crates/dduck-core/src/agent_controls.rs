//! Turning agents off and on, and moving agent definitions to the Trash (team-presets-editable 2.3, D-081).
//!
//! "Off" is Claude Code's own permission rule `Agent(<name>)` in a settings file's `permissions.deny`, written
//! through the permissions-only save path (`permissions::plan_save`/`commit_save`, D-076/D-088). Only that exact
//! string is added or removed. Deleting a definition goes to the macOS Trash (the shell calls NSFileManager); this
//! module only decides which file that may be and re-checks it right before.

use std::path::{Path, PathBuf};

use crate::agents_catalog::{AgentDef, Source};
use crate::permissions::{PermEdit, ScopeView};

/// `Agent(<name>)`. Plugin agents keep their `plugin:name` form (checked with `claude -p` 2.1.319, D-089).
pub fn deny_rule(name: &str) -> String {
    format!("Agent({name})")
}

/// Agent names denied in this scope: `Agent(<name>)` and the legacy `Task(<name>)`. A bare `Agent`/`Task` rule
/// turns every subagent off and is reported as `"*"`.
pub fn denied_names(view: &ScopeView) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in &view.deny {
        let name = if r == "Agent" || r == "Task" {
            Some("*")
        } else {
            r.strip_prefix("Agent(").or_else(|| r.strip_prefix("Task(")).and_then(|x| x.strip_suffix(')')).filter(|n| crate::agents_catalog::valid_name(n))
        };
        if let Some(n) = name {
            if !out.iter().any(|x| x == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

/// True when `name` is off in a scope whose denied names are `names`.
pub fn is_denied(names: &[String], name: &str) -> bool {
    names.iter().any(|n| n == name || n == "*")
}

/// The scope's permissions with `Agent(<name>)` added to or removed from `deny`. Everything else is kept as read.
pub fn with_agent_denied(view: &ScopeView, name: &str, off: bool) -> PermEdit {
    let rule = deny_rule(name);
    let mut deny: Vec<String> = view.deny.iter().filter(|r| **r != rule).cloned().collect();
    if off {
        deny.push(rule);
    }
    PermEdit { allow: view.allow.clone(), ask: view.ask.clone(), deny, default_mode: view.default_mode.clone() }
}

/// Settings file that turns this agent off: project agents in their project's personal settings, everything else
/// globally. `None` for a project agent without a project folder.
pub fn deny_target(def: &AgentDef, config_dir: &Path) -> Option<PathBuf> {
    match def.source {
        Source::Project => def.scope.as_ref().map(|s| Path::new(s).join(".claude/settings.local.json")),
        _ => Some(config_dir.join("settings.json")),
    }
}

/// True when `.claude/settings.local.json` is not mentioned in the project's ignore files (it may get committed).
pub fn local_settings_unignored(project: &Path) -> bool {
    if std::fs::symlink_metadata(project.join(".git")).is_err() {
        return false;
    }
    let mentions = |p: PathBuf| std::fs::read_to_string(p).is_ok_and(|t| t.lines().any(|l| l.trim().trim_start_matches('/').ends_with("settings.local.json") || l.trim() == ".claude/" || l.trim() == ".claude"));
    !(mentions(project.join(".gitignore")) || mentions(project.join(".claude/.gitignore")))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TrashError {
    #[error("{}", crate::i18n::Fail::AgentBundledKeep.str())]
    NotDeletable,
    #[error("{0}")]
    Unsafe(&'static str),
    #[error("{}", crate::i18n::Fail::ChangedWhileChecking.str())]
    Changed,
}

/// What the dialog showed; re-checked right before the file goes to the Trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashPlan {
    pub path: PathBuf,
    pub name: String,
    pub source: Source,
    /// Project folder for project agents.
    pub project: Option<PathBuf>,
    dev: u64,
    ino: u64,
    token: String,
}

#[cfg(unix)]
fn ids(m: &std::fs::Metadata) -> (u64, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino(), m.nlink())
}

#[cfg(not(unix))]
fn ids(_m: &std::fs::Metadata) -> (u64, u64, u64) {
    (0, 0, 1)
}

/// Checks one catalog entry (found by id, never by a webview path): user or project source, a regular `.md` file
/// with one link, directly inside the agents folder it was discovered in, with no link on the way.
pub fn plan_trash(def: &AgentDef, config_dir: &Path) -> Result<TrashPlan, TrashError> {
    let expected_dir = match def.source {
        Source::User => config_dir.join("agents"),
        Source::Project => Path::new(def.scope.as_deref().ok_or(TrashError::Unsafe(crate::i18n::Fail::ProjectFolderUnknown.str()))?).join(".claude/agents"),
        Source::Plugin | Source::Builtin => return Err(TrashError::NotDeletable),
    };
    let path = def.file.clone().ok_or(TrashError::NotDeletable)?;
    if path.extension().is_none_or(|e| e != "md") {
        return Err(TrashError::Unsafe(crate::i18n::Fail::AgentMdOnly.str()));
    }
    if path.parent() != Some(expected_dir.as_path()) {
        return Err(TrashError::Unsafe(crate::i18n::Fail::OutsideAgents.str()));
    }
    // No component of the agents folder may be a link (the catalog scanned it the same way).
    let base = if def.source == Source::User { config_dir.to_path_buf() } else { expected_dir.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default() };
    let mut p = base;
    for part in expected_dir.strip_prefix(&p).map_err(|_| TrashError::Unsafe(crate::i18n::Fail::BadPathValue.str()))?.components() {
        p.push(part);
        if !std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_dir()) {
            return Err(TrashError::Unsafe(crate::i18n::Fail::AgentsDirBad.str()));
        }
    }
    let m = std::fs::symlink_metadata(&path).map_err(|_| TrashError::Unsafe(crate::i18n::Fail::FileNotFound.str()))?;
    let (dev, ino, nlink) = ids(&m);
    if !m.file_type().is_file() || nlink != 1 {
        return Err(TrashError::Unsafe(crate::i18n::Fail::NotRegularOrLinked.str()));
    }
    let bytes = std::fs::read(&path).map_err(|_| TrashError::Unsafe(crate::i18n::Fail::ReadFileFailed.str()))?;
    Ok(TrashPlan {
        name: def.name.clone(),
        source: def.source,
        project: (def.source == Source::Project).then(|| PathBuf::from(def.scope.clone().unwrap_or_default())),
        token: crate::agents_catalog::token_of(&bytes),
        dev,
        ino,
        path,
    })
}

/// Same file as when the dialog opened: device, inode, link count, type and content.
pub fn recheck(plan: &TrashPlan) -> Result<(), TrashError> {
    let m = std::fs::symlink_metadata(&plan.path).map_err(|_| TrashError::Changed)?;
    let (dev, ino, nlink) = ids(&m);
    if !m.file_type().is_file() || nlink != 1 || dev != plan.dev || ino != plan.ino {
        return Err(TrashError::Changed);
    }
    let bytes = std::fs::read(&plan.path).map_err(|_| TrashError::Changed)?;
    if crate::agents_catalog::token_of(&bytes) != plan.token {
        return Err(TrashError::Changed);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::platform::CanonExt;
    use super::*;
    use crate::agents_catalog::{Roots, discover};

    #[test]
    fn deny_rules_are_exact() {
        let view = ScopeView { deny: vec!["Bash(rm:*)".into(), "Agent(code-reviewer)".into(), "Agent(x) ".into()], allow: vec!["Read".into()], ..Default::default() };
        assert_eq!(denied_names(&view), ["code-reviewer"]);
        let legacy = ScopeView { deny: vec!["Task(old)".into(), "Agent".into()], ..Default::default() };
        assert_eq!(denied_names(&legacy), ["old", "*"]);
        assert!(is_denied(&denied_names(&legacy), "anything"));
        let off = with_agent_denied(&view, "agent-skills:test-engineer", true);
        assert_eq!(off.deny.last().unwrap(), "Agent(agent-skills:test-engineer)");
        assert_eq!(off.allow, ["Read"]);
        let on = with_agent_denied(&view, "code-reviewer", false);
        assert_eq!(on.deny, ["Bash(rm:*)", "Agent(x) "], "only the exact rule is removed");
        let twice = with_agent_denied(&ScopeView { deny: off.deny.clone(), ..Default::default() }, "agent-skills:test-engineer", true);
        assert_eq!(twice.deny.iter().filter(|r| r.contains("test-engineer")).count(), 1);
    }

    #[test]
    fn off_then_on_restores_the_file() {
        use crate::permissions::{commit_save, plan_save, read_scope};
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        let original = "{\n  \"env\": {\"X\": \"1\"},\n  \"permissions\": {\n    \"allow\": [\"Read\"],\n    \"deny\": [\"Bash(rm:*)\"]\n  }\n}\n";
        std::fs::write(&path, original).unwrap();
        for off in [true, false] {
            let view = read_scope(&path);
            let plan = plan_save(&path, with_agent_denied(&view, "code-reviewer", off), &view.token, false).unwrap();
            commit_save(&plan, &tmp.path().join("backups"), "t", 0).unwrap();
            assert_eq!(denied_names(&read_scope(&path)).contains(&"code-reviewer".to_string()), off);
        }
        let back: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<serde_json::Value>(original).unwrap(), "same content after off → on");
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"env\": {\"X\": \"1\"}"), "other keys keep their bytes");
    }

    #[test]
    fn trash_only_plain_user_and_project_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(cfg.join("agents")).unwrap();
        std::fs::create_dir_all(proj.join(".claude/agents")).unwrap();
        std::fs::write(cfg.join("agents/mine.md"), "---\nname: mine\ndescription: d\n---\n").unwrap();
        std::fs::write(proj.join(".claude/agents/old-helper.md"), "---\nname: old-helper\ndescription: d\n---\n").unwrap();
        let roots = Roots { config_dir: cfg.clone(), project_dirs: vec![proj.clone()] };
        let defs = discover(&roots);
        let def = |n: &str| defs.iter().find(|d| d.name == n).unwrap().clone();
        let plan = plan_trash(&def("mine"), &cfg).unwrap();
        recheck(&plan).unwrap();
        let p = plan_trash(&def("old-helper"), &cfg).unwrap();
        assert_eq!(p.project.as_deref(), Some(proj.canon().unwrap().as_path()));
        assert_eq!(plan_trash(&def("Explore"), &cfg).unwrap_err(), TrashError::NotDeletable);
        // Changed after the dialog: refused.
        std::fs::write(cfg.join("agents/mine.md"), "---\nname: mine\ndescription: other\n---\n").unwrap();
        assert_eq!(recheck(&plan).unwrap_err(), TrashError::Changed);
        // A hard link is refused.
        std::fs::hard_link(cfg.join("agents/mine.md"), tmp.path().join("elsewhere.md")).unwrap();
        assert!(plan_trash(&def("mine"), &cfg).is_err());
        // A path the catalog did not produce (outside the agents folder) is refused.
        let mut fake = def("old-helper");
        fake.file = Some(tmp.path().join("elsewhere.md"));
        assert!(plan_trash(&fake, &cfg).is_err());
    }

    #[test]
    fn project_agents_are_turned_off_in_personal_project_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = Roots { config_dir: tmp.path().join("cfg"), project_dirs: vec![] };
        let builtin = discover(&roots).into_iter().find(|d| d.name == "Explore").unwrap();
        assert_eq!(deny_target(&builtin, &roots.config_dir).unwrap(), roots.config_dir.join("settings.json"));
        let proj = tmp.path().join("p");
        std::fs::create_dir_all(proj.join(".git")).unwrap();
        assert!(local_settings_unignored(&proj));
        std::fs::write(proj.join(".gitignore"), "node_modules\n.claude/settings.local.json\n").unwrap();
        assert!(!local_settings_unignored(&proj));
    }
}
