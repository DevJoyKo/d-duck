//! Locates and validates the `claude` executable and the effective Claude config directory (D-018, D-028).
//!
//! GUI apps get no shell PATH or env, so resolution tries: the user's setting → well-known install locations →
//! a one-time login-shell probe. Every candidate must pass [`validate`] before it is used.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

pub const SUPPORTED_MAJOR_MINOR: (u32, u32) = (2, 1);
/// The first CLI that honours `"onFailure": "block"` on hooks (decision 8): hook-gated features need it.
pub const HOOK_BLOCK_MIN: (u64, u64, u64) = (2, 1, 295);

/// `version` (`x.y.z` as in [`CliInfo`]) is at least [`HOOK_BLOCK_MIN`]. An unreadable version is refused.
pub fn supports_hook_block(version: &str) -> bool {
    parse_semver(version).is_some_and(|v| v >= HOOK_BLOCK_MIN)
}
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const CLI_MARK: &str = "__DDUCK_CLI__=";
const CFG_MARK: &str = "__DDUCK_CFG__=";
const PATH_MARK: &str = "__DDUCK_PATH__=";
const MAX_PATH_ENTRIES: usize = 64;
const MAX_PATH_BYTES: usize = 8 * 1024;

static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\.(\d+)\.(\d+) \(Claude Code\)").unwrap_or_else(|_| unreachable!()));

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    /// The path as found (may be a symlink); this is what gets executed.
    pub path: PathBuf,
    pub version: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    NotAbsolute,
    WrongName,
    NotFound,
    NotRegularFile,
    NotExecutable,
    WritableByOthers,
    NotOwned,
    UnsafeCharacters,
    VersionCheckFailed,
    NotClaudeCode,
    /// Windows: an npm `claude.cmd` that does not wrap the native `claude.exe` the way the app can check.
    UnsupportedShim,
}

impl Rejection {
    pub fn message(self) -> &'static str {
        match self {
            Rejection::NotAbsolute => crate::i18n::Fail::CliNotAbsolute.str(),
            Rejection::WrongName => crate::i18n::Fail::CliNotClaude.str(),
            Rejection::NotFound => crate::i18n::Fail::FileNotFound.str(),
            Rejection::NotRegularFile => crate::i18n::Fail::NotRegularFile.str(),
            Rejection::NotExecutable => crate::i18n::Fail::CliNoExec.str(),
            Rejection::WritableByOthers => crate::i18n::Fail::CliWritable.str(),
            Rejection::NotOwned => crate::i18n::Fail::CliOwner.str(),
            Rejection::UnsafeCharacters if cfg!(windows) => crate::i18n::Fail::CliBadCharsWin.str(),
            Rejection::UnsafeCharacters => crate::i18n::Fail::CliBadChars.str(),
            Rejection::UnsupportedShim => crate::i18n::Fail::CliShimUnsupported.str(),
            Rejection::VersionCheckFailed => crate::i18n::Fail::CliVersionFailed.str(),
            Rejection::NotClaudeCode => crate::i18n::Fail::CliNotCode.str(),
        }
    }
}

/// Result of the one-time login-shell probe. Holds only these values; the rest of the shell env is never read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellProbe {
    pub cli: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    /// The login shell's `PATH`, cleaned by [`clean_path`]. GUI launches have a minimal PATH, so MCP servers and
    /// tools in `~/.local/bin`, Homebrew, etc. were not found (D-073).
    pub path: Option<String>,
}

