//! Drives `ClaudeCli` against a fake `claude` script that records argv/env and replays canned output.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dduck_core::claude_cli::{ClaudeCli, CliError, DangerApproval, PermissionMode, ResumeOutput, ResumeSpec, SessionStatus, TaskSpec};

struct Fake {
    _tmp: tempfile::TempDir,
    cli: ClaudeCli,
    argv_log: PathBuf,
    workdir: PathBuf,
}

/// `mode` selects the canned behaviour of the fake CLI.
fn fake(mode: &str) -> Fake {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin/claude");
    let argv_log = tmp.path().join("argv.log");
    let workdir = tmp.path().join("work");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&workdir).unwrap();
    let script = format!(
        r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a" >> '{log}'; done
printf 'CFG=%s\n' "$CLAUDE_CONFIG_DIR" >> '{log}'
printf 'CWD=%s\n' "$(pwd)" >> '{log}'
case "{mode}:$1" in
  ok:--bg)       echo 'backgrounded · ab12cd34 · job'; echo '  claude agents  list sessions';;
  untrusted:--bg) echo 'Workspace not trusted. Run `claude` in /x once' >&2; exit 1;;
  garbage:--bg)  echo 'unexpected';;
  copy:--bg)     echo 'backgrounded · ee11ff22 · job'; echo 'Session already running, started a copy';;
  fail:--bg)     echo 'Error: boom' >&2; exit 2;;
  ok:agents)     echo '[{{"pid":1,"id":"ab12cd34","kind":"background","sessionId":"s-1","name":"job","status":"waiting","state":"working","startedAt":5}}]';;
  ok:logs)       printf 'old\033[2J'; head -c 2000000 /dev/zero | tr '\0' 'x'; printf '\033[2Jscreen';;
  ok:stop)       echo "stopped $2";;
  fail:stop)     echo 'Error: no session' >&2; exit 1;;
  slow:agents)   sleep 10;;
esac
"#,
        log = argv_log.display(),
        mode = mode
    );
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cli = ClaudeCli::new(bin, Some(PathBuf::from("/cfg/claude-work")));
    Fake { _tmp: tmp, cli, argv_log, workdir }
}

fn logged(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(str::to_string).collect()
}

fn spec(f: &Fake, prompt: &str, mode: Option<PermissionMode>) -> TaskSpec {
    let mut s = TaskSpec::basic(f.workdir.clone(), prompt.into(), Some("job".into()), mode);
    s.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    s
}

#[test]
fn start_background_passes_argv_cwd_and_config_dir() {
    let f = fake("ok");
    let id = f.cli.start_background(&spec(&f, "-p x --dangerously-skip-permissions", Some(PermissionMode::Plan)), None).unwrap();
    assert_eq!(id, "ab12cd34");
    let argv = logged(&f.argv_log);
    assert_eq!(&argv[..3], ["--bg", "--name=job", "--permission-mode=plan"]);
    assert!(argv[3].starts_with("--disallowedTools=Bash(gh pr merge *),"), "{}", argv[3]);
    assert_eq!(&argv[4..6], ["--", "-p x --dangerously-skip-permissions"]);
    assert_eq!(argv[6], "CFG=/cfg/claude-work");
    assert!(argv[7].ends_with("/work"), "{}", argv[7]);
}

#[test]
fn start_background_refuses_without_a_delivery_scope() {
    let f = fake("ok");
    let mut s = spec(&f, "hi", None);
    s.guard_ctx = None;
    assert!(matches!(f.cli.start_background(&s, None), Err(CliError::InvalidInput { .. })));
    // local_only without a hook command: refused, never started with the default scope.
    s.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx { guard: dduck_core::delivery_guard::Guard::LocalOnly, ..dduck_core::delivery_guard::GuardCtx::for_tests() });
    assert!(matches!(f.cli.start_background(&s, None), Err(CliError::InvalidInput { .. })));
    assert!(logged(&f.argv_log).is_empty(), "claude never ran");
    // With the hook: one --settings carrying it.
    s.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx { guard: dduck_core::delivery_guard::Guard::LocalOnly, hook_cmd: Some("'/x' --dduck-guard-local-only".into()), ..dduck_core::delivery_guard::GuardCtx::for_tests() });
    f.cli.start_background(&s, None).unwrap();
    let argv = logged(&f.argv_log);
    assert!(argv.iter().any(|a| a.starts_with("--disallowedTools=") && a.contains("Bash(git push *)")));
    assert_eq!(argv.iter().filter(|a| a.starts_with("--settings=")).count(), 1);
}

#[test]
fn default_config_dir_is_not_passed() {
    let mut f = fake("ok");
    f.cli.config_dir_env = None;
    f.cli.start_background(&spec(&f, "hi", None), None).unwrap();
    assert!(logged(&f.argv_log).contains(&"CFG=".to_string()), "CLAUDE_CONFIG_DIR must be unset for the default dir");
}

