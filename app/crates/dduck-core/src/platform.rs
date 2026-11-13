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

/// Windows ACL check for executables (decision 2, re-check fixes): refuses a file, its folder or any ancestor that
/// someone other than the current user, SYSTEM, Administrators or TrustedInstaller can change.
#[cfg(windows)]
pub mod acl {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetTokenInformation, INHERIT_ONLY_ACE, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const FILE_WRITE_DATA: u32 = 0x2;
    const FILE_APPEND_DATA: u32 = 0x4;
    const FILE_DELETE_CHILD: u32 = 0x40;
    const DELETE: u32 = 0x0001_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    const WRITE_OWNER: u32 = 0x0008_0000;
    const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
    const GENERIC_ALL: u32 = 0x1000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    /// SYSTEM, Administrators, TrustedInstaller (the current user is added at run time).
    const TRUSTED: [&str; 3] = ["S-1-5-18", "S-1-5-32-544", "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"];

    /// Where an entry sits relative to the executable.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Level {
        /// The executable itself.
        File,
        /// Its folder: adding a file there could plant a DLL the exe loads first.
        Folder,
        /// Folders above: only rights that replace or re-permission an entry on the path count.
        Ancestor,
    }

    /// Rights that refuse an entry at `level` when an untrusted SID holds any of them.
    pub fn refused_mask(level: Level) -> u32 {
        let base = DELETE | WRITE_DAC | WRITE_OWNER | GENERIC_ALL | MAXIMUM_ALLOWED;
        match level {
            Level::File => base | FILE_WRITE_DATA | FILE_APPEND_DATA | GENERIC_WRITE,
            Level::Folder => base | FILE_WRITE_DATA | FILE_APPEND_DATA | GENERIC_WRITE | FILE_DELETE_CHILD,
            Level::Ancestor => base | FILE_DELETE_CHILD,
        }
    }

    struct LocalSid(PSID);
    impl Drop for LocalSid {
        fn drop(&mut self) {
            // SAFETY: allocated by ConvertStringSidToSidW.
            unsafe { LocalFree(self.0) };
        }
    }

    struct Trusted {
        fixed: Vec<LocalSid>,
        /// TOKEN_USER buffer; the user SID points into it.
        user: Vec<u64>,
    }

    impl Trusted {
        fn new() -> Option<Trusted> {
            let mut fixed = Vec::new();
            for s in TRUSTED {
                let w: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
                let mut sid: PSID = std::ptr::null_mut();
                // SAFETY: NUL-terminated string; `sid` receives a LocalAlloc'd SID freed by LocalSid.
                if unsafe { ConvertStringSidToSidW(w.as_ptr(), &mut sid) } == 0 {
                    return None;
                }
                fixed.push(LocalSid(sid));
            }
            let mut token: HANDLE = std::ptr::null_mut();
            // SAFETY: query-only token of this process, closed below.
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
                return None;
            }
            let mut user = vec![0u64; 128];
            let mut len = 0u32;
            // SAFETY: `user` has room for 1 KB (a TOKEN_USER with any SID fits); 8-byte aligned.
            let ok = unsafe { GetTokenInformation(token, TokenUser, user.as_mut_ptr().cast(), (user.len() * 8) as u32, &mut len) } != 0;
            // SAFETY: owned token handle.
            unsafe { CloseHandle(token) };
            ok.then_some(Trusted { fixed, user })
        }

        fn has(&self, sid: PSID) -> bool {
            // SAFETY: the buffer holds a TOKEN_USER written by GetTokenInformation.
            let me = unsafe { (*(self.user.as_ptr() as *const TOKEN_USER)).User.Sid };
            // SAFETY: valid SIDs for the duration of the calls.
            unsafe { EqualSid(sid, me) != 0 || self.fixed.iter().any(|t| EqualSid(sid, t.0) != 0) }
        }
    }

    struct LocalDesc(PSECURITY_DESCRIPTOR);
    impl Drop for LocalDesc {
        fn drop(&mut self) {
            // SAFETY: allocated by GetNamedSecurityInfoW.
            unsafe { LocalFree(self.0) };
        }
    }

    /// `true` when `file` and every folder above it pass at their level. Any read failure refuses.
    pub fn check_chain(file: &Path) -> bool {
        let Some(t) = Trusted::new() else { return false };
        check(file, Level::File, &t).is_ok() && file.ancestors().skip(1).filter(|d| !d.as_os_str().is_empty()).enumerate().all(|(i, dir)| check(dir, if i == 0 { Level::Folder } else { Level::Ancestor }, &t).is_ok())
    }

    fn check(p: &Path, level: Level, t: &Trusted) -> Result<(), ()> {
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
        let (mut owner, mut dacl, mut sd): (PSID, *mut ACL, PSECURITY_DESCRIPTOR) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: NUL-terminated path; out pointers are valid; `sd` owns the returned owner and DACL.
        let rc = unsafe {
            GetNamedSecurityInfoW(wide.as_ptr(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, &mut owner, std::ptr::null_mut(), &mut dacl, std::ptr::null_mut(), &mut sd)
        };
        if rc != ERROR_SUCCESS {
            return Err(());
        }
        let _sd = LocalDesc(sd);
        // A NULL DACL grants everyone everything; an untrusted owner can rewrite the DACL.
        if owner.is_null() || dacl.is_null() || !t.has(owner) {
            return Err(());
        }
        let refused = refused_mask(level);
        // SAFETY: `dacl` points into `sd`, alive for this function.
        let count = unsafe { (*dacl).AceCount };
        for i in 0..u32::from(count) {
            let mut ace: *mut c_void = std::ptr::null_mut();
            // SAFETY: index below AceCount.
            if unsafe { GetAce(dacl, i, &mut ace) } == 0 || ace.is_null() {
                return Err(());
            }
            // SAFETY: every ACE starts with an ACE_HEADER.
            let h = unsafe { *(ace as *const ACE_HEADER) };
            if u32::from(h.AceFlags) & INHERIT_ONLY_ACE != 0 {
                continue;
            }
            match u32::from(h.AceType) {
                ACCESS_DENIED_ACE_TYPE => {}
                ACCESS_ALLOWED_ACE_TYPE => {
                    // SAFETY: an ACCESS_ALLOWED ACE; the SID starts at SidStart.
                    let a = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
                    if a.Mask & refused != 0 && !t.has((&raw const a.SidStart) as PSID) {
                        return Err(());
                    }
                }
                // Object, callback or unknown allow ACEs are not understood here, so they refuse.
                _ => return Err(()),
            }
        }
        Ok(())
    }
}

