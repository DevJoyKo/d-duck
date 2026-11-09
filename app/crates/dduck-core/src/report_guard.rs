//! 보고서 폴더에만 쓰기 (report-evidence 6.1, D-152 (2)·(4)): a role without a write tool (review, security, QA)
//! that has a report gets `Write`, and a PreToolUse hook `report-dir` in the same `--settings` as delivery-guard
//! (`delivery_guard::guard_args_with`): writes inside the report folder are allowed without a prompt, anything else is
//! denied, and so is anything the hook cannot resolve. For a role that had no plain `Bash`, shell output options and
//! redirections are denied too.
//!
//! Spike 1.1 (plan/manual measurement) needs the user's approval: until then [`MEASURED`] is false and the shell never
//! grants the tool. The decision logic below is complete and tested.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

pub const FLAG: &str = "--dduck-guard-report-dir";
/// Spike 1.1 has not run: no task gets `Write` for its report yet (only the "[필수]" rule is attached).
pub const MEASURED: bool = false;
pub const NO_HOOKS_NOTE: &str = "이 환경에서는 보고서를 쓸 수 없어요";
const REASON: &str = "D.Duck: 이 작업은 보고서 폴더에만 쓸 수 있어요";
const FILE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Explicit allow: the write lands in the report folder (no prompt, also in plan mode).
    Allow,
    /// No output: the normal permission flow decides.
    NoDecision,
    Deny,
}

/// (stdout, exit code) for a decision.
pub fn output(d: Decision) -> (String, i32) {
    let json = |decision: &str| serde_json::json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": decision, "permissionDecisionReason": REASON}}).to_string();
    match d {
        Decision::Allow => (json("allow"), 0),
        Decision::NoDecision => (String::new(), 0),
        Decision::Deny => (json("deny"), 2),
    }
}

/// Shell-quoted hook command: `'<exe>' --dduck-guard-report-dir '<dir>' <0|1>` (1 = deny shell output forms).
pub fn hook_command(exe: &Path, report_dir: &Path, bash_strict: bool) -> String {
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    format!("{} {FLAG} {} {}", q(exe), q(report_dir), u8::from(bash_strict))
}

/// The `--settings` fragment, merged with delivery-guard's by `guard_args_with`.
pub fn settings(hook_cmd: &str) -> serde_json::Value {
    serde_json::json!({
        "disableAllHooks": false,
        "hooks": { "PreToolUse": [ { "matcher": "*", "hooks": [
            { "type": "command", "command": hook_cmd, "timeout": 10, "onFailure": "block" }
        ] } ] }
    })
}

/// The written path, resolved: parent canonicalized, last part not a link (a new file passes).
fn resolve(path: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let p = Path::new(path);
    let p = if p.is_absolute() { p.to_path_buf() } else { cwd?.join(p) };
    let name = p.file_name()?;
    if name == ".." || name == "." {
        return None;
    }
    let parent = std::fs::canonicalize(p.parent()?).ok()?;
    let target = parent.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(m) if m.file_type().is_symlink() => None,
        Ok(_) => Some(target),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(target),
        Err(_) => None,
    }
}

