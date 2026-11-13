//! Login-state cache and the single `claude auth login` job (claude-login, design 3 and 6).
//!
//! Kept free of Tauri so the rate limit, single-flight and child lifetime are tested here; the shell only wires
//! triggers, events and dialogs. The sign-in URL stays in this process: the webview only learns `has_link`.

use std::process::Child;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use url::Url;

use crate::auth_status::{AuthState, Login};
use crate::claude_cli::{ClaudeCli, LoginMethod};
use crate::login_output::{self, Finding};

/// Focus checks run at most once per this long; the gate re-checks a cache older than this.
pub const MAX_AGE: Duration = Duration::from_secs(60);
pub const POLL_EVERY: Duration = Duration::from_secs(2);
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// A child that exits 0 may have written the login just before a slow `auth status` ran: keep checking this long
/// before calling it a failure (QA HIGH: success race).
pub const EXIT_GRACE: Duration = Duration::from_secs(10);
/// After a successful login a child still running gets this long to exit on its own.
const AFTER_SUCCESS: Duration = Duration::from_secs(2);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---------- state cache ----------

struct Inner {
    state: AuthState,
    checked_at: Option<Instant>,
    runs: u64,
}

/// Cached login state with single-flight checks: while one check runs, others wait and take its result.
pub struct AuthCache {
    inner: Mutex<Inner>,
    flight: Mutex<()>,
}

impl Default for AuthCache {
    fn default() -> Self {
        Self { inner: Mutex::new(Inner { state: AuthState::unknown(), checked_at: None, runs: 0 }), flight: Mutex::new(()) }
    }
}

/// Result of [`AuthCache::ensure`]: the state and whether it differs from the previous one.
pub struct Checked {
    pub state: AuthState,
    pub changed: bool,
}

impl AuthCache {
    pub fn current(&self) -> AuthState {
        lock(&self.inner).state.clone()
    }

    /// Whether a result younger than `max_age` exists (lets callers skip spawning a thread at all).
    pub fn is_fresh(&self, now: Instant, max_age: Duration) -> bool {
        lock(&self.inner).checked_at.is_some_and(|t| now.saturating_duration_since(t) < max_age)
    }

    /// Runs `check` unless a result younger than `max_age` exists (`Duration::ZERO` forces a run) or another call
    /// finished a check while this one waited. `clock` is read after the flight lock is taken (injected for tests).
    pub fn ensure(&self, clock: impl Fn() -> Instant, max_age: Duration, check: impl FnOnce() -> AuthState) -> Checked {
        let seen = lock(&self.inner).runs;
        let _flight = lock(&self.flight);
        let now = clock();
        {
            let inner = lock(&self.inner);
            let fresh = inner.checked_at.is_some_and(|t| now.saturating_duration_since(t) < max_age);
            if inner.runs != seen || (fresh && !max_age.is_zero()) {
                return Checked { state: inner.state.clone(), changed: false };
            }
        }
        let state = check();
        let mut inner = lock(&self.inner);
        let changed = inner.state != state;
        inner.state = state.clone();
        inner.checked_at = Some(now);
        inner.runs += 1;
        Checked { state, changed }
    }
}

// ---------- the login job ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailReason {
    /// 5 minutes without a login.
    Timeout,
    /// The child ended without a login.
    Exited,
    /// The CLI asked for a pasted code: the app never takes it (D-004).
    NeedsTerminal,
    /// The child could not be started.
    Spawn,
}

/// What the webview may see. Never the URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    pub phase: Phase,
    pub reason: Option<FailReason>,
    pub has_link: bool,
    pub method: Option<LoginMethod>,
}

#[derive(Default)]
struct Found {
    link: Option<Url>,
    needs_terminal: bool,
}

struct LoginJob {
    child: Child,
    /// Windows: the job holding the child's tree (ended on drop). Unix: nothing; the process group is used.
    tree: crate::process::TreeGuard,
    started: Instant,
    method: LoginMethod,
    found: Arc<Mutex<Found>>,
    /// When a clean exit (code 0) was first seen; the job keeps polling for [`EXIT_GRACE`].
    exited_ok_at: Option<Instant>,
}

impl Drop for LoginJob {
    fn drop(&mut self) {
        self.tree.kill();
        end_child(&mut self.child, Duration::from_secs(1));
    }
}

/// Whether the leader has exited, without reaping it (`waitid` with `WNOWAIT`). The leader stays a zombie, so its
/// pid, and with it the process group id, cannot be reused until [`end_child`] reaps it. Nothing else in this
/// module may call `try_wait`/`wait` on a job's child.
fn leader_exited(child: &Child) -> bool {
    leader_exit(child).is_some()
}