/// Keeps absolute, unique entries without control characters; at most 64 entries and 8 KB. `None` if nothing is left.
/// Unix only (`:`-separated). `/` itself trims to empty and is dropped. Existing folders that other users can write
/// to (other-write bit; group-writable admin folders such as Homebrew's stay) are dropped too: background agents reuse this PATH unattended (security review MEDIUM).
pub fn clean_path(raw: &str) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    let mut len = 0;
    for e in raw.split(':') {
        let e = e.trim_end_matches('/');
        if e.is_empty() || !e.starts_with('/') || e.chars().any(char::is_control) || !seen.insert(e) {
            continue;
        }
        if shared_writable(Path::new(e)) {
            continue;
        }
        if out.len() >= MAX_PATH_ENTRIES || len + e.len() + 1 > MAX_PATH_BYTES {
            break;
        }
        len += e.len() + 1;
        out.push(e);
    }
    (!out.is_empty()).then(|| out.join(":"))
}

/// Runs `$SHELL -ilc` once to learn `command -v claude` and `$CLAUDE_CONFIG_DIR`.
/// The shell must be an absolute path listed in `allowed_shells` (normally `/etc/shells`).
pub fn probe_login_shell(shell: &Path, allowed_shells: &str, timeout: Duration) -> ShellProbe {
    let listed = allowed_shells.lines().map(str::trim).any(|l| !l.starts_with('#') && Path::new(l) == shell);
    if !shell.is_absolute() || !listed {
        return ShellProbe::default();
    }
    let script = format!(r#"printf '\n{CLI_MARK}%s\n{CFG_MARK}%s\n{PATH_MARK}%s\n' "$(command -v claude)" "$CLAUDE_CONFIG_DIR" "$PATH""#);
    let Some(out) = run_with_timeout(Command::new(shell).args(["-ilc", &script]), timeout, 64 * 1024) else {
        return ShellProbe::default();
    };
    parse_probe_output(&out)
}

fn shared_writable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o002 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = p;
        false
    }
}

/// Default probe for this OS: `$SHELL` checked against `/etc/shells` (no-op on Windows).
pub fn probe_default_shell() -> ShellProbe {
    #[cfg(unix)]
    {
        let Some(shell) = std::env::var_os("SHELL").map(PathBuf::from) else { return ShellProbe::default() };
        let allowed = std::fs::read_to_string("/etc/shells").unwrap_or_default();
        probe_login_shell(&shell, &allowed, PROBE_TIMEOUT)
    }
    #[cfg(not(unix))]
    {
        ShellProbe::default()
    }
}

fn parse_probe_output(out: &str) -> ShellProbe {
    let value = |mark: &str| {
        out.lines()
            .rev()
            .find_map(|l| l.strip_prefix(mark))
            .map(str::trim)
            .filter(|v| !v.is_empty() && Path::new(v).is_absolute())
            .map(PathBuf::from)
    };
    let path = out.lines().rev().find_map(|l| l.strip_prefix(PATH_MARK)).and_then(|v| clean_path(v.trim()));
    ShellProbe { cli: value(CLI_MARK), config_dir: value(CFG_MARK), path }
}

/// The Claude config directory D.Duck uses for ingestion and passes to every child `claude` (D-028 ⑤, D-032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDir {
    pub path: PathBuf,
    /// True when chosen in settings or found as `CLAUDE_CONFIG_DIR`. Only then is the variable set on children:
    /// setting it to the default `~/.claude` changes where the CLI looks for `.claude.json` (it would then miss the
    /// user's real `~/.claude.json`, including workspace trust).
    pub explicit: bool,
}

impl ConfigDir {
    /// Value for the child's `CLAUDE_CONFIG_DIR`; `None` means "remove it so the CLI uses its default".
    pub fn env_value(&self) -> Option<&Path> {
        self.explicit.then_some(self.path.as_path())
    }
}

