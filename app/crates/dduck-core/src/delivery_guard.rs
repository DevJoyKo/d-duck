//! 전달 범위 (delivery-guard, D-142·D-143): how far a delegated task may ship its work.
//!
//! - `draft_pr` (default): commit and push the work branch, open a draft PR. Never merge, mark ready, or push the
//!   default branch. Enforced with `--disallowedTools` prefix rules (best effort, documented gaps).
//! - `local_only` (projects the user ticked in "원격 push 막기"): no push, no `gh`, no GitHub-like MCP. Enforced with
//!   the deny rules plus an app-owned PreToolUse hook (`--settings=`), which fails closed.
//!
//! Everything injected is built here from constants; the webview never supplies rules. Scope lookups return
//! `Result`: a caller that cannot tell the scope must refuse the start, never fall back to `draft_pr`.

use crate::platform::CanonExt;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::Error;

/// Flag that turns the D.Duck executable into the local-only PreToolUse hook.
pub const LOCAL_ONLY_FLAG: &str = "--dduck-guard-local-only";
/// settings key: `[{folder, root}]` JSON. Only the dedicated functions below write it (never the generic setter).
pub const LOCAL_ONLY_PROJECTS: &str = "local_only_projects";
pub const MAX_LOCAL_ONLY: usize = 200;
/// Deny reason the hook prints (fixed text, never the command).
pub const HOOK_REASON: &str = "로컬만 프로젝트라 원격 작업은 막혀요";
pub const UNREADABLE: &str = "원격 push 막기 설정을 못 읽었어요";

/// `[안전]` line appended to the team rules for `local_only` tasks.
pub const LOCAL_ONLY_RULE: &str = "원격으로 push하거나 PR을 열지 않는다 · 로컬 작업까지만 한다. 끝나면 커밋한 브랜치 이름을 보고한다. 커밋 메시지에 push라는 낱말이 있으면 막히니 `git commit -F <파일>`을 쓴다.";
/// [`LOCAL_ONLY_RULE`] in English (app-language, D-171): sent when the task starts in English.
pub const LOCAL_ONLY_RULE_EN: &str = "Do not push to a remote or open a PR · keep the work local. When done, report the name of the branch you committed to. A commit message containing the word push is blocked, so use `git commit -F <file>`.";

/// The local-only line in the start language.
pub fn local_only_rule(lang: crate::i18n::Lang) -> &'static str {
    if lang == crate::i18n::Lang::En { LOCAL_ONLY_RULE_EN } else { LOCAL_ONLY_RULE }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guard {
    DraftPr,
    LocalOnly,
}

impl Guard {
    pub fn code(self) -> &'static str {
        match self {
            Guard::DraftPr => "draft_pr",
            Guard::LocalOnly => "local_only",
        }
    }
    pub fn from_code(s: &str) -> Option<Self> {
        match s {
            "draft_pr" => Some(Guard::DraftPr),
            "local_only" => Some(Guard::LocalOnly),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Deny rules

static BRANCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9._/-]{1,100}\z").unwrap_or_else(|_| unreachable!()));

/// A branch name safe to put inside a permission rule: no spaces, parentheses or commas, no option look-alike.
pub fn valid_branch(b: &str) -> bool {
    BRANCH_RE.is_match(b) && !b.starts_with('-') && !b.contains("..") && !b.ends_with('/') && !b.starts_with('/')
}

const BASE_RULES: &[&str] = &[
    // merge / ready
    "Bash(gh pr merge *)",
    "Bash(gh pr ready *)",
    "Bash(gh api *merge*)",
    "Bash(gh api *ReadyForReview*)",
    "Bash(gh api *draft=false*)",
    "mcp__*__merge_pull_request",
    // Direct commits through the API (can target the default branch) (M1).
    "mcp__*__push_files",
    "mcp__*__create_or_update_file",
    // `gh --repo o/r pr merge`, `gh -R o/r pr ready`: global flags before the subcommand.
    "Bash(gh * pr merge*)",
    "Bash(gh * pr ready*)",
    // Write methods on the REST API (contents, merges, pulls/N/merge, refs …), any spelling gh accepts.
    "Bash(gh api *-X*PUT*)",
    "Bash(gh api *-X*PATCH*)",
    "Bash(gh api *-X*DELETE*)",
    "Bash(gh api *-X*put*)",
    "Bash(gh api *-X*patch*)",
    "Bash(gh api *-X*delete*)",
    "Bash(gh api *--method*PUT*)",
    "Bash(gh api *--method*PATCH*)",
    "Bash(gh api *--method*DELETE*)",
    "Bash(gh api *merges*)",
    "mcp__*__update_pull_request(draft:false)",
    "mcp__*__create_pull_request(draft:false)",
    // everything at once
    "Bash(git push --all*)",
    "Bash(git push * --all*)",
    "Bash(git push --mirror*)",
    "Bash(git push * --mirror*)",
    // force (`--force-with-lease` stays allowed: the space / end boundary does not match it)
    "Bash(git push --force *)",
    "Bash(git push * --force)",
    "Bash(git push * --force *)",
    "Bash(git push -f *)",
    "Bash(git push * -f)",
    "Bash(git push * -f *)",
    "Bash(git push * +*)",
    // Git forms that write files or run a program although `GIT_READ` allows the command (security HIGH):
    // `--output=<f>` / `--output <f>` on diff/log/show (verified with git 2.43; `-o` is rejected there and long
    // options cannot be abbreviated), format-patch `-o`/`--output-directory`, grep `-O`/`--open-files-in-pager`.
    "Bash(git * --output*)",
    "Bash(git *--output=*)",
    "Bash(git format-patch -o*)",
    "Bash(git format-patch * -o*)",
    "Bash(git grep -O*)",
    "Bash(git grep * -O*)",
    "Bash(git * --open-files-in-pager*)",
];

/// Extra rules when the task starts on the default branch (routines, skill authoring: no worktree there).
const ON_DEFAULT_RULES: &[&str] = &["Bash(git push)", "Bash(git push origin)", "Bash(git push * HEAD*)"];

/// GitHub MCP write tools (official github-mcp-server names), denied by name glob for `local_only`.
pub const GITHUB_WRITE_TOOLS: &[&str] = &[
    "create_pull_request",
    "merge_pull_request",
    "push_files",
    "update_pull_request",
    "update_pull_request_branch",
    "create_or_update_file",
    "delete_file",
    "create_branch",
    "create_repository",
    "fork_repository",
    "create_issue",
    "update_issue",
    "add_issue_comment",
    "create_pull_request_review",
    "add_comment_to_pending_review",
    "submit_pending_pull_request_review",
    "request_copilot_review",
];

const LOCAL_ONLY_RULES: &[&str] = &[
    "Bash(git push *)",
    "Bash(git * push*)",
    "Bash(gh *)",
    "Bash(hub *)",
    "mcp__github__*",
    // Files that make git run code or push elsewhere: hooks, `.husky`, the repository config (aliases, URLs).
    // `Edit(...)` covers every file-writing tool; CLI 2.1.319 warns that `Write(...)` deny rules never match (D-163).
    "Edit(**/.git/hooks/**)",
    "Edit(**/.husky/**)",
    "Edit(**/.git/config)",
    "Edit(~/.gitconfig)",
    "Edit(~/.config/git/**)",
    "Bash(git config *alias*)",
    "Bash(git config *url.*)",
    "Bash(git config *include*)",
    "Bash(git config *hooksPath*)",
    "Bash(git * core.hooksPath*)",
];

/// Environment for every process of a `local_only` session (`--settings` `env`), the backstop for what no rule sees
/// (scripts, interpreters, Makefiles): every git remote URL — fetch, push and `pushurl` — is rewritten to the
/// `dduck-blocked://` transport and that transport is never allowed, so git fails before any network or helper
/// (verified with git 2.43 and a local bare remote). `gh` gets a token that cannot authenticate.
pub fn backstop_env(global_config: Option<&str>) -> serde_json::Value {
    // Command-scope config (`GIT_CONFIG_COUNT`) outranks system, global and repository config for the same key.
    // URL rewrites lose to a longer `insteadOf` prefix from another file, so the real stop is `protocol.*.allow =
    // never` for every transport (a per-protocol key would otherwise beat `protocol.allow`): whatever URL a rewrite
    // produces, git refuses its transport before any network or helper. The user's global and system config are
    // not read at all (a planted `~/.gitconfig` rule cannot apply); identity comes from the app-owned copy.
    let mut keys: Vec<(&str, &str)> = vec![
        ("url.dduck-blocked://.insteadOf", ""),
        ("url.dduck-blocked://.pushInsteadOf", ""),
        ("protocol.allow", "never"),
    ];
    for proto in ["dduck-blocked", "file", "git", "ssh", "http", "https", "ext", "fd"] {
        keys.push((PROTO_KEYS.iter().find(|(p, _)| *p == proto).map_or("", |(_, k)| k), "never"));
    }
    let mut env = serde_json::Map::new();
    env.insert("GIT_CONFIG_COUNT".into(), keys.len().to_string().into());
    for (n, (k, v)) in keys.iter().enumerate() {
        env.insert(format!("GIT_CONFIG_KEY_{n}"), (*k).into());
        env.insert(format!("GIT_CONFIG_VALUE_{n}"), (*v).into());
    }
    env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    // Git reads this before `protocol.*.allow`: only the blocked transport is allowed.
    env.insert("GIT_ALLOW_PROTOCOL".into(), "dduck-blocked".into());
    env.insert("GIT_CONFIG_GLOBAL".into(), global_config.unwrap_or("/dev/null").into());
    for t in ["GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"] {
        env.insert(t.into(), BLOCKED_TOKEN.into());
    }
    serde_json::Value::Object(env)
}

const PROTO_KEYS: &[(&str, &str)] = &[
    ("dduck-blocked", "protocol.dduck-blocked.allow"),
    ("file", "protocol.file.allow"),
    ("git", "protocol.git.allow"),
    ("ssh", "protocol.ssh.allow"),
    ("http", "protocol.http.allow"),
    ("https", "protocol.https.allow"),
    ("ext", "protocol.ext.allow"),
    ("fd", "protocol.fd.allow"),
];

/// Where the app keeps the local-only global git config (inside its own data folder).
pub fn global_config_path(home: &Path) -> PathBuf {
    crate::platform::app_data_under(home).join("local-only-gitconfig")
}

/// Writes the local-only global git config: only identity and harmless keys copied from the user's own global config
/// (read with the app's hardened git), mode 0600. No URL, alias, include, helper or program key ever goes in.
pub fn write_global_config(path: &Path, home: &Path) -> Result<(), Error> {
    const KEYS: &[(&str, &str, &str)] = &[("user", "name", "user.name"), ("user", "email", "user.email"), ("core", "autocrlf", "core.autocrlf"), ("core", "excludesFile", "core.excludesfile"), ("init", "defaultBranch", "init.defaultbranch")];
    let mut out = String::from("# D.Duck local-only session config (identity only). Not the user's ~/.gitconfig.\n");
    let mut section = "";
    for (sec, name, key) in KEYS {
        // `--global` is `$HOME/.gitconfig` (or XDG) of the given home, read with the app's hardened git.
        let mut cmd = app_git(home);
        cmd.args(["config", "--global", "--get", key]).env("HOME", home).env_remove("GIT_CONFIG_GLOBAL");
        let got = crate::process::run(&mut cmd, Duration::from_secs(2), 16 * 1024, crate::process::Keep::Head, 1024).map_err(|_| Error::Config("git did not run"))?;
        let v = got.status.success().then(|| String::from_utf8_lossy(&got.stdout).trim().to_string());
        let Some(v) = v.filter(|v| !v.is_empty() && v.len() <= 200 && !v.chars().any(|c| c.is_control())) else { continue };
        if *sec != section {
            out.push_str(&format!("[{sec}]\n"));
            section = sec;
        }
        out.push_str(&format!("\t{name} = \"{}\"\n", v.replace('\\', "\\\\").replace('"', "\\\"")));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Not a token format GitHub issues: requests with it fail authentication.
pub const BLOCKED_TOKEN: &str = "dduck-local-only-no-remote";

/// Default-branch names: `main`, `master`, plus the detected one when it is a safe name.
pub fn default_branches(detected: Option<&str>) -> Vec<String> {
    let mut out = vec!["main".to_string(), "master".to_string()];
    if let Some(d) = detected.filter(|d| valid_branch(d)) {
        if !out.iter().any(|b| b == d) {
            out.push(d.to_string());
        }
    }
    out
}

/// The task starts on a default branch (a detached HEAD counts as one).
pub fn on_default(start_branch: Option<&str>, detected: Option<&str>) -> bool {
    match start_branch {
        None => true,
        Some(b) => default_branches(detected).iter().any(|d| d == b),
    }
}

/// The `--disallowedTools` list. `start_branch` `None` = detached HEAD (treated as default); not a repository at all
/// is `on_default = false` at the call site via [`GuardCtx::no_repo`].
pub fn deny_rules(guard: Guard, start_on_default: bool, detected_default: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = BASE_RULES.iter().map(|s| s.to_string()).collect();
    for b in default_branches(detected_default) {
        out.push(format!("Bash(git push * {b})"));
        out.push(format!("Bash(git push * {b} *)"));
        out.push(format!("Bash(git push *:{b})"));
        out.push(format!("Bash(git push *refs/heads/{b}*)"));
    }
    if start_on_default {
        out.extend(ON_DEFAULT_RULES.iter().map(|s| s.to_string()));
    }
    if guard == Guard::LocalOnly {
        out.extend(LOCAL_ONLY_RULES.iter().map(|s| s.to_string()));
        out.extend(GITHUB_WRITE_TOOLS.iter().map(|t| format!("mcp__*__{t}")));
    }
    if cfg!(windows) {
        out = windows_variants(out);
    }
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|r| seen.insert(r.clone()));
    out
}

/// Windows additions (decision 9, re-check fixes): the `PowerShell` and `Monitor` tools are off, so Bash (Git Bash)
/// is the only shell tool the `Bash(...)` rules cover; Bash commands that start another shell are refused; and each
/// `git`/`gh` rule also gets its `git.exe`/`gh.exe` form. Absolute-path and letter-case variants of git stay a known
/// gap, as on macOS. Real enforcement by the Windows CLI is a [manual] scenario.
pub fn windows_variants(rules: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(rules.len() * 2 + 6);
    for r in rules {
        let exe = ["git", "gh"].iter().find_map(|t| r.strip_prefix(&format!("Bash({t} ")).map(|rest| format!("Bash({t}.exe {rest}")));
        out.push(r);
        out.extend(exe);
    }
    out.extend(WINDOWS_RULES.iter().map(|s| s.to_string()));
    out
}

/// Tools and shell launches refused on Windows on top of the shared rules.
pub const WINDOWS_RULES: &[&str] = &["PowerShell", "Monitor", "Bash(powershell*)", "Bash(pwsh*)", "Bash(cmd *)", "Bash(cmd.exe *)"];

/// Shell-quoted hook command: `'<exe>' --dduck-guard-local-only`.
pub fn hook_command(exe: &Path) -> String {
    let quoted = exe.to_string_lossy().replace('\'', "'\\''");
    format!("'{quoted}' {LOCAL_ONLY_FLAG}")
}

/// `--settings` JSON with the local-only PreToolUse hook. `onFailure: block` makes a crash or timeout deny (hooks
/// docs, 2.1.295+); `disableAllHooks: false` outranks a user or project `true` (flag settings win).
pub fn hook_settings(hook_cmd: &str) -> String {
    hook_settings_value(hook_cmd, None).to_string()
}

fn hook_settings_value(hook_cmd: &str, global_config: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "disableAllHooks": false,
        "env": backstop_env(global_config),
        "hooks": { "PreToolUse": [ { "matcher": "*", "hooks": [
            { "type": "command", "command": hook_cmd, "timeout": 10, "onFailure": "block" }
        ] } ] }
    })
}