/// Lexical check of an untrusted path before any filesystem call (decision 1). macOS: always true. Windows: see
/// [`windows_lexical_ok`], so a path from a transcript or report can never reach a share, a device or a stream.
pub fn lexical_guard(p: &Path) -> bool {
    !cfg!(windows) || p.to_str().is_some_and(windows_lexical_ok)
}

/// The Windows rule of [`lexical_guard`], callable on every OS for tests. Allows a drive path (`C:\…`, `C:/…`) or a
/// relative one. Refuses UNC and device prefixes (`\\host\…`, `\\?\…`, `\\.\…`), drive-relative `C:x`, `:` in a
/// name (alternate streams), reserved device names (`CON`, `NUL`, `COM1`, `LPT1`, … also with an extension), names
/// ending in a dot or a space, and control characters.
pub fn windows_lexical_ok(s: &str) -> bool {
    if s.is_empty() || s.chars().any(char::is_control) || s.contains(['?', '*']) || s.starts_with(['\\', '/']) && s[1..].starts_with(['\\', '/']) {
        return false;
    }
    let b = s.as_bytes();
    let rest = if b.len() >= 2 && b[1] == b':' {
        if !b[0].is_ascii_alphabetic() || !s[2..].starts_with(['\\', '/']) {
            return false;
        }
        &s[2..]
    } else {
        s
    };
    rest.split(['\\', '/']).all(|c| {
        if matches!(c, "" | "." | "..") {
            return true;
        }
        let stem = c.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
            || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit())
            || ((stem.starts_with("COM") || stem.starts_with("LPT")) && ["¹", "²", "³"].iter().any(|d| stem.len() == 3 + d.len() && stem.ends_with(d)));
        !c.contains(':') && !device && !c.ends_with(['.', ' '])
    })
}

