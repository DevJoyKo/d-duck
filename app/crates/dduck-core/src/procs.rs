//! Force-stopping a Claude session process (local-agent-tasks: 강제 종료, D-043).
//!
//! A pid from `claude agents --json` is only signalled after checking, right before the signal, that it still
//! belongs to the current user and runs a `claude` executable — so a recycled pid is never killed.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KillError {
    /// The pid is gone, belongs to someone else, or is not a claude process.
    NotAClaudeProcess,
    /// The process ignored SIGTERM and SIGKILL within the grace period.
    StillRunning,
    Unsupported,
}

/// `ps -o uid=,comm=` for one pid, parsed. `None` when the process does not exist.
#[cfg(unix)]
fn owner_and_command(pid: i64) -> Option<(u32, String)> {
    let out = Command::new("/bin/ps")
        .args(["-o", "uid=,comm=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (uid, comm) = line.split_once(char::is_whitespace)?;
    Some((uid.trim().parse().ok()?, comm.trim().to_string()))
}

/// True when `pid` runs an executable named `claude` (or `claude.exe`) owned by the current user.
#[cfg(unix)]
pub fn is_own_claude_process(pid: i64) -> bool {
    if pid <= 1 || pid > i64::from(i32::MAX) {
        return false;
    }
    // SAFETY: getuid has no preconditions.
    let me = unsafe { libc::getuid() };
    owner_and_command(pid).is_some_and(|(uid, comm)| matches_claude(uid, &comm, me))
}

/// Pure part of the check: owned by `me` and the executable's file name is `claude`/`claude.exe`.
fn matches_claude(uid: u32, comm: &str, me: u32) -> bool {
    let name = comm.rsplit('/').next().unwrap_or("");
    uid == me && (name == "claude" || name == "claude.exe")
}

#[cfg(unix)]
fn alive(pid: i64) -> bool {
    // SAFETY: signal 0 only checks for existence/permission.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// SIGTERM, wait up to `grace`, then SIGKILL. Re-verifies ownership before each signal.
#[cfg(unix)]
pub fn terminate(pid: i64, grace: Duration) -> Result<(), KillError> {
    if !is_own_claude_process(pid) {
        return Err(KillError::NotAClaudeProcess);
    }
    // SAFETY: plain syscall on a pid verified just above.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if !alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if is_own_claude_process(pid) {
        // SAFETY: as above; still our claude process after the grace period.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        std::thread::sleep(Duration::from_millis(300));
    }
    if alive(pid) && is_own_claude_process(pid) { Err(KillError::StillRunning) } else { Ok(()) }
}

#[cfg(not(unix))]
pub fn is_own_claude_process(_pid: i64) -> bool {
    false
}

#[cfg(not(unix))]
pub fn terminate(_pid: i64, _grace: Duration) -> Result<(), KillError> {
    Err(KillError::Unsupported)
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn refuses_non_claude_and_bogus_pids() {
        assert!(!is_own_claude_process(0));
        assert!(!is_own_claude_process(1));
        assert!(!is_own_claude_process(-5));
        assert!(!is_own_claude_process(i64::MAX));
        // Our own test process is not named claude.
        assert!(!is_own_claude_process(std::process::id() as i64));
        let mut sleeper = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        assert_eq!(terminate(sleeper.id() as i64, Duration::from_millis(200)), Err(KillError::NotAClaudeProcess));
        assert!(sleeper.try_wait().unwrap().is_none(), "a non-claude process must not be signalled");
        sleeper.kill().unwrap();
        let _ = sleeper.wait();
    }

    #[test]
    fn identification_rules() {
        // Real `ps comm` output for a Claude Code session on 2.1.x.
        let real = "/Users/me/.antigravity-ide/extensions/anthropic.claude-code-2.1.318-darwin-arm64/resources/native-binary/claude";
        assert!(matches_claude(502, real, 502));
        assert!(matches_claude(502, "/Users/me/.nvm/versions/node/v22/lib/node_modules/x/bin/claude.exe", 502));
        assert!(!matches_claude(501, real, 502), "another user's process");
        assert!(!matches_claude(502, "/bin/bash", 502));
        assert!(!matches_claude(502, "/usr/bin/claude-helper", 502));
        assert!(!matches_claude(502, "", 502));
    }
}
