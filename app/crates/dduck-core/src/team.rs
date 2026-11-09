//! Tech-lead workflow (D-061, D-063): effort levels, the structured brief,
//! worktree naming and a git-free repository check. Team rules and task types are editable data (`team_config`,
//! D-081); everything injected into `claude` is still built by the backend (AG-09).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    pub fn as_cli(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub goal: String,
    #[serde(default)]
    pub out_of_scope: String,
    #[serde(default)]
    pub constraints: String,
    #[serde(default)]
    pub done: String,
}

/// Assembles the prompt with fixed headings; empty sections are left out. The goal is required.
pub fn compose_brief(b: &Brief) -> Result<String, &'static str> {
    if b.goal.trim().is_empty() {
        return Err(crate::i18n::Fail::GoalRequired.str());
    }
    let parts = [
        ("## 목표", &b.goal),
        ("## 범위 밖 (손대지 말 것)", &b.out_of_scope),
        ("## 제약", &b.constraints),
        ("## 완료 기준", &b.done),
    ];
    Ok(parts.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(h, v)| format!("{h}\n{}", v.trim())).collect::<Vec<_>>().join("\n\n"))
}

/// Worktree name derived by the backend from the task name: `[A-Za-z0-9._-]`, no `..`, not ending in `.`/`.lock`,
/// at most 64 chars, starting alphanumeric; falls back to `dduck-<type>-<time>` (AG-12).
pub fn worktree_name(task_name: Option<&str>, task_type: Option<&str>, now_ms: i64) -> String {
    let mut s: String = task_name
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '-' })
        .collect();
    while s.contains("..") {
        s = s.replace("..", ".");
    }
    let mut s = s.trim_matches(|c: char| !c.is_ascii_alphanumeric()).chars().take(64).collect::<String>();
    while s.ends_with(".lock") || s.ends_with('.') || s.ends_with('-') {
        s = s.trim_end_matches(".lock").trim_end_matches(['.', '-']).to_string();
    }
    if s.is_empty() || !s.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        format!("dduck-{}-{}", task_type.filter(|k| crate::team_config::valid_key(k)).unwrap_or("task"), now_ms / 1000)
    } else {
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RepoStatus {
    NotRepo,
    HomeRoot,
    NoCommits,
    Ok,
}

/// Git-free check (AG-12): finds `.git` (directory, or a `gitdir:` file for linked worktrees) upward from `cwd`,
/// refuses a repository rooted at `home`, and requires `HEAD` to resolve to a commit ref.
pub fn repo_status(cwd: &Path, home: &Path) -> RepoStatus {
    let Ok(cwd) = cwd.canonicalize() else { return RepoStatus::NotRepo };
    let home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    for dir in cwd.ancestors() {
        let dot = dir.join(".git");
        let Ok(m) = std::fs::symlink_metadata(&dot) else { continue };
        if dir == home {
            return RepoStatus::HomeRoot;
        }
        let git_dir: Option<PathBuf> = if m.file_type().is_dir() {
            Some(dot)
        } else if m.file_type().is_file() {
            std::fs::read_to_string(&dot)
                .ok()
                .and_then(|t| t.strip_prefix("gitdir:").map(|p| PathBuf::from(p.trim())))
                .map(|p| if p.is_absolute() { p } else { dir.join(p) })
        } else {
            None
        };
        let Some(git_dir) = git_dir else { return RepoStatus::NotRepo };
        return if has_commit(&git_dir) { RepoStatus::Ok } else { RepoStatus::NoCommits };
    }
    RepoStatus::NotRepo
}

fn has_commit(git_dir: &Path) -> bool {
    let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) else { return false };
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        return head.len() >= 40 && head.chars().all(|c| c.is_ascii_hexdigit()); // detached HEAD
    };
    if !reference.starts_with("refs/") || reference.contains("..") {
        return false;
    }
    // Linked worktrees keep refs in the common dir.
    let common = std::fs::read_to_string(git_dir.join("commondir")).ok().map(|c| git_dir.join(c.trim())).unwrap_or_else(|| git_dir.to_path_buf());
    for base in [git_dir, common.as_path()] {
        if std::fs::read_to_string(base.join(reference)).is_ok_and(|v| v.trim().len() >= 40) {
            return true;
        }
        if std::fs::read_to_string(base.join("packed-refs")).is_ok_and(|p| p.lines().any(|l| l.ends_with(&format!(" {reference}")))) {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn brief_is_assembled_with_fixed_headings() {
        assert!(compose_brief(&Brief::default()).is_err());
        let b = Brief { goal: "버그 고치기".into(), done: " 테스트 통과 ".into(), ..Default::default() };
        assert_eq!(compose_brief(&b).unwrap(), "## 목표\n버그 고치기\n\n## 완료 기준\n테스트 통과");
    }

    #[test]
    fn worktree_names_cannot_escape() {
        assert_eq!(worktree_name(Some("fix-flaky-test"), None, 0), "fix-flaky-test");
        assert_eq!(worktree_name(Some("a/../../../tmp/x"), None, 0), "a-.-.-.-tmp-x");
        assert!(!worktree_name(Some("a/../../../tmp/x"), None, 0).contains(".."));
        assert_eq!(worktree_name(Some("결제 화면"), Some("dev"), 5000), "dduck-dev-5");
        assert_eq!(worktree_name(Some("release.lock"), None, 0), "release");
        assert_eq!(worktree_name(Some("-x."), None, 0), "x");
        assert!(worktree_name(Some(&"a".repeat(100)), None, 0).len() <= 64);
    }

    #[test]
    fn repo_status_without_git() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let repo = home.join("proj");
        std::fs::create_dir_all(repo.join(".git/refs/heads")).unwrap();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(repo_status(&repo.join("src"), &home), RepoStatus::NoCommits);
        std::fs::write(repo.join(".git/refs/heads/main"), format!("{}\n", "a".repeat(40))).unwrap();
        assert_eq!(repo_status(&repo.join("src"), &home), RepoStatus::Ok);
        std::fs::create_dir_all(home.join(".git")).unwrap();
        std::fs::create_dir_all(home.join("loose")).unwrap();
        assert_eq!(repo_status(&home.join("loose"), &home), RepoStatus::HomeRoot);
        assert_eq!(repo_status(tmp.path(), &home), RepoStatus::NotRepo);
        // packed refs
        let packed = tmp.path().join("p");
        std::fs::create_dir_all(packed.join(".git")).unwrap();
        std::fs::write(packed.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(packed.join(".git/packed-refs"), format!("{} refs/heads/main\n", "b".repeat(40))).unwrap();
        assert_eq!(repo_status(&packed, &home), RepoStatus::Ok);
    }
}
