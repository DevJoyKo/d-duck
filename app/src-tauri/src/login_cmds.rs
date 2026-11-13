//! Claude login state, the feature gate and browser login (claude-login, D-150, D-155).
//!
//! The gate lives here, in the backend, ahead of every claim and native confirmation: the webview only mirrors it.
//! The app never reads credentials. `claude auth login` runs as a child (or in Terminal.app after a native
//! confirmation); its sign-in URL stays in this process and the webview learns `has_link` only.

use std::ffi::OsString;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use dduck_core::auth_status::{self, AuthState, AuthView, Block, Login, Need};
use dduck_core::claude_cli::{ClaudeCli, CliError, LoginMethod};
use dduck_core::login_session::{self as ls, AuthCache, LoginRunner, LoginStatus, Phase, Started};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, Res, chrono_now_ms};

static AUTH: LazyLock<AuthCache> = LazyLock::new(AuthCache::default);
static LOGIN: LazyLock<LoginRunner> = LazyLock::new(LoginRunner::default);
/// Last view sent with `auth-changed`: `expires_soon` depends on the clock, so the view is compared, not the state.
static SENT: Mutex<Option<AuthView>> = Mutex::new(None);

/// Error of a gated command. A gate refusal is the `CliError` JSON (`{"kind":"login_required"}`) for every command,
/// whatever its other errors look like; the rest stay plain messages.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CmdError {
    Cli(CliError),
    Msg(String),
}

/// windows-support decision 7: hook-gated features refuse off macOS before any login gate, claim or CLI run.
pub fn mac_only() -> Result<(), CmdError> {
    if cfg!(target_os = "macos") { Ok(()) } else { Err(CmdError::Cli(CliError::invalid(dduck_core::i18n::ReasonCode::MacOnly))) }
}

impl From<String> for CmdError {
    fn from(m: String) -> Self {
        Self::Msg(m)
    }
}

impl From<&str> for CmdError {
    fn from(m: &str) -> Self {
        Self::Msg(m.to_string())
    }
}

impl From<CliError> for CmdError {
    fn from(e: CliError) -> Self {
        Self::Cli(e)
    }
}

/// One `auth status` run. No CLI → unknown without running anything (the "Claude Code를 찾지 못했어요" flow wins).
/// The stdout buffer (email/org/paths) is dropped right after judging.
fn check_now(app: &AppHandle) -> AuthState {
    check_with(app.state::<AppState>().client().ok().as_ref())
}

/// The check itself, given the resolved CLI (or none).
fn check_with(client: Option<&ClaudeCli>) -> AuthState {
    let Some(client) = client else { return AuthState::unknown() };
    match client.auth_status_raw() {
        Ok((code, out)) => {
            let st = auth_status::judge(code, &out, chrono_now_ms());
            drop(out);
            st
        }
        Err(_) => AuthState::unknown(),
    }
}

fn ensure(app: &AppHandle, max_age: Duration) -> AuthState {
    let state = AUTH.ensure(Instant::now, max_age, || check_now(app)).state;
    let view = state.view(chrono_now_ms());
    let changed = SENT.lock().map(|mut sent| (sent.as_ref() != Some(&view)).then(|| *sent = Some(view.clone())).is_some()).unwrap_or(true);
    if changed {
        let _ = app.emit("auth-changed", view);
    }
    state
}

/// Startup check, off the main thread.
pub fn on_start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        ensure(&app, Duration::ZERO);
    });
}

/// Window focus: at most one check per [`ls::MAX_AGE`]; a fresh cache does not even start a thread.
pub fn on_focus(app: &AppHandle) {
    if AUTH.is_fresh(Instant::now(), ls::MAX_AGE) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        ensure(&app, ls::MAX_AGE);
    });
}

/// App exit: no `claude auth login` group may outlive the app.
pub fn on_exit() {
    LOGIN.cancel();
}

/// A feature hit a login error after passing the gate (state was unknown or stale): re-check once.
pub fn recheck_after_failure(app: &AppHandle) -> AuthState {
    ensure(app, Duration::ZERO)
}