/// Effective config dir: setting → login-shell `CLAUDE_CONFIG_DIR` → `~/.claude` (default, not explicit).
pub fn effective_config_dir(setting: Option<&Path>, probe: &ShellProbe, home: &Path) -> ConfigDir {
    let default = home.join(".claude");
    match setting.filter(|p| p.is_absolute()).map(Path::to_path_buf).or_else(|| probe.config_dir.clone()) {
        // Choosing the default dir explicitly must still behave like the default (D-032).
        Some(path) if same_dir(&path, &default) => ConfigDir { path: default, explicit: false },
        Some(path) => ConfigDir { path, explicit: true },
        None => ConfigDir { path: default, explicit: false },
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (crate::platform::canon(a), crate::platform::canon(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a.components().eq(b.components()),
    }
}

const SYSTEM_BIN_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

/// Resolves the CLI: setting → known locations → login-shell result. Returns the first valid candidate.
pub fn resolve(setting: Option<&Path>, home: &Path, probe: &ShellProbe) -> Option<CliInfo> {
    resolve_in(setting, home, probe, SYSTEM_BIN_DIRS)
}

fn resolve_in(setting: Option<&Path>, home: &Path, probe: &ShellProbe, system_dirs: &[&str]) -> Option<CliInfo> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(setting.map(Path::to_path_buf));
    candidates.extend(known_locations_in(home, system_dirs));
    candidates.extend(probe.cli.clone());
    candidates.into_iter().find_map(|c| validate(&c).ok())
}

/// Well-known install locations, most specific first. For nvm, only the highest installed Node version is used.
pub fn known_locations(home: &Path) -> Vec<PathBuf> {
    known_locations_in(home, SYSTEM_BIN_DIRS)
}

fn known_locations_in(home: &Path, system_dirs: &[&str]) -> Vec<PathBuf> {
    if cfg!(windows) {
        let var = |k: &str| std::env::var_os(k).map(PathBuf::from).filter(|p| p.is_absolute());
        return windows_locations(home, var("APPDATA").as_deref(), var("LOCALAPPDATA").as_deref(), std::env::var_os("PATH").as_deref());
    }
    let exe = exe_name();
    let mut out = vec![home.join(".local/bin").join(exe), home.join(".claude/local").join(exe)];
    out.extend(system_dirs.iter().map(|d| PathBuf::from(d).join(exe)));
    if let Some(dir) = highest_nvm_bin(home) {
        out.push(dir.join(exe));
    }
    out
}

/// Windows order (decision 2): the native installer, the npm shim (resolved before anything runs), Volta, scoop,
/// then each absolute `PATH` entry last.
fn windows_locations(home: &Path, appdata: Option<&Path>, local: Option<&Path>, path: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    let mut out = vec![home.join(".local").join("bin").join("claude.exe")];
    out.extend(appdata.map(|a| a.join("npm").join("claude.cmd")));
    out.extend(local.map(|l| l.join("Volta").join("bin").join("claude.exe")));
    out.push(home.join("scoop").join("shims").join("claude.exe"));
    for dir in path.map(std::env::split_paths).into_iter().flatten().filter(|d| d.is_absolute()).take(MAX_PATH_ENTRIES) {
        out.push(dir.join("claude.exe"));
        out.push(dir.join("claude.cmd"));
    }
    out
}

/// The one shim target the app accepts, relative to the shim's folder (npm's `cmd-shim` for a native binary).
const NPM_SHIM_TARGET: &str = r"node_modules\@anthropic-ai\claude-code\bin\claude.exe";

/// Reads an npm `claude.cmd` (at most 16 KB, never run) and returns the `claude.exe` it wraps: every `%dp0%\…`
/// reference must be that one target, which must exist under the shim's folder after resolving links (junction
/// case). Anything else is refused, so a `.cmd` or `.bat` is never executed.
pub fn resolve_npm_shim(shim: &Path) -> Result<PathBuf, Rejection> {
    let dir = shim.parent().ok_or(Rejection::UnsupportedShim)?;
    let text = std::fs::read(shim).ok().filter(|b| b.len() <= 16 * 1024).ok_or(Rejection::NotFound)?;
    let text = String::from_utf8(text).map_err(|_| Rejection::UnsupportedShim)?;
    let targets: Vec<&str> = text.match_indices("%dp0%\\").map(|(i, m)| &text[i + m.len()..]).map(|rest| rest.split(['"', ' ', '\r', '\n']).next().unwrap_or("")).collect();
    if targets.is_empty() || !targets.iter().all(|t| t.eq_ignore_ascii_case(NPM_SHIM_TARGET)) {
        return Err(Rejection::UnsupportedShim);
    }
    let exe = dir.join(NPM_SHIM_TARGET);
    let (real_dir, real_exe) = (crate::platform::canon(dir).map_err(|_| Rejection::NotFound)?, crate::platform::canon(&exe).map_err(|_| Rejection::NotFound)?);
    if !real_exe.starts_with(&real_dir) {
        return Err(Rejection::UnsupportedShim);
    }
    Ok(exe)
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "claude.exe" } else { "claude" }
}