/// Merges `--settings` fragments into ONE object (the CLI takes one `--settings`; a second flag could replace the
/// first). `hooks.<Event>` arrays are concatenated in order; `disableAllHooks` is false when any fragment says so
/// (a guard hook must never be switched off by another fragment); other keys: the first fragment wins.
pub fn merge_settings(parts: &[serde_json::Value]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let mut hooks = serde_json::Map::new();
    let mut disable: Option<bool> = None;
    for part in parts {
        let Some(obj) = part.as_object() else { continue };
        for (k, v) in obj {
            match k.as_str() {
                "hooks" => {
                    if let Some(events) = v.as_object() {
                        for (event, list) in events {
                            let slot = hooks.entry(event.clone()).or_insert_with(|| serde_json::Value::Array(Vec::new()));
                            if let (Some(dst), Some(src)) = (slot.as_array_mut(), list.as_array()) {
                                dst.extend(src.iter().cloned());
                            }
                        }
                    }
                }
                "env" => {
                    let slot = out.entry("env".to_string()).or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                    if let (Some(dst), Some(src)) = (slot.as_object_mut(), v.as_object()) {
                        for (ek, ev) in src {
                            dst.entry(ek.clone()).or_insert_with(|| ev.clone());
                        }
                    }
                }
                "disableAllHooks" => {
                    let b = v.as_bool().unwrap_or(true);
                    disable = Some(disable.map_or(b, |d| d && b));
                }
                _ => {
                    out.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
    }
    if let Some(d) = disable {
        out.insert("disableAllHooks".into(), serde_json::Value::Bool(d));
    }
    if !hooks.is_empty() {
        out.insert("hooks".into(), serde_json::Value::Object(hooks));
    }
    serde_json::Value::Object(out)
}

/// Everything the scope arguments need, computed by the shell right before a start or resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardCtx {
    pub guard: Guard,
    pub start_on_default: bool,
    pub default_branch: Option<String>,
    /// Required for `local_only` (the app executable as the hook).
    pub hook_cmd: Option<String>,
    /// `local_only`: the app-owned global git config the session uses instead of the user's (`GIT_CONFIG_GLOBAL`).
    /// `None` = `/dev/null` (no global config at all).
    pub global_config: Option<String>,
}

impl GuardCtx {
    /// Tests and examples only: the default scope with nothing detected.
    pub fn for_tests() -> Self {
        GuardCtx { guard: Guard::DraftPr, start_on_default: false, default_branch: None, hook_cmd: None, global_config: None }
    }
}

/// `--disallowedTools=` (always) and, for `local_only`, `--settings=` with the hook. Shared by `TaskSpec::bg_args`
/// and `ResumeSpec::args`. `local_only` without a hook command is an error (the caller refuses the start).
pub fn guard_args(ctx: &GuardCtx) -> Result<Vec<OsString>, &'static str> {
    guard_args_with(ctx, &[])
}

/// [`guard_args`] plus other `--settings` fragments (e.g. a report-dir hook later), merged into one `--settings=`.
pub fn guard_args_with(ctx: &GuardCtx, extra_settings: &[serde_json::Value]) -> Result<Vec<OsString>, &'static str> {
    let rules = deny_rules(ctx.guard, ctx.start_on_default, ctx.default_branch.as_deref());
    let mut args: Vec<OsString> = vec![format!("--disallowedTools={}", rules.join(",")).into()];
    let mut parts: Vec<serde_json::Value> = Vec::new();
    if ctx.guard == Guard::LocalOnly {
        let cmd = ctx.hook_cmd.as_deref().filter(|c| !c.is_empty()).ok_or(HOOK_MISSING)?;
        parts.push(hook_settings_value(cmd, ctx.global_config.as_deref()));
    }
    parts.extend(extra_settings.iter().cloned());
    if !parts.is_empty() {
        args.push(format!("--settings={}", merge_settings(&parts)).into());
    }
    Ok(args)
}

pub const HOOK_MISSING: &str = "로컬만 작업에 필요한 D.Duck 훅을 준비하지 못했어요";
pub const MANAGED_BLOCKS: &str = "관리 설정이 D.Duck 훅을 막아 로컬만 작업을 시작할 수 없어요";
pub const DANGER_REFUSED: &str = "로컬만 프로젝트는 이 권한 모드를 아직 쓸 수 없어요";
/// Spike 1.1 measured (D-163, CLI 2.1.319): in auto and bypassPermissions the deny rules, the PreToolUse hook and the
/// env backstop each block a push on their own, in `-p` and in `--bg` starts.
pub const DANGER_MODES_MEASURED: bool = true;

/// What a start (or resume) needs, computed right before it. Any error refuses the start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCtx {
    pub ctx: GuardCtx,
    /// Turn the worktree on (the task would start on the default branch and a worktree is possible).
    pub force_worktree: bool,
    pub default_branch: Option<String>,
    /// Local head of the default branch at start, for the post-check.
    pub default_head: Option<String>,
}

/// The scope for a start in `cwd`. `hook_exe` = the app executable (the hook); `None` leaves `local_only` without a
/// hook, which `guard_args` then refuses. With a hook it is self-tested first (a hook that cannot run would be a
/// non-blocking error in the CLI).
/// Whether a start may be moved into a worktree (and so forced into one on the default branch): a repository with a
/// commit that isn't the home folder. Decided by the folder, not by where the draft came from (test review C1), so a
/// news or insight draft in a project on its default branch gets its own branch like any other task.
pub fn worktree_possible(cwd: &Path, home: &Path) -> bool {
    let canon = |p: &Path| p.canon().unwrap_or_else(|_| p.to_path_buf());
    canon(cwd) != canon(home) && crate::team::repo_status(cwd, home) == crate::team::RepoStatus::Ok
}

pub fn start_ctx(conn: &Connection, cwd: &Path, home: Option<&Path>, worktree_possible: bool, hook_exe: Option<&Path>) -> Result<StartCtx, &'static str> {
    start_ctx_with(conn, cwd, home, worktree_possible, hook_exe, None)
}

/// [`start_ctx`] with the local-only global git config path (written here for a `local_only` start).
pub fn start_ctx_with(conn: &Connection, cwd: &Path, home: Option<&Path>, worktree_possible: bool, hook_exe: Option<&Path>, global_config: Option<&Path>) -> Result<StartCtx, &'static str> {
    let items = load_items(conn).map_err(|_| UNREADABLE)?;
    start_ctx_for_items(&items, cwd, home, worktree_possible, hook_exe, global_config)
}

/// [`start_ctx_with`] from an already-read "로컬만" list, so the caller need not hold the DB during the git reads and
/// the hook self-test (up to 10 s) that follow (D-161).
pub fn start_ctx_for_items(items: &[LocalOnlyItem], cwd: &Path, home: Option<&Path>, worktree_possible: bool, hook_exe: Option<&Path>, global_config: Option<&Path>) -> Result<StartCtx, &'static str> {
    let folder = cwd.canon().map_err(|_| UNREADABLE)?;
    let repo = repo_info(&folder, home).map_err(|_| UNREADABLE)?;
    let guard = guard_for(&folder, repo.as_ref(), items);
    let (detected, on_default, head) = match &repo {
        Some(r) => {
            let detected = detect_default_branch(&r.root).map_err(|_| UNREADABLE)?;
            let start = start_branch(&folder).map_err(|_| UNREADABLE)?;
            let on = on_default(start.as_deref(), detected.as_deref());
            let b = detected.clone().unwrap_or_else(|| if branch_head(&r.root, "main").ok().flatten().is_some() { "main".into() } else { "master".into() });
            let head = branch_head(&r.root, &b).map_err(|_| UNREADABLE)?;
            (detected, on, head)
        }
        None => (None, false, None),
    };
    let force_worktree = on_default && worktree_possible;
    let hook_cmd = match (guard, hook_exe) {
        (Guard::LocalOnly, Some(exe)) => {
            let cmd = hook_command(exe);
            if !crate::cloud_routines::guard_self_test_denies(&cmd, SELF_TEST_DENY) {
                return Err(HOOK_MISSING);
            }
            Some(cmd)
        }
        _ => None,
    };
    let global = match (guard, global_config, home) {
        (Guard::LocalOnly, Some(path), Some(h)) => {
            write_global_config(path, h).map_err(|_| UNREADABLE)?;
            Some(path.to_string_lossy().into_owned())
        }
        _ => None,
    };
    Ok(StartCtx {
        ctx: GuardCtx { guard, start_on_default: on_default && !force_worktree, default_branch: detected.clone(), hook_cmd, global_config: global },
        force_worktree,
        default_branch: detected,
        default_head: head,
    })
}

/// Refusals for a `local_only` start that do not depend on the folder: dangerous modes before spike 1.1, and
/// managed settings that switch user hooks off (`allowManagedHooksOnly`, `disableAllHooks`; unreadable = worst).
pub fn local_only_refusal(guard: Guard, dangerous_mode: bool, managed: &ManagedSources) -> Option<&'static str> {
    if guard != Guard::LocalOnly {
        return None;
    }
    if dangerous_mode && !DANGER_MODES_MEASURED {
        return Some(DANGER_REFUSED);
    }
    managed.blocks_hooks().then_some(MANAGED_BLOCKS)
}

pub const MANAGED_SETTINGS: &str = "/Library/Application Support/ClaudeCode/managed-settings.json";

/// Every managed source that could switch the app's hook off (managed-settings docs): the system file and its
/// `managed-settings.d/` drop-ins, the `com.anthropic.claudecode` MDM plist (machine and per user), and the
/// server-managed cache (`remote-settings.json` in the Claude config folder). A source that exists but cannot be
/// read or parsed counts as blocking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSources {
    pub files: Vec<PathBuf>,
    pub dropin_dirs: Vec<PathBuf>,
    pub plists: Vec<PathBuf>,
}

impl ManagedSources {
    /// The macOS locations, for this user and Claude config folder.
    pub fn system(config_dir: Option<&Path>, user: Option<&str>) -> Self {
        let mut files = vec![PathBuf::from(MANAGED_SETTINGS)];
        if let Some(c) = config_dir {
            files.push(c.join("remote-settings.json"));
        }
        let mut plists = vec![PathBuf::from("/Library/Managed Preferences/com.anthropic.claudecode.plist")];
        if let Some(u) = user.filter(|u| !u.is_empty() && !u.contains('/')) {
            plists.push(PathBuf::from(format!("/Library/Managed Preferences/{u}/com.anthropic.claudecode.plist")));
        }
        ManagedSources { files, dropin_dirs: vec![PathBuf::from("/Library/Application Support/ClaudeCode/managed-settings.d")], plists }
    }

    /// From the environment: `CLAUDE_CONFIG_DIR` or `~/.claude`, and the login name.
    pub fn from_env(config_dir: Option<&Path>) -> Self {
        let cfg = config_dir.map(Path::to_path_buf).or_else(|| std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from)).or_else(|| crate::platform::home().map(|h| h.join(".claude")));
        let user = crate::platform::user_name();
        Self::system(cfg.as_deref(), user.as_deref())
    }

    pub fn blocks_hooks(&self) -> bool {
        fn locks(v: &serde_json::Value) -> bool {
            v.get("allowManagedHooksOnly").and_then(|x| x.as_bool()) == Some(true) || v.get("disableAllHooks").and_then(|x| x.as_bool()) == Some(true)
        }
        fn json_file(p: &Path) -> Option<bool> {
            match std::fs::read_to_string(p) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => Some(true),
                Ok(text) => Some(serde_json::from_str::<serde_json::Value>(&text).map_or(true, |v| !v.is_object() || locks(&v))),
            }
        }
        for f in &self.files {
            if json_file(f) == Some(true) {
                return true;
            }
        }
        for d in &self.dropin_dirs {
            match std::fs::read_dir(d) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return true,
                Ok(entries) => {
                    for e in entries {
                        let Ok(e) = e else { return true };
                        let p = e.path();
                        if p.extension().is_some_and(|x| x == "json") && json_file(&p) != Some(false) {
                            return true;
                        }
                    }
                }
            }
        }
        for p in &self.plists {
            match std::fs::symlink_metadata(p) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return true,
                Ok(_) => {}
            }
            let mut cmd = Command::new("/usr/bin/plutil");
            cmd.args(["-convert", "json", "-o", "-"]).arg(p);
            let ok = match crate::process::run(&mut cmd, Duration::from_secs(3), 4 * 1024 * 1024, crate::process::Keep::Head, 4 * 1024) {
                Ok(out) if out.status.success() => serde_json::from_slice::<serde_json::Value>(&out.stdout).is_ok_and(|v| v.is_object() && !locks(&v)),
                _ => false,
            };
            if !ok {
                return true;
            }
        }
        false
    }
}

/// Samples for the start-time self-test of the hook command (the CLI treats a hook that cannot start as a
/// non-blocking error, measured for the cloud-routine guard with 2.1.319).
pub const SELF_TEST_DENY: &str = r#"{"tool_name":"Bash","tool_input":{"command":"git -C . push origin x"}}"#;
pub const SELF_TEST_PASS: &str = r#"{"tool_name":"Read","tool_input":{"file_path":"/tmp/x"}}"#;

// ---------------------------------------------------------------------------------------------------------------
// Paths and repositories

/// Comparison key: NFC, lowercase (APFS is case-insensitive by default; matching more is the safe side).
pub fn norm_key(p: &Path) -> String {
    let s: String = p.to_string_lossy().nfc().collect();
    // Windows: `/` between parts, so `key_within` reads `C:/Users/me/app` like a macOS path (decision 1).
    let s = if cfg!(windows) { s.replace('\\', "/") } else { s };
    let s = s.to_lowercase();
    let t = s.trim_end_matches('/');
    if t.is_empty() { "/".into() } else { t.to_string() }
}

