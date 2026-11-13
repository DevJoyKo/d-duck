//! Every OS split in one place (windows-support decision 1). On macOS each function is the std behaviour the code
//! used before, so macOS outputs do not change; Windows gets paths without `\\?\`, `USERPROFILE` and NTFS letter case.

use std::io;
use std::path::{Path, PathBuf};

/// The canonical path. macOS: `std::fs::canonicalize`. Windows: the same file, but `C:\x` instead of `\\?\C:\x` and
/// `\\server\share` instead of `\\?\UNC\server\share`, so paths compare and display as users and the CLI write them.
/// Every other call in the crate goes through here (clippy `disallowed-methods`).
#[allow(clippy::disallowed_methods)]
pub fn canon(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        dunce::canonicalize(p.as_ref()).map(|c| PathBuf::from(strip_verbatim(&c.to_string_lossy())))
    }
    #[cfg(not(windows))]
    {
        std::fs::canonicalize(p)
    }
}

/// `path.canon()` for call sites that used `path.canonicalize()`.
pub trait CanonExt {
    fn canon(&self) -> io::Result<PathBuf>;
}

impl CanonExt for Path {
    fn canon(&self) -> io::Result<PathBuf> {
        canon(self)
    }
}

/// Drops the verbatim prefix dunce keeps for long paths and reserved names: `\\?\C:\…` → `C:\…`,
/// `\\?\UNC\host\share\…` → `\\host\share\…`. Anything else (device paths, `\\?\Volume{…}`) is returned unchanged,
/// and `lexical_guard` refuses it on untrusted input.
pub fn strip_verbatim(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return rest.to_string();
        }
    }
    s.to_string()
}

/// An absolute path on this OS: `/…` on macOS; `C:\…` or `\\server\share\…` on Windows.
pub fn is_abs(p: impl AsRef<Path>) -> bool {
    p.as_ref().is_absolute()
}

/// The user's home folder, never empty. Windows reads `USERPROFILE` first (Claude Code does), macOS `HOME`.
pub fn home() -> Option<PathBuf> {
    let order: [&str; 2] = if cfg!(windows) { ["USERPROFILE", "HOME"] } else { ["HOME", "USERPROFILE"] };
    order.iter().find_map(|k| std::env::var_os(k).filter(|v| !v.is_empty())).map(PathBuf::from)
}

/// The app's own data folder under `home`, for code that has no Tauri handle: macOS
/// `~/Library/Application Support/dev.dduck.desktop`, Windows `~\AppData\Local\dev.dduck.desktop` (Tauri's
/// `app_local_data_dir`: local, not roaming).
pub fn app_data_under(home: &Path) -> PathBuf {
    if cfg!(windows) { home.join("AppData").join("Local").join(APP_ID) } else { home.join("Library/Application Support").join(APP_ID) }
}

/// The bundle identifier (tauri.conf.json `identifier`).
pub const APP_ID: &str = "dev.dduck.desktop";

/// The login name (`USER` on macOS, `USERNAME` on Windows).
pub fn user_name() -> Option<String> {
    std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).ok().filter(|u| !u.is_empty())
}

/// Identity of a file for hard-link and swap checks: (device or volume serial, inode or file index, link count),
/// plus whether the name itself is a link (symlink, or any reparse point such as a junction on Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileId {
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    pub link: bool,
}

/// The identity `ids()` helpers compare, read from `path` without following a final link. macOS: the same fields as
/// `symlink_metadata` (`m` when given, so the value is the one already checked). Windows: `GetFileInformationByHandle`
/// on a handle opened with `FILE_FLAG_OPEN_REPARSE_POINT`; when it cannot be read the result is
/// `(0, 0, u64::MAX)`, which every caller refuses as "more than one link" (fail closed, security MEDIUM 5).
pub fn ids(path: &Path, m: &std::fs::Metadata) -> (u64, u64, u64) {
    #[cfg(unix)]
    {
        let _ = path;
        use std::os::unix::fs::MetadataExt;
        (m.dev(), m.ino(), m.nlink())
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        match file_id_at(path) {
            Ok(f) if !f.link => (f.dev, f.ino, f.nlink),
            _ => (0, 0, u64::MAX),
        }
    }
}

