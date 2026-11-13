//! Child-process helpers: bounded output and hard timeouts.

use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub struct Finished {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    Spawn,
    Timeout,
}

/// Which part of an over-long stdout to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    Head,
    /// The last bytes — for screen dumps whose newest frame is at the end (`claude logs`).
    Tail,
}

/// Runs `cmd` with stdin closed in its own process group, keeping at most `max_stdout` bytes of stdout.
/// Windows: no console window, and the process runs in a kill-on-close Job Object (decision 3), so its whole tree
/// ends when the run ends; use [`run_detached`] for commands that must leave the background service running.
///
/// On timeout the whole group is killed (the leader is still unreaped, so its id cannot have been reused) and the
/// call returns. On normal exit the group is left alone: `claude --bg` hands its work to a long-lived service, and
/// waiting on output is bounded by the deadline in case a grandchild keeps a pipe open.
pub fn run(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep, max_stderr: u64) -> Result<Finished, RunError> {
    run_until(cmd, timeout, max_stdout, keep, max_stderr, &|| false)
}

/// [`run`] that also kills the process group as soon as `stop()` returns true; that ends as `RunError::Timeout`,
/// and the caller, which owns the flag, tells the two apart.
pub fn run_until(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep, max_stderr: u64, stop: &dyn Fn() -> bool) -> Result<Finished, RunError> {
    run_tree(cmd, timeout, max_stdout, keep, max_stderr, stop, Tree::Job)
}

/// [`run`] for commands that hand work to the CLI's long-lived background service (`--bg` starts and resumes,
/// `agents`, `logs`, `stop`). macOS: identical to `run`. Windows: never put in a job, so the service survives this
/// call and app exit (decision 3); a timeout kills only the started process.
pub fn run_detached(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep, max_stderr: u64) -> Result<Finished, RunError> {
    run_tree(cmd, timeout, max_stdout, keep, max_stderr, &|| false, Tree::Detached)
}

/// How a started process and its descendants end (Windows only; macOS always uses its own process group).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tree {
    /// Windows: a kill-on-close Job Object, so the whole tree ends with the run (awaited runs).
    Job,
    /// Windows: no job; the process may leave a background service behind.
    Detached,
}

fn run_tree(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep, max_stderr: u64, stop: &dyn Fn() -> bool, tree: Tree) -> Result<Finished, RunError> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    let _ = tree;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    let (mut child, job) = match tree {
        Tree::Job => win::spawn_in_job(cmd).map(|(c, j)| (c, Some(j))).map_err(|_| RunError::Spawn)?,
        Tree::Detached => (win::spawn_no_window(cmd).map_err(|_| RunError::Spawn)?, None),
    };
    #[cfg(not(windows))]
    let mut child = cmd.spawn().map_err(|_| RunError::Spawn)?;
    let out = drain(child.stdout.take(), max_stdout, keep);
    let err = drain(child.stderr.take(), max_stderr, Keep::Head);
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Windows awaited run: end what the process left running first, so a descendant holding the pipes
                // cannot keep the join waiting until the deadline (review MEDIUM).
                #[cfg(windows)]
                if let Some(job) = &job {
                    job.terminate();
                }
                let stdout = join_until(out, deadline + Duration::from_secs(1));
                let stderr = join_until(err, deadline + Duration::from_secs(1));
                return Ok(Finished { status, stdout, stderr });
            }
            Ok(None) if Instant::now() < deadline && !stop() => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                #[cfg(windows)]
                if let Some(job) = &job {
                    job.terminate();
                }
                kill_group(&mut child);
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
        }
    }
}

type Drain = std::sync::mpsc::Receiver<Vec<u8>>;