/// `child` is `parent` or below it, by path components on the comparison keys.
pub fn key_within(child: &str, parent: &str) -> bool {
    child == parent || (parent == "/" && child.starts_with('/')) || child.strip_prefix(parent).is_some_and(|r| r.starts_with('/'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    /// The main working tree (for a worktree: the repository it belongs to).
    pub root: PathBuf,
    /// The shared git dir (`commondir`).
    pub common_dir: PathBuf,
}

const MAX_GITFILE: u64 = 64 * 1024;

fn read_small(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let f = std::fs::File::open(p)?;
    let mut s = String::new();
    f.take(MAX_GITFILE).read_to_string(&mut s)?;
    Ok(s)
}

/// Finds the repository of a canonical folder: walks up looking for `.git` (a directory, or a file pointing at a
/// worktree git dir whose `commondir` is the shared one). Stops below `home` (the home folder itself is never a
/// project root) and after 40 levels. `Ok(None)` = not inside a repository; a `.git` that cannot be read is `Err`.
pub fn repo_info(cwd: &Path, home: Option<&Path>) -> Result<Option<RepoInfo>, Error> {
    let home_key = home.and_then(|h| h.canon().ok()).map(|h| norm_key(&h));
    let mut cur = Some(cwd);
    let mut depth = 0;
    while let Some(dir) = cur {
        if depth > 40 || home_key.as_deref() == Some(norm_key(dir).as_str()) {
            return Ok(None);
        }
        let dot = dir.join(".git");
        match std::fs::symlink_metadata(&dot) {
            Ok(m) if m.is_dir() => return Ok(Some(RepoInfo { root: dir.to_path_buf(), common_dir: dot })),
            Ok(m) if m.is_file() => {
                let text = read_small(&dot)?;
                let line = text.lines().next().unwrap_or("");
                let target = line.strip_prefix("gitdir:").map(str::trim).filter(|t| !t.is_empty()).ok_or(Error::Config("unreadable .git file"))?;
                let gitdir = dir.join(target).canon()?;
                let common = match read_small(&gitdir.join("commondir")) {
                    Ok(c) => {
                        let c = c.trim();
                        if c.is_empty() {
                            return Err(Error::Config("unreadable commondir"));
                        }
                        gitdir.join(c).canon()?
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => gitdir.clone(),
                    Err(e) => return Err(e.into()),
                };
                // A worktree's shared dir is `<root>/.git`; a submodule's git dir has no commondir and stays here.
                let root = if common != gitdir && common.file_name().is_some_and(|n| n == ".git") {
                    common.parent().map(Path::to_path_buf).unwrap_or_else(|| dir.to_path_buf())
                } else {
                    dir.to_path_buf()
                };
                return Ok(Some(RepoInfo { root, common_dir: common }));
            }
            Ok(_) => return Err(Error::Config("unexpected .git entry")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        cur = dir.parent();
        depth += 1;
    }
    Ok(None)
}

/// One ticked project: the folder and its repository root (the folder itself when it is not in a repository).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalOnlyItem {
    pub folder: String,
    pub root: String,
    /// `host/owner/repo` of its `origin` when ticked, so a second clone or a moved repository stays local-only (M3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

/// The `origin` slug of a repository (offline, from its config file).
pub fn repo_slug(repo: &RepoInfo) -> Option<String> {
    origin_url(&repo.common_dir).as_deref().and_then(remote_slug)
}

/// The scope for a canonical task folder: `local_only` when its repository root equals a ticked root, or the folder
/// is (under) a ticked folder.
pub fn guard_for(cwd: &Path, repo: Option<&RepoInfo>, items: &[LocalOnlyItem]) -> Guard {
    let cwd_key = norm_key(cwd);
    let root_key = repo.map(|r| norm_key(&r.root));
    let hit = items.iter().any(|it| {
        let folder = norm_key(Path::new(&it.folder));
        let root = norm_key(Path::new(&it.root));
        key_within(&cwd_key, &folder) || root_key.as_deref() == Some(root.as_str())
    });
    if hit {
        return Guard::LocalOnly;
    }
    // Same `origin` as a ticked project (another clone, a moved folder): the stored slug, or the ticked
    // repository's current one for items saved before slugs were kept.
    let Some(mine) = repo.and_then(repo_slug) else { return Guard::DraftPr };
    let same = items.iter().any(|it| {
        it.slug.clone().or_else(|| repo_info(Path::new(&it.root), None).ok().flatten().as_ref().and_then(repo_slug)).is_some_and(|s| s == mine)
    });
    if same { Guard::LocalOnly } else { Guard::DraftPr }
}

/// A folder the user may tick: an existing directory that is not a symlink. Returns the canonical path.
pub fn checked_folder(p: &Path) -> Option<PathBuf> {
    if !p.is_absolute() {
        return None;
    }
    let meta = std::fs::symlink_metadata(p).ok()?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return None;
    }
    p.canon().ok()
}

// ---------------------------------------------------------------------------------------------------------------
// Storage (settings table). Read failures are errors: the caller refuses to start.

pub fn load_items(conn: &Connection) -> Result<Vec<LocalOnlyItem>, Error> {
    match crate::settings::get(conn, LOCAL_ONLY_PROJECTS)? {
        None => Ok(Vec::new()),
        Some(v) => {
            let items: Vec<LocalOnlyItem> = serde_json::from_str(&v).map_err(|_| Error::Config("local_only_projects is malformed"))?;
            if items.len() > MAX_LOCAL_ONLY || items.iter().any(|i| !Path::new(&i.folder).is_absolute() || !Path::new(&i.root).is_absolute()) {
                return Err(Error::Config("local_only_projects is malformed"));
            }
            Ok(items)
        }
    }
}

fn save_items(conn: &Connection, items: &[LocalOnlyItem]) -> Result<(), Error> {
    let v = serde_json::to_string(items).map_err(|_| Error::Config("local_only_projects"))?;
    crate::settings::put_guarded(conn, LOCAL_ONLY_PROJECTS, &v)
}

/// Ticks a canonical folder (the safer direction, no confirmation). Stores its repository root too.
pub fn add(conn: &Connection, folder: &Path, home: Option<&Path>) -> Result<(), Error> {
    let mut items = load_items(conn)?;
    let key = norm_key(folder);
    if items.iter().any(|i| norm_key(Path::new(&i.folder)) == key) {
        return Ok(());
    }
    if items.len() >= MAX_LOCAL_ONLY {
        return Err(Error::Config("too many local-only projects"));
    }
    let repo = repo_info(folder, home)?;
    let slug = repo.as_ref().and_then(repo_slug);
    let root = repo.map(|r| r.root).unwrap_or_else(|| folder.to_path_buf());
    items.push(LocalOnlyItem { folder: folder.to_string_lossy().into_owned(), root: root.to_string_lossy().into_owned(), slug });
    save_items(conn, &items)
}

/// Unticks a folder. Call only after the shell's native confirmation.
pub fn remove(conn: &Connection, folder: &Path) -> Result<(), Error> {
    let mut items = load_items(conn)?;
    let key = norm_key(folder);
    items.retain(|i| norm_key(Path::new(&i.folder)) != key);
    save_items(conn, &items)
}

/// Is this folder ticked itself (for the checkbox state)?
pub fn is_ticked(items: &[LocalOnlyItem], folder: &Path) -> bool {
    let key = norm_key(folder);
    items.iter().any(|i| norm_key(Path::new(&i.folder)) == key)
}

/// The scope of a canonical task folder from the stored list.
pub fn guard_for_folder(conn: &Connection, cwd: &Path, home: Option<&Path>) -> Result<Guard, Error> {
    let items = load_items(conn)?;
    if items.is_empty() {
        return Ok(Guard::DraftPr);
    }
    let repo = repo_info(cwd, home)?;
    Ok(guard_for(cwd, repo.as_ref(), &items))
}

// ---------------------------------------------------------------------------------------------------------------
// Local git (no network: symbolic refs, rev-parse, config, reflog files)

/// A git process the app runs with its own rights: a planted `.git/config` must not run code (fsmonitor, hooks, and
/// filter drivers: `git status` runs a repo's `filter.<x>.clean`/`process` on racily-clean files) and the system
/// config is not read. No prompts, no optional locks. Every configured filter driver is blanked with `-c`; when the
/// filter keys cannot be listed safely the command is made to fail (fail closed).
pub fn app_git(dir: &Path) -> Command {
    let mut cmd = base_git(dir);
    match filter_names(dir) {
        Some(names) => {
            for n in names {
                for k in ["clean", "smudge", "process"] {
                    cmd.arg("-c").arg(format!("filter.{n}.{k}="));
                }
                cmd.arg("-c").arg(format!("filter.{n}.required=false"));
            }
        }
        // Windows: a "nonexistent" path like C:\nonexistent could be created by another user, so nothing runs.
        None if cfg!(windows) => return refused_git(),
        None => {
            cmd.env("GIT_DIR", "/nonexistent/dduck-filter-check-failed");
        }
    }
    cmd
}

/// A command that fails to start (an empty program name): the Windows fail-closed branch runs no git at all.
fn refused_git() -> Command {
    Command::new("")
}

/// macOS: `git` with hooks off through `/dev/null` and no system config (as before). Windows (task 1.5): the
/// resolved, ACL-checked `git.exe` and an empty app-owned hooks folder, or no git; the system config is read there
/// (it sits under Program Files, admin-only, and holds Git for Windows' `core.autocrlf`, without which every checked
/// out text file would count as changed).
fn base_git(dir: &Path) -> Command {
    let (Some(program), Some(hooks)) = (crate::platform::git_program(), crate::platform::git_hooks_path()) else { return refused_git() };
    let mut cmd = Command::new(program);
    cmd.arg("-c").arg("core.fsmonitor=false").arg("-c").arg(format!("core.hooksPath={}", hooks.display())).args(["-c", "protocol.allow=never"]).arg("-C").arg(dir);
    if !cfg!(windows) {
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0");
    cmd
}

/// Filter driver names in the config that applies to `dir` (reading config runs no filter). `None` = could not be
/// read, or a name that cannot be passed through `-c` safely.
fn filter_names(dir: &Path) -> Option<Vec<String>> {
    let mut cmd = base_git(dir);
    cmd.args(["config", "--null", "--get-regexp", r"^filter\."]);
    let out = crate::process::run(&mut cmd, Duration::from_secs(2), 256 * 1024, crate::process::Keep::Head, 1024).ok()?;
    if !out.status.success() {
        // Exit 1 with no output = no filter keys (also outside a repository).
        return (out.status.code() == Some(1) && out.stdout.is_empty()).then(Vec::new);
    }
    if out.stdout.len() >= 256 * 1024 {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    for entry in out.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let text = std::str::from_utf8(entry).ok()?;
        let key = text.split('\n').next()?;
        let rest = key.strip_prefix("filter.")?;
        let (name, _var) = rest.rsplit_once('.')?;
        if name.is_empty() || name.contains('=') || name.chars().any(char::is_control) {
            return None;
        }
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    Some(names)
}

fn git(dir: &Path, args: &[&str], secs: u64) -> Result<Option<String>, Error> {
    let mut cmd = app_git(dir);
    cmd.args(args);
    let out = crate::process::run(&mut cmd, Duration::from_secs(secs), 64 * 1024, crate::process::Keep::Head, 4 * 1024).map_err(|_| Error::Config("git did not run"))?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

/// `origin/HEAD` target without the remote prefix, when it is a safe branch name.
pub fn detect_default_branch(root: &Path) -> Result<Option<String>, Error> {
    let v = git(root, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"], 2)?;
    Ok(v.and_then(|s| s.strip_prefix("origin/").map(str::to_string)).filter(|b| valid_branch(b)))
}

/// The checked-out branch; `None` = detached HEAD.
pub fn start_branch(dir: &Path) -> Result<Option<String>, Error> {
    git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"], 2)
}

/// Local commit id of `refs/heads/<branch>` (40 or 64 hex), `None` when the branch does not exist.
pub fn branch_head(dir: &Path, branch: &str) -> Result<Option<String>, Error> {
    if !valid_branch(branch) {
        return Ok(None);
    }
    let v = git(dir, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")], 2)?;
    Ok(v.filter(|h| (h.len() == 40 || h.len() == 64) && h.bytes().all(|b| b.is_ascii_hexdigit())))
}

/// Post-check `default_moved`: the default branch head changed since the start.
pub fn default_moved(start_head: Option<&str>, now_head: Option<&str>) -> bool {
    matches!((start_head, now_head), (Some(a), Some(b)) if a != b) || matches!((start_head, now_head), (Some(_), None))
}

const MAX_REFLOG_FILES: usize = 200;
const MAX_REFLOG_BYTES: u64 = 1024 * 1024;

/// One reflog line: `<old> <new> <name> <<email>> <unix> <tz>\t<message>`. Only the time and whether the message is
/// a push update are kept; the identity fields are never returned.
fn reflog_push_at(line: &str) -> Option<i64> {
    let (left, msg) = line.split_once('\t')?;
    if !msg.starts_with("update by push") {
        return None;
    }
    let mut it = left.rsplit(' ');
    let _tz = it.next()?;
    it.next()?.parse::<i64>().ok()
}

/// Post-check `push_detected`: a "update by push" reflog entry for an `origin` tracking ref inside
/// `[from_s, to_s]` (unix seconds). `draft_pr` looks only at the default branches; `local_only` at every ref.
/// `Ok(None)` = the repository keeps no remote reflogs (cannot tell).
pub fn push_detected(common_dir: &Path, guard: Guard, detected_default: Option<&str>, from_s: i64, to_s: i64) -> Result<Option<bool>, Error> {
    let base = common_dir.join("logs/refs/remotes/origin");
    if !base.is_dir() {
        return Ok(None);
    }
    let wanted: Vec<String> = default_branches(detected_default);
    let mut files = 0;
    for entry in walkdir::WalkDir::new(&base).follow_links(false).max_depth(8) {
        let entry = entry.map_err(|_| Error::Config("reflog walk failed"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        files += 1;
        if files > MAX_REFLOG_FILES {
            break;
        }
        if guard == Guard::DraftPr {
            let rel = entry.path().strip_prefix(&base).map(|r| r.to_string_lossy().into_owned()).unwrap_or_default();
            if !wanted.contains(&rel) {
                continue;
            }
        }
        use std::io::Read;
        let mut s = String::new();
        std::fs::File::open(entry.path())?.take(MAX_REFLOG_BYTES).read_to_string(&mut s).ok();
        if s.lines().filter_map(reflog_push_at).any(|t| t >= from_s && t <= to_s) {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

// ---------------------------------------------------------------------------------------------------------------
// Remotes (cloud routines): `owner/repo` slugs, offline

/// `https://github.com/O/R(.git)`, `git@github.com:O/R.git`, `ssh://git@host/O/R` → `host/o/r` (lowercase).
pub fn remote_slug(url: &str) -> Option<String> {
    let u = url.trim();
    let (host, path) = if let Some(rest) = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).or_else(|| u.strip_prefix("ssh://")).or_else(|| u.strip_prefix("git://")) {
        let (h, p) = rest.split_once('/')?;
        let h = h.rsplit_once('@').map(|(_, h)| h).unwrap_or(h);
        (h.split(':').next()?.to_string(), p.to_string())
    } else if let Some((userhost, p)) = u.split_once(':') {
        let h = userhost.rsplit_once('@').map(|(_, h)| h).unwrap_or(userhost);
        if h.is_empty() || h.contains('/') {
            return None;
        }
        (h.to_string(), p.to_string())
    } else {
        return None;
    };
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() || host.is_empty() {
        return None;
    }
    Some(format!("{}/{}/{}", host.to_lowercase(), owner.to_lowercase(), repo.to_lowercase()))
}

/// `remote.origin.url` read from the shared git dir's `config` file (no git process, no network).
pub fn origin_url(common_dir: &Path) -> Option<String> {
    let text = read_small(&common_dir.join("config")).ok()?;
    let mut in_origin = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_origin = l.replace(' ', "") == "[remote\"origin\"]";
            continue;
        }
        if in_origin {
            if let Some((k, v)) = l.split_once('=') {
                if k.trim() == "url" {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// Slugs of the ticked projects' `origin` remotes.
pub fn local_only_slugs(items: &[LocalOnlyItem], home: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();
    for it in items {
        let s = it.slug.clone().or_else(|| repo_info(Path::new(&it.root), home).ok().flatten().as_ref().and_then(repo_slug));
        if let Some(s) = s {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// The cloud-routine check (item 12): a list that cannot be read blocks every cloud routine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudGuard {
    Slugs(Vec<String>),
    Unreadable,
}

impl CloudGuard {
    pub fn load(conn: Option<&Connection>, home: Option<&Path>) -> Self {
        match conn.map(load_items) {
            Some(Ok(items)) => CloudGuard::Slugs(local_only_slugs(&items, home)),
            _ => CloudGuard::Unreadable,
        }
    }
    pub fn blocks(&self, routine_slugs: &[String]) -> bool {
        match self {
            CloudGuard::Unreadable => true,
            CloudGuard::Slugs(local) => cloud_blocked(routine_slugs, local),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Post-checks at report time (local git only)

pub const ALERT_PUSH: i64 = 1;
pub const ALERT_DEFAULT_MOVED: i64 = 2;

/// Runs the two post-checks for a recorded task and stores `task_meta.delivery_alert` (bits; 0 = nothing seen).
/// `Ok(None)` = nothing to check (no scope recorded, not a repository, or the reflog is off) — left NULL.
pub fn post_check(conn: &Connection, bg_id: &str, home: Option<&Path>, now_s: i64) -> Result<Option<i64>, Error> {
    use rusqlite::OptionalExtension;
    type PostRow = (Option<String>, Option<String>, i64, Option<String>, Option<String>);
    let row: Option<PostRow> = conn
        .query_row("SELECT cwd, delivery_guard, created_at, default_branch, default_head FROM task_meta WHERE bg_id = ?1", [bg_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()?;
    let Some((Some(cwd), Some(code), created_ms, branch, head)) = row else { return Ok(None) };
    let Some(guard) = Guard::from_code(&code) else { return Ok(None) };
    let Ok(folder) = Path::new(&cwd).canon() else { return Ok(None) };
    let Some(repo) = repo_info(&folder, home)? else { return Ok(None) };
    let pushed = push_detected(&repo.common_dir, guard, branch.as_deref(), created_ms / 1000 - 1, now_s + 1)?;
    let b = branch.clone().unwrap_or_else(|| "main".into());
    let moved = match head.as_deref() {
        Some(h) => default_moved(Some(h), branch_head(&repo.root, &b)?.as_deref()),
        None => false,
    };
    let Some(pushed) = pushed else {
        if !moved {
            return Ok(None);
        }
        conn.execute("UPDATE task_meta SET delivery_alert = ?2 WHERE bg_id = ?1", rusqlite::params![bg_id, ALERT_DEFAULT_MOVED])?;
        return Ok(Some(ALERT_DEFAULT_MOVED));
    };
    let bits = if pushed { ALERT_PUSH } else { 0 } | if moved { ALERT_DEFAULT_MOVED } else { 0 };
    conn.execute("UPDATE task_meta SET delivery_alert = ?2 WHERE bg_id = ?1", rusqlite::params![bg_id, bits])?;
    Ok(Some(bits))
}

/// Every git-remote-looking string anywhere in a cloud routine's JSON, as slugs (schema-independent: only the slug
/// is kept, never the value).
pub fn repo_slugs_in(v: &serde_json::Value) -> Vec<String> {
    fn walk(v: &serde_json::Value, out: &mut Vec<String>, depth: usize) {
        if depth > 12 || out.len() > 20 {
            return;
        }
        match v {
            serde_json::Value::String(s) if s.len() < 300 && (s.contains("github.com") || s.contains("gitlab") || s.contains("bitbucket") || s.ends_with(".git")) => {
                if let Some(slug) = remote_slug(s) {
                    if !out.contains(&slug) {
                        out.push(slug);
                    }
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out, depth + 1)),
            serde_json::Value::Object(o) => o.values().for_each(|x| walk(x, out, depth + 1)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out, 0);
    out
}


/// A cloud routine that uses a ticked project's repository.
pub fn cloud_blocked(routine_slugs: &[String], local_slugs: &[String]) -> bool {
    routine_slugs.iter().any(|s| local_slugs.contains(s))
}

// ---------------------------------------------------------------------------------------------------------------
// The local-only PreToolUse hook

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDecision {
    /// No output, exit 0: the normal permission flow decides (never an explicit allow).
    NoDecision,
    Deny,
}

/// Hook output for a decision: (stdout, exit code).
pub fn hook_output(d: HookDecision) -> (String, i32) {
    match d {
        HookDecision::NoDecision => (String::new(), 0),
        HookDecision::Deny => (
            serde_json::json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": HOOK_REASON}}).to_string(),
            2,
        ),
    }
}

/// Resolves `git <word>` aliases: `dir` is the `cd`/`-C` folder (relative to the task folder, or absolute), `None` =
/// the task folder. `Ok(None)` = no alias, `Err` = could not tell (deny).
pub type AliasLookup<'a> = &'a dyn Fn(Option<&str>, &str) -> Result<Option<String>, ()>;

/// Real alias lookup: `git -C <folder> config --get alias.<word>` (1 s, local only). A folder that does not exist
/// (yet) cannot be checked: deny.
pub fn git_alias_lookup(cwd: Option<PathBuf>) -> impl Fn(Option<&str>, &str) -> Result<Option<String>, ()> {
    move |dir: Option<&str>, word: &str| {
        if !word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') || word.len() > 64 {
            return Err(());
        }
        let folder = match (dir, &cwd) {
            (None, Some(c)) => c.clone(),
            (None, None) => PathBuf::from("/"),
            (Some(d), _) if d.starts_with('/') => PathBuf::from(d),
            (Some(d), Some(c)) => c.join(d),
            (Some(_), None) => return Err(()),
        };
        if !folder.is_dir() {
            return Err(());
        }
        // Runs inside the task's hook (the agent's rights, not the app's). `core.fsmonitor=false` still applies; the
        // system config stays readable so a system-wide `alias.p = push` is resolved (and denied).
        let mut cmd = Command::new("git");
        cmd.args(["-c", "core.fsmonitor=false"]).arg("-C").arg(&folder).args(["config", "--get", &format!("alias.{word}")]).env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0");
        match crate::process::run(&mut cmd, Duration::from_secs(1), 16 * 1024, crate::process::Keep::Head, 1024) {
            Ok(out) if out.status.success() => Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())),
            // `config --get` exits 1 when the key is missing; anything else is unknown.
            Ok(out) if out.status.code() == Some(1) => Ok(None),
            _ => Err(()),
        }
    }
}

/// Input fields that carry a shell command line (Bash `command`, and MCP or other tools' `cmd` / `script`).
const SHELL_FIELDS: &[&str] = &["command", "cmd", "script", "shell_command"];
const FILE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

/// The decision on Claude Code's PreToolUse JSON for a `local_only` task. Fails closed: input that is not the
/// expected JSON, or a command that cannot be analysed, is denied.
pub fn local_only_hook(stdin: &str, alias: AliasLookup) -> HookDecision {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(stdin) else { return HookDecision::Deny };
    let Some(tool) = v.get("tool_name").and_then(|t| t.as_str()) else { return HookDecision::Deny };
    let input = v.get("tool_input");
    let base = v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from).filter(|p| p.is_absolute());
    let shell: Vec<&serde_json::Value> = input.map(|i| SHELL_FIELDS.iter().filter_map(|k| i.get(*k)).collect()).unwrap_or_default();
    if tool.starts_with("mcp__") {
        // An MCP tool that runs a shell (desktop-commander `start_process` and the like) is outside the CLI's Bash
        // rules: never in a local-only task.
        return if mcp_is_remote(tool) || !shell.is_empty() || input.is_some_and(mentions_remote) { HookDecision::Deny } else { HookDecision::NoDecision };
    }
    if FILE_TOOLS.contains(&tool) {
        let path = input.and_then(|i| i.get("file_path").or_else(|| i.get("notebook_path"))).and_then(|p| p.as_str()).unwrap_or("");
        // A file written now (Makefile, script, .env) that sets a backstop variable or a protocol rule would switch
        // transports back on when run later. Heuristic: the names, not their meaning.
        let writes_backstop = input.is_some_and(|i| {
            let mut texts: Vec<&str> = ["content", "new_string", "new_source"].iter().filter_map(|k| i.get(*k).and_then(|v| v.as_str())).collect();
            if let Some(edits) = i.get("edits").and_then(|e| e.as_array()) {
                texts.extend(edits.iter().filter_map(|e| e.get("new_string").and_then(|v| v.as_str())));
            }
            texts.iter().any(|t| names_protected(t) || PROTOCOL_RULE_RE.is_match(t))
        });
        return if writes_backstop || GIT_CONTROL_PATHS.iter().any(|p| path.contains(p)) { HookDecision::Deny } else { HookDecision::NoDecision };
    }
    if tool == "Bash" || !shell.is_empty() {
        if shell.is_empty() {
            return HookDecision::Deny;
        }
        let scan = Scan { alias, dir: None, base: base.as_deref() };
        for field in shell {
            let Some(cmd) = field.as_str() else { return HookDecision::Deny };
            if shell_reaches_remote(cmd, &scan, 0) {
                return HookDecision::Deny;
            }
        }
        return HookDecision::NoDecision;
    }
    HookDecision::NoDecision
}

/// Any string in an MCP input, at any depth (argv arrays, `code`, `input` …), with a `git`, `gh` or `push` token, or a
/// protected variable name.
fn mentions_remote(v: &serde_json::Value) -> bool {
    static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(^|[^A-Za-z0-9_])(git|gh|push|git-[a-z-]+)([^A-Za-z0-9_]|$)").unwrap_or_else(|_| unreachable!()));
    fn walk(v: &serde_json::Value, depth: usize) -> bool {
        if depth > 16 {
            return true;
        }
        match v {
            serde_json::Value::String(s) => TOKEN_RE.is_match(s) || names_protected(s),
            serde_json::Value::Array(a) => a.iter().any(|x| walk(x, depth + 1)),
            serde_json::Value::Object(o) => o.iter().any(|(k, x)| TOKEN_RE.is_match(k) || names_protected(k) || walk(x, depth + 1)),
            _ => false,
        }
    }
    walk(v, 0)
}

/// GitHub-like MCP: the server name mentions a code host, or the tool is a known GitHub write tool.
pub fn mcp_is_remote(tool: &str) -> bool {
    let rest = &tool["mcp__".len()..];
    let (server, name) = rest.split_once("__").unwrap_or((rest, ""));
    let s = server.to_lowercase();
    ["github", "gitlab", "bitbucket", "gitea", "codeberg"].iter().any(|h| s.contains(h)) || GITHUB_WRITE_TOOLS.contains(&name)
}

const MAX_DEPTH: usize = 4;

/// Analysis failed: treat as reaching the remote.
struct Unparseable;

#[derive(Default)]
struct Lexed {
    commands: Vec<Vec<String>>,
    /// Text of subshells, `$(…)` and backticks, analysed recursively.
    nested: Vec<String>,
    /// A here-document feeds a command's stdin.
    heredoc_cmds: Vec<usize>,
}

/// Small POSIX-ish lexer: quotes, escapes, separators, subshells, command substitution, here-documents, comments.
fn lex(src: &str) -> Result<Lexed, Unparseable> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Lexed::default();
    let mut words: Vec<String> = Vec::new();
    let mut word: Option<String> = None;
    let mut pending_heredocs: Vec<(String, bool)> = Vec::new();
    let mut i = 0;
    let end_word = |word: &mut Option<String>, words: &mut Vec<String>| {
        if let Some(w) = word.take() {
            words.push(w);
        }
    };
    let end_cmd = |words: &mut Vec<String>, out: &mut Lexed| {
        if !words.is_empty() {
            out.commands.push(std::mem::take(words));
        }
    };
    // Finds the index of the `)` closing an opening at `start` (just after the `(`), honouring quotes.
    fn close_paren(chars: &[char], mut i: usize) -> Result<usize, Unparseable> {
        let mut depth = 1;
        while i < chars.len() {
            match chars[i] {
                '\\' => i += 1,
                '\'' => {
                    i += 1;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err(Unparseable);
                    }
                }
                '"' => {
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err(Unparseable);
                    }
                }
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Err(Unparseable)
    }
    fn backtick_end(chars: &[char], mut i: usize) -> Result<usize, Unparseable> {
        while i < chars.len() {
            match chars[i] {
                '\\' => i += 1,
                '`' => return Ok(i),
                _ => {}
            }
            i += 1;
        }
        Err(Unparseable)
    }
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => end_word(&mut word, &mut words),
            '\n' => {
                end_word(&mut word, &mut words);
                if !pending_heredocs.is_empty() {
                    out.heredoc_cmds.push(out.commands.len());
                }
                end_cmd(&mut words, &mut out);
                // Skip here-document bodies (data, not commands).
                for (delim, strip) in std::mem::take(&mut pending_heredocs) {
                    loop {
                        let start = i + 1;
                        if start > chars.len() {
                            return Err(Unparseable);
                        }
                        let mut j = start;
                        while j < chars.len() && chars[j] != '\n' {
                            j += 1;
                        }
                        let line: String = chars[start..j].iter().collect();
                        i = j;
                        let l = if strip { line.trim_start_matches('\t') } else { line.as_str() };
                        if l == delim {
                            break;
                        }
                        if j >= chars.len() {
                            return Err(Unparseable);
                        }
                    }
                }
            }
            ';' | '&' | '|' => {
                end_word(&mut word, &mut words);
                if !pending_heredocs.is_empty() {
                    out.heredoc_cmds.push(out.commands.len());
                }
                end_cmd(&mut words, &mut out);
            }
            '#' if word.is_none() => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '(' if word.is_none() => {
                let close = close_paren(&chars, i + 1)?;
                out.nested.push(chars[i + 1..close].iter().collect());
                i = close;
            }
            ')' => return Err(Unparseable),
            '`' => {
                let close = backtick_end(&chars, i + 1)?;
                out.nested.push(chars[i + 1..close].iter().collect());
                word.get_or_insert_with(String::new).push_str("$()");
                i = close;
            }
            '$' if chars.get(i + 1) == Some(&'(') => {
                let close = close_paren(&chars, i + 2)?;
                out.nested.push(chars[i + 2..close].iter().collect());
                word.get_or_insert_with(String::new).push_str("$()");
                i = close;
            }
            '\\' => {
                i += 1;
                match chars.get(i) {
                    Some('\n') => {}
                    Some(&n) => word.get_or_insert_with(String::new).push(n),
                    None => return Err(Unparseable),
                }
            }
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    w.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(Unparseable);
                }
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                i += 1;
                loop {
                    let Some(&d) = chars.get(i) else { return Err(Unparseable) };
                    match d {
                        '"' => break,
                        '\\' => {
                            i += 1;
                            let Some(&n) = chars.get(i) else { return Err(Unparseable) };
                            if !matches!(n, '"' | '\\' | '$' | '`' | '\n') {
                                w.push('\\');
                            }
                            if n != '\n' {
                                w.push(n);
                            }
                        }
                        '`' => {
                            let close = backtick_end(&chars, i + 1)?;
                            out.nested.push(chars[i + 1..close].iter().collect());
                            w.push_str("$()");
                            i = close;
                        }
                        '$' if chars.get(i + 1) == Some(&'(') => {
                            let close = close_paren(&chars, i + 2)?;
                            out.nested.push(chars[i + 2..close].iter().collect());
                            w.push_str("$()");
                            i = close;
                        }
                        _ => w.push(d),
                    }
                    i += 1;
                }
            }
            '<' if chars.get(i + 1) == Some(&'<') && chars.get(i + 2) != Some(&'<') => {
                end_word(&mut word, &mut words);
                i += 2;
                let strip = chars.get(i) == Some(&'-');
                if strip {
                    i += 1;
                }
                while chars.get(i).is_some_and(|c| *c == ' ' || *c == '\t') {
                    i += 1;
                }
                let mut delim = String::new();
                while let Some(&d) = chars.get(i) {
                    if d.is_whitespace() || matches!(d, ';' | '&' | '|' | '<' | '>' | '(' | ')') {
                        break;
                    }
                    if d != '\'' && d != '"' && d != '\\' {
                        delim.push(d);
                    }
                    i += 1;
                }
                if delim.is_empty() {
                    return Err(Unparseable);
                }
                pending_heredocs.push((delim, strip));
                continue;
            }
            '<' | '>' => {
                end_word(&mut word, &mut words);
                // `>&`, `&>`-style duplications: swallow the `&` so it is not read as a separator.
                if chars.get(i + 1) == Some(&'&') || chars.get(i + 1) == Some(&'>') || chars.get(i + 1) == Some(&'<') {
                    i += 1;
                }
            }
            _ => word.get_or_insert_with(String::new).push(c),
        }
        i += 1;
    }
    if !pending_heredocs.is_empty() {
        return Err(Unparseable);
    }
    end_word(&mut word, &mut words);
    end_cmd(&mut words, &mut out);
    Ok(out)
}

fn basename(w: &str) -> &str {
    w.rsplit('/').next().unwrap_or(w)
}

fn is_assignment(w: &str) -> bool {
    match w.split_once('=') {
        Some((name, _)) => !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !name.as_bytes()[0].is_ascii_digit(),
        None => false,
    }
}

/// Environment names an agent may not set or unset in a `local_only` task: they change what git runs or where it
/// pushes (and the backstop itself), or hand `gh` a token.
fn protected_env(name: &str) -> bool {
    const SAFE_GIT: &[&str] = &[
        "GIT_TERMINAL_PROMPT", "GIT_OPTIONAL_LOCKS", "GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE", "GIT_TRACE", "GIT_TRACE_PERFORMANCE", "GIT_FLUSH",
    ];
    let n = name.to_ascii_uppercase();
    (n.starts_with("GIT_") && !SAFE_GIT.contains(&n.as_str()))
        || n.starts_with("GH_")
        || (n.starts_with("GITHUB_") && (n.contains("TOKEN") || n.contains("HOST") || n.contains("API")))
        || matches!(n.as_str(), "HOME" | "XDG_CONFIG_HOME" | "PATH")
}

fn assigns_protected(w: &str) -> bool {
    is_assignment(w) && w.split_once('=').is_some_and(|(n, _)| protected_env(n))
}

const KEYWORDS: &[&str] = &["if", "then", "else", "elif", "fi", "do", "done", "while", "until", "!", "{", "}", "time", "case", "esac", "coproc"];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish", "csh", "tcsh", "osascript"];
/// Programs that are the remote, or define commands the hook cannot see.
const DENY_PROGRAMS: &[&str] = &["gh", "hub", "glab", "function", "alias", "shopt", "parallel", "eval", "source", ".", "enable", "hash"];
/// Programs that copy or link files: one that names git or gh could plant a renamed git.
const LINKERS: &[&str] = &["ln", "cp", "mv", "install", "rsync", "ditto"];
/// Programs that only read, for the `.git/config` / hooks path check.
const READERS: &[&str] = &["cat", "ls", "head", "tail", "less", "more", "grep", "rg", "stat", "file", "wc", "diff", "test", "["];
/// Paths whose change makes git run code or push elsewhere.
const GIT_CONTROL_PATHS: &[&str] = &[".git/hooks", ".git/config", ".husky", ".git/info/attributes", ".gitmodules", ".gitconfig", ".config/git", "git/config", "local-only-gitconfig"];

/// Git subcommands that are not aliases (so no alias lookup is needed).
const GIT_BUILTINS: &[&str] = &[
    "add", "am", "annotate", "apply", "archive", "bisect", "blame", "branch", "bundle", "cat-file", "check-attr", "check-ignore", "checkout", "cherry", "cherry-pick",
    "clean", "clone", "commit", "config", "count-objects", "describe", "diff", "diff-files", "diff-index", "diff-tree", "difftool", "fetch", "for-each-ref", "format-patch", "fsck",
    "gc", "grep", "hash-object", "help", "init", "log", "ls-files", "ls-remote", "ls-tree", "merge", "merge-base", "mergetool", "mv", "notes", "pull", "range-diff", "read-tree",
    "rebase", "reflog", "remote", "repack", "replace", "reset", "restore", "rev-list", "rev-parse", "revert", "rm", "shortlog", "show", "show-branch", "show-ref", "sparse-checkout",
    "stash", "status", "submodule", "switch", "symbolic-ref", "tag", "update-index", "update-ref", "var", "verify-commit", "version", "whatchanged", "worktree", "write-tree",
    "maintenance", "rerere", "prune", "citool", "gui", "instaweb", "request-pull", "lfs",
];
/// Subcommands that push, or run a command or tool of the caller's choosing.
const GIT_DENY_SUBS: &[&str] = &["push", "send-pack", "http-push", "svn", "p4", "difftool", "mergetool", "receive-pack", "upload-pack", "daemon", "credential", "filter-branch"];
/// Global options that take a separate value.
const GIT_VALUE_OPTS: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--super-prefix", "--list-cmds"];
/// `-c` keys that cannot run code or redirect a remote.
const GIT_SAFE_C: &[&str] = &[
    "color.", "core.quotepath", "core.abbrev", "user.name", "user.email", "advice.", "log.date", "log.decorate", "log.showsignature", "init.defaultbranch", "commit.gpgsign",
    "core.autocrlf", "core.safecrlf", "status.", "column.", "i18n.", "gc.auto", "core.fsmonitor=false", "core.untrackedcache", "diff.renames", "diff.noprefix", "merge.conflictstyle",
];
/// `git config` keys an agent may not write (or read back via an editor): aliases, URL rewrites, includes, hooks,
/// helpers and every key that names a program.
const GIT_CONFIG_DENY: &[&str] = &[
    "alias", "url.", "include", "core.hookspath", "core.sshcommand", "core.fsmonitor", "core.pager", "core.editor", "core.askpass", "credential", "remote.", "pager.",
    "sequence.editor", "diff.", "merge.", "filter.", "protocol.", "http.", "gpg.", "ssh.", "push.", "branch.", "submodule.", "uploadpack.", "receive.", "interactive.",
];

fn c_key_safe(kv: &str) -> bool {
    let l = kv.to_ascii_lowercase();
    GIT_SAFE_C.iter().any(|p| l.starts_with(p))
}

/// What the hook knows while walking one command line.
#[derive(Clone)]
struct Scan<'a> {
    alias: AliasLookup<'a>,
    /// The folder the shell is in: `None` = the task folder, `Some(None)` = unknown (`cd $X`).
    dir: Option<Option<String>>,
    base: Option<&'a Path>,
}

impl Scan<'_> {
    /// Folder for an alias lookup: the `cd` target joined with `-C`, or `None` (unknown → deny on lookup).
    fn alias_dir(&self, c: Option<&str>) -> Option<Option<String>> {
        let cur = match &self.dir {
            None => None,
            Some(None) => return Some(None),
            Some(Some(d)) => Some(d.clone()),
        };
        match (cur, c) {
            (cur, None) => cur.map(Some),
            (None, Some(c)) => Some(Some(c.to_string())),
            (Some(d), Some(c)) if c.starts_with('/') => {
                let _ = d;
                Some(Some(c.to_string()))
            }
            (Some(d), Some(c)) => Some(Some(format!("{d}/{c}"))),
        }
    }

    fn lookup(&self, c: Option<&str>, word: &str) -> Result<Option<String>, ()> {
        match self.alias_dir(c) {
            Some(None) => Err(()),
            Some(Some(d)) if d.contains('$') || d.contains('`') || d.starts_with('~') => Err(()),
            Some(Some(d)) => (self.alias)(Some(&d), word),
            None => (self.alias)(None, word),
        }
    }
}

/// `protocol.allow` / `protocol.<name>.allow` in any spelling git accepts (`[protocol "file"] allow`, too).
static PROTOCOL_RULE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)protocol(\.[a-z0-9+-]*)?\.allow|\[\s*protocol[^\]]*\][^\[]*\ballow\b"#).unwrap_or_else(|_| unreachable!()));

/// A word anywhere in the line that names a protected variable (`read GIT_CONFIG_COUNT <<<`, `printf -v`,
/// `make VAR=`, `declare +x`, `os.environ.pop('GIT_CONFIG_COUNT')` …): any of them can switch the backstop off.
fn names_protected(src: &str) -> bool {
    static NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap_or_else(|_| unreachable!()));
    // Upper-case names also inside other text (`\tGIT_ALLOW_PROTOCOL`, `${GIT_DIR}`), any case as whole words.
    static UPPER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:GITHUB|GIT|GH)_[A-Z0-9_]+").unwrap_or_else(|_| unreachable!()));
    let check = |n: &str| {
        let n = n.to_ascii_uppercase();
        (n.starts_with("GIT_") || n.starts_with("GH_") || n.starts_with("GITHUB_")) && protected_env(&n)
    };
    UPPER_RE.find_iter(src).any(|m| check(m.as_str())) || NAME_RE.find_iter(src).any(|m| check(m.as_str()))
}

fn shell_reaches_remote(src: &str, scan: &Scan, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return true;
    }
    if names_protected(src) || PROTOCOL_RULE_RE.is_match(src) {
        return true;
    }
    let Ok(lexed) = lex(src) else { return true };
    // `.git/config`, hooks, `.husky` written by a redirect or a writer program.
    if src.contains('>') && GIT_CONTROL_PATHS.iter().any(|p| src.contains(p)) {
        return true;
    }
    let mut scan = scan.clone();
    if lexed.nested.iter().any(|n| shell_reaches_remote(n, &scan, depth + 1)) {
        return true;
    }
    for (idx, words) in lexed.commands.iter().enumerate() {
        // `cd` / `pushd` move later alias lookups.
        if let Some(first) = words.iter().find(|w| !KEYWORDS.contains(&w.as_str())) {
            if first == "cd" || first == "pushd" {
                let target = words.iter().skip_while(|w| *w != first).nth(1).filter(|t| !t.starts_with('-'));
                scan.dir = Some(match target {
                    None => None,
                    Some(t) if t.contains('$') || t.contains('`') || t.starts_with('~') => None,
                    Some(t) if t.starts_with('/') => Some(t.clone()),
                    Some(t) => match &scan.dir {
                        None => Some(t.clone()),
                        Some(None) => None,
                        Some(Some(d)) => Some(format!("{d}/{t}")),
                    },
                });
                continue;
            }
        }
        if command_reaches_remote(words, &scan, depth, lexed.heredoc_cmds.contains(&idx)) {
            return true;
        }
    }
    false
}

/// Strips keywords, assignments and wrappers; returns the real command words and whether they run through `xargs`
/// (which appends words the hook cannot see). A protected assignment or an environment-rewriting `env` is an error.
fn unwrap_command(words: &[String]) -> Result<(Vec<String>, bool), Unparseable> {
    let mut w: Vec<String> = words.to_vec();
    let mut via_xargs = false;
    for _ in 0..16 {
        while let Some(first) = w.first() {
            if assigns_protected(first) {
                return Err(Unparseable);
            }
            if KEYWORDS.contains(&first.as_str()) || is_assignment(first) {
                w.remove(0);
            } else {
                break;
            }
        }
        let Some(first) = w.first() else { return Ok((w, via_xargs)) };
        let prog = basename(first).to_string();
        let skip_opts = |w: &mut Vec<String>, value_opts: &[&str]| -> Result<(), Unparseable> {
            w.remove(0);
            while let Some(a) = w.first() {
                if a == "--" {
                    w.remove(0);
                    break;
                }
                if assigns_protected(a) {
                    return Err(Unparseable);
                }
                if a.starts_with('-') && a.len() > 1 {
                    let takes = value_opts.contains(&a.as_str());
                    w.remove(0);
                    if takes && !w.is_empty() {
                        w.remove(0);
                    }
                } else if is_assignment(a) {
                    w.remove(0);
                } else {
                    break;
                }
            }
            Ok(())
        };
        match prog.as_str() {
            "env" => {
                // `-S` splits a string into a new command line; `-i` / `-u` drop the backstop variables.
                if w.iter().skip(1).take_while(|a| a.starts_with('-') || is_assignment(a)).any(|a| {
                    a.starts_with("-S") || a.starts_with("-i") || a.starts_with("-u") || a == "-" || a.starts_with("--split-string") || a.starts_with("--ignore-environment") || a.starts_with("--unset")
                }) {
                    return Err(Unparseable);
                }
                skip_opts(&mut w, &["-C", "-P", "--chdir"])?
            }
            "nohup" | "builtin" | "exec" | "noglob" | "nocorrect" | "caffeinate" => skip_opts(&mut w, &["-a", "-w", "-t"])?,
            "command" => {
                if w.get(1).is_some_and(|a| a == "-v" || a == "-V") {
                    return Ok((Vec::new(), via_xargs));
                }
                skip_opts(&mut w, &[])?
            }
            "nice" => skip_opts(&mut w, &["-n"])?,
            "sudo" | "doas" => skip_opts(&mut w, &["-u", "-g", "-h", "-p", "-C", "-D", "-U"])?,
            "stdbuf" => skip_opts(&mut w, &["-i", "-o", "-e"])?,
            "arch" => skip_opts(&mut w, &[])?,
            "xargs" => {
                via_xargs = true;
                skip_opts(&mut w, &["-I", "-n", "-P", "-L", "-d", "-E", "-s", "-a", "-J", "-R", "-S"])?
            }
            "timeout" | "gtimeout" => {
                skip_opts(&mut w, &["-s", "-k", "--signal", "--kill-after"])?;
                if !w.is_empty() {
                    w.remove(0); // duration
                }
            }
            "time" => skip_opts(&mut w, &["-o", "-f"])?,
            "watch" => skip_opts(&mut w, &["-n", "-d"])?,
            "script" => {
                skip_opts(&mut w, &["-t", "-T", "-F", "-B", "-I", "-O", "-E", "-c"])?;
                if !w.is_empty() {
                    w.remove(0); // the typescript file
                }
            }
            _ => return Ok((w, via_xargs)),
        }
    }
    Err(Unparseable)
}

fn is_git_like(name: &str) -> bool {
    name == "git" || name.starts_with("git-") || name == "gh" || name == "hub" || name == "glab"
}

/// A program path (`./g`, `../bin/x`, `/opt/x/git2`) that is git under another name. Relative paths that do not
/// exist yet (created in the same command line) cannot be checked and count as git.
fn path_is_git(prog: &str, scan: &Scan) -> bool {
    if !prog.contains('/') {
        return false;
    }
    let p = if prog.starts_with('/') {
        PathBuf::from(prog)
    } else {
        let Some(base) = scan.base else { return true };
        match scan.alias_dir(None) {
            Some(Some(d)) if d.starts_with('/') => PathBuf::from(d).join(prog),
            Some(Some(d)) => base.join(d).join(prog),
            Some(None) => return true,
            None => base.join(prog),
        }
    };
    match p.canon() {
        Ok(real) => real.file_name().map(|n| n.to_string_lossy().into_owned()).is_some_and(|n| is_git_like(&n)),
        Err(_) => !prog.starts_with('/'),
    }
}

fn command_reaches_remote(words: &[String], scan: &Scan, depth: usize, has_heredoc: bool) -> bool {
    let Ok((w, via_xargs)) = unwrap_command(words) else { return true };
    let Some(first) = w.first() else { return false };
    // A program name we cannot know (`$G push`, `"$(…)"`), or one the shell expands (`/usr/bin/gi?`, `{git,push}`).
    if first.contains('$') || first.chars().any(|c| matches!(c, '*' | '?' | '[' | ']' | '{' | '}' | '~')) {
        return true;
    }
    let prog = basename(first);
    if DENY_PROGRAMS.contains(&prog) || is_git_like(prog) && prog != "git" {
        return true;
    }
    if matches!(prog, "export" | "declare" | "typeset" | "local" | "readonly" | "set" | "unset" | "setenv" | "launchctl")
        && w[1..].iter().any(|a| assigns_protected(a) || protected_env(a.trim_start_matches('-')) || (prog == "launchctl" && a == "setenv"))
    {
        return true;
    }
    if LINKERS.contains(&prog) && w[1..].iter().any(|a| is_git_like(basename(a)) || GIT_CONTROL_PATHS.iter().any(|p| a.contains(p))) {
        return true;
    }
    if !READERS.contains(&prog) && !matches!(prog, "git" | "cd") && w[1..].iter().any(|a| GIT_CONTROL_PATHS.iter().any(|p| a.contains(p))) {
        return true;
    }
    let real_git = prog == "git" || path_is_git(first, scan);
    // Interpreters given code on the command line: code that names git or gh is denied (other code is a gap, see
    // the environment backstop).
    if matches!(prog, "python" | "python3" | "perl" | "ruby" | "node" | "php" | "deno" | "bun")
        && w[1..].windows(2).any(|p| matches!(p[0].as_str(), "-c" | "-e" | "-E" | "--eval" | "-r") && (p[1].contains("git") || p[1].contains("gh ") || p[1].contains("push")))
    {
        return true;
    }
    if via_xargs && (real_git || SHELLS.contains(&prog)) {
        return true;
    }
    // ssh straight to the pack programs, and git transports given a program of the caller's choosing.
    if matches!(prog, "ssh" | "scp" | "sftp") && w[1..].iter().any(|a| a.contains("git-receive-pack") || a.contains("git-upload-pack") || a.contains("git-upload-archive")) {
        return true;
    }
    if matches!(prog, "npm" | "pnpm" | "yarn") && w.windows(2).any(|p| p[0] == "config" || p[0] == "set") && w.iter().any(|a| a == "git") {
        return true;
    }
    if prog == "find" {
        // `-exec … ;` / `-execdir` / `-ok`: the command runs once per file.
        let mut i = 1;
        while i < w.len() {
            if matches!(w[i].as_str(), "-exec" | "-execdir" | "-ok" | "-okdir") {
                let end = w[i + 1..].iter().position(|a| a == ";" || a == "+").map_or(w.len(), |p| i + 1 + p);
                let inner: Vec<String> = w[i + 1..end].to_vec();
                let inner_prog = inner.first().map(|p| basename(p).to_string()).unwrap_or_default();
                if inner.is_empty() || is_git_like(&inner_prog) || SHELLS.contains(&inner_prog.as_str()) || command_reaches_remote(&inner, scan, depth + 1, false) {
                    return true;
                }
                i = end;
            }
            i += 1;
        }
        return false;
    }
    if SHELLS.contains(&prog) {
        // `sh -c '<cmd>'` (also `-lc`, `-ec`): analyse the string. Reading commands from stdin (pipe, here-doc) or
        // with no script cannot be checked.
        let mut i = 1;
        while i < w.len() {
            let a = &w[i];
            if a.starts_with('-') && a.len() > 1 && !a.starts_with("--") && (a.contains('c') || (prog == "osascript" && a == "-e")) {
                return match w.get(i + 1) {
                    Some(s) if prog == "osascript" => s.contains("do shell script") || s.contains("git") || s.contains("gh "),
                    Some(s) => shell_reaches_remote(s, scan, depth + 1),
                    None => true,
                };
            }
            if a == "-s" || a == "-i" {
                return true;
            }
            if !a.starts_with('-') {
                return has_heredoc; // a script file: not visible (documented gap) unless fed by a here-doc
            }
            i += 1;
        }
        return true;
    }
    if real_git {
        return git_reaches_remote(&w[1..], scan, depth);
    }
    false
}

fn git_reaches_remote(args: &[String], scan: &Scan, depth: usize) -> bool {
    let mut i = 0;
    let mut c_dir: Option<String> = None;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-c" || (a.starts_with("-c") && a.len() > 2 && !a.starts_with("--")) {
            let kv = if a == "-c" { args.get(i + 1).map(String::as_str).unwrap_or("") } else { &a[2..] };
            if !c_key_safe(kv) {
                return true;
            }
            i += if a == "-c" { 2 } else { 1 };
            continue;
        }
        // `--config-env=<key>=<VAR>` and `--exec-path=<dir>` change what git runs.
        if a.starts_with("--config-env") || a.starts_with("--exec-path") || a == "--git-dir" && args.get(i + 1).is_some_and(|v| v.contains('$')) {
            return true;
        }
        if a == "-C" {
            let Some(v) = args.get(i + 1) else { return true };
            c_dir = Some(match c_dir {
                Some(prev) if !v.starts_with('/') => format!("{prev}/{v}"),
                _ => v.clone(),
            });
            i += 2;
            continue;
        }
        if GIT_VALUE_OPTS.contains(&a) {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        break;
    }
    let Some(sub) = args.get(i) else { return false };
    let sub = sub.as_str();
    let rest = &args[i + 1..];
    // Write-output forms (also in the deny list): files outside the work, or a pager program for grep.
    if rest.iter().any(|a| a.starts_with("--output") || a.starts_with("--open-files-in-pager") || a.starts_with("--upload-pack") || a.starts_with("--receive-pack") || a.starts_with("--exec=") || (a == "-u" && matches!(sub, "ls-remote" | "clone" | "fetch" | "pull" | "archive")))
        || (sub == "grep" && rest.iter().any(|a| a.starts_with("-O")))
        || (sub == "format-patch" && rest.iter().any(|a| a.starts_with("-o")))
    {
        return true;
    }
    if GIT_DENY_SUBS.contains(&sub) {
        return true;
    }
    match sub {
        "lfs" => return rest.first().is_some_and(|s| s == "push"),
        "config" => {
            // Writes of aliases, URL rewrites, includes, hooks and program keys; also `--edit` and `-f <file>`.
            return rest.iter().any(|a| {
                let l = a.to_ascii_lowercase();
                l == "-e" || l == "--edit" || l.contains("alias") || GIT_CONFIG_DENY.iter().any(|k| l.starts_with(k) || l.contains(&format!(".{k}")))
            });
        }
        "rebase" => return rest.iter().any(|a| a == "-x" || a.starts_with("--exec") || (a.starts_with("-x") && a.len() > 2) || a.starts_with("-i") || a == "--interactive"),
        "bisect" => return rest.first().is_some_and(|s| s == "run"),
        "submodule" => {
            // `git submodule foreach '<cmd>'`
            if let Some(pos) = rest.iter().position(|s| s == "foreach") {
                let cmd: Vec<&str> = rest[pos + 1..].iter().map(String::as_str).filter(|s| *s != "--recursive" && *s != "-q" && *s != "--quiet").collect();
                return shell_reaches_remote(&cmd.join(" "), scan, depth + 1);
            }
            return false;
        }
        "remote" => return rest.iter().any(|a| a == "set-url" || a == "add" || a == "rename"),
        _ => {}
    }
    if GIT_BUILTINS.contains(&sub) {
        return false;
    }
    match scan.lookup(c_dir.as_deref(), sub) {
        Err(()) => true,
        Ok(None) => false,
        Ok(Some(def)) => {
            let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
            if let Some(shell) = def.strip_prefix('!') {
                shell_reaches_remote(&format!("{shell} {}", rest.join(" ")), scan, depth + 1)
            } else {
                shell_reaches_remote(&format!("git {def} {}", rest.join(" ")), scan, depth + 1)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn no_alias(_: Option<&str>, _: &str) -> Result<Option<String>, ()> {
        Ok(None)
    }

    fn bash(cmd: &str) -> HookDecision {
        local_only_hook(&serde_json::json!({"tool_name": "Bash", "tool_input": {"command": cmd}}).to_string(), &no_alias)
    }

    #[test]
    fn base_rules_cover_merge_ready_default_and_force() {
        let r = deny_rules(Guard::DraftPr, false, None);
        for must in [
            "Bash(gh pr merge *)",
            "Bash(gh pr ready *)",
            "Bash(gh api *merge*)",
            "mcp__*__merge_pull_request",
            "Bash(git push * main)",
            "Bash(git push *:master)",
            "Bash(git push *refs/heads/main*)",
            "Bash(git push --all*)",
            "Bash(git push * --mirror*)",
            "Bash(git push * --force)",
            "Bash(git push -f *)",
        ] {
            assert!(r.iter().any(|x| x == must), "{must}");
        }
        assert!(!r.iter().any(|x| x == "Bash(git push *)"), "draft PR may push the work branch");
        assert!(!r.iter().any(|x| x == "Bash(git push)"), "only when starting on the default branch");
        assert!(!r.iter().any(|x| x.contains("force-with-lease")));
    }

    #[test]
    fn git_write_output_forms_are_always_denied() {
        for g in [Guard::DraftPr, Guard::LocalOnly] {
            let r = deny_rules(g, false, None);
            for must in ["Bash(git * --output*)", "Bash(git *--output=*)", "Bash(git format-patch -o*)", "Bash(git grep * -O*)", "Bash(git * --open-files-in-pager*)"] {
                assert!(r.iter().any(|x| x == must), "{must}");
            }
        }
        for cmd in ["git diff --output=/tmp/x", "git log --output /tmp/x -1", "git -C . show --output=/etc/x", "git grep -Ovim foo", "git grep --open-files-in-pager=sh x", "git format-patch -o /tmp HEAD~1"] {
            assert_eq!(bash(cmd), HookDecision::Deny, "{cmd}");
        }
        assert_eq!(bash("git diff -O order.txt"), HookDecision::NoDecision, "diff -O only reads an order file");
    }

    #[test]
    fn app_git_never_runs_planted_fsmonitor() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let repo = base.join("r");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let marker = base.join("PWNED");
        let script = base.join("fsmon.sh");
        std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(Command::new("git").arg("-C").arg(&repo).args(["config", "core.fsmonitor"]).arg(&script).status().unwrap().success());
        // Sanity: a plain `git status` does run it.
        let _ = Command::new("git").arg("-C").arg(&repo).args(["status", "--porcelain"]).output();
        let plain_ran = marker.exists();
        let _ = std::fs::remove_file(&marker);
        let mut st = app_git(&repo);
        st.args(["status", "--porcelain"]);
        assert!(st.output().unwrap().status.success());
        start_branch(&repo).unwrap();
        branch_head(&repo, "main").unwrap();
        detect_default_branch(&repo).unwrap();
        assert!(!marker.exists(), "the app's git must not run core.fsmonitor (plain git ran it: {plain_ran})");
        let c = app_git(&repo);
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.windows(2).any(|w| w[0] == "-c" && w[1] == "core.fsmonitor=false"));
        // macOS skips the system config; Windows reads its admin-only one (autocrlf) and uses an app-owned hooks folder.
        assert_eq!(c.get_envs().any(|(k, v)| k == "GIT_CONFIG_NOSYSTEM" && v.is_some_and(|v| v == "1")), !cfg!(windows));
        assert!(c.get_envs().any(|(k, v)| k == "GIT_TERMINAL_PROMPT" && v.is_some_and(|v| v == "0")));
        let hooks = args.iter().find_map(|a| a.strip_prefix("core.hooksPath=")).unwrap();
        if cfg!(windows) {
            assert!(hooks.ends_with("git-empty-hooks") && Path::new(hooks).is_dir(), "{hooks}");
        } else {
            assert_eq!(hooks, "/dev/null");
        }
    }

    #[test]
    fn settings_fragments_merge_into_one() {
        let ctx = GuardCtx { guard: Guard::LocalOnly, start_on_default: false, default_branch: None, hook_cmd: Some("'/x' --dduck-guard-local-only".into()), global_config: None };
        let report = serde_json::json!({"hooks": {"PreToolUse": [{"matcher": "Write", "hooks": [{"type": "command", "command": "r"}]}], "PostToolUse": [{"matcher": "*", "hooks": []}]}, "disableAllHooks": true});
        let args: Vec<String> = guard_args_with(&ctx, &[report]).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(args.iter().filter(|a| a.starts_with("--settings=")).count(), 1, "one --settings");
        let v: serde_json::Value = serde_json::from_str(args[1].strip_prefix("--settings=").unwrap()).unwrap();
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "*", "the guard hook comes first");
        assert!(v["hooks"]["PostToolUse"].is_array());
        assert_eq!(v["disableAllHooks"], false, "a fragment cannot switch the guard hook off");
        let draft: Vec<String> = guard_args_with(&GuardCtx::for_tests(), &[serde_json::json!({"hooks": {"PreToolUse": []}})]).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(draft.len(), 2);
    }

    // Local-only and its guard are Mac-only in the Windows preview (windows-support decision 7).
    #[cfg(unix)]
    #[test]
    fn local_only_refusals() {
        let t = tempfile::tempdir().unwrap();
        let src = |files: Vec<PathBuf>, dirs: Vec<PathBuf>, plists: Vec<PathBuf>| ManagedSources { files, dropin_dirs: dirs, plists };
        let none = src(vec![t.path().join("missing.json")], vec![t.path().join("no.d")], vec![t.path().join("no.plist")]);
        assert_eq!(local_only_refusal(Guard::DraftPr, true, &none), None);
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &none), None);
        assert_eq!(local_only_refusal(Guard::LocalOnly, true, &none), None, "measured: deny rules and the hook hold in auto/bypassPermissions (D-163)");
        let m = t.path().join("managed.json");
        let one = || src(vec![m.clone()], vec![], vec![]);
        for (text, blocked) in [(r#"{"allowManagedHooksOnly": true}"#, true), (r#"{"disableAllHooks": true}"#, true), ("{oops", true), ("[]", true), (r#"{"permissions": {}}"#, false)] {
            std::fs::write(&m, text).unwrap();
            assert_eq!(local_only_refusal(Guard::LocalOnly, false, &one()).is_some(), blocked, "{text}");
        }
        // managed-settings.d drop-ins (M2)
        let d = t.path().join("managed-settings.d");
        std::fs::create_dir(&d).unwrap();
        let dropin = || src(vec![], vec![d.clone()], vec![]);
        std::fs::write(d.join("10-a.json"), r#"{"model": "x"}"#).unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &dropin()), None);
        std::fs::write(d.join("20-b.json"), r#"{"allowManagedHooksOnly": true}"#).unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &dropin()), Some(MANAGED_BLOCKS));
        std::fs::write(d.join("20-b.json"), "not json").unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &dropin()), Some(MANAGED_BLOCKS), "present but unparseable");
        // MDM plist (converted with plutil)
        let pl = t.path().join("com.anthropic.claudecode.plist");
        let plist = || src(vec![], vec![], vec![pl.clone()]);
        std::fs::write(&pl, r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>model</key><string>x</string></dict></plist>"#).unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &plist()), None);
        std::fs::write(&pl, r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>allowManagedHooksOnly</key><true/></dict></plist>"#).unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &plist()), Some(MANAGED_BLOCKS));
        std::fs::write(&pl, "garbage").unwrap();
        assert_eq!(local_only_refusal(Guard::LocalOnly, false, &plist()), Some(MANAGED_BLOCKS));
        // The real system set includes the server-managed cache in the config folder.
        let sys = ManagedSources::system(Some(Path::new("/cfg")), Some("me"));
        assert!(sys.files.contains(&PathBuf::from("/cfg/remote-settings.json")));
        assert!(sys.plists.contains(&PathBuf::from("/Library/Managed Preferences/me/com.anthropic.claudecode.plist")));
    }

    #[test]
    fn start_ctx_forces_worktree_on_default_and_refuses_bad_list() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let repo = t.path().canon().unwrap().join("r");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let s = start_ctx(&conn, &repo, None, true, None).unwrap();
        assert!(s.force_worktree);
        assert!(!s.ctx.start_on_default, "the worktree branch is not the default branch");
        assert!(s.default_head.is_some());
        let s = start_ctx(&conn, &repo, None, false, None).unwrap();
        assert!(!s.force_worktree);
        assert!(s.ctx.start_on_default, "no worktree: bare push rules");
        add(&conn, &repo, None).unwrap();
        let s = start_ctx(&conn, &repo, None, true, None).unwrap();
        assert_eq!(s.ctx.guard, Guard::LocalOnly);
        assert!(guard_args(&s.ctx).is_err(), "no hook executable: refused at spawn");
        // A hook command that does not deny fails the self-test.
        assert_eq!(start_ctx(&conn, &repo, None, true, Some(Path::new("/usr/bin/true"))).err(), Some(HOOK_MISSING));
        crate::settings::put_guarded(&conn, LOCAL_ONLY_PROJECTS, "nope").unwrap();
        assert_eq!(start_ctx(&conn, &repo, None, true, None).err(), Some(UNREADABLE));
    }

    #[test]
    fn start_on_default_adds_bare_push() {
        let r = deny_rules(Guard::DraftPr, true, None);
        assert!(r.iter().any(|x| x == "Bash(git push)"));
        assert!(r.iter().any(|x| x == "Bash(git push * HEAD*)"));
        assert!(r.iter().any(|x| x == "Bash(git push origin)"));
    }

    #[test]
    fn detected_default_branch_is_validated() {
        assert!(deny_rules(Guard::DraftPr, false, Some("develop")).iter().any(|x| x == "Bash(git push * develop)"));
        for bad in ["x) Edit(", "-x", "a..b", "a b", "a,b", ""] {
            let r = deny_rules(Guard::DraftPr, false, Some(bad));
            assert!(!r.iter().any(|x| x.contains(bad) && !bad.is_empty() && !x.contains("main") && !x.contains("master")), "{bad}");
            assert_eq!(r, deny_rules(Guard::DraftPr, false, None), "{bad}");
        }
        assert!(on_default(None, None), "detached HEAD counts as default");
        assert!(on_default(Some("develop"), Some("develop")));
        assert!(!on_default(Some("fix-1"), Some("develop")));
    }

    #[test]
    fn local_only_rules_and_hook_args() {
        let r = deny_rules(Guard::LocalOnly, false, None);
        for must in ["Bash(git push *)", "Bash(git * push*)", "Bash(gh *)", "mcp__github__*", "mcp__*__create_pull_request", "mcp__*__push_files"] {
            assert!(r.iter().any(|x| x == must), "{must}");
        }
        let ctx = GuardCtx { guard: Guard::LocalOnly, start_on_default: false, default_branch: None, hook_cmd: Some(hook_command(Path::new("/Apps/D'Duck/dduck"))), global_config: None };
        let args: Vec<String> = guard_args(&ctx).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(args[0].starts_with("--disallowedTools=Bash(gh pr merge *),"));
        let settings = args[1].strip_prefix("--settings=").unwrap();
        let v: serde_json::Value = serde_json::from_str(settings).unwrap();
        let h = &v["hooks"]["PreToolUse"][0]["hooks"][0];
        assert_eq!(h["onFailure"], "block");
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(v["disableAllHooks"], false);
        assert_eq!(h["command"], "'/Apps/D'\\''Duck/dduck' --dduck-guard-local-only");
        let no_hook = GuardCtx { hook_cmd: None, ..ctx };
        assert!(guard_args(&no_hook).is_err(), "local_only never runs without the hook");
        let draft: Vec<String> = guard_args(&GuardCtx::for_tests()).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(draft.len(), 1, "draft PR has no hook");
    }

    #[test]
    fn hook_denies_bypass_forms() {
        for cmd in [
            "git push",
            "git push origin fix-1",
            "git -C . push origin fix-1",
            "git -c push.default=current push",
            "git --git-dir=.git push",
            "git 'push' origin main",
            "env X=1 git push",
            "FOO=1 git push",
            "/usr/bin/git push",
            "timeout 30 git push",
            "nohup git push &",
            "sh -c 'git push'",
            "bash -lc \"cd a && git push\"",
            "zsh -c 'sh -c \"git push\"'",
            "echo $(git push)",
            "echo `git push`",
            "(cd sub; git push)",
            "true && git push || false",
            "ls | xargs git push",
            "git -c alias.x=push x",
            "git submodule foreach 'git push'",
            "git lfs push origin",
            "gh repo view",
            "gh api repos/o/r/pulls/1/merge -X PUT",
            "/opt/homebrew/bin/gh pr create",
            "hub push",
            "echo 'git push' | sh",
            "sh <<EOF\ngit push\nEOF",
            "G=git; $G push",
            "eval git push",
            "git push 'unclosed",
            "git push $(",
            "for b in a b; do git push origin $b; done",
            "if true; then git push; fi",
        ] {
            assert_eq!(bash(cmd), HookDecision::Deny, "{cmd}");
        }
    }

    /// The security probe cases (scratchpad/sec-dg/cases.txt) and the QA bypasses, as unit tests.
    fn probe(cmd: &str) -> HookDecision {
        let alias = |dir: Option<&str>, w: &str| -> Result<Option<String>, ()> {
            match (dir, w) {
                (None, "p") => Ok(Some("push".into())),
                (Some("../other"), "q") => Ok(Some("push".into())),
                (Some(_), "q") => Ok(None),
                (_, "z") => Ok(None),
                _ => Ok(None),
            }
        };
        local_only_hook(&serde_json::json!({"tool_name": "Bash", "tool_input": {"command": cmd}}).to_string(), &alias)
    }

    #[test]
    fn security_probe_cases_are_denied() {
        for cmd in [
            "git push origin x",
            "git -C . push origin x",
            "env GIT_DIR=.git git push",
            "/usr/bin/git push",
            "git p origin x",
            "sh -c 'git push'",
            "bash -lc \"git push\"",
            "echo $(git push)",
            "echo `git push`",
            "eval git push",
            "python3 -c \"import subprocess; subprocess.run(['git','push'])\"",
            "git --config-env=alias.z=ZV z origin x",
            "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.z GIT_CONFIG_VALUE_0=push git z origin x",
            "git -c include.path=/tmp/x.cfg z origin x",
            "git -C ../other q origin x",
            "cd ../other && git q origin x",
            "echo push | xargs git",
            "echo origin x | xargs git push",
            "env -S 'git push origin x'",
            "{git,push} origin x",
            "/usr/bin/gi? push origin x",
            "ln -s /usr/bin/git ./g && ./g push origin x",
            "/Library/Developer/CommandLineTools/usr/libexec/git-core/git-push origin x",
            "git-receive-pack x",
            "git pus\\h origin x",
            "shopt -s expand_aliases\nalias pp='git push'\npp origin x",
            "git -c remote.origin.pushurl=x push",
            "git -c core.sshCommand='sh -c \"git push\"' fetch",
            "git -c core.pager='git push' log",
            "GIT_SSH_COMMAND=x git fetch",
            "git -c credential.helper='!git push' fetch",
            "perl -e 'system(\"git push\")'",
            "osascript -e 'do shell script \"git push\"'",
            "gh pr merge 1",
            "/opt/homebrew/bin/gh pr merge 1",
            "f() { git push; }; f",
            "function f { git push; }",
            "exec git push",
            "nice -n 5 git push",
            "timeout 5 git push",
            "caffeinate -i git push",
            "script -q /dev/null git push",
            "nohup git push &",
            "watch -n1 git push",
            "find . -maxdepth 0 -exec git push \\;",
            "find . -exec git push \\;",
            "parallel git ::: push",
            "git submodule foreach git push",
            "git rebase -x 'git push' HEAD~1",
            "git bisect run sh -c 'git push'",
            "git -c alias.x=push x",
            "git difftool -x 'sh -c \"git push\"' HEAD",
            // QA: an alias defined in the same command line
            "git config alias.q 'pu''sh' && git q",
            "git config --global alias.y push",
            "git config url.x.pushInsteadOf y",
            "git config core.hooksPath /tmp/h",
            "git -c core.hooksPath=/tmp/h commit -m x",
            "echo hi > .git/hooks/pre-commit",
            "cp evil .git/hooks/pre-push",
            "unset GIT_CONFIG_COUNT; git fetch",
            "export GH_TOKEN=abc",
            "GH_TOKEN=abc gh api user",
            "env -u GIT_CONFIG_COUNT git fetch",
            "env -i git push",
            "git remote set-url origin x",
            "git --exec-path=/tmp git status",
        ] {
            assert_eq!(probe(cmd), HookDecision::Deny, "{cmd}");
        }
        // Documented gaps the environment backstop covers (git inside scripts, curl with a token of its own).
        for cmd in ["npx some-pkg", "make push", "bash ./deploy.sh", "curl -X PUT https://api.github.com/repos/o/r/pulls/1/merge", "\u{ff47}\u{ff49}\u{ff54} push"] {
            assert_eq!(probe(cmd), HookDecision::NoDecision, "{cmd}");
        }
        // Re-audit (cases2.txt): switching the backstop off, or reaching the remote around it.
        for cmd in [
            "git config --global url.https://github.com/.pushInsteadOf https://github.com/",
            "git config url.https://github.com/.pushInsteadOf https://github.com/",
            "git config --file ~/.gitconfig url.https://github.com/.pushInsteadOf x",
            "printf '[url \"https://github.com/\"]\n\tpushInsteadOf = https://github.com/\n' >> ~/.gitconfig",
            "printf 'x' >> ~/.config/git/config",
            "printf 'x' | tee -a .git/config",
            "python3 -c \"open('.git/config','a').write('x')\"",
            "GIT_CONFIG_COUNT=0 git fetch",
            "env -u GIT_CONFIG_COUNT make push",
            "unset GIT_CONFIG_COUNT",
            "export -n GIT_CONFIG_COUNT",
            "declare +x GIT_CONFIG_COUNT",
            "typeset GIT_CONFIG_COUNT=0",
            "env -i make push",
            "env -i PATH=/usr/bin git status",
            "read GIT_CONFIG_COUNT <<< 0; make push",
            "printf -v GIT_CONFIG_COUNT 0; make push",
            "mapfile GIT_CONFIG_KEY_0 < f",
            "git -c url.x.insteadOf= fetch",
            "git fetch --upload-pack='sh -c x' origin",
            "git ls-remote --upload-pack='git push' origin",
            "git clone -u x origin d",
            "ssh git@github.com git-receive-pack o/r",
            "GIT_SSH_COMMAND=x git fetch",
            "git -c http.extraHeader='Authorization: x' fetch",
            "gh --hostname github.com api user",
            "node -e \"require('child_process').execSync('git push')\"",
            "ruby -e 'system(\"git push\")'",
            "python3 -c \"import os; os.environ.pop('GIT_CONFIG_COUNT'); os.system('make push')\"",
            "make push GIT_CONFIG_COUNT=0",
            "npm config set git x",
            "echo hi >> ~/.gitconfig",
            "cat x | tee ~/.config/git/config",
        ] {
            assert_eq!(probe(cmd), HookDecision::Deny, "{cmd}");
        }
        // Final re-audit: scripts that would turn transports back on later.
        for cmd in [
            "printf 'push:\\n\\tGIT_ALLOW_PROTOCOL=file git push\\n' > Makefile",
            "cat > run.sh <<'EOF'\nexport GIT_ALLOW_PROTOCOL=file\ngit push\nEOF",
            "echo 'protocol.file.allow=always' >> cfg",
            "GIT_ALLOW_PROTOCOL=file git push",
        ] {
            assert_eq!(probe(cmd), HookDecision::Deny, "{cmd}");
        }
        // The remaining case is the plain backstop case: make runs git with the protected environment.
        assert_eq!(probe("git status && make push"), HookDecision::NoDecision);
        for cmd in ["git commit -m 'fix' --cleanup=strip", "git status", "git config --get user.email", "git -c color.ui=never log -1", "cat .git/config", "git remote -v", "git log --oneline -3"] {
            assert_eq!(probe(cmd), HookDecision::NoDecision, "{cmd}");
        }
    }

    #[test]
    fn hook_mcp_shell_fields_and_file_tools() {
        let d = |s: serde_json::Value| local_only_hook(&s.to_string(), &no_alias);
        assert_eq!(d(serde_json::json!({"tool_name": "mcp__desktop-commander__start_process", "tool_input": {"command": "ls"}})), HookDecision::Deny);
        assert_eq!(d(serde_json::json!({"tool_name": "mcp__runner__exec", "tool_input": {"cmd": "echo"}})), HookDecision::Deny);
        assert_eq!(d(serde_json::json!({"tool_name": "mcp__notion__search", "tool_input": {"query": "회의록"}})), HookDecision::NoDecision);
        // Any field at any depth (M1): argv arrays, code, nested input, keys.
        for input in [
            serde_json::json!({"query": "git push"}),
            serde_json::json!({"argv": ["git", "push", "origin"]}),
            serde_json::json!({"code": "import os; os.system('gh pr create')"}),
            serde_json::json!({"opts": {"steps": [{"run": "git-receive-pack x"}]}}),
            serde_json::json!({"env": {"GIT_CONFIG_COUNT": "0"}}),
            serde_json::json!({"x": "PUSH it"}),
        ] {
            assert_eq!(d(serde_json::json!({"tool_name": "mcp__runner__run", "tool_input": input})), HookDecision::Deny, "{input}");
        }
        assert_eq!(d(serde_json::json!({"tool_name": "mcp__notion__search", "tool_input": {"q": "digit pushover github"}})), HookDecision::NoDecision, "substrings are not tokens");
        for path in ["/Users/me/.gitconfig", "/Users/me/.config/git/config", "/x/local-only-gitconfig"] {
            assert_eq!(d(serde_json::json!({"tool_name": "Write", "tool_input": {"file_path": path}})), HookDecision::Deny, "{path}");
        }
        assert_eq!(d(serde_json::json!({"tool_name": "Task", "tool_input": {"script": "git push"}})), HookDecision::Deny);
        assert_eq!(d(serde_json::json!({"tool_name": "Task", "tool_input": {"script": 5}})), HookDecision::Deny, "non-string shell field");
        for (tool, path) in [("Write", "/r/.git/hooks/pre-push"), ("Edit", "/r/.husky/pre-commit"), ("MultiEdit", "/r/.git/config"), ("NotebookEdit", "/r/.git/hooks/x")] {
            let key = if tool == "NotebookEdit" { "notebook_path" } else { "file_path" };
            assert_eq!(d(serde_json::json!({"tool_name": tool, "tool_input": {key: path}})), HookDecision::Deny, "{tool} {path}");
        }
        assert_eq!(d(serde_json::json!({"tool_name": "Write", "tool_input": {"file_path": "/r/src/main.rs"}})), HookDecision::NoDecision);
        for input in [
            serde_json::json!({"file_path": "/r/Makefile", "content": "push:\n\tGIT_ALLOW_PROTOCOL=file git push\n"}),
            serde_json::json!({"file_path": "/r/.env", "content": "GH_TOKEN=abc"}),
            serde_json::json!({"file_path": "/r/x.sh", "old_string": "a", "new_string": "export GIT_CONFIG_COUNT=0"}),
            serde_json::json!({"file_path": "/r/x.cfg", "edits": [{"old_string": "a", "new_string": "[protocol \"file\"]\n\tallow = always"}]}),
            serde_json::json!({"file_path": "/r/x.cfg", "content": "git -c protocol.https.allow=always push"}),
            serde_json::json!({"notebook_path": "/r/n.ipynb", "new_source": "os.environ['GIT_SSH_COMMAND']='x'"}),
        ] {
            let tool = if input.get("edits").is_some() { "MultiEdit" } else if input.get("new_source").is_some() { "NotebookEdit" } else if input.get("new_string").is_some() { "Edit" } else { "Write" };
            assert_eq!(d(serde_json::json!({"tool_name": tool, "tool_input": input})), HookDecision::Deny, "{tool} {input}");
        }
        assert_eq!(d(serde_json::json!({"tool_name": "Write", "tool_input": {"file_path": "/r/a.md", "content": "GIT_AUTHOR_NAME and git push notes"}})), HookDecision::NoDecision, "safe names and prose pass");
        let r = deny_rules(Guard::LocalOnly, false, None);
        for must in ["Edit(**/.git/hooks/**)", "Edit(**/.husky/**)", "Edit(**/.git/config)", "Bash(git config *alias*)"] {
            assert!(r.iter().any(|x| x == must), "{must}");
        }
        assert!(!r.iter().any(|x| x.starts_with("Write(")), "Write deny rules never match (CLI warning); Edit covers writes");
    }

    #[test]
    fn hook_relative_and_linked_programs() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/usr/bin/git", base.join("g")).unwrap();
        std::fs::write(base.join("build.sh"), "#!/bin/sh\n").unwrap();
        let run = |c: &str| local_only_hook(&serde_json::json!({"tool_name": "Bash", "tool_input": {"command": c}, "cwd": base.to_string_lossy()}).to_string(), &no_alias);
        #[cfg(unix)]
        assert_eq!(run("./g push origin x"), HookDecision::Deny, "symlink to git");
        assert_eq!(run("./missing push"), HookDecision::Deny, "unknown relative program");
        assert_eq!(run("./build.sh"), HookDecision::NoDecision);
    }

    #[test]
    fn draft_rules_cover_repo_flags_and_api_writes() {
        let r = deny_rules(Guard::DraftPr, false, None);
        for must in ["mcp__*__push_files", "mcp__*__create_or_update_file", "Bash(gh * pr merge*)", "Bash(gh api *-X*PUT*)", "Bash(gh api *--method*DELETE*)", "Bash(gh api *merges*)"] {
            assert!(r.iter().any(|x| x == must), "{must}");
        }
    }

    #[test]
    fn backstop_env_blocks_every_push_without_network() {
        // Settings carry the env, merged once; a later fragment cannot override a backstop key.
        let ctx = GuardCtx { guard: Guard::LocalOnly, start_on_default: false, default_branch: None, hook_cmd: Some("'/x' --dduck-guard-local-only".into()), global_config: None };
        let other = serde_json::json!({"env": {"GH_TOKEN": "real", "FOO": "1"}});
        let args: Vec<String> = guard_args_with(&ctx, &[other]).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        let v: serde_json::Value = serde_json::from_str(args[1].strip_prefix("--settings=").unwrap()).unwrap();
        assert_eq!(v["env"]["GH_TOKEN"], BLOCKED_TOKEN);
        assert_eq!(v["env"]["FOO"], "1");
        assert_eq!(v["env"]["GIT_CONFIG_KEY_2"], "protocol.allow");
        assert_eq!(v["env"]["GIT_CONFIG_NOSYSTEM"], "1");
        assert_eq!(v["env"]["GIT_ALLOW_PROTOCOL"], "dduck-blocked");
        assert_eq!(v["env"]["GIT_CONFIG_GLOBAL"], "/dev/null");
        // A local bare remote: with the env every push form fails before any transport, and the remote stays empty.
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let remote = base.join("remote.git");
        assert!(Command::new("git").args(["init", "-q", "--bare"]).arg(&remote).status().unwrap().success());
        let work = base.join("w");
        std::fs::create_dir(&work).unwrap();
        init_repo(&work);
        assert!(Command::new("git").arg("-C").arg(&work).args(["remote", "add", "origin"]).arg(&remote).status().unwrap().success());
        assert!(Command::new("git").arg("-C").arg(&work).args(["config", "remote.origin.pushurl"]).arg(&remote).status().unwrap().success());
        let env = backstop_env(None);
        let git = |args: &[&str]| {
            let mut c = Command::new("git");
            c.arg("-C").arg(&work).args(args).env("GIT_TERMINAL_PROMPT", "0");
            for (k, val) in env.as_object().unwrap() {
                c.env(k, val.as_str().unwrap());
            }
            c.output().unwrap()
        };
        let file_url = format!("file://{}", remote.display());
        let remote_s = remote.to_string_lossy().into_owned();
        for target in ["origin", remote_s.as_str(), file_url.as_str()] {
            let out = git(&["push", target, "main"]);
            assert!(!out.status.success(), "{target}");
            assert!(String::from_utf8_lossy(&out.stderr).contains("dduck-blocked"), "{target}: {}", String::from_utf8_lossy(&out.stderr));
        }
        let pushurl = git(&["-c", &format!("remote.origin.pushurl={remote_s}"), "push", "origin", "main"]);
        assert!(!pushurl.status.success(), "pushurl is rewritten too");
        let empty = Command::new("git").arg("-C").arg(&remote).args(["rev-parse", "--verify", "-q", "main"]).output().unwrap();
        assert!(!empty.status.success(), "nothing reached the remote");
        // Without the env the same push works (the fixture is real).
        assert!(Command::new("git").arg("-C").arg(&work).args(["push", "-q", "origin", "main"]).status().unwrap().success());
    }

    // Local-only and its guard are Mac-only in the Windows preview (windows-support decision 7).
    #[cfg(unix)]
    #[test]
    fn backstop_beats_longer_rewrites_global_local_and_included() {
        // H1: a longer `pushInsteadOf` from the global, the repository or an included file wins the URL rewrite, but
        // the protocol rules still refuse the transport. A `-c` override is denied by the hook (not tested here:
        // command-line config is the same scope as the backstop and the later one wins).
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let remote = base.join("remote.git");
        assert!(Command::new("git").args(["init", "-q", "--bare"]).arg(&remote).status().unwrap().success());
        let work = base.join("w");
        std::fs::create_dir(&work).unwrap();
        init_repo(&work);
        let remote_s = remote.to_string_lossy().into_owned();
        let file_url = format!("file://{remote_s}");
        assert!(Command::new("git").arg("-C").arg(&work).args(["remote", "add", "origin", &file_url]).status().unwrap().success());
        // A planted user global config with a longer rewrite back to the real remote.
        let user_global = base.join("user-gitconfig");
        std::fs::write(&user_global, format!("[url \"{file_url}\"]\n\tpushInsteadOf = {file_url}\n\tinsteadOf = {file_url}\n[user]\n\tname = Real Name\n\temail = real@example.invalid\n")).unwrap();
        // The same in the repository config and through an include.
        let inc = base.join("inc.cfg");
        std::fs::write(&inc, format!("[url \"{file_url}\"]\n\tpushInsteadOf = {file_url}\n")).unwrap();
        assert!(Command::new("git").arg("-C").arg(&work).args(["config", &format!("url.{file_url}.pushInsteadOf"), &file_url]).status().unwrap().success());
        assert!(Command::new("git").arg("-C").arg(&work).args(["config", "include.path"]).arg(&inc).status().unwrap().success());
        assert!(Command::new("git").arg("-C").arg(&work).args(["config", "protocol.file.allow", "always"]).status().unwrap().success());
        // The app-owned global copy keeps identity only.
        let home = base.join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::copy(&user_global, home.join(".gitconfig")).unwrap();
        let global = base.join("app/local-only-gitconfig");
        // `write_global_config` reads `--global` through the app's git, which honours HOME for ~/.gitconfig.
        let mut probe_cmd = app_git(&home);
        probe_cmd.args(["config", "--global", "--get", "user.name"]).env("HOME", &home);
        assert_eq!(String::from_utf8_lossy(&probe_cmd.output().unwrap().stdout).trim(), "Real Name");
        let env = backstop_env(Some(&global.to_string_lossy()));
        let push = |extra: &[(&str, &str)], target: &str| {
            let mut c = Command::new("git");
            c.arg("-C").arg(&work).args(["push", target, "main"]).env("GIT_TERMINAL_PROMPT", "0").env("HOME", &home);
            for (k, v) in env.as_object().unwrap() {
                c.env(k, v.as_str().unwrap());
            }
            for (k, v) in extra {
                c.env(k, v);
            }
            c.output().unwrap()
        };
        std::fs::create_dir_all(global.parent().unwrap()).unwrap();
        std::fs::write(&global, "").unwrap();
        for target in ["origin", file_url.as_str(), remote_s.as_str()] {
            let out = push(&[], target);
            assert!(!out.status.success(), "{target}: {}", String::from_utf8_lossy(&out.stderr));
        }
        // Even with the user's file forced back in as the global config, the transport is refused.
        let user_g = user_global.to_string_lossy().into_owned();
        let out = push(&[("GIT_CONFIG_GLOBAL", user_g.as_str())], "origin");
        assert!(!out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(!Command::new("git").arg("-C").arg(&remote).args(["rev-parse", "--verify", "-q", "main"]).output().unwrap().status.success(), "nothing reached the remote");
        // Without the backstop the planted rules do push (the fixture is real).
        assert!(Command::new("git").arg("-C").arg(&work).args(["push", "-q", "origin", "main"]).env("HOME", &home).status().unwrap().success());
    }

    #[test]
    fn local_only_global_config_keeps_identity_only() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().canon().unwrap();
        std::fs::write(home.join(".gitconfig"), "[user]\n\tname = Kim \"K\"\n\temail = k@example.invalid\n[url \"x\"]\n\tpushInsteadOf = y\n[alias]\n\tp = push\n[core]\n\tsshCommand = evil\n").unwrap();
        let path = if cfg!(windows) { home.join(r"AppData\Local\dev.dduck.desktop\local-only-gitconfig") } else { home.join("Library/Application Support/dev.dduck.desktop/local-only-gitconfig") };
        assert_eq!(global_config_path(&home), path);
        write_global_config(&path, &home).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("name = \"Kim K\"") && text.contains("email = \"k@example.invalid\""), "{text}");
        for bad in ["pushInsteadOf", "alias", "sshCommand", "url"] {
            assert!(!text.contains(bad), "{bad} copied: {text}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn second_clone_matches_by_origin_slug() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let mk = |name: &str| {
            let d = base.join(name);
            std::fs::create_dir(&d).unwrap();
            init_repo(&d);
            assert!(Command::new("git").arg("-C").arg(&d).args(["remote", "add", "origin", "git@github.com:Acme/Client.git"]).status().unwrap().success());
            d
        };
        let a = mk("clientA");
        let b = mk("clientA-clone");
        add(&conn, &a, None).unwrap();
        assert_eq!(load_items(&conn).unwrap()[0].slug.as_deref(), Some("github.com/acme/client"));
        assert_eq!(guard_for_folder(&conn, &b, None).unwrap(), Guard::LocalOnly, "another clone of the same origin");
        // Moving the ticked folder away keeps the stored slug.
        std::fs::rename(&a, base.join("moved")).unwrap();
        assert_eq!(guard_for_folder(&conn, &base.join("moved"), None).unwrap(), Guard::LocalOnly);
        let other = base.join("other");
        std::fs::create_dir(&other).unwrap();
        init_repo(&other);
        assert_eq!(guard_for_folder(&conn, &other, None).unwrap(), Guard::DraftPr);
    }

    #[test]
    fn cloud_guard_fails_closed() {
        assert!(CloudGuard::Unreadable.blocks(&[]));
        assert!(CloudGuard::load(None, None).blocks(&["github.com/a/b".into()]));
        let conn = crate::db::open_in_memory().unwrap();
        crate::settings::put_guarded(&conn, LOCAL_ONLY_PROJECTS, "{").unwrap();
        assert_eq!(CloudGuard::load(Some(&conn), None), CloudGuard::Unreadable);
        crate::settings::put_guarded(&conn, LOCAL_ONLY_PROJECTS, "[]").unwrap();
        assert!(!CloudGuard::load(Some(&conn), None).blocks(&["github.com/a/b".into()]));
    }

    #[test]
    fn post_check_stores_alert_bits() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let remote = base.join("remote.git");
        assert!(Command::new("git").args(["init", "-q", "--bare"]).arg(&remote).status().unwrap().success());
        let work = base.join("w");
        std::fs::create_dir(&work).unwrap();
        init_repo(&work);
        assert!(Command::new("git").arg("-C").arg(&work).args(["remote", "add", "origin"]).arg(&remote).status().unwrap().success());
        let head = branch_head(&work, "main").unwrap().unwrap();
        let start = chrono::Utc::now().timestamp_millis() - 2000;
        conn.execute("INSERT INTO task_meta(bg_id, created_at, cwd) VALUES ('ab12cd34', ?1, ?2)", rusqlite::params![start, work.to_string_lossy()]).unwrap();
        assert_eq!(post_check(&conn, "ab12cd34", None, chrono::Utc::now().timestamp()).unwrap(), None, "no scope recorded");
        crate::task_meta::record_guard(&conn, "ab12cd34", Guard::LocalOnly, Some("main"), Some(&head)).unwrap();
        assert_eq!(post_check(&conn, "ab12cd34", None, chrono::Utc::now().timestamp()).unwrap(), None, "no remote reflog yet, head unchanged");
        assert!(Command::new("git").arg("-C").arg(&work).args(["push", "-q", "origin", "main:fix-1"]).status().unwrap().success());
        assert_eq!(post_check(&conn, "ab12cd34", None, chrono::Utc::now().timestamp()).unwrap(), Some(ALERT_PUSH));
        assert!(Command::new("git").arg("-C").arg(&work).args(["commit", "-q", "--allow-empty", "-m", "on main"]).status().unwrap().success());
        assert_eq!(post_check(&conn, "ab12cd34", None, chrono::Utc::now().timestamp()).unwrap(), Some(ALERT_PUSH | ALERT_DEFAULT_MOVED));
        let stored: Option<i64> = conn.query_row("SELECT delivery_alert FROM task_meta WHERE bg_id = 'ab12cd34'", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, Some(3));
        assert_eq!(crate::task_report::delegated(&conn, "ab12cd34").unwrap().unwrap().delivery_alert, Some(3));
    }

    #[test]
    fn hook_leaves_local_work_alone() {
        for cmd in [
            "git commit -m '정리'",
            "git status && git diff",
            "npm test",
            "git log --grep push",
            "cat <<'EOF' > notes.md\ngit push is not run here\nEOF",
            "git commit -F - <<'EOF'\nfix: push button\nEOF",
            "echo done 2>&1",
            "command -v git",
            "# git push\nls",
            "git fetch origin",
        ] {
            assert_eq!(bash(cmd), HookDecision::NoDecision, "{cmd}");
        }
    }

    #[test]
    fn hook_alias_resolution() {
        let alias = |_: Option<&str>, w: &str| -> Result<Option<String>, ()> {
            match w {
                "p" => Ok(Some("push".into())),
                "sync" => Ok(Some("!git fetch && git push".into())),
                "st" => Ok(Some("status".into())),
                "broken" => Err(()),
                _ => Ok(None),
            }
        };
        let run = |c: &str| local_only_hook(&serde_json::json!({"tool_name": "Bash", "tool_input": {"command": c}}).to_string(), &alias);
        assert_eq!(run("git p origin x"), HookDecision::Deny);
        assert_eq!(run("git sync"), HookDecision::Deny);
        assert_eq!(run("git st"), HookDecision::NoDecision);
        assert_eq!(run("git broken"), HookDecision::Deny, "unknown alias state fails closed");
        assert_eq!(run("git unknown-ext"), HookDecision::NoDecision);
    }

    #[test]
    fn hook_input_and_mcp() {
        let d = |s: &str| local_only_hook(s, &no_alias);
        assert_eq!(d("not json"), HookDecision::Deny);
        assert_eq!(d(r#"{"tool_input":{}}"#), HookDecision::Deny);
        assert_eq!(d(r#"{"tool_name":"Bash","tool_input":{}}"#), HookDecision::Deny);
        assert_eq!(d(r#"{"tool_name":"Read","tool_input":{"file_path":"/x"}}"#), HookDecision::NoDecision);
        assert_eq!(d(r#"{"tool_name":"Monitor","tool_input":{"command":"git push"}}"#), HookDecision::Deny);
        for t in ["mcp__github__create_issue", "mcp__github__get_me", "mcp__claude_ai_GitHub__list_prs", "mcp__my-gitlab__x", "mcp__other__push_files", "mcp__x__merge_pull_request"] {
            assert_eq!(d(&format!(r#"{{"tool_name":"{t}","tool_input":{{}}}}"#)), HookDecision::Deny, "{t}");
        }
        assert_eq!(d(r#"{"tool_name":"mcp__notion__search","tool_input":{}}"#), HookDecision::NoDecision);
        assert_eq!(d(SELF_TEST_DENY), HookDecision::Deny);
        assert_eq!(d(SELF_TEST_PASS), HookDecision::NoDecision);
        let (out, code) = hook_output(HookDecision::Deny);
        assert_eq!(code, 2);
        assert!(out.contains("\"deny\"") && !out.contains("git"));
        assert_eq!(hook_output(HookDecision::NoDecision), (String::new(), 0), "never an explicit allow");
    }

    #[test]
    fn path_keys_nfc_case_and_components() {
        let nfd = "/Work/Client/\u{1100}\u{1161}"; // 가 decomposed
        let nfc = "/work/client/\u{AC00}";
        assert_eq!(norm_key(Path::new(nfd)), norm_key(Path::new(nfc)));
        assert!(key_within("/work/client/packages/api", "/work/client"));
        assert!(!key_within("/work/client2", "/work/client"));
        assert!(key_within("/work/client", "/work/client"));
    }

    #[test]
    fn worktree_is_possible_by_folder_not_by_origin() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let home = base.join("home");
        let repo = home.join("proj");
        std::fs::create_dir_all(&repo).unwrap();
        init_repo(&repo);
        // A news draft in a project repo on its default branch: the shell passes this to start_ctx, which forces a worktree.
        assert!(worktree_possible(&repo, &home));
        assert!(start_ctx(&conn, &repo, Some(&home), worktree_possible(&repo, &home), None).unwrap().force_worktree);
        // The home folder (where insight drafts may run) never gets one, repository or not.
        assert!(!worktree_possible(&home, &home));
        init_repo(&home);
        assert!(!worktree_possible(&home, &home));
        assert!(!worktree_possible(&base.join("nope"), &home));
    }

    fn init_repo(dir: &Path) {
        let ok = Command::new("git").arg("-C").arg(dir).args(["init", "-q", "-b", "main"]).status().unwrap().success();
        assert!(ok);
        for args in [&["config", "user.email", "t@example.invalid"][..], &["config", "user.name", "CANARY_NAME"], &["commit", "-q", "--allow-empty", "-m", "init"]] {
            assert!(Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success());
        }
    }

    #[test]
    fn repo_info_worktrees_and_guard() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let client = base.join("client");
        std::fs::create_dir_all(client.join("packages/api")).unwrap();
        init_repo(&client);
        let info = repo_info(&client.join("packages/api"), None).unwrap().unwrap();
        assert_eq!(info.root, client);
        // External `git worktree add`
        let wt = base.join("wt");
        assert!(Command::new("git").arg("-C").arg(&client).args(["worktree", "add", "-q", "-b", "fix-1"]).arg(&wt).status().unwrap().success());
        let wt_info = repo_info(&wt.canon().unwrap(), None).unwrap().unwrap();
        assert_eq!(wt_info.root, client, "worktree resolves to its main repository");
        // D.Duck worktree location
        let dd = client.join(".claude/worktrees/x");
        assert!(Command::new("git").arg("-C").arg(&client).args(["worktree", "add", "-q", "-b", "x"]).arg(&dd).status().unwrap().success());
        let items = vec![LocalOnlyItem { folder: client.to_string_lossy().into(), root: client.to_string_lossy().into(), slug: None }];
        for cwd in [client.clone(), client.join("packages/api"), wt.canon().unwrap(), dd.canon().unwrap()] {
            let r = repo_info(&cwd, None).unwrap();
            assert_eq!(guard_for(&cwd, r.as_ref(), &items), Guard::LocalOnly, "{}", cwd.display());
        }
        // Neighbour folder and a non-repo
        let other = base.join("client2");
        std::fs::create_dir_all(&other).unwrap();
        assert_eq!(repo_info(&other, None).unwrap(), None);
        assert_eq!(guard_for(&other, None, &items), Guard::DraftPr);
        // Case and NFC differences in the stored path
        let upper = vec![LocalOnlyItem { folder: client.to_string_lossy().to_uppercase(), root: "/nowhere".into(), slug: None }];
        assert_eq!(guard_for(&client, None, &upper), Guard::LocalOnly);
        // Broken .git file → error
        let broken = base.join("broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join(".git"), "nonsense").unwrap();
        assert!(repo_info(&broken, None).is_err());
        // Home boundary: a repo at home is never the root
        assert_eq!(repo_info(&client.join("packages"), Some(&client)).unwrap(), None);
        // Branches
        assert_eq!(start_branch(&client).unwrap().as_deref(), Some("main"));
        assert_eq!(start_branch(&wt).unwrap().as_deref(), Some("fix-1"));
        assert_eq!(detect_default_branch(&client).unwrap(), None, "no origin, no network");
        let head = branch_head(&client, "main").unwrap();
        assert!(head.as_deref().is_some_and(|h| h.len() == 40));
    }

    #[test]
    fn storage_fails_closed_and_round_trips() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(guard_for_folder(&conn, Path::new("/x"), None).unwrap(), Guard::DraftPr);
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().canon().unwrap();
        add(&conn, &dir, None).unwrap();
        add(&conn, &dir, None).unwrap();
        let items = load_items(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert!(is_ticked(&items, &dir));
        assert_eq!(guard_for_folder(&conn, &dir.join("sub"), None).unwrap(), Guard::LocalOnly);
        remove(&conn, &dir).unwrap();
        assert_eq!(guard_for_folder(&conn, &dir, None).unwrap(), Guard::DraftPr);
        crate::settings::put_guarded(&conn, LOCAL_ONLY_PROJECTS, "not json").unwrap();
        assert!(load_items(&conn).is_err());
        assert!(guard_for_folder(&conn, &dir, None).is_err(), "unreadable list refuses, never draft_pr");
        assert!(crate::settings::set_user_pref(&conn, LOCAL_ONLY_PROJECTS, "[]").is_err(), "generic setter cannot touch it");
    }

    #[test]
    fn checked_folder_rejects_links_and_files() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().canon().unwrap();
        let real = dir.join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(dir.join("f"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, dir.join("link")).unwrap();
        assert_eq!(checked_folder(&real), Some(real.clone()));
        assert_eq!(checked_folder(&dir.join("f")), None);
        assert_eq!(checked_folder(Path::new("relative")), None);
        #[cfg(unix)]
        assert_eq!(checked_folder(&dir.join("link")), None);
    }

    #[test]
    fn post_checks_default_moved_and_push_reflog() {
        assert!(default_moved(Some("a"), Some("b")));
        assert!(!default_moved(Some("a"), Some("a")));
        assert!(!default_moved(None, Some("a")));
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canon().unwrap();
        let remote = base.join("remote.git");
        assert!(Command::new("git").args(["init", "-q", "--bare"]).arg(&remote).status().unwrap().success());
        let work = base.join("work");
        std::fs::create_dir(&work).unwrap();
        init_repo(&work);
        assert!(Command::new("git").arg("-C").arg(&work).args(["remote", "add", "origin"]).arg(&remote).status().unwrap().success());
        let info = repo_info(&work, None).unwrap().unwrap();
        let before = chrono::Utc::now().timestamp() - 1;
        assert_eq!(push_detected(&info.common_dir, Guard::LocalOnly, None, before, before + 3600).unwrap(), None, "no remote reflog yet");
        assert!(Command::new("git").arg("-C").arg(&work).args(["push", "-q", "origin", "main:fix-1"]).status().unwrap().success());
        let after = chrono::Utc::now().timestamp() + 1;
        assert_eq!(push_detected(&info.common_dir, Guard::LocalOnly, None, before, after).unwrap(), Some(true));
        assert_eq!(push_detected(&info.common_dir, Guard::DraftPr, None, before, after).unwrap(), Some(false), "work-branch push is fine for draft PR");
        assert_eq!(push_detected(&info.common_dir, Guard::LocalOnly, None, before - 7200, before - 3600).unwrap(), Some(false), "outside the window");
        assert!(Command::new("git").arg("-C").arg(&work).args(["push", "-q", "origin", "main"]).status().unwrap().success());
        assert_eq!(push_detected(&info.common_dir, Guard::DraftPr, None, before, chrono::Utc::now().timestamp() + 1).unwrap(), Some(true));
        // Only time and message are read: the function returns a bool, the identity never leaves.
        assert_eq!(reflog_push_at("0 1 CANARY_NAME <t@example.invalid> 1700000000 +0900\tupdate by push"), Some(1_700_000_000));
        assert_eq!(reflog_push_at("0 1 n <e> 1700000000 +0900\tfetch: fast-forward"), None);
        // origin slug for cloud routines
        // (Windows writes the local path with escaped backslashes; only GitHub URLs matter for slugs.)
        #[cfg(unix)]
        assert_eq!(origin_url(&info.common_dir).as_deref(), Some(remote.to_string_lossy().as_ref()));
    }

    #[test]
    fn remote_slugs_and_cloud_block() {
        let want = Some("github.com/acme/app".to_string());
        assert_eq!(remote_slug("https://github.com/Acme/App.git"), want);
        assert_eq!(remote_slug("git@github.com:acme/app.git"), want);
        assert_eq!(remote_slug("ssh://git@github.com/acme/app"), want);
        assert_eq!(remote_slug("https://user@github.com/acme/app/"), want);
        assert_eq!(remote_slug("/local/path.git"), None);
        assert_eq!(remote_slug("https://github.com/acme"), None);
        let v = serde_json::json!({"id": "trig_1", "job_config": {"ccr": {"session_context": {"sources": [{"git_repository": {"url": "https://github.com/acme/app"}}]}}}});
        let slugs = repo_slugs_in(&v);
        assert_eq!(slugs, vec!["github.com/acme/app".to_string()]);
        assert!(cloud_blocked(&slugs, &["github.com/acme/app".into()]));
        assert!(!cloud_blocked(&slugs, &["github.com/acme/other".into()]));
    }

    #[test]
    fn guard_codes() {
        assert_eq!(Guard::from_code("local_only"), Some(Guard::LocalOnly));
        assert_eq!(Guard::from_code("x"), None);
        assert_eq!(Guard::DraftPr.code(), "draft_pr");
    }
}

/// windows-support 2.4 [CI]: the rules a Windows task gets.
#[cfg(test)]
mod windows_rules {
    use super::*;

    #[test]
    fn git_and_gh_rules_get_exe_forms_and_other_shells_are_refused() {
        let base = deny_rules(Guard::DraftPr, false, Some("main"));
        let w = windows_variants(base.clone());
        for r in base.iter().filter(|r| r.starts_with("Bash(git ") || r.starts_with("Bash(gh ")) {
            let exe = r.replacen("Bash(git ", "Bash(git.exe ", 1).replacen("Bash(gh ", "Bash(gh.exe ", 1);
            assert!(w.contains(&exe), "{exe} missing");
        }
        assert!(w.contains(&"Bash(git.exe push * main)".to_string()));
        for r in WINDOWS_RULES {
            assert!(w.contains(&r.to_string()), "{r}");
        }
        assert!(base.iter().all(|r| w.contains(r)), "nothing shared is dropped");
    }

    #[test]
    fn emitted_rules_follow_the_os() {
        let rules = deny_rules(Guard::DraftPr, true, Some("main"));
        assert_eq!(rules.iter().any(|r| r == "PowerShell"), cfg!(windows));
        assert_eq!(rules.iter().any(|r| r.starts_with("Bash(git.exe ")), cfg!(windows));
    }
}
