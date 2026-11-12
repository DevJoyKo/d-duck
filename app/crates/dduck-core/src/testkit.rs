//! Test-only helpers shared by unit and integration tests (oss-publish-prep 1.2).
//!
//! The `basic` ingestion fixture is not committed as JSONL (the repository ignores `*.jsonl`).
//! [`ensure_basic`] regenerates it from `tests/fixtures/gen_fixtures.py` when it is missing.
//! The generator is deterministic, so the output is byte-identical on every machine.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, Once};

/// Files the generator writes. If any is missing the fixture is regenerated.
const OUTPUTS: [&str; 3] = [
    "projects/-tmp-PATHMARK-proj/11111111-1111-4111-8111-111111111111.jsonl",
    "projects/-tmp-PATHMARK-proj/11111111-1111-4111-8111-111111111111/subagents/agent-a1b2c3.jsonl",
    "projects/-tmp-PATHMARK-proj/11111111-1111-4111-8111-111111111111/subagents/agent-a1b2c3.meta.json",
];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

/// Returns the absolute path of `tests/fixtures/basic`, generating it first if needed.
///
/// Safe to call from many tests at once: a `Once` covers threads in one test binary and a
/// file lock covers test binaries running at the same time. Panics with the reason (in Korean,
/// the project's working language) when the fixture cannot be generated.
pub fn ensure_basic() -> PathBuf {
    static ONCE: Once = Once::new();
    static ERR: Mutex<Option<String>> = Mutex::new(None);
    let dir = fixtures_dir().join("basic");
    ONCE.call_once(|| {
        if let Err(e) = with_lock(&fixtures_dir(), || ensure_at(&dir, &python())) {
            if let Ok(mut slot) = ERR.lock() {
                *slot = Some(e);
            }
        }
    });
    if let Some(e) = ERR.lock().ok().and_then(|g| g.clone()) {
        panic!("{e}");
    }
    dir
}

/// The first interpreter that runs: `python3`, `python`, then the Windows launcher `py -3` (windows-support 5.2; on
/// Windows `python3` can be the Store alias that only prints an error). Falls back to `python3` for the error message.
#[doc(hidden)]
pub fn python() -> String {
    static FOUND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            ["python3", "python", "py"]
                .into_iter()
                .find(|p| python_command(p).arg("--version").output().is_ok_and(|o| o.status.success()))
                .unwrap_or("python3")
                .to_string()
        })
        .clone()
}

/// `py` is the Windows launcher and needs `-3` to pick Python 3.
fn python_command(python: &str) -> Command {
    let mut c = Command::new(python);
    if python == "py" {
        c.arg("-3");
    }
    c
}

/// Generates the fixture into `dir` with `python` unless every output already exists.
#[doc(hidden)]
pub fn ensure_at(dir: &Path, python: &str) -> Result<(), String> {
    if OUTPUTS.iter().all(|f| dir.join(f).is_file()) {
        return Ok(());
    }
    generate(dir, python)
}

/// Always runs the generator: `python -I <abs>/gen_fixtures.py <abs dir>`.
#[doc(hidden)]
pub fn generate(dir: &Path, python: &str) -> Result<(), String> {
    let script = fixtures_dir().join("gen_fixtures.py");
    let out = python_command(python).arg("-I").arg(&script).arg(dir).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "테스트 픽스처를 만들 수 없어요: `{python}`을 찾지 못했어요. \
                 python3를 설치하세요(macOS: `xcode-select --install`). 생성기: {}",
                script.display()
            )
        } else {
            format!("테스트 픽스처를 만들 수 없어요: `{python}` 실행 실패({e}). 생성기: {}", script.display())
        }
    })?;
    if !out.status.success() {
        return Err(format!(
            "테스트 픽스처 생성기가 실패했어요({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    match OUTPUTS.iter().find(|f| !dir.join(f).is_file()) {
        Some(f) => Err(format!("테스트 픽스처 생성기가 끝났지만 {f} 파일이 없어요.")),
        None => Ok(()),
    }
}

#[cfg(unix)]
fn with_lock<T>(dir: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    use std::os::unix::io::AsRawFd;
    let path = dir.join(".gen.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("테스트 픽스처 잠금 파일을 열 수 없어요({}): {e}", path.display()))?;
    // SAFETY: flock on a file descriptor we own; released when `file` is dropped.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(format!("테스트 픽스처 잠금을 얻지 못했어요: {}", std::io::Error::last_os_error()));
    }
    let r = f();
    drop(file);
    r
}

#[cfg(not(unix))]
fn with_lock<T>(_dir: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    f()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn digest(dir: &Path) -> Vec<u8> {
        let mut h = Sha256::new();
        for f in OUTPUTS {
            h.update(f.as_bytes());
            h.update(std::fs::read(dir.join(f)).unwrap());
        }
        h.finalize().to_vec()
    }

    #[test]
    fn generator_is_deterministic() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        generate(a.path(), &python()).unwrap();
        generate(b.path(), &python()).unwrap();
        assert_eq!(digest(a.path()), digest(b.path()));
        // The shared fixture is the same generator output.
        assert_eq!(digest(&ensure_basic()), digest(a.path()));
    }

    #[test]
    fn missing_python_gives_a_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("no-python3-here");
        let err = ensure_at(tmp.path(), missing.to_str().unwrap()).unwrap_err();
        assert!(err.contains("찾지 못했어요") && err.contains("python3를 설치"), "{err}");
    }

    #[test]
    fn existing_fixture_is_not_regenerated() {
        let tmp = tempfile::tempdir().unwrap();
        generate(tmp.path(), &python()).unwrap();
        // With every output present the interpreter is never invoked.
        ensure_at(tmp.path(), "/nonexistent/python3").unwrap();
    }
}
