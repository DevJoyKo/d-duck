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
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|_| RunError::Spawn)?;
    let out = drain(child.stdout.take(), max_stdout, keep);
    let err = drain(child.stderr.take(), max_stderr, Keep::Head);
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_until(out, deadline + Duration::from_secs(1));
                let stderr = join_until(err, deadline + Duration::from_secs(1));
                return Ok(Finished { status, stdout, stderr });
            }
            Ok(None) if Instant::now() < deadline && !stop() => std::thread::sleep(Duration::from_millis(20)),
            _ => {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_end() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let tail = read_tail(&mut std::io::Cursor::new(data.clone()), 1000);
        assert_eq!(tail, data[9000..]);
        assert_eq!(read_tail(&mut std::io::Cursor::new(b"abc".to_vec()), 1000), b"abc");
    }
}