/// The link count from `ids` (refused as `u64::MAX` when unknown on Windows).
pub fn nlink(path: &Path, m: &std::fs::Metadata) -> u64 {
    ids(path, m).2
}

/// `ids` for an already open file (its own handle on Windows, so nothing is re-opened by name).
pub fn handle_ids(f: &std::fs::File, m: &std::fs::Metadata) -> (u64, u64, u64) {
    #[cfg(unix)]
    {
        let _ = f;
        use std::os::unix::fs::MetadataExt;
        (m.dev(), m.ino(), m.nlink())
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        match file_id(f) {
            Ok(i) if !i.link => (i.dev, i.ino, i.nlink),
            _ => (0, 0, u64::MAX),
        }
    }
}

/// Read-only open that never follows a final link: `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` on unix (no FIFO block),
/// `FILE_FLAG_OPEN_REPARSE_POINT` on Windows (the link itself is opened, and `handle_ids` refuses it).
pub fn open_nofollow(path: &Path) -> io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    opts.open(path)
}

/// The path an open file really refers to, on Windows (`GetFinalPathNameByHandleW`, without `\\?\`).
#[cfg(windows)]
pub fn final_path(f: &std::fs::File) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS};
    let mut buf = vec![0u16; 1024];
    loop {
        // SAFETY: the handle is owned by `f`; `buf` has `buf.len()` u16 slots.
        let n = unsafe { GetFinalPathNameByHandleW(f.as_raw_handle() as _, buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS) } as usize;
        if n == 0 || n > 32_768 {
            return None;
        }
        if n < buf.len() {
            let s = std::ffi::OsString::from_wide(&buf[..n]);
            return Some(PathBuf::from(strip_verbatim(&s.to_string_lossy())));
        }
        buf.resize(n + 1, 0);
    }
}

/// `FileId` of `path` itself (a final symlink or reparse point is described, not followed).
pub fn file_id_at(path: &Path) -> io::Result<FileId> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        Ok(FileId { dev: m.dev(), ino: m.ino(), nlink: m.nlink(), link: m.file_type().is_symlink() })
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES};
        // Attributes only (no read access), any sharing, folders allowed, the name itself rather than its target.
        let f = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(7)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?;
        file_id(&f)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// `FileId` of an open file.
pub fn file_id(f: &std::fs::File) -> io::Result<FileId> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = f.metadata()?;
        Ok(FileId { dev: m.dev(), ino: m.ino(), nlink: m.nlink(), link: m.file_type().is_symlink() })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle};
        // SAFETY: zeroed is a valid BY_HANDLE_FILE_INFORMATION (plain integers).
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: the handle is owned by `f` and open for the call; `info` is a valid out pointer.
        if unsafe { GetFileInformationByHandle(f.as_raw_handle() as _, &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(FileId {
            dev: u64::from(info.dwVolumeSerialNumber),
            ino: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            nlink: u64::from(info.nNumberOfLinks),
            link: info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = f;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// What the webview needs to know about the OS (`platform` command, lib/platform.ts).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: &'static str,
    /// Git for Windows (Git Bash) is installed, so Claude Code has a Bash tool. Always true off Windows.
    pub git_bash: bool,
}

pub fn info() -> PlatformInfo {
    let os = if cfg!(target_os = "macos") { "macos" } else if cfg!(windows) { "windows" } else { "linux" };
    PlatformInfo { os, git_bash: !cfg!(windows) || git_bash().is_some() }
}

/// Git Bash as Claude Code finds it on Windows: `CLAUDE_CODE_GIT_BASH_PATH`, then `Git\bin\bash.exe` under the
/// Program Files folders. `None` off Windows.
pub fn git_bash() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let env = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from);
    let roots = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"].into_iter().filter_map(std::env::var_os);
    env.into_iter().chain(roots.map(|r| PathBuf::from(r).join("Git").join("bin").join("bash.exe"))).find(|p| p.is_absolute() && p.is_file())
}

/// Two paths name the same location: component by component, and on Windows without letter-case differences (NTFS).
pub fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) { same_path_ci(a, b) } else { a == b }
}