/// An untrusted relative path or file name (report evidence, attachments, skill parts) on this OS. macOS: true (the
/// callers' own rules apply). Windows: [`windows_relative_ok`] (review H3).
pub fn relative_ok(s: &str) -> bool {
    !cfg!(windows) || windows_relative_ok(s)
}

/// The Windows rule of [`relative_ok`], callable on every OS: [`windows_lexical_ok`] without any drive, root or `:`
/// (so `C:/x`, `C:x`, `\\x` and `a.md:s` are refused), and no `?` or `*`.
pub fn windows_relative_ok(s: &str) -> bool {
    windows_lexical_ok(s) && !s.contains([':', '?', '*']) && !s.starts_with(['\\', '/'])
}

/// A file or folder name equal to `want`: exact on macOS, ignoring ASCII case on Windows (NTFS), for deny lists.
pub fn name_is(name: &std::ffi::OsStr, want: &str) -> bool {
    if cfg!(windows) { name.to_str().is_some_and(|n| n.eq_ignore_ascii_case(want)) } else { name == want }
}

/// The git the app runs (decision 2, task 1.5). macOS: `git` from PATH, as before. Windows: resolved once to an
/// absolute `git.exe` (Git for Windows' usual folders, then PATH) that passes the ACL check; `None` when there is
/// none, and then the app runs no git at all.
pub fn git_program() -> Option<PathBuf> {
    if !cfg!(windows) {
        return Some(PathBuf::from("git"));
    }
    static FOUND: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            let roots = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"].into_iter().filter_map(std::env::var_os).map(PathBuf::from);
            let user = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Programs"));
            let fixed = roots.chain(user).map(|r| r.join("Git").join("cmd").join("git.exe"));
            let path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).filter(|d| d.is_absolute()).map(|d| d.join("git.exe")).collect::<Vec<_>>()).unwrap_or_default();
            // The checked final path is what runs (review MEDIUM: no link swap between check and use).
            fixed.chain(path).filter(|g| g.is_file()).find_map(|g| canon(g).ok().filter(|real| windows_exe_trusted(real)))
        })
        .clone()
}

#[cfg(windows)]
fn windows_exe_trusted(real: &Path) -> bool {
    acl::check_chain(real)
}

#[cfg(not(windows))]
fn windows_exe_trusted(_p: &Path) -> bool {
    false
}

/// `core.hooksPath` for the app's git. macOS: `/dev/null`, as before. Windows: an empty folder the app owns
/// (`/dev/null` there means `C:\dev\null`, which any user may create, security MEDIUM 8); `None` when it cannot be made
/// or is not empty, and then no git runs.
pub fn git_hooks_path() -> Option<PathBuf> {
    if !cfg!(windows) {
        return Some(PathBuf::from("/dev/null"));
    }
    let dir = app_data_under(&home()?).join("git-empty-hooks");
    std::fs::create_dir_all(&dir).ok()?;
    let m = std::fs::symlink_metadata(&dir).ok()?;
    let empty = std::fs::read_dir(&dir).ok()?.next().is_none();
    (m.file_type().is_dir() && empty).then_some(dir)
}

/// Fills `buf` from the OS random source: `/dev/urandom` on macOS (as before), `BCryptGenRandom` with the system RNG
/// on Windows (there is no `/dev/urandom`). `false` when the source fails; callers have no weaker fallback.
pub fn random_bytes(buf: &mut [u8]) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
        // SAFETY: `buf` is valid for `buf.len()` bytes; no algorithm handle with the system-preferred flag.
        unsafe { BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0 }
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).is_ok()
    }
}

