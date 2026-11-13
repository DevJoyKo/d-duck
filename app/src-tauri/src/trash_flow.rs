//! What happens after a confirmed delete (windows-support 9.1). The move itself is injected, so the "gone but the
//! Recycle Bin did not show it" case (`Ok(false)` from `move_to_trash`) is tested on every OS: it is a success with
//! `unconfirmed: true`, the follow-up cleanup still runs, and the screens refresh as after any delete.

use std::path::Path;

use dduck_core::i18n::Fail;
use serde::Serialize;

type Res<T> = Result<T, String>;

/// Result of a delete command: `done: false` = the user cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trashed {
    pub done: bool,
    /// Gone, but not seen in the Recycle Bin (Windows): it may be deleted for good.
    pub unconfirmed: bool,
}

impl Trashed {
    pub const CANCELLED: Trashed = Trashed { done: false, unconfirmed: false };

    fn moved(in_bin: bool) -> Trashed {
        Trashed { done: true, unconfirmed: !in_bin }
    }
}

/// Skill, command or workflow: re-check, then move.
pub fn finish_ext(plan: &dduck_core::ext_trash::ExtTrash, trash: impl FnOnce(&Path) -> Res<bool>) -> Res<Trashed> {
    dduck_core::ext_trash::recheck(plan).map_err(|e| e.to_string())?;
    Ok(Trashed::moved(trash(&plan.path)?))
}

/// Memory file: re-check, move, then drop its index line (also when the bin did not confirm the move).
pub fn finish_memory(dir: &Path, plan: &dduck_core::memory::TrashPlan, trash: impl FnOnce(&Path) -> Res<bool>) -> Res<Trashed> {
    dduck_core::memory::recheck_trash(dir, plan).map_err(|_| Fail::ChangedWhileChecking.now())?;
    let in_bin = trash(&plan.path)?;
    // A line left in MEMORY.md must not hide that the bin did not confirm the move: both are said (group 9 check).
    dduck_core::memory::forget_after_trash(dir, &plan.file, plan.enabled).map_err(|_| {
        if in_bin { Fail::MemoryLineLeft.now() } else { format!("{} · {}", Fail::MemoryLineLeft.now(), Fail::TrashUnconfirmed.now()) }
    })?;
    Ok(Trashed::moved(in_bin))
}

/// Rule file: re-check, then move.
pub fn finish_rule(plan: &dduck_core::rules::RuleTrash, trash: impl FnOnce(&Path) -> Res<bool>) -> Res<Trashed> {
    dduck_core::rules::recheck_trash(plan).map_err(|_| Fail::ChangedWhileChecking.now())?;
    Ok(Trashed::moved(trash(&plan.path)?))
}

/// Agent definition: re-check, move, refresh, then work out the deny rule and hidden entry left behind, so the
/// cleanup offer appears whether or not the bin confirmed the move.
pub struct AgentTrashed {
    pub off: bool,
    pub hidden: bool,
    pub unconfirmed: bool,
}