/// The Windows rule of `same_path`, kept callable on every OS so it is tested on macOS too.
pub fn same_path_ci(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| strip_verbatim(&p.to_string_lossy()).replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_drive_prefix_is_dropped() {
        assert_eq!(strip_verbatim(r"\\?\C:\Users\me\app"), r"C:\Users\me\app");
    }

    #[test]
    fn verbatim_unc_becomes_plain_unc() {
        assert_eq!(strip_verbatim(r"\\?\UNC\host\share\a.png"), r"\\host\share\a.png");
    }

    #[test]
    fn long_path_keeps_every_component() {
        let long = format!(r"\\?\C:\{}\end", "a".repeat(300));
        let out = strip_verbatim(&long);
        assert!(out.starts_with(r"C:\aaa") && out.ends_with(r"\end") && out.len() == long.len() - 4);
    }

    #[test]
    fn device_and_volume_paths_are_left_for_the_guard() {
        assert_eq!(strip_verbatim(r"\\?\Volume{1234}\x"), r"\\?\Volume{1234}\x");
        assert_eq!(strip_verbatim(r"\\.\COM1"), r"\\.\COM1");
        assert_eq!(strip_verbatim("/Users/me/app"), "/Users/me/app");
    }

    #[test]
    fn windows_comparison_ignores_case_and_prefix() {
        assert!(same_path_ci(Path::new(r"C:\Users\Me\App"), Path::new(r"\\?\c:\users\me\app\")));
        assert!(!same_path_ci(Path::new(r"C:\Users\me\app"), Path::new(r"C:\Users\me\app2")));
    }

    #[cfg(not(windows))]
    #[test]
    fn macos_comparison_is_case_sensitive_and_canon_is_std() {
        assert!(!same_path(Path::new("/a/B"), Path::new("/a/b")));
        let t = tempfile::tempdir().unwrap();
        #[allow(clippy::disallowed_methods)]
        let std_c = std::fs::canonicalize(t.path()).unwrap();
        assert_eq!(canon(t.path()).unwrap(), std_c);
    }

    #[cfg(windows)]
    #[test]
    fn windows_canon_has_no_verbatim_prefix() {
        let t = tempfile::tempdir().unwrap();
        let c = canon(t.path()).unwrap();
        assert!(!c.to_string_lossy().starts_with(r"\\?\"), "{}", c.display());
        assert!(same_path(&c, &t.path().canon().unwrap()));
    }

    #[test]
    fn hard_link_is_counted_on_every_os() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.md");
        std::fs::write(&a, "x").unwrap();
        let m = std::fs::symlink_metadata(&a).unwrap();
        assert_eq!(nlink(&a, &m), 1);
        std::fs::hard_link(&a, t.path().join("b.md")).unwrap();
        let m = std::fs::symlink_metadata(&a).unwrap();
        assert_eq!(nlink(&a, &m), 2, "a hard link must be seen (Windows: GetFileInformationByHandle)");
        let (fa, fb) = (file_id_at(&a).unwrap(), file_id_at(&t.path().join("b.md")).unwrap());
        assert_eq!((fa.dev, fa.ino), (fb.dev, fb.ino));
        let f = open_nofollow(&a).unwrap();
        assert_eq!(handle_ids(&f, &f.metadata().unwrap()).2, 2);
    }

    #[test]
    fn missing_file_has_no_identity() {
        let t = tempfile::tempdir().unwrap();
        assert!(file_id_at(&t.path().join("nope")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn junction_is_refused_as_a_link() {
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("f.md"), "x").unwrap();
        let j = t.path().join("j");
        // Junctions need no privilege (unlike directory symlinks).
        let ok = std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(&j).arg(&target).output().unwrap().status.success();
        assert!(ok, "mklink /J failed");
        let id = file_id_at(&j).unwrap();
        assert!(id.link, "a junction is a reparse point");
        let m = std::fs::symlink_metadata(&j).unwrap();
        assert_eq!(ids(&j, &m).2, u64::MAX, "refused as more than one link");
        assert!(m.file_type().is_symlink(), "std reports a junction as a link, so the existing symlink checks apply");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_not_followed() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("a"), "x").unwrap();
        std::os::unix::fs::symlink(t.path().join("a"), t.path().join("l")).unwrap();
        assert!(file_id_at(&t.path().join("l")).unwrap().link);
        assert!(open_nofollow(&t.path().join("l")).is_err());
    }

    #[test]
    fn home_is_never_empty() {
        assert!(home().is_none_or(|h| !h.as_os_str().is_empty()));
    }
}