/// `None` while the leader runs; `Some(Some(code))` after a normal exit, `Some(None)` after a signal. Never reaps.
fn leader_exit(child: &Child) -> Option<Option<i32>> {
    #[cfg(unix)]
    {
        /// POSIX `si_code` for a normal exit (same value on macOS and Linux; not exported by libc for Apple).
        const CLD_EXITED: libc::c_int = 1;
        // SAFETY: zeroed siginfo is a valid out-parameter; waitid only writes into it.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::waitid(libc::P_PID, child.id() as libc::id_t, &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT) };
        // WNOHANG with nothing to report leaves si_pid at 0.
        if r != 0 || unsafe { info.si_pid() } == 0 {
            return None;
        }
        Some((info.si_code == CLD_EXITED).then(|| unsafe { info.si_status() }))
    }
    #[cfg(windows)]
    {
        // Exit code without reaping: the Child keeps its process handle open, so the pid cannot be reused.
        use std::os::windows::io::AsRawHandle;
        const STILL_ACTIVE: u32 = 259;
        let mut code: u32 = 0;
        // SAFETY: the handle is owned by `child` and open; `code` is a valid out pointer.
        let ok = unsafe { windows_sys::Win32::System::Threading::GetExitCodeProcess(child.as_raw_handle() as _, &mut code) } != 0;
        (ok && code != STILL_ACTIVE).then_some(Some(code as i32))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child;
        None
    }
}

/// SIGTERM to the whole group, SIGKILL after `grace`, then reap. The leader is never reaped before this point, so
/// the group id is still ours even when the leader already exited and only its children are left.
fn end_child(child: &mut Child, grace: Duration) {
    signal_group(child, Signal::Term);
    let until = Instant::now() + grace;
    while Instant::now() < until && !leader_exited(child) {
        std::thread::sleep(Duration::from_millis(20));
    }
    signal_group(child, Signal::Kill);
    let _ = child.wait();
}

enum Signal {
    Term,
    Kill,
}

fn signal_group(child: &mut Child, sig: Signal) {
    #[cfg(unix)]
    {
        let s = match sig {
            Signal::Term => libc::SIGTERM,
            Signal::Kill => libc::SIGKILL,
        };
        // SAFETY: plain syscall on the group created with process_group(0); the leader is unreaped (see end_child).
        unsafe {
            libc::killpg(child.id() as libc::pid_t, s);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = sig;
        let _ = child.kill();
    }
}

struct RunnerInner {
    job: Option<LoginJob>,
    phase: Phase,
    reason: Option<FailReason>,
    method: Option<LoginMethod>,
    /// Bumped on every start, so a poll loop of an older job stops.
    generation: u64,
}

/// At most one `claude auth login` at a time; the check and the spawn happen under one lock.
pub struct LoginRunner {
    inner: Mutex<RunnerInner>,
}

impl Default for LoginRunner {
    fn default() -> Self {
        Self { inner: Mutex::new(RunnerInner { job: None, phase: Phase::Idle, reason: None, method: None, generation: 0 }) }
    }
}

/// Whether [`LoginRunner::start`] spawned a new child or found one running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    New(u64),
    AlreadyRunning,
    Failed,
}

impl LoginRunner {
    fn status_of(inner: &RunnerInner) -> LoginStatus {
        let has_link = inner.job.as_ref().is_some_and(|j| lock(&j.found).link.is_some());
        LoginStatus { phase: inner.phase, reason: inner.reason, has_link, method: inner.method }
    }

    pub fn status(&self) -> LoginStatus {
        Self::status_of(&lock(&self.inner))
    }

    pub fn generation(&self) -> u64 {
        lock(&self.inner).generation
    }

    /// Spawns the child and its two output readers. A running job is never doubled.
    pub fn start(&self, cli: &ClaudeCli, method: LoginMethod, now: Instant) -> Started {
        let mut inner = lock(&self.inner);
        if inner.job.is_some() {
            return Started::AlreadyRunning;
        }
        inner.method = Some(method);
        let crate::process::Spawned { mut child, tree } = match cli.login_child(method) {
            Ok(c) => c,
            Err(_) => {
                inner.phase = Phase::Failed;
                inner.reason = Some(FailReason::Spawn);
                return Started::Failed;
            }
        };
        let found = Arc::new(Mutex::new(Found::default()));
        let readers: [Option<Box<dyn std::io::Read + Send>>; 2] =
            [child.stdout.take().map(|p| Box::new(p) as _), child.stderr.take().map(|p| Box::new(p) as _)];
        for pipe in readers.into_iter().flatten() {
            let found = Arc::clone(&found);
            std::thread::spawn(move || {
                login_output::scan(pipe, |f| {
                    let mut g = lock(&found);
                    match f {
                        Finding::Link(u) if g.link.is_none() => g.link = Some(u),
                        Finding::Link(_) => {}
                        Finding::NeedsTerminal => g.needs_terminal = true,
                    }
                });
            });
        }
        inner.job = Some(LoginJob { child, tree, started: now, method, found, exited_ok_at: None });
        inner.phase = Phase::Running;
        inner.reason = None;
        inner.generation += 1;
        Started::New(inner.generation)
    }