/// After a generic CLI failure: re-check once; true when that shows logged out (the failure becomes the gate error).
pub fn logged_out_after_failure(app: &AppHandle) -> bool {
    recheck_after_failure(app).login == Login::LoggedOut
}

/// [`gate_cli`] off the async runtime (the check may run `auth status` for up to 5 s).
pub async fn gate_async(app: &AppHandle, need: Need) -> Result<(), CliError> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || gate_cli(&app, need)).await.map_err(|_| CliError::Spawn)?
}

/// The gate. A cache older than a minute is re-checked first (bounded by the 5 s `auth status` timeout).
pub fn require(app: &AppHandle, need: Need) -> Result<(), Block> {
    ensure(app, ls::MAX_AGE).check(need)
}

/// [`require`] for commands that return [`CliError`].
pub fn gate_cli(app: &AppHandle, need: Need) -> Result<(), CliError> {
    require(app, need).map_err(CliError::from)
}

/// The limit-resume loop's contract (`auth_status::should_tick`): while logged out `tick` is not called, so `auto`
/// rows stay `auto`. Only a logged-out cache is re-checked (once a minute), so a logged-in app runs no extra CLI.
pub fn should_tick(app: &AppHandle) -> bool {
    let current = AUTH.current();
    if current.login != Login::LoggedOut {
        return true;
    }
    auth_status::should_tick(&ensure(app, ls::MAX_AGE))
}

#[tauri::command]
pub async fn auth_state(app: AppHandle) -> Res<AuthView> {
    tauri::async_runtime::spawn_blocking(move || ensure(&app, ls::MAX_AGE).view(chrono_now_ms())).await.map_err(crate::err)
}

/// [다시 확인].
#[tauri::command]
pub async fn refresh_auth(app: AppHandle) -> Res<AuthView> {
    tauri::async_runtime::spawn_blocking(move || ensure(&app, Duration::ZERO).view(chrono_now_ms())).await.map_err(crate::err)
}

#[tauri::command]
pub fn login_status() -> LoginStatus {
    LOGIN.status()
}

/// [로그인] / "Console 계정으로 로그인". A running login is never doubled: its status comes back and the webview
/// brings the sheet forward.
#[tauri::command]
pub async fn start_login(app: AppHandle, method: LoginMethod) -> Res<LoginStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = app.state::<AppState>().client()?;
        if let Started::New(generation) = LOGIN.start(&client, method, Instant::now()) {
            spawn_poll(app.clone(), generation);
        }
        let status = LOGIN.status();
        let _ = app.emit("login-status", &status);
        Ok(status)
    })
    .await
    .map_err(crate::err)?
}

/// Every 2 s: force a status check and step the job until it is no longer running or a newer job replaced it.
fn spawn_poll(app: AppHandle, generation: u64) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(ls::POLL_EVERY);
            if LOGIN.generation() != generation {
                return;
            }
            // Always a fresh check: a cached "logged out" must never end a login that just succeeded.
            let state = ensure(&app, Duration::ZERO);
            let status = LOGIN.step(generation, Instant::now(), state.login);
            if LOGIN.generation() != generation {
                return;
            }
            if status.phase == Phase::Failed {
                // One more fresh check, so the cache and the gates never keep a stale logged-out value.
                ensure(&app, Duration::ZERO);
            }
            let _ = app.emit("login-status", &status);
            if status.phase != Phase::Running {
                return;
            }
        }
    });
}

/// [취소]: ends the child group, then re-checks the state.
#[tauri::command]
pub async fn cancel_login(app: AppHandle) -> Res<LoginStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let status = LOGIN.cancel();
        ensure(&app, Duration::ZERO);
        let _ = app.emit("login-status", &status);
        Ok(status)
    })
    .await
    .map_err(crate::err)?
}