fn highest_nvm_bin(home: &Path) -> Option<PathBuf> {
    let versions = home.join(".nvm/versions/node");
    let mut best: Option<((u64, u64, u64), PathBuf)> = None;
    for e in std::fs::read_dir(versions).ok()?.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(v) = parse_semver(name.trim_start_matches('v')) else { continue };
        if best.as_ref().is_none_or(|(b, _)| v > *b) {
            best = Some((v, e.path().join("bin")));
        }
    }
    best.map(|(_, p)| p)
}

fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// Checks a candidate: name on the path as given (symlinks allowed — nvm links `claude` → `claude.exe`),
/// everything else on the resolved file, then `--version` must look like Claude Code.
pub fn validate(candidate: &Path) -> Result<CliInfo, Rejection> {
    if !candidate.is_absolute() {
        return Err(Rejection::NotAbsolute);
    }
    if !is_shell_safe_path(candidate) {
        return Err(Rejection::UnsafeCharacters);
    }
    let name = candidate.file_name().and_then(|n| n.to_str()).ok_or(Rejection::WrongName)?;
    // Windows: an npm shim is read, never run; the exe it wraps is what gets checked and executed.
    let resolved;
    let candidate = if cfg!(windows) && name.eq_ignore_ascii_case("claude.cmd") {
        resolved = resolve_npm_shim(candidate)?;
        resolved.as_path()
    } else {
        candidate
    };
    let name = candidate.file_name().and_then(|n| n.to_str()).ok_or(Rejection::WrongName)?;
    let named = if cfg!(windows) { name.eq_ignore_ascii_case("claude.exe") } else { name == "claude" };
    if !named {
        return Err(Rejection::WrongName);
    }
    let real = crate::platform::canon(candidate).map_err(|_| Rejection::NotFound)?;
    let meta = std::fs::metadata(&real).map_err(|_| Rejection::NotFound)?;
    if !meta.is_file() {
        return Err(Rejection::NotRegularFile);
    }
    check_unix_mode(&meta)?;
    // The file's directory, and the link's directory when it is a symlink, must not be writable by others either:
    // otherwise someone else could swap the binary between checks.
    for dir in [real.parent(), candidate.parent()].into_iter().flatten() {
        check_dir_not_shared(dir)?;
    }
    // Windows: the final file and every folder above it, before anything runs, even `--version`.
    #[cfg(windows)]
    if !crate::platform::acl::check_chain(&real) {
        return Err(Rejection::WritableByOthers);
    }
    let out = run_with_timeout(Command::new(candidate).arg("--version"), PROBE_TIMEOUT, 4096)
        .ok_or(Rejection::VersionCheckFailed)?;
    let caps = VERSION_RE.captures(out.trim()).ok_or(Rejection::NotClaudeCode)?;
    let num = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
    let version = format!("{}.{}.{}", num(1), num(2), num(3));
    Ok(CliInfo { path: candidate.to_path_buf(), version, supported: (num(1), num(2)) == SUPPORTED_MAJOR_MINOR })
}

