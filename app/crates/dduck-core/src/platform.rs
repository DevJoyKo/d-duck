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
    fn home_is_never_empty() {
        assert!(home().is_none_or(|h| !h.as_os_str().is_empty()));
    }
}