/// "브라우저에서 직접 열기": only the vetted `Url::as_str()` goes to `/usr/bin/open`, as one argv entry.
#[tauri::command]
pub async fn open_login_link() -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(url) = LOGIN.link() else { return Ok(false) };
        #[cfg(windows)]
        return Ok(dduck_core::platform::shell::open_url(url.as_str()));
        #[cfg(not(windows))]
        let ok = std::process::Command::new("/usr/bin/open")
            .arg(url.as_str())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        #[cfg(not(windows))]
        Ok(ok)
    })
    .await
    .map_err(crate::err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogin {
    pub opened: bool,
    /// Shown for copying when Automation permission is denied.
    pub command: String,
}

/// [터미널에서 로그인]: ends the in-app child first (never two logins), asks natively, then opens Terminal.app. The
/// app does not read the terminal; the state check keeps going through focus and [다시 확인].
#[tauri::command]
pub async fn login_in_terminal(app: AppHandle, method: Option<LoginMethod>) -> Res<Option<TerminalLogin>> {
    tauri::async_runtime::spawn_blocking(move || {
        let method = method.or_else(|| LOGIN.method()).unwrap_or(LoginMethod::ClaudeAi);
        LOGIN.cancel();
        let client = app.state::<AppState>().client()?;
        if !crate::add_items::confirm(&app, &dduck_core::i18n::tr(dduck_core::i18n::Code::DlgLoginTitle), dduck_core::i18n::tr(dduck_core::i18n::Code::DlgLoginBody), &dduck_core::i18n::tr(dduck_core::i18n::Code::BtnOpen), false)? {
            return Ok(None);
        }
        let opened = open_terminal_login(&client, method);
        Ok(Some(TerminalLogin { opened, command: command_line(&client, method) }))
    })
    .await
    .map_err(crate::err)?
}

/// osascript argv for the terminal login. The flag is a fixed string inside the script; the CLI path and the config
/// dir go in only as argv items. The default profile clears `CLAUDE_CONFIG_DIR` (a user shell value would sign in to
/// another profile than the app's).
pub fn terminal_args(cli: &Path, config_dir: Option<&Path>, method: LoginMethod) -> Vec<OsString> {
    let flag = match method {
        LoginMethod::ClaudeAi => "--claudeai",
        LoginMethod::Console => "--console",
    };
    let prefix = match config_dir {
        Some(_) => r#""CLAUDE_CONFIG_DIR=" & (quoted form of item 2 of argv) & " " & "#,
        None => r#""env -u CLAUDE_CONFIG_DIR " & "#,
    };
    let script = format!(r#"tell application "Terminal" to do script {prefix}(quoted form of item 1 of argv) & " auth login {flag}""#);
    let mut args: Vec<OsString> =
        ["-e", "on run argv", "-e", &script, "-e", "tell application \"Terminal\" to activate", "-e", "end run", "--"].iter().map(OsString::from).collect();
    args.push(cli.as_os_str().to_owned());
    if let Some(dir) = config_dir {
        args.push(dir.as_os_str().to_owned());
    }
    args
}

fn login_flag(method: LoginMethod) -> &'static str {
    match method {
        LoginMethod::ClaudeAi => "--claudeai",
        LoginMethod::Console => "--console",
    }
}

fn open_terminal_login(client: &ClaudeCli, method: LoginMethod) -> bool {
    // Windows: PowerShell with the login as `-EncodedCommand` (decision 6).
    if cfg!(windows) {
        let (cli, cfg) = client.login_terminal_parts();
        return dduck_core::terminal_win::launch(&dduck_core::terminal_win::Target::Login { cli, flag: login_flag(method), config_dir: cfg });
    }
    if !cfg!(target_os = "macos") {
        return false;
    }
    let (cli, cfg) = client.login_terminal_parts();
    std::process::Command::new("/usr/bin/osascript")
        .args(terminal_args(cli, cfg, method))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn command_line(client: &ClaudeCli, method: LoginMethod) -> String {
    let (cli, cfg) = client.login_terminal_parts();
    if cfg!(windows) {
        return dduck_core::terminal_win::copy_line(&dduck_core::terminal_win::Target::Login { cli, flag: login_flag(method), config_dir: cfg }).unwrap_or_default();
    }
    let prefix = match cfg {
        Some(dir) => format!("CLAUDE_CONFIG_DIR={} ", quote(&dir.to_string_lossy())),
        None => "env -u CLAUDE_CONFIG_DIR ".to_string(),
    };
    format!("{prefix}{} auth login {}", quote(&cli.to_string_lossy()), method.flag())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(v: &[OsString]) -> Vec<String> {
        v.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn terminal_args_keep_values_out_of_the_script() {
        let cli = Path::new("/Users/a b/.local/bin/claude\"; do shell script \"x");
        let args = strs(&terminal_args(cli, None, LoginMethod::ClaudeAi));
        assert_eq!(args.len(), 10);
        assert!(args[3].contains(r#""env -u CLAUDE_CONFIG_DIR " & (quoted form of item 1 of argv) & " auth login --claudeai""#));
        assert!(!args[3].contains("a b"), "the path is argv only");
        assert_eq!(args[8], "--");
        assert_eq!(args[9], cli.to_string_lossy());

        let cfg = Path::new("/tmp/prof 'x'");
        let args = strs(&terminal_args(Path::new("/c/claude"), Some(cfg), LoginMethod::Console));
        assert_eq!(args.len(), 11);
        assert!(args[3].contains(r#""CLAUDE_CONFIG_DIR=" & (quoted form of item 2 of argv) & " " & (quoted form of item 1 of argv) & " auth login --console""#));
        assert!(!args[3].contains("prof"));
        assert_eq!(&args[9..], ["/c/claude", "/tmp/prof 'x'"]);
    }

    /// Shell-level gate with a fake CLI: logged out, a gated command gets `login_required` and only `auth status`
    /// ever ran; a CLI that answers garbage (unknown) lets the command through.
    #[cfg(unix)]
    #[test]
    fn gate_with_a_fake_cli_runs_nothing_but_auth_status() {
        use std::os::unix::fs::PermissionsExt;
        // No tempfile dev-dependency in the shell crate: a unique folder under the temp dir, removed at the end.
        let dir = std::env::temp_dir().join(format!("dduck-gate-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("argv.log");
        let bin = dir.join("claude");
        std::fs::write(
            &bin,
            format!("#!/bin/sh\necho \"$*\" >> '{}'\nprintf '{{\"loggedIn\":false,\"authMethod\":\"none\",\"email\":\"CANARY\"}}'\nexit 1\n", log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cli = ClaudeCli::new(bin.clone(), None);
        let cache = AuthCache::default();
        let state = cache.ensure(Instant::now, ls::MAX_AGE, || check_with(Some(&cli))).state;
        let refused = state.check(Need::Model).map_err(CliError::from).unwrap_err();
        assert_eq!(serde_json::to_value(CmdError::from(refused)).unwrap(), serde_json::json!({"kind": "login_required"}));
        assert_eq!(serde_json::to_value(CmdError::from("x")).unwrap(), serde_json::json!("x"));
        // Within a minute the gate reuses the cache: still one CLI run, and never `--bg`/`-p`.
        cache.ensure(Instant::now, ls::MAX_AGE, || check_with(Some(&cli)));
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "auth status --json\n");
        assert!(!format!("{:?}", state.view(0)).contains("CANARY"));
        assert_eq!(check_with(None), AuthState::unknown(), "no CLI: nothing runs");
        std::fs::write(&bin, "#!/bin/sh\necho not json\n").unwrap();
        assert_eq!(check_with(Some(&cli)).check(Need::Model), Ok(()), "unknown passes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gate_codes_go_into_routine_records() {
        assert_eq!(crate::cli_error_kind(&CliError::LoginRequired), "login_required");
        assert_eq!(crate::cli_error_kind(&CliError::ClaudeAiRequired), "claude_ai_required");
    }

    #[test]
    fn copy_command_matches_the_profile() {
        let c = ClaudeCli::new("/c/claude".into(), None);
        assert_eq!(command_line(&c, LoginMethod::ClaudeAi), "env -u CLAUDE_CONFIG_DIR '/c/claude' auth login --claudeai");
        let c = ClaudeCli::new("/c/claude".into(), Some("/p/it's".into()));
        assert_eq!(command_line(&c, LoginMethod::Console), r"CLAUDE_CONFIG_DIR='/p/it'\''s' '/c/claude' auth login --console");
    }
}