#[test]
fn dangerous_mode_needs_native_approval() {
    let f = fake("ok");
    for mode in [PermissionMode::BypassPermissions, PermissionMode::Auto] {
        assert_eq!(f.cli.start_background(&spec(&f, "hi", Some(mode)), None), Err(CliError::DangerNotConfirmed));
    }
    assert!(logged(&f.argv_log).is_empty(), "CLI must not run without approval");
    let id = f.cli
        .start_background(&spec(&f, "hi", Some(PermissionMode::BypassPermissions)), Some(DangerApproval::after_native_confirmation()))
        .unwrap();
    assert_eq!(id, "ab12cd34");
}

#[test]
fn error_mapping() {
    let f = fake("untrusted");
    assert!(matches!(f.cli.start_background(&spec(&f, "hi", None), None), Err(CliError::UntrustedWorkspace { .. })));
    let f = fake("garbage");
    assert_eq!(f.cli.start_background(&spec(&f, "hi", None), None), Err(CliError::UnsupportedOutput));
    let f = fake("fail");
    match f.cli.stop("ab12cd34") {
        Err(CliError::Failed { code, stderr_tail }) => assert_eq!((code, stderr_tail.as_str()), (1, "Error: no session")),
        other => panic!("{other:?}"),
    }
    assert_eq!(f.cli.stop("$(id)"), Err(CliError::InvalidId));
    assert!(logged(&f.argv_log).iter().all(|a| a != "$(id)"));
}

#[test]
fn agents_logs_stop() {
    let f = fake("ok");
    let s = f.cli.list_agents().unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].status, SessionStatus::WaitingInput);
    assert_eq!(&logged(&f.argv_log)[..3], ["agents", "--json", "--all"]);

    // Output is 2 MB with the newest frame at the very end: the tail must be kept, not the head.
    let screen = f.cli.logs("ab12cd34").unwrap();
    assert_eq!(screen, b"\x1b[2Jscreen");
    assert!(screen.len() as u64 <= dduck_core::claude_cli::MAX_LOG_BYTES);

    f.cli.stop("ab12cd34").unwrap();
}

#[test]
fn slow_cli_times_out() {
    let f = fake("slow");
    let started = std::time::Instant::now();
    assert_eq!(f.cli.list_agents(), Err(CliError::Timeout));
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
}

fn resume_spec(f: &Fake) -> ResumeSpec {
    let spec = spec(f, dduck_core::limit_resume::RESUME_PROMPT, Some(PermissionMode::AcceptEdits));
    let mut r = ResumeSpec::from_prepared(&spec, "0f3b2a8e-1111-4222-8333-444455556666", f.workdir.clone()).unwrap();
    r.guard = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    r
}

#[test]
fn resume_runs_in_the_session_folder_and_reports_copies_and_failures() {
    let f = fake("ok");
    assert_eq!(f.cli.resume_background(&resume_spec(&f), None).unwrap(), ResumeOutput::Started("ab12cd34".into()));
    let argv = logged(&f.argv_log);
    assert_eq!(&argv[..3], ["--bg", "--resume=0f3b2a8e-1111-4222-8333-444455556666", "--name=job"]);
    assert!(argv.contains(&"--permission-mode=acceptEdits".to_string()));
    assert!(argv.contains(&dduck_core::limit_resume::RESUME_PROMPT.to_string()));
    let cwd = std::fs::canonicalize(&f.workdir).unwrap();
    assert!(argv.iter().any(|l| l == &format!("CWD={}", cwd.display())));

    let c = fake("copy");
    assert_eq!(c.cli.resume_background(&resume_spec(&c), None).unwrap(), ResumeOutput::Copy(Some("ee11ff22".into())));
    let u = fake("untrusted");
    assert!(matches!(u.cli.resume_background(&resume_spec(&u), None), Err(CliError::UntrustedWorkspace { .. })));
    let e = fake("fail");
    assert!(matches!(e.cli.resume_background(&resume_spec(&e), None), Err(CliError::Failed { code: 2, .. })));
    let g = fake("garbage");
    assert_eq!(g.cli.resume_background(&resume_spec(&g), None), Err(CliError::UnsupportedOutput));
}

#[test]
fn ids_starting_with_a_dash_never_reach_the_cli() {
    let f = fake("ok");
    assert_eq!(f.cli.stop("-h"), Err(CliError::InvalidId));
    assert_eq!(f.cli.logs("--all"), Err(CliError::InvalidId));
    assert!(f.cli.attach_command_line("-x").is_err());
    assert!(logged(&f.argv_log).is_empty());
}