static SHELL_OUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(>|(^|[\s;&|(])tee(\s|$)|--output|--output-directory|(^|\s)-o(\s|=|$|[^\s-])|(^|[\s;&|(])(ba|z|da|k|c|tc|fi)?sh\s+-c|(^|[\s;&|(])eval(\s|$)|\$\(|`)")
        .unwrap_or_else(|_| unreachable!())
});

/// The decision on Claude Code's PreToolUse JSON. `report_dir` is the real report folder.
pub fn decide(stdin: &str, report_dir: &Path, bash_strict: bool) -> Decision {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(stdin) else { return Decision::Deny };
    let Some(tool) = v.get("tool_name").and_then(|t| t.as_str()) else { return Decision::Deny };
    let input = v.get("tool_input");
    let cwd = v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from).filter(|p| p.is_absolute());
    if FILE_TOOLS.contains(&tool) {
        let Some(path) = input.and_then(|i| i.get("file_path").or_else(|| i.get("notebook_path"))).and_then(|p| p.as_str()) else { return Decision::Deny };
        let Ok(real_dir) = std::fs::canonicalize(report_dir) else { return Decision::Deny };
        return match resolve(path, cwd.as_deref()) {
            Some(t) if t.starts_with(&real_dir) && t != real_dir => Decision::Allow,
            _ => Decision::Deny,
        };
    }
    if tool == "Bash" && bash_strict {
        let Some(cmd) = input.and_then(|i| i.get("command")).and_then(|c| c.as_str()) else { return Decision::Deny };
        return if SHELL_OUT_RE.is_match(cmd) { Decision::Deny } else { Decision::NoDecision };
    }
    Decision::NoDecision
}

/// What a start does for a report: grant `Write` with the hook, or not (and why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    /// The role can already write, or has no report: nothing to add.
    NotNeeded,
    /// `Write` added to the inline role and this `--settings` fragment to merge.
    Granted { settings: serde_json::Value },
    /// Hooks are switched off by managed settings: no `Write`, the task shows [`NO_HOOKS_NOTE`].
    NoHooks,
    /// Spike 1.1 has not run.
    NotMeasured,
}

/// For an inline role (`--agents` JSON) without `Write`: adds it and returns the hook fragment. `measured` is
/// [`MEASURED`] in the shell (tests pass `true`).
pub fn grant(roles_json: &mut Option<String>, role: Option<&str>, report_dir: Option<&Path>, exe: Option<&Path>, hooks_blocked: bool, measured: bool) -> Grant {
    let (Some(json), Some(role), Some(dir)) = (roles_json.as_ref(), role, report_dir) else { return Grant::NotNeeded };
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else { return Grant::NotNeeded };
    let Some(tools) = v.get_mut(role).and_then(|r| r.get_mut("tools")).and_then(|t| t.as_array_mut()) else { return Grant::NotNeeded };
    let has = |name: &str| tools.iter().any(|t| t.as_str() == Some(name));
    if has("Write") || has("*") {
        return Grant::NotNeeded;
    }
    if !measured {
        return Grant::NotMeasured;
    }
    if hooks_blocked {
        return Grant::NoHooks;
    }
    let Some(exe) = exe else { return Grant::NoHooks };
    let bash_strict = !has("Bash");
    tools.push(serde_json::Value::String("Write".into()));
    *roles_json = Some(v.to_string());
    Grant::Granted { settings: settings(&hook_command(exe, dir, bash_strict)) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn input(tool: &str, field: &str, value: &str, cwd: &Path) -> String {
        serde_json::json!({"tool_name": tool, "tool_input": {field: value}, "cwd": cwd}).to_string()
    }

    #[test]
    fn writes_only_inside_the_report_folder() {
        let t = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(t.path()).unwrap();
        let rep = base.join("reports/abc");
        std::fs::create_dir_all(rep.join("evidence")).unwrap();
        let repo = base.join("repo");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        let d = |tool: &str, p: &str| decide(&input(tool, "file_path", p, &repo), &rep, true);
        assert_eq!(d("Write", &rep.join("report.json").to_string_lossy()), Decision::Allow);
        assert_eq!(d("Write", &rep.join("evidence/new.png").to_string_lossy()), Decision::Allow, "a new file passes");
        assert_eq!(d("Write", "src/a.ts"), Decision::Deny);
        assert_eq!(d("Edit", &repo.join("src/a.ts").to_string_lossy()), Decision::Deny);
        assert_eq!(d("Write", &format!("{}/../../repo/x", rep.display())), Decision::Deny);
        assert_eq!(d("Write", &rep.join("nofolder/x.json").to_string_lossy()), Decision::Deny, "unresolvable parent");
        assert_eq!(d("Write", &rep.to_string_lossy()), Decision::Deny);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(repo.join("src/a.ts"), rep.join("evidence/l.txt")).unwrap();
            assert_eq!(d("Write", &rep.join("evidence/l.txt").to_string_lossy()), Decision::Deny, "last part a link");
            std::os::unix::fs::symlink(&repo, rep.join("linkdir")).unwrap();
            assert_eq!(d("Write", &rep.join("linkdir/x").to_string_lossy()), Decision::Deny, "parent resolves outside");
        }
        assert_eq!(decide("not json", &rep, true), Decision::Deny);
        assert_eq!(decide(r#"{"tool_name":"Write","tool_input":{}}"#, &rep, true), Decision::Deny);
        assert_eq!(d("Read", "src/a.ts"), Decision::NoDecision);
    }

    #[test]
    fn shell_output_forms_are_denied_for_roles_without_bash() {
        let t = tempfile::tempdir().unwrap();
        let rep = t.path();
        let b = |cmd: &str, strict: bool| decide(&input("Bash", "command", cmd, rep), rep, strict);
        for bad in ["git diff --output=/tmp/x", "git log --output /tmp/x", "git diff > /tmp/x", "git diff >> x", "git diff | tee x", "git format-patch -o /tmp HEAD~1", "git format-patch --output-directory /tmp", "sh -c 'git diff'", "bash -c x", "git diff $(echo x)", "git diff `x`", "eval x"] {
            assert_eq!(b(bad, true), Decision::Deny, "{bad}");
        }
        for ok in ["git diff HEAD~1", "git log --oneline -5", "git show HEAD:src/a.ts", "git status --porcelain"] {
            assert_eq!(b(ok, true), Decision::NoDecision, "{ok}");
        }
        assert_eq!(b("npm test > evidence/o.txt", false), Decision::NoDecision, "QA's Bash keeps its prompts");
    }

    #[test]
    fn grant_adds_write_only_when_measured_and_hooks_work() {
        let roles = || Some(r#"{"dduck-reviewer":{"description":"d","prompt":"p","tools":["Read","Grep","Bash(git diff:*)"]}}"#.to_string());
        let dir = Path::new("/data/reports/abc");
        let exe = Path::new("/Applications/D.Duck.app/Contents/MacOS/dduck");
        let mut r = roles();
        assert_eq!(grant(&mut r, Some("dduck-reviewer"), Some(dir), Some(exe), false, false), Grant::NotMeasured);
        assert_eq!(r, roles(), "unchanged before spike 1.1");
        assert_eq!(grant(&mut r, Some("dduck-reviewer"), Some(dir), Some(exe), true, true), Grant::NoHooks);
        assert_eq!(r, roles());
        let Grant::Granted { settings } = grant(&mut r, Some("dduck-reviewer"), Some(dir), Some(exe), false, true) else { panic!() };
        assert!(r.as_deref().unwrap().contains(r#""Write""#));
        let cmd = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains(FLAG) && cmd.ends_with("'/data/reports/abc' 1"));
        // Merged with delivery-guard into ONE --settings argument.
        let ctx = crate::delivery_guard::GuardCtx { guard: crate::delivery_guard::Guard::LocalOnly, hook_cmd: Some("'/x' --dduck-guard-local-only".into()), ..crate::delivery_guard::GuardCtx::for_tests() };
        let args = crate::delivery_guard::guard_args_with(&ctx, &[settings]).unwrap();
        let s: Vec<_> = args.iter().filter(|a| a.to_string_lossy().starts_with("--settings=")).collect();
        assert_eq!(s.len(), 1);
        let v: serde_json::Value = serde_json::from_str(&s[0].to_string_lossy()["--settings=".len()..]).unwrap();
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(v["disableAllHooks"], false);
        // A role that can write needs nothing.
        let mut w = Some(r#"{"x":{"tools":["Read","Write"]}}"#.to_string());
        assert_eq!(grant(&mut w, Some("x"), Some(dir), Some(exe), false, true), Grant::NotNeeded);
    }

    #[test]
    fn output_json() {
        assert!(output(Decision::Allow).0.contains(r#""permissionDecision":"allow""#) && output(Decision::Allow).1 == 0);
        assert_eq!(output(Decision::Deny).1, 2);
        assert_eq!(output(Decision::NoDecision), (String::new(), 0));
    }
}