/// Paths are later embedded in a shell command line for "open in terminal"; reject characters whose meaning differs
/// between shells inside single quotes (fish `\\'`, csh `!`) and anything that breaks a line.
/// On Windows the rule is [`is_windows_safe_path`].
pub fn is_shell_safe_path(p: &Path) -> bool {
    if cfg!(windows) {
        return p.to_str().is_some_and(is_windows_safe_path);
    }
    p.to_str().is_some_and(|s| !s.chars().any(|c| c.is_control() || c == '\\' || c == '!'))
}

/// Windows path rule (decision 2): `\` and `X:` are fine; control characters and `"` are not, and no name may end
/// in a dot or a space (Windows drops them, so the checked name and the opened one would differ).
pub fn is_windows_safe_path(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_control() || c == '"') && s.split(['\\', '/']).all(|c| matches!(c, "" | "." | "..") || !(c.ends_with('.') || c.ends_with(' ')))
}

#[cfg(unix)]
fn check_unix_mode(meta: &std::fs::Metadata) -> Result<(), Rejection> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mode = meta.permissions().mode();
    if mode & 0o111 == 0 {
        return Err(Rejection::NotExecutable);
    }
    if mode & 0o022 != 0 {
        return Err(Rejection::WritableByOthers);
    }
    // SAFETY: getuid has no preconditions.
    let me = unsafe { libc::getuid() };
    if meta.uid() != me && meta.uid() != 0 {
        return Err(Rejection::NotOwned);
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_unix_mode(_meta: &std::fs::Metadata) -> Result<(), Rejection> {
    Ok(())
}

#[cfg(unix)]
fn check_dir_not_shared(dir: &Path) -> Result<(), Rejection> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(dir).map_err(|_| Rejection::NotFound)?;
    // Sticky shared dirs like /tmp are still writable by others, so they are rejected as well.
    if meta.permissions().mode() & 0o022 != 0 {
        return Err(Rejection::WritableByOthers);
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_dir_not_shared(_dir: &Path) -> Result<(), Rejection> {
    Ok(())
}

/// Runs a command, returning stdout (truncated to `max_bytes`) if it exits 0 within `timeout`.
fn run_with_timeout(cmd: &mut Command, timeout: Duration, max_bytes: u64) -> Option<String> {
    let done = crate::process::run(cmd, timeout, max_bytes, crate::process::Keep::Head, 4096).ok()?;
    done.status.success().then(|| String::from_utf8_lossy(&done.stdout).into_owned())
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::time::Instant;
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn script(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn fake_cli(path: &Path, version: &str) {
        script(path, &format!("echo '{version} (Claude Code)'"));
    }

    #[test]
    fn accepts_valid_cli_and_flags_unsupported_version() {
        let tmp = tempfile::tempdir().unwrap();
        let ok = tmp.path().join("a/claude");
        fake_cli(&ok, "2.1.319");
        let info = validate(&ok).unwrap();
        assert_eq!((info.version.as_str(), info.supported), ("2.1.319", true));

        let newer = tmp.path().join("b/claude");
        fake_cli(&newer, "3.0.0");
        let info = validate(&newer).unwrap();
        assert_eq!((info.version.as_str(), info.supported), ("3.0.0", false));
    }

    #[test]
    fn nvm_style_symlink_to_claude_exe_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("lib/claude.exe");
        fake_cli(&real, "2.1.319");
        let link = tmp.path().join("bin/claude");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&real, &link).unwrap();
        assert_eq!(validate(&link).unwrap().path, link);
    }

    #[test]
    fn rejects_bad_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let wrong = tmp.path().join("claude2");
        fake_cli(&wrong, "2.1.319");
        assert_eq!(validate(&wrong), Err(Rejection::WrongName));
        assert_eq!(validate(Path::new("claude")), Err(Rejection::NotAbsolute));
        assert_eq!(validate(&tmp.path().join("none/claude")), Err(Rejection::NotFound));

        let shared = tmp.path().join("shared/claude");
        fake_cli(&shared, "2.1.319");
        std::fs::set_permissions(shared.parent().unwrap(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(validate(&shared), Err(Rejection::WritableByOthers));

        let bang = tmp.path().join("we!rd/claude");
        fake_cli(&bang, "2.1.319");
        assert_eq!(validate(&bang), Err(Rejection::UnsafeCharacters));

        let impostor = tmp.path().join("x/claude");
        script(&impostor, "echo 'hello 1.0'");
        assert_eq!(validate(&impostor), Err(Rejection::NotClaudeCode));

        let writable = tmp.path().join("w/claude");
        fake_cli(&writable, "2.1.319");
        std::fs::set_permissions(&writable, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(validate(&writable), Err(Rejection::WritableByOthers));

        let noexec = tmp.path().join("n/claude");
        fake_cli(&noexec, "2.1.319");
        std::fs::set_permissions(&noexec, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(validate(&noexec), Err(Rejection::NotExecutable));
    }

    #[test]
    fn nvm_highest_version_wins_in_empty_env() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        for v in ["v18.20.0", "v22.14.0", "v9.11.2"] {
            fake_cli(&home.join(format!(".nvm/versions/node/{v}/bin/claude")), "2.1.319");
        }
        let locs = known_locations_in(home, &[]);
        assert_eq!(locs.last().unwrap(), &home.join(".nvm/versions/node/v22.14.0/bin/claude"));
        // Resolution needs no PATH/env at all (GUI launch). System dirs are excluded so the machine's own install
        // cannot affect the test.
        let info = resolve_in(None, home, &ShellProbe::default(), &[]).unwrap();
        assert!(info.path.starts_with(home.join(".nvm")));
    }

    #[test]
    fn setting_takes_priority_and_probe_is_last_resort() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let set = tmp.path().join("set/claude");
        fake_cli(&set, "2.1.300");
        let probed = tmp.path().join("probe/claude");
        fake_cli(&probed, "2.1.301");
        let probe = ShellProbe { cli: Some(probed.clone()), ..Default::default() };
        assert_eq!(resolve_in(Some(&set), &home, &probe, &[]).unwrap().path, set);
        assert_eq!(resolve_in(None, &home, &probe, &[]).unwrap().path, probed);
    }

    #[test]
    fn shell_probe_parses_markers_only_and_respects_allowlist() {
        let tmp = tempfile::tempdir().unwrap();
        let shell = tmp.path().join("fakesh");
        // Simulates an rc banner around the real output; ignores its arguments.
        script(&shell, &format!(
            "echo 'Welcome! nvm loaded'\necho '{CLI_MARK}/opt/x/claude'\necho '{CFG_MARK}/Users/me/.claude-work'\necho '{PATH_MARK}/opt/homebrew/bin:relative:/usr/bin:/opt/homebrew/bin/:'\necho 'bye'"
        ));
        let allowed = format!("# shells\n{}\n", shell.display());
        let p = probe_login_shell(&shell, &allowed, Duration::from_secs(3));
        assert_eq!(p.cli, Some(PathBuf::from("/opt/x/claude")));
        assert_eq!(p.config_dir, Some(PathBuf::from("/Users/me/.claude-work")));
        assert_eq!(p.path.as_deref(), Some("/opt/homebrew/bin:/usr/bin"), "relative, empty and duplicate entries dropped");
        assert_eq!(probe_login_shell(&shell, "/bin/zsh\n", Duration::from_secs(3)), ShellProbe::default());
    }

    #[test]
    fn clean_path_limits() {
        assert_eq!(clean_path(""), None);
        assert_eq!(clean_path(".:bin:~/x"), None);
        assert_eq!(clean_path("/a:/a/:/b\u{7}c:/d"), Some("/a:/d".into()));
        let many = (0..200).map(|i| format!("/p{i}")).collect::<Vec<_>>().join(":");
        assert_eq!(clean_path(&many).unwrap().split(':').count(), MAX_PATH_ENTRIES);
        let long = (0..3).map(|i| format!("/{}{i}", "x".repeat(3000))).collect::<Vec<_>>().join(":");
        assert_eq!(clean_path(&long).unwrap().split(':').count(), 2, "8 KB cap");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let tmp = tempfile::tempdir().unwrap();
            let open = tmp.path().join("open");
            std::fs::create_dir(&open).unwrap();
            std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
            let raw = format!("{}:/usr/bin", open.display());
            assert_eq!(clean_path(&raw).as_deref(), Some("/usr/bin"), "world-writable folder dropped");
        }
    }

    #[test]
    fn shell_probe_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let shell = tmp.path().join("slowsh");
        script(&shell, "sleep 10");
        let started = Instant::now();
        let p = probe_login_shell(&shell, &shell.display().to_string(), Duration::from_millis(300));
        assert_eq!(p, ShellProbe::default());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn effective_config_dir_priority() {
        let home = Path::new("/home/me");
        let probe = ShellProbe { config_dir: Some(PathBuf::from("/env/claude")), ..Default::default() };
        let set = effective_config_dir(Some(Path::new("/set/claude")), &probe, home);
        assert_eq!((set.path.as_path(), set.env_value()), (Path::new("/set/claude"), Some(Path::new("/set/claude"))));
        let env = effective_config_dir(None, &probe, home);
        assert_eq!(env.env_value(), Some(Path::new("/env/claude")));
        let default = effective_config_dir(None, &ShellProbe::default(), home);
        assert_eq!(default.path, PathBuf::from("/home/me/.claude"));
        assert_eq!(default.env_value(), None, "default dir must not be passed explicitly");
        let chosen_default = effective_config_dir(Some(Path::new("/home/me/.claude")), &probe, home);
        assert_eq!(chosen_default.env_value(), None, "explicitly choosing ~/.claude still behaves as the default");
    }
}

/// Rules that hold on every OS, plus the Windows CI scenarios (windows-support 2.1).
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod portable_tests {
    use super::*;

    #[test]
    fn hook_block_needs_2_1_295() {
        assert!(supports_hook_block("2.1.295") && supports_hook_block("2.1.330") && supports_hook_block("2.2.0") && supports_hook_block("3.0.0"));
        assert!(!supports_hook_block("2.1.294") && !supports_hook_block("1.9.999") && !supports_hook_block("") && !supports_hook_block("2.1"));
    }

    #[test]
    fn windows_path_rule() {
        for ok in [r"C:\Users\me\.local\bin\claude.exe", r"\\server\share\claude.exe", r"C:\Program Files\nodejs\claude.cmd", r"C:\a\.\b\..\claude.exe"] {
            assert!(is_windows_safe_path(ok), "{ok}");
        }
        for bad in ["", "C:\\a\u{7}\\claude.exe", r#"C:\a"b\claude.exe"#, r"C:\dir.\claude.exe", r"C:\dir \claude.exe", r"C:\a\claude.exe.", "C:\\a\nb\\claude.exe"] {
            assert!(!is_windows_safe_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn windows_candidates_put_the_native_installer_first_and_path_last() {
        let t = tempfile::tempdir().unwrap();
        let (home, appdata, local, p1, p2) = (t.path().join("home"), t.path().join("roaming"), t.path().join("local"), t.path().join("p1"), t.path().join("p2"));
        let path = std::env::join_paths([p1.clone(), PathBuf::from("relative"), p2.clone()]).unwrap();
        let got = windows_locations(&home, Some(&appdata), Some(&local), Some(&path));
        assert_eq!(got[0], home.join(".local").join("bin").join("claude.exe"));
        assert_eq!(got[1], appdata.join("npm").join("claude.cmd"));
        assert_eq!(got[2], local.join("Volta").join("bin").join("claude.exe"));
        assert_eq!(got[3], home.join("scoop").join("shims").join("claude.exe"));
        assert_eq!(&got[4..], [p1.join("claude.exe"), p1.join("claude.cmd"), p2.join("claude.exe"), p2.join("claude.cmd")], "relative PATH entries dropped");
    }

    fn npm_dir(t: &Path) -> (PathBuf, PathBuf) {
        let dir = t.join("npm");
        let exe = dir.join("node_modules").join("@anthropic-ai").join("claude-code").join("bin").join("claude.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "x").unwrap();
        (dir, exe)
    }

    /// The text npm's cmd-shim writes for a native binary.
    const SHIM: &str = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\bin\\claude.exe\"   %*\r\n";

    #[test]
    fn npm_shim_resolves_only_to_the_wrapped_exe() {
        let t = tempfile::tempdir().unwrap();
        let (dir, _) = npm_dir(t.path());
        let shim = dir.join("claude.cmd");
        std::fs::write(&shim, SHIM).unwrap();
        if cfg!(windows) {
            assert_eq!(resolve_npm_shim(&shim).unwrap(), dir.join(NPM_SHIM_TARGET));
        }
        for bad in [SHIM.replace("claude-code\\bin\\claude.exe", "claude-code\\cli.js"), SHIM.replace("%dp0%\\", "C:\\elsewhere\\"), format!("{SHIM}\"%dp0%\\node.exe\" x\r\n"), "@echo off\r\n".into()] {
            std::fs::write(&shim, &bad).unwrap();
            assert_eq!(resolve_npm_shim(&shim), Err(Rejection::UnsupportedShim), "{bad}");
        }
        std::fs::write(&shim, "x".repeat(17 * 1024)).unwrap();
        assert!(resolve_npm_shim(&shim).is_err(), "oversized shim");
    }

    #[cfg(windows)]
    mod windows_ci {
        use super::super::*;

        fn installed(at: &Path) -> PathBuf {
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::copy(crate::testkit::fake_claude().expect("fake_claude example"), at).unwrap();
            at.to_path_buf()
        }

        /// Spec "Native installer" [CI].
        #[test]
        fn native_installer_location_is_found() {
            let t = tempfile::tempdir().unwrap();
            let home = crate::platform::canon(t.path()).unwrap();
            let exe = installed(&home.join(".local").join("bin").join("claude.exe"));
            let info = resolve_in(None, &home, &ShellProbe::default(), &[]).unwrap();
            assert_eq!((info.path, info.version.as_str(), info.supported), (exe, "2.1.330", true));
        }

        /// Spec "npm shim" [CI]: the wrapped exe is checked and shown; the .cmd is only read.
        #[test]
        fn npm_shim_is_resolved_to_its_exe() {
            let t = tempfile::tempdir().unwrap();
            let dir = crate::platform::canon(t.path()).unwrap().join("npm");
            let exe = installed(&dir.join(NPM_SHIM_TARGET));
            std::fs::write(dir.join("claude.cmd"), super::SHIM).unwrap();
            assert_eq!(validate(&dir.join("claude.cmd")).unwrap().path, exe);
            assert_eq!(validate(&dir.join("claude.bat")), Err(Rejection::WrongName));
        }

        /// Spec "Shared-writable folder" [CI]: refused by the ACL check before `--version` runs.
        #[test]
        fn folder_writable_by_authenticated_users_is_refused() {
            let t = tempfile::tempdir().unwrap();
            let dir = crate::platform::canon(t.path()).unwrap().join("shared");
            let exe = installed(&dir.join("claude.exe"));
            assert!(validate(&exe).is_ok(), "a private folder passes");
            let granted = Command::new("icacls").arg(&dir).args(["/grant", "*S-1-5-11:(OI)(CI)M"]).output().unwrap().status.success();
            assert!(granted, "icacls failed");
            assert_eq!(validate(&exe), Err(Rejection::WritableByOthers));
        }
    }
}