pub fn finish_agent(
    plan: &dduck_core::agent_controls::TrashPlan,
    config_dir: &Path,
    trash: impl FnOnce(&Path) -> Res<bool>,
    refresh: impl FnOnce(),
    hidden_names: impl FnOnce() -> Res<Vec<String>>,
) -> Res<AgentTrashed> {
    dduck_core::agent_controls::recheck(plan).map_err(|e| e.to_string())?;
    let in_bin = trash(&plan.path)?;
    refresh();
    let target = match &plan.project {
        Some(p) => p.join(".claude/settings.local.json"),
        None => config_dir.join("settings.json"),
    };
    let off = dduck_core::agent_controls::denied_names(&dduck_core::permissions::read_scope(&target)).contains(&plan.name);
    let hidden = hidden_names()?.contains(&plan.name);
    Ok(AgentTrashed { off, hidden, unconfirmed: !in_bin })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// The Windows case: the file is gone, the bin did not show it.
    fn gone_unconfirmed(p: &Path) -> Res<bool> {
        if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }.map_err(|e| e.to_string())?;
        Ok(false)
    }

    fn canon(p: &Path) -> std::path::PathBuf {
        dduck_core::platform::canon(p).unwrap()
    }

    #[test]
    fn memory_unconfirmed_is_a_success_and_forgets_the_line() {
        let t = tempfile::tempdir().unwrap();
        let dir = canon(t.path()).join("memory");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "---\nname: a\n---\nA\n").unwrap();
        std::fs::write(dir.join("MEMORY.md"), "- [A](a.md) — note\n").unwrap();
        let l = dduck_core::memory::list(&dir);
        let e = l.entries.iter().find(|e| e.file == "a.md").unwrap();
        let plan = dduck_core::memory::plan_trash(&dir, "a.md", &e.token, &l.index_token).unwrap();
        let r = finish_memory(&dir, &plan, gone_unconfirmed).unwrap();
        assert_eq!(r, Trashed { done: true, unconfirmed: true });
        assert!(!std::fs::read_to_string(dir.join("MEMORY.md")).unwrap().contains("a.md"), "index line removed");
    }

    #[test]
    fn memory_line_left_still_says_unconfirmed() {
        let t = tempfile::tempdir().unwrap();
        let dir = canon(t.path()).join("memory");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "---\nname: a\n---\nA\n").unwrap();
        std::fs::write(dir.join("MEMORY.md"), "- [A](a.md) — note\n").unwrap();
        let l = dduck_core::memory::list(&dir);
        let e = l.entries.iter().find(|e| e.file == "a.md").unwrap();
        let plan = dduck_core::memory::plan_trash(&dir, "a.md", &e.token, &l.index_token).unwrap();
        // The move also turns MEMORY.md into a folder, so the index line cannot be removed.
        let err = finish_memory(&dir, &plan, |p| {
            std::fs::remove_file(p).unwrap();
            std::fs::remove_file(dir.join("MEMORY.md")).unwrap();
            std::fs::create_dir(dir.join("MEMORY.md")).unwrap();
            Ok(false)
        })
        .unwrap_err();
        assert!(err.contains(&Fail::MemoryLineLeft.now()) && err.contains(&Fail::TrashUnconfirmed.now()), "{err}");
    }

    #[test]
    fn rule_unconfirmed_is_a_success() {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path());
        std::fs::create_dir_all(base.join("cfg/rules")).unwrap();
        std::fs::write(base.join("cfg/rules/style.md"), "s").unwrap();
        let roots = dduck_core::rules::Roots { config_dir: base.join("cfg"), project: None, home: Some(base.clone()) };
        let g = dduck_core::rules::list_group(&roots, dduck_core::rules::Scope::Global).unwrap();
        let row = g.rules.iter().find(|r| r.rel == "style.md").unwrap();
        let plan = dduck_core::rules::plan_trash(&roots, &row.id, &row.token).unwrap();
        assert_eq!(finish_rule(&plan, gone_unconfirmed).unwrap(), Trashed { done: true, unconfirmed: true });
        assert!(!base.join("cfg/rules/style.md").exists());
        assert!(finish_rule(&plan, |_| Ok(true)).is_err(), "a second delete is refused by the re-check");
    }

    #[test]
    fn ext_unconfirmed_is_a_success() {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path());
        let cfg = base.join("cfg");
        std::fs::create_dir_all(cfg.join("commands")).unwrap();
        std::fs::write(cfg.join("commands/tidy.md"), "---\ndescription: d\n---\nbody\n").unwrap();
        let item = dduck_core::skills_catalog::discover(&cfg, &[]).into_iter().find(|s| s.kind == "command" && s.name == "tidy").unwrap();
        let roots = dduck_core::ext_trash::TrashRoots { config_dir: cfg.clone(), projects: vec![], home: Some(base.join("home")), linked: vec![] };
        let plan = dduck_core::ext_trash::plan_trash(&item, &roots).unwrap();
        assert_eq!(finish_ext(&plan, gone_unconfirmed).unwrap(), Trashed { done: true, unconfirmed: true });
        assert!(!cfg.join("commands/tidy.md").exists());
    }

    #[test]
    fn agent_unconfirmed_still_reports_the_deny_rule_and_hidden_entry() {
        let t = tempfile::tempdir().unwrap();
        let cfg = canon(t.path()).join("cfg");
        std::fs::create_dir_all(cfg.join("agents")).unwrap();
        std::fs::write(cfg.join("agents/mine.md"), "---\nname: mine\ndescription: d\n---\n").unwrap();
        std::fs::write(cfg.join("settings.json"), r#"{"permissions":{"deny":["Agent(mine)"]}}"#).unwrap();
        let roots = dduck_core::agents_catalog::Roots { config_dir: cfg.clone(), project_dirs: vec![] };
        let def = dduck_core::agents_catalog::discover(&roots).into_iter().find(|d| d.name == "mine").unwrap();
        let plan = dduck_core::agent_controls::plan_trash(&def, &cfg).unwrap();
        let mut refreshed = false;
        let r = finish_agent(&plan, &cfg, gone_unconfirmed, || refreshed = true, || Ok(vec!["mine".into()])).unwrap();
        assert!(r.unconfirmed && r.off && r.hidden && refreshed);
    }

    #[test]
    fn a_failed_move_is_an_error_and_cancel_is_not_done() {
        assert_eq!(serde_json::to_string(&Trashed::CANCELLED).unwrap(), r#"{"done":false,"unconfirmed":false}"#);
        assert_eq!(Trashed::moved(true), Trashed { done: true, unconfirmed: false });
        let json = serde_json::to_string(&Trashed::moved(false)).unwrap();
        assert_eq!(json, r#"{"done":true,"unconfirmed":true}"#);
    }
}