fn drain<R: Read + Send + 'static>(pipe: Option<R>, max: u64, keep: Keep) -> Drain {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Some(mut pipe) = pipe else {
            let _ = tx.send(Vec::new());
            return;
        };
        match keep {
            Keep::Head => {
                let mut buf = Vec::new();
                let mut limited = (&mut pipe).take(max);
                let _ = limited.read_to_end(&mut buf);
                let _ = tx.send(buf);
                // Keep draining so the writer never blocks on a full pipe.
                let _ = std::io::copy(&mut pipe, &mut std::io::sink());
            }
            Keep::Tail => {
                let _ = tx.send(read_tail(&mut pipe, max as usize));
            }
        }
    });
    rx
}

/// Reads to EOF keeping only the last `max` bytes (memory stays bounded by ~2×max).
fn read_tail<R: Read>(r: &mut R, max: usize) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 2 * max {
                    buf.drain(..buf.len() - max);
                }
            }
        }
    }
    if buf.len() > max {
        buf.drain(..buf.len() - max);
    }
    buf
}

fn join_until(rx: Drain, deadline: Instant) -> Vec<u8> {
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_default()
}

fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    {
        // SAFETY: plain syscall on the process group we created with process_group(0); the leader is not yet
        // reaped, so the id still refers to our group.
        unsafe {
            libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

/// Which processes below `root` a login end may terminate (re-review 8.4, group 9 check), deepest first. Only those
/// whose listed name is in `names`, and only along edges where the parent was created no later than the child and the
/// child no later than `snap_at` (the moment before the process list was taken): a pid reused after its parent died
/// is older than nothing it claims, or newer than the list, and is skipped with its whole branch. `created` answers
/// a pid's creation time (`None` = gone or unreadable: skipped). Pure, so it is tested on every OS.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn select_descendants(root: u32, edges: &[(u32, u32)], names: &[&str], snap_at: u64, created: impl Fn(u32) -> Option<u64>, name: &dyn Fn(u32) -> Option<String>) -> Vec<u32> {
    let Some(root_born) = created(root) else { return Vec::new() };
    let mut tree: Vec<(u32, u64)> = vec![(root, root_born)];
    let mut i = 0;
    while i < tree.len() && tree.len() < 256 {
        let (parent, parent_born) = tree[i];
        for &(pid, ppid) in edges {
            if ppid != parent || pid == root || tree.iter().any(|(p, _)| *p == pid) {
                continue;
            }
            match created(pid) {
                Some(born) if parent_born <= born && born <= snap_at => tree.push((pid, born)),
                _ => {}
            }
        }
        i += 1;
    }
    tree.iter().skip(1).rev().map(|(p, _)| *p).filter(|p| name(*p).is_some_and(|n| names.iter().any(|w| n.eq_ignore_ascii_case(w)))).collect()
}

/// Windows process plumbing (windows-support decision 3).
#[cfg(windows)]
pub(crate) mod win {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    /// A Job Object that kills every process in it when the last handle closes. Not inheritable, no breakaway.
    pub struct Job(HANDLE);

    // SAFETY: a job handle may be used and closed from any thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> std::io::Result<Job> {
            // SAFETY: null attributes (not inheritable) and no name.
            let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if h.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Job(h);
            // SAFETY: zeroed is a valid value for this plain struct.
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `info` lives for the call and the size matches the class.
            let ok = unsafe {
                SetInformationJobObject(job.0, JobObjectExtendedLimitInformation, (&raw const info).cast(), std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32)
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }

        fn assign(&self, child: &Child) -> std::io::Result<()> {
            // SAFETY: both handles are open for the call (the child is not reaped).
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        /// Ends every process in the job now.
        pub fn terminate(&self) {
            // SAFETY: the handle is open until Drop.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: owned handle, closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Resumes the only thread of a process created suspended.
    fn resume_once(pid: u32) -> std::io::Result<()> {
        // SAFETY: a thread snapshot of the whole system; closed below.
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snap == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: zeroed is valid; dwSize is set as the API requires.
        let mut te: THREADENTRY32 = unsafe { std::mem::zeroed() };
        te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut resumed = 0;
        // SAFETY: `snap` is a valid snapshot and `te` a valid out pointer for every call.
        let mut more = unsafe { Thread32First(snap, &mut te) } != 0;
        while more {
            if te.th32OwnerProcessID == pid {
                // SAFETY: plain open/resume/close of one thread of our own suspended child.
                unsafe {
                    let t = OpenThread(THREAD_SUSPEND_RESUME, 0, te.th32ThreadID);
                    if !t.is_null() {
                        if ResumeThread(t) != u32::MAX {
                            resumed += 1;
                        }
                        CloseHandle(t);
                    }
                }
            }
            // SAFETY: as above.
            more = unsafe { Thread32Next(snap, &mut te) } != 0;
        }
        // SAFETY: owned snapshot handle.
        unsafe { CloseHandle(snap) };
        if resumed == 0 { Err(std::io::Error::other("no thread to resume")) } else { Ok(()) }
    }

    /// [`resume_once`] with a short retry: a thread snapshot taken right after creation can miss the new thread.
    fn resume(pid: u32) -> std::io::Result<()> {
        let mut last = Ok(());
        for _ in 0..3 {
            last = resume_once(pid);
            if last.is_ok() {
                return last;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        last
    }

    /// Ends the processes below `root` whose executable is one of `names` (ASCII case ignored), deepest first. Used
    /// when a login ends (re-review 8.4): the CLI's own helpers go, a browser it opened stays. `root` must be a live,
    /// unreaped child, so no process that names it as parent can belong to someone else.
    pub fn kill_descendants_named(root: u32, names: &[&str]) {
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess};
        let snap_at = now_filetime();
        // SAFETY: a process snapshot of the system; closed below.
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap == INVALID_HANDLE_VALUE {
            return;
        }
        let mut all: Vec<(u32, u32, String)> = Vec::new();
        // SAFETY: zeroed is valid; dwSize set as the API requires.
        let mut pe: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        // SAFETY: valid snapshot and out pointer for every call.
        let mut more = unsafe { Process32FirstW(snap, &mut pe) } != 0;
        while more {
            let len = pe.szExeFile.iter().position(|&c| c == 0).unwrap_or(pe.szExeFile.len());
            all.push((pe.th32ProcessID, pe.th32ParentProcessID, String::from_utf16_lossy(&pe.szExeFile[..len])));
            // SAFETY: as above.
            more = unsafe { Process32NextW(snap, &mut pe) } != 0;
        }
        // SAFETY: owned snapshot handle.
        unsafe { CloseHandle(snap) };
        // `snap_at` (taken just before the snapshot) bounds every creation time from above (group 9 check).
        let edges: Vec<(u32, u32)> = all.iter().map(|(pid, ppid, _)| (*pid, *ppid)).collect();
        let listed = |pid: u32| all.iter().find(|(p, _, _)| *p == pid).map(|(_, _, n)| n.clone());
        for pid in super::select_descendants(root, &edges, names, snap_at, created, &listed) {
            // SAFETY: open, query, terminate and close one process handle.
            unsafe {
                let h = OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    continue;
                }
                // Same process as checked: its creation time and name read again on this handle.
                let same = handle_created(h).is_some_and(|t| t <= snap_at) && handle_exe_name(h).is_some_and(|n| names.iter().any(|w| n.eq_ignore_ascii_case(w)));
                if same {
                    TerminateProcess(h, 1);
                }
                CloseHandle(h);
            }
        }
    }

    fn now_filetime() -> u64 {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
        let mut f = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        // SAFETY: valid out struct.
        unsafe { GetSystemTimeAsFileTime(&mut f) };
        (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)
    }

    /// Creation time (100 ns units) of an open process handle.
    fn handle_created(h: HANDLE) -> Option<u64> {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::GetProcessTimes;
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        // SAFETY: valid handle and four out structs.
        (unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) } != 0).then(|| (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime))
    }

    fn created(pid: u32) -> Option<u64> {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        // SAFETY: open, query and close one handle.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let t = handle_created(h);
            CloseHandle(h);
            t
        }
    }

    /// The executable's file name of an open process handle.
    fn handle_exe_name(h: HANDLE) -> Option<String> {
        use windows_sys::Win32::System::Threading::{PROCESS_NAME_WIN32, QueryFullProcessImageNameW};
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` has `len` slots; `len` receives the written length.
        if unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) } == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit(['\\', '/']).next().map(str::to_string)
    }

    /// Spawns without a console window.
    pub fn spawn_no_window(cmd: &mut Command) -> std::io::Result<Child> {
        cmd.creation_flags(CREATE_NO_WINDOW).spawn()
    }

    /// Spawns suspended, assigns the process to a new kill-on-close job, then resumes it, so nothing it starts can
    /// run outside the job. On any failure the child is killed and nothing is left running.
    pub fn spawn_in_job(cmd: &mut Command) -> std::io::Result<(Child, Job)> {
        let job = Job::new()?;
        let mut child = cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED).spawn()?;
        if let Err(e) = job.assign(&child).and_then(|()| resume(child.id())) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        Ok((child, job))
    }
}

/// A spawned child the caller owns, with its Windows job (if any) so the tree can be ended together.
pub struct Spawned {
    pub child: Child,
    pub tree: TreeGuard,
}

/// The Windows job of a [`Spawned`] child; nothing elsewhere (the caller signals the process group).
pub struct TreeGuard {
    #[cfg(windows)]
    job: Option<win::Job>,
}

impl TreeGuard {
    /// Windows: ends every process in the job now. Elsewhere a no-op.
    pub fn kill(&self) {
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.terminate();
        }
    }
}

impl Spawned {
    /// `claude auth login` (review H2). Unix: its own process group, as before. Windows: no job, because the CLI
    /// opens the browser as its child and a kill-on-close job would end the browser with the login; ending the login
    /// kills only the started process.
    pub fn login(cmd: &mut Command) -> std::io::Result<Spawned> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            Ok(Spawned { child: win::spawn_no_window(cmd)?, tree: TreeGuard { job: None } })
        }
        #[cfg(not(windows))]
        {
            Ok(Spawned { child: cmd.spawn()?, tree: TreeGuard {} })
        }
    }

    /// Unix: in its own process group (killed with `killpg` by the caller). Windows: in a kill-on-close job.
    pub fn awaited(cmd: &mut Command) -> std::io::Result<Spawned> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            let (child, job) = win::spawn_in_job(cmd)?;
            Ok(Spawned { child, tree: TreeGuard { job: Some(job) } })
        }
        #[cfg(not(windows))]
        {
            Ok(Spawned { child: cmd.spawn()?, tree: TreeGuard {} })
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Group 9 check: an older (reused) pid, a pid newer than the list and a different name are never picked.
    #[test]
    fn login_descendants_are_picked_by_time_and_name() {
        // root 10 (t=100) → 11 claude (t=110) → 12 claude (t=120); 13 named claude but born before 10 (reused pid);
        // 14 claude born after the list; 15 browser (t=115) under 11; 16 claude under 13 (t=130).
        let edges = [(11, 10), (12, 11), (13, 10), (14, 10), (15, 11), (16, 13)];
        let born = |p: u32| Some(match p { 10 => 100, 11 => 110, 12 => 120, 13 => 50, 14 => 999, 15 => 115, 16 => 130, _ => return None });
        let name = |p: u32| Some(if p == 15 { "msedge.exe".to_string() } else { "claude.exe".to_string() });
        let got = select_descendants(10, &edges, &["claude.exe", "node.exe"], 200, born, &name);
        assert_eq!(got, vec![12, 11], "deepest first; 13, 14, 16 (time) and 15 (name) left alone");
        assert!(select_descendants(10, &edges, &["claude.exe"], 200, |_| None, &name).is_empty(), "unreadable root");
    }

    #[test]
    fn tail_keeps_the_end() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let tail = read_tail(&mut std::io::Cursor::new(data.clone()), 1000);
        assert_eq!(tail, data[9000..]);
        assert_eq!(read_tail(&mut std::io::Cursor::new(b"abc".to_vec()), 1000), b"abc");
    }

    /// windows-support 2.2 [CI]: the job ends grandchildren of awaited runs; detached runs leave them alone.
    #[cfg(windows)]
    mod windows_tree {
        use super::super::*;
        use std::path::Path;

        fn alive(pid: u32) -> bool {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
            // SAFETY: open/query/close of a process handle by pid.
            unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    return false;
                }
                let mut code = 0u32;
                let ok = GetExitCodeProcess(h, &mut code) != 0;
                CloseHandle(h);
                ok && code == 259
            }
        }

        fn wait_for(f: impl Fn() -> bool) -> bool {
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until {
                if f() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            f()
        }

        fn pid_in(f: &Path) -> Option<u32> {
            std::fs::read_to_string(f).ok()?.trim().parse().ok()
        }

        fn fake() -> std::path::PathBuf {
            crate::testkit::fake_claude().expect("fake_claude example is built by cargo test")
        }

        #[test]
        fn cancel_ends_the_grandchild() {
            let t = tempfile::tempdir().unwrap();
            let pidfile = t.path().join("pid");
            let mut cmd = Command::new(fake());
            cmd.arg("spawn").arg(&pidfile).arg("30");
            let r = run_until(&mut cmd, Duration::from_secs(20), 1024, Keep::Head, 1024, &|| pid_in(&pidfile).is_some());
            assert_eq!(r.err(), Some(RunError::Timeout));
            let pid = pid_in(&pidfile).unwrap();
            assert!(wait_for(|| !alive(pid)), "grandchild {pid} still running after cancel");
        }

        #[test]
        fn awaited_run_ends_leftovers_on_normal_exit() {
            let t = tempfile::tempdir().unwrap();
            let pidfile = t.path().join("pid");
            let started = Instant::now();
            let done = run(Command::new(fake()).arg("spawn").arg(&pidfile).arg("30").arg("exit"), Duration::from_secs(20), 1024, Keep::Head, 1024).unwrap();
            assert!(started.elapsed() < Duration::from_secs(5), "returned after {:?}: the grandchild's pipe held the join", started.elapsed());
            assert!(done.status.success());
            let pid = pid_in(&pidfile).unwrap();
            assert!(wait_for(|| !alive(pid)), "child {pid} left behind by an awaited run");
        }

        #[test]
        fn detached_run_keeps_the_background_child() {
            let t = tempfile::tempdir().unwrap();
            let pidfile = t.path().join("pid");
            let done = run_detached(Command::new(fake()).arg("spawn").arg(&pidfile).arg("30").arg("exit"), Duration::from_secs(20), 1024, Keep::Head, 1024).unwrap();
            assert!(done.status.success());
            let pid = pid_in(&pidfile).unwrap();
            std::thread::sleep(Duration::from_millis(500));
            let kept = alive(pid);
            let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
            assert!(kept, "a --bg style child must survive its parent");
        }

        /// Re-review 8.4: a login's own helpers are ended with it; other programs are not named and stay.
        #[test]
        fn named_descendants_are_ended() {
            let t = tempfile::tempdir().unwrap();
            let pidfile = t.path().join("pid");
            let mut child = Command::new(fake()).arg("spawn").arg(&pidfile).arg("30").spawn().unwrap();
            assert!(wait_for(|| pid_in(&pidfile).is_some()));
            let grandchild = pid_in(&pidfile).unwrap();
            win::kill_descendants_named(child.id(), &["notepad.exe"]);
            assert!(alive(grandchild), "an unnamed program stays");
            win::kill_descendants_named(child.id(), &["FAKE_CLAUDE.EXE"]);
            assert!(wait_for(|| !alive(grandchild)), "the named helper is ended");
            let _ = child.kill();
            let _ = child.wait();
        }

        #[test]
        fn version_runs_without_a_console_and_in_a_job() {
            let done = run(Command::new(fake()).arg("--version"), Duration::from_secs(10), 1024, Keep::Head, 1024).unwrap();
            assert_eq!(String::from_utf8_lossy(&done.stdout).trim(), "2.1.330 (Claude Code)");
        }
    }

}
