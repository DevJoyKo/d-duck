//! Review 7.10 [CI]: the real start path on Windows against the `fake_claude` example (shell scripts cannot stand in
//! for `claude.exe` there): emitted delivery rules and the command-line limit.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dduck_core::claude_cli::{ClaudeCli, CliError, TaskSpec};

fn fake() -> (tempfile::TempDir, ClaudeCli, std::path::PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let bin = t.path().join("bin").join("claude.exe");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::copy(dduck_core::testkit::fake_claude().expect("fake_claude example"), &bin).unwrap();
    let work = t.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    (t, ClaudeCli::new(bin, None), work)
}

fn spec(work: &std::path::Path, prompt: String) -> TaskSpec {
    let mut s = TaskSpec::basic(work.to_path_buf(), prompt, Some("job".into()), None);
    s.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    s
}

#[test]
fn start_background_emits_the_windows_rules() {
    let (t, cli, work) = fake();
    assert_eq!(cli.start_background(&spec(&work, "fix it".into()), None).unwrap(), "ab12cd34");
    let argv: Vec<String> = std::fs::read_to_string(t.path().join("bin").join("argv.log")).unwrap().lines().map(str::to_string).collect();
    assert_eq!(argv[0], "--bg");
    let tools = argv.iter().find_map(|a| a.strip_prefix("--disallowedTools=")).expect("disallowed tools");
    let rules: Vec<&str> = tools.split(',').collect();
    for r in ["PowerShell", "Monitor", "Bash(git.exe push * main)", "Bash(Git push * main)", "Bash(cmd *)", "Bash(wsl*)"] {
        assert!(rules.contains(&r), "{r} missing");
    }
    assert_eq!(argv.last().map(String::as_str), Some("fix it"));
}

#[test]
fn a_prompt_too_long_for_windows_never_starts() {
    let (t, cli, work) = fake();
    let r = cli.start_background(&spec(&work, "가 ".repeat(16_000)), None);
    assert!(matches!(&r, Err(CliError::InvalidInput { .. })), "{r:?}");
    assert!(format!("{r:?}").contains("WindowsCommandTooLong"), "{r:?}");
    assert!(!t.path().join("bin").join("argv.log").exists(), "claude never ran");
}