    /// One poll step (every [`POLL_EVERY`]) of job `generation` with the freshly checked `login`. A step for an
    /// older job (cancelled or replaced meanwhile) changes nothing. Returns the current status.
    pub fn step(&self, generation: u64, now: Instant, login: Login) -> LoginStatus {
        let (done, status) = {
            let mut inner = lock(&self.inner);
            if inner.generation != generation {
                return Self::status_of(&inner);
            }
            let Some(job) = inner.job.as_mut() else { return Self::status_of(&inner) };
            let exit = leader_exit(&job.child);
            let needs_terminal = lock(&job.found).needs_terminal;
            let outcome = if login == Login::LoggedIn {
                Some((Phase::Succeeded, None))
            } else if needs_terminal {
                Some((Phase::Failed, Some(FailReason::NeedsTerminal)))
            } else if exit == Some(Some(0)) {
                // Exit 0 without a login yet: the CLI may have just written it. Keep checking for a while.
                let at = *job.exited_ok_at.get_or_insert(now);
                (now.saturating_duration_since(at) >= EXIT_GRACE).then_some((Phase::Failed, Some(FailReason::Exited)))
            } else if exit.is_some() {
                Some((Phase::Failed, Some(FailReason::Exited)))
            } else if now.saturating_duration_since(job.started) >= LOGIN_TIMEOUT {
                Some((Phase::Failed, Some(FailReason::Timeout)))
            } else {
                None
            };
            match outcome {
                Some((phase, reason)) => {
                    inner.phase = phase;
                    inner.reason = reason;
                    let job = inner.job.take();
                    let status = Self::status_of(&inner);
                    (job.map(|j| (j, phase)), status)
                }
                None => (None, Self::status_of(&inner)),
            }
        };
        // End the child outside the lock.
        if let Some((job, phase)) = done {
            if phase == Phase::Succeeded {
                let until = Instant::now() + AFTER_SUCCESS;
                while Instant::now() < until && !leader_exited(&job.child) {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            drop(job);
        }
        status
    }

    /// [취소], the terminal fallback and app exit: ends the child group and goes idle.
    pub fn cancel(&self) -> LoginStatus {
        let job = {
            let mut inner = lock(&self.inner);
            inner.phase = Phase::Idle;
            inner.reason = None;
            inner.generation += 1;
            inner.job.take()
        };
        drop(job);
        self.status()
    }

    /// The vetted sign-in URL, for `/usr/bin/open` only. Never sent to the webview or logged.
    pub fn link(&self) -> Option<Url> {
        lock(&self.inner).job.as_ref().and_then(|j| lock(&j.found).link.clone())
    }

    /// The method of the running or last job (the terminal fallback reuses it).
    pub fn method(&self) -> Option<LoginMethod> {
        let inner = lock(&self.inner);
        inner.job.as_ref().map(|j| j.method).or(inner.method)
    }

    #[cfg(test)]
    fn pid(&self) -> Option<u32> {
        lock(&self.inner).job.as_ref().map(|j| j.child.id())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn logged_out() -> AuthState {
        crate::auth_status::judge(Some(1), br#"{"loggedIn":false}"#, 0)
    }

    #[test]
    fn focus_checks_are_rate_limited() {
        let cache = AuthCache::default();
        let runs = AtomicUsize::new(0);
        let t0 = Instant::now();
        for s in [0, 5, 12, 20, 29] {
            cache.ensure(|| t0 + Duration::from_secs(s), MAX_AGE, || {
                runs.fetch_add(1, Ordering::SeqCst);
                logged_out()
            });
        }
        assert_eq!(runs.load(Ordering::SeqCst), 1, "five focus events in 30 s → one run");
        cache.ensure(|| t0 + Duration::from_secs(61), MAX_AGE, || {
            runs.fetch_add(1, Ordering::SeqCst);
            logged_out()
        });
        assert_eq!(runs.load(Ordering::SeqCst), 2, "after 60 s it runs again");
        let forced = cache.ensure(|| t0 + Duration::from_secs(62), Duration::ZERO, AuthState::unknown);
        assert!(forced.changed, "[다시 확인] always runs and reports a change");
    }

    #[test]
    fn concurrent_requests_share_one_run() {
        let cache = Arc::new(AuthCache::default());
        let runs = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let (cache, runs, barrier) = (Arc::clone(&cache), Arc::clone(&runs), Arc::clone(&barrier));
                std::thread::spawn(move || {
                    barrier.wait();
                    cache
                        .ensure(Instant::now, Duration::ZERO, || {
                            runs.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(300));
                            logged_out()
                        })
                        .state
                })
            })
            .collect();
        let states: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert_eq!(states[0], states[1]);
        assert_eq!(states[0].login, Login::LoggedOut);
    }

    #[cfg(unix)]
    mod child {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn fake(body: &str) -> (tempfile::TempDir, ClaudeCli) {
            let tmp = tempfile::tempdir().unwrap();
            let bin = tmp.path().join("claude");
            std::fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
            let cli = ClaudeCli::new(bin, None);
            (tmp, cli)
        }

        fn group_alive(pid: u32) -> bool {
            // SAFETY: signal 0 only checks existence.
            unsafe { libc::killpg(pid as libc::pid_t, 0) == 0 }
        }

        fn wait_for(f: impl Fn() -> bool) -> bool {
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until {
                if f() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        }

        fn started(s: Started) -> u64 {
            match s {
                Started::New(g) => g,
                other => panic!("not started: {other:?}"),
            }
        }

        #[test]
        fn link_is_kept_in_memory_and_cancel_ends_the_group() {
            let (_tmp, cli) = fake("echo 'Opening browser. If it did not open, visit: https://claude.ai/oauth/authorize?state=SECRET_STATE'\nsleep 60 &\nsleep 60");
            let r = LoginRunner::default();
            let t0 = Instant::now();
            let g = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            assert_eq!(r.start(&cli, LoginMethod::ClaudeAi, t0), Started::AlreadyRunning, "never two at once");
            assert!(wait_for(|| r.status().has_link));
            let status = r.status();
            let json = serde_json::to_string(&status).unwrap();
            assert!(!json.contains("SECRET_STATE") && !json.contains("claude.ai"), "{json}");
            assert!(!format!("{status:?}").contains("SECRET_STATE"));
            assert_eq!(r.link().unwrap().host_str(), Some("claude.ai"));
            let pid = r.pid().unwrap();
            assert!(group_alive(pid));
            assert_eq!(r.step(g, t0 + POLL_EVERY, Login::LoggedOut).phase, Phase::Running);
            assert_eq!(r.cancel().phase, Phase::Idle);
            assert!(wait_for(|| !group_alive(pid)), "the background grandchild is gone too");
            assert!(r.link().is_none());
        }

        #[test]
        fn success_timeout_exit_and_paste_prompt() {
            let (_tmp, cli) = fake("sleep 60");
            let r = LoginRunner::default();
            let t0 = Instant::now();
            let g = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            let pid = r.pid().unwrap();
            assert_eq!(r.step(g, t0 + Duration::from_secs(10), Login::Unknown).phase, Phase::Running);
            let s = r.step(g, t0 + LOGIN_TIMEOUT, Login::LoggedOut);
            assert_eq!((s.phase, s.reason), (Phase::Failed, Some(FailReason::Timeout)));
            assert!(wait_for(|| !group_alive(pid)));

            let g = started(r.start(&cli, LoginMethod::Console, t0));
            let pid = r.pid().unwrap();
            assert_eq!(r.step(g, t0, Login::LoggedIn).phase, Phase::Succeeded);
            assert!(!group_alive(pid), "a child left after success is ended");

            let (_t2, quits) = fake("exit 1");
            let g = started(r.start(&quits, LoginMethod::ClaudeAi, t0));
            assert!(wait_for(|| r.step(g, t0, Login::LoggedOut).phase == Phase::Failed));
            assert_eq!(r.status().reason, Some(FailReason::Exited));

            let (_t3, paste) = fake("echo 'Paste code here if prompted >'\nsleep 60");
            let g = started(r.start(&paste, LoginMethod::ClaudeAi, t0));
            assert!(wait_for(|| r.step(g, t0, Login::LoggedOut).phase == Phase::Failed));
            assert_eq!(r.status().reason, Some(FailReason::NeedsTerminal));
        }

        #[test]
        fn children_of_an_exited_leader_are_ended() {
            // Security review MEDIUM: the leader exits first and leaves a child in its group.
            let (_tmp, cli) = fake("(sleep 120) &\nexit 0");
            let r = LoginRunner::default();
            let t0 = Instant::now();
            let g = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            let pid = r.pid().unwrap();
            assert!(wait_for(|| leader_exited(&lock(&r.inner).job.as_ref().unwrap().child)), "leader exit is seen without reaping it");
            assert!(group_alive(pid), "the zombie leader keeps the group id reserved");
            r.step(g, t0, Login::LoggedOut);
            assert_eq!(r.step(g, t0 + EXIT_GRACE, Login::LoggedOut).phase, Phase::Failed);
            assert_eq!(r.status().reason, Some(FailReason::Exited));
            assert!(wait_for(|| !group_alive(pid)), "the orphaned sleep in the group was killed");
        }

        #[test]
        fn only_allowlisted_links_are_kept() {
            let (_tmp, cli) = fake("echo 'visit https://claude.ai.example.com/oauth?state=X'\necho 'or http://claude.ai/oauth'\necho 'or https://user@claude.ai/x'\nsleep 60");
            let r = LoginRunner::default();
            started(r.start(&cli, LoginMethod::ClaudeAi, Instant::now()));
            std::thread::sleep(Duration::from_millis(300));
            assert!(!r.status().has_link);
            assert!(r.link().is_none(), "open_login_link gets nothing to open");
            r.cancel();
            let (_t2, ok) = fake("echo 'visit https://evil.com/x then https://platform.claude.com/oauth/authorize?a=1'\nsleep 60");
            started(r.start(&ok, LoginMethod::Console, Instant::now()));
            assert!(wait_for(|| r.status().has_link));
            assert_eq!(r.link().unwrap().as_str(), "https://platform.claude.com/oauth/authorize?a=1");
        }

        /// QA HIGH: the child writes the login and exits 0 while `auth status` still says logged out.
        #[test]
        fn clean_exit_keeps_polling_until_the_login_shows() {
            let (_tmp, cli) = fake("exit 0");
            let r = LoginRunner::default();
            let t0 = Instant::now();
            let g = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            assert!(wait_for(|| leader_exited(&lock(&r.inner).job.as_ref().unwrap().child)));
            assert_eq!(r.step(g, t0, Login::LoggedOut).phase, Phase::Running, "stale logged-out right after exit 0");
            assert_eq!(r.step(g, t0 + Duration::from_secs(2), Login::LoggedOut).phase, Phase::Running);
            assert_eq!(r.step(g, t0 + Duration::from_secs(4), Login::LoggedIn).phase, Phase::Succeeded, "fresh check sees it");

            let g = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            assert!(wait_for(|| leader_exited(&lock(&r.inner).job.as_ref().unwrap().child)));
            assert_eq!(r.step(g, t0, Login::LoggedOut).phase, Phase::Running);
            let s = r.step(g, t0 + EXIT_GRACE, Login::LoggedOut);
            assert_eq!((s.phase, s.reason), (Phase::Failed, Some(FailReason::Exited)), "gives up after the grace");

            let (_t2, bad) = fake("exit 3");
            let g = started(r.start(&bad, LoginMethod::ClaudeAi, t0));
            assert!(wait_for(|| r.step(g, t0, Login::LoggedOut).phase == Phase::Failed), "non-zero exit fails at once");
        }

        #[test]
        fn a_step_for_an_old_job_changes_nothing() {
            let (_tmp, cli) = fake("sleep 60");
            let r = LoginRunner::default();
            let t0 = Instant::now();
            let old = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            r.cancel();
            let new = started(r.start(&cli, LoginMethod::ClaudeAi, t0));
            assert_ne!(old, new);
            assert_eq!(r.step(old, t0, Login::LoggedIn).phase, Phase::Running, "stale poll loop ignored");
            assert_eq!(r.step(new, t0 + LOGIN_TIMEOUT, Login::LoggedOut).reason, Some(FailReason::Timeout));
        }

        #[test]
        fn big_output_does_not_block_and_drop_ends_the_group() {
            // 1 MB on stdout, then stays alive: the readers keep draining, so the child reaches `sleep`.
            let (tmp, cli) = fake("head -c 1048576 /dev/zero | tr '\\0' x\necho\necho done > \"$(dirname \"$0\")/reached\"\nsleep 60");
            let pid;
            {
                let r = LoginRunner::default();
                r.start(&cli, LoginMethod::ClaudeAi, Instant::now());
                pid = r.pid().unwrap();
                assert!(wait_for(|| tmp.path().join("reached").exists()), "child was not blocked by a full pipe");
            } // runner dropped → job dropped → group ended
            assert!(wait_for(|| !group_alive(pid)));
        }
    }
}