/// Opening and deleting through the Windows shell (decision 6). Callers vet their values first; these functions
/// re-check what they can and never pass anything through a command interpreter.
#[cfg(windows)]
pub mod shell {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }

    /// The default browser for an `https://` URL (re-checked here; the caller keeps its own allow list).
    pub fn open_url(url: &str) -> bool {
        if !url.starts_with("https://") || url.chars().any(|c| c.is_control() || c == ' ' || c == '"') {
            return false;
        }
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let (verb, file) = (wide("open".as_ref()), wide(url.as_ref()));
        // SAFETY: NUL-terminated strings live for the call; no parameters or directory.
        let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
        r as isize > 32
    }

    /// File Explorer with `path` selected (`SHOpenFolderAndSelectItems`; nothing is executed).
    pub fn reveal(path: &Path) -> bool {
        use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
        use windows_sys::Win32::UI::Shell::{ILFree, SHOpenFolderAndSelectItems, SHParseDisplayName};
        if !path.is_absolute() || !super::lexical_guard(path) {
            return false;
        }
        let name = wide(path.as_os_str());
        // SAFETY: COM for this thread, balanced below when it was initialized here (S_OK or S_FALSE).
        let init = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        let mut pidl = std::ptr::null_mut();
        // SAFETY: NUL-terminated path; `pidl` receives an ID list freed with ILFree.
        let ok = unsafe { SHParseDisplayName(name.as_ptr(), std::ptr::null_mut(), &mut pidl, 0, std::ptr::null_mut()) } >= 0 && !pidl.is_null();
        // SAFETY: `pidl` is valid when ok; with no child items the folder of the item opens with it selected.
        let shown = ok && unsafe { SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0) } >= 0;
        if !pidl.is_null() {
            // SAFETY: allocated by SHParseDisplayName.
            unsafe { ILFree(pidl) };
        }
        if init >= 0 {
            // SAFETY: matches the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
        shown
    }

    /// Notepad from System32 for a `.md`/`.txt` file (the same intent as `open -t` on macOS). One argv entry.
    pub fn open_in_editor(path: &Path) -> bool {
        let text = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("txt"));
        let Some(root) = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).filter(|r| r.is_absolute()) else { return false };
        if !text || !path.is_absolute() || !super::lexical_guard(path) {
            return false;
        }
        std::process::Command::new(root.join("System32").join("notepad.exe"))
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }

    /// Items in the Recycle Bin of the drive holding `path` (`None` = no bin there, or not a local fixed drive).
    pub fn bin_items(path: &Path) -> Option<i64> {
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        use windows_sys::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
        const DRIVE_FIXED: u32 = 3;
        let s = path.to_str()?;
        let b = s.as_bytes();
        if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return None;
        }
        let root = wide(s[..3].as_ref());
        // SAFETY: NUL-terminated root path.
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
            return None;
        }
        // SAFETY: zeroed is valid; cbSize set as the API requires.
        let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
        // SAFETY: valid root and out struct.
        (unsafe { SHQueryRecycleBinW(root.as_ptr(), &mut info) } >= 0).then_some(info.i64NumItems)
    }

    /// Moves one file or folder to the Recycle Bin (review H1). Only on a local fixed drive with a Recycle Bin; the
    /// shell recycles without asking but warns before destroying anything it cannot recycle (`FOF_WANTNUKEWARNING`,
    /// never `FOF_NO_UI`), and afterwards the bin must hold more items than before. A link is moved itself, never its
    /// target. Wildcards are refused (the shell would expand them).
    pub fn trash(path: &Path) -> std::io::Result<()> {
        use windows_sys::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW, SHFileOperationW};
        if !path.is_absolute() || !super::lexical_guard(path) || path.to_str().is_none_or(|s| s.contains(['*', '?'])) {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        let Some(before) = bin_items(path) else { return Err(std::io::Error::other("no Recycle Bin on this drive")) };
        // pFrom is a list: the path, NUL, then a final NUL.
        let mut from = wide(path.as_os_str());
        from.push(0);
        // SAFETY: zeroed is a valid SHFILEOPSTRUCTW (null handles and pointers).
        let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
        op.wFunc = FO_DELETE;
        op.pFrom = from.as_ptr();
        op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING) as u16;
        // SAFETY: `op` and `from` live for the call.
        let rc = unsafe { SHFileOperationW(&mut op) };
        if rc != 0 || op.fAnyOperationsAborted != 0 {
            return Err(std::io::Error::other(format!("SHFileOperationW {rc}")));
        }
        match bin_items(path) {
            Some(after) if after > before => Ok(()),
            _ => Err(std::io::Error::other("not found in the Recycle Bin afterwards")),
        }
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
    fn windows_lexical_rule() {
        for ok in [r"C:\Users\me\a.png", "C:/Users/me/a.png", r"docs\a.png", "a.png", r"C:\a\.\b\..\c.md", r"C:\console\x.md", r"C:\COM10\x.md", r"C:\a\nul-ish.md"] {
            assert!(windows_lexical_ok(ok), "{ok}");
        }
        for bad in [r"\\host\share\a.png", "//host/share/a.png", r"\\?\C:\a.png", r"\\.\COM1", r"C:a.png", r"C:\a.png:secret", r"C:\x\CON", r"C:\x\nul.txt", r"C:\x\com1.md", "C:\\x\\LPT9 .md", r"C:\x\COM¹", r"C:\dir.\a.md", r"C:\dir \a.md", r"C:\a.md.", "C:\\a\u{1}b", ""] {
            assert!(!windows_lexical_ok(bad), "{bad}");
        }
    }

    /// Review H3 [CI]: untrusted relative paths on Windows.
    #[test]
    fn windows_relative_rule() {
        for ok in ["evidence/a.png", r"docs\a.md", "a.md", "con-notes.md"] {
            assert!(windows_relative_ok(ok), "{ok}");
        }
        for bad in ["C:/Users/me/a.md", r"C:\a.md", "C:a.md", "a.md:secret", "CON", "nul", "nul.txt", "docs/COM1.md", r"\\host\share\a.md", r"\??\C:\a", "/abs.md", "a?.md", "a*.md", "dir./a.md", "a.md ", ""] {
            assert!(!windows_relative_ok(bad), "{bad}");
        }
        assert!(!windows_lexical_ok(r"\??\C:\x"), "NT object prefix");
        assert!(relative_ok("CON") == !cfg!(windows));
    }

    #[test]
    fn deny_names_ignore_case_only_on_windows() {
        let n = std::ffi::OsStr::new("appdata");
        assert_eq!(name_is(n, "AppData"), cfg!(windows));
        assert!(name_is(std::ffi::OsStr::new("Library"), "Library"));
    }

    #[test]
    fn random_bytes_come_from_the_os() {
        let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
        assert!(random_bytes(&mut a) && random_bytes(&mut b));
        assert_ne!(a, b);
    }

    /// windows-support 3.1 [CI]: the Recycle Bin takes a junction itself, not what it points to.
    #[cfg(windows)]
    #[test]
    fn trash_moves_a_link_not_its_target() {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path()).unwrap();
        let target = base.join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.md"), "x").unwrap();
        let j = base.join("j");
        assert!(std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(&j).arg(&target).output().unwrap().status.success());
        let before = shell::bin_items(&j).expect("the runner's C: has a Recycle Bin");
        shell::trash(&j).unwrap();
        assert!(shell::bin_items(&j).unwrap() > before, "the junction went to the Recycle Bin");
        assert!(std::fs::symlink_metadata(&j).is_err(), "the junction is gone");
        assert!(target.join("keep.md").is_file(), "its target is untouched");
        let f = base.join("plain.md");
        std::fs::write(&f, "x").unwrap();
        shell::trash(&f).unwrap();
        assert!(!f.exists());
        assert!(shell::trash(Path::new(r"\\host\share\x.md")).is_err(), "UNC refused before the shell");
        assert!(shell::trash(&base.join("*.md")).is_err(), "wildcards refused");
        assert!(shell::trash(&base.join("a?.md")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn only_https_urls_open() {
        assert!(!shell::open_url("http://example.com/"));
        assert!(!shell::open_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!shell::open_url("https://example.com/\" x\""));
        assert!(!shell::open_in_editor(Path::new(r"C:\Windows\System32\calc.exe")), "only .md and .txt");
    }

    #[test]
    fn home_is_never_empty() {
        assert!(home().is_none_or(|h| !h.as_os_str().is_empty()));
    }
}
