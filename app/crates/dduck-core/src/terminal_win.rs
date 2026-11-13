//! Windows terminals and copyable PowerShell commands (windows-support decision 6). Pure text functions, so the
//! quoting rules are tested on every OS; only [`launch`] starts a process, and only on Windows.
//!
//! - Terminal-bound values (folder, CLI path, config folder) must pass [`terminal_value_ok`]: no control characters
//!   and none of ``;`'‘’‚‛$``, and a folder must be a drive-letter absolute path (so `-d <dir>` is never an option).
//! - What runs in the terminal goes in as `-EncodedCommand` (UTF-16LE base64): no command interpreter splits it.
//! - Copyable commands use PowerShell single quotes with all five single-quote characters doubled.

use std::path::{Path, PathBuf};

/// The single-quote characters PowerShell treats as `'` inside a single-quoted string.
const PS_QUOTES: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'];

/// A PowerShell single-quoted literal. `None` for control characters (they break a pasted line).
pub fn ps_quote(s: &str) -> Option<String> {
    if s.chars().any(char::is_control) {
        return None;
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if PS_QUOTES.contains(&c) {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    Some(out)
}

/// The rule for values that reach a terminal command line.
pub fn terminal_value_ok(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_control() || matches!(c, ';' | '`' | '$') || PS_QUOTES.contains(&c))
}

/// A folder for `wt -d` / the PowerShell start folder: `X:\…` only, passing [`terminal_value_ok`].
pub fn terminal_dir_ok(dir: &Path) -> bool {
    let Some(s) = dir.to_str() else { return false };
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && terminal_value_ok(s) && crate::platform::windows_lexical_ok(s)
}

/// What to run, already validated by [`script`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target<'a> {
    /// A PowerShell prompt in the folder, nothing run.
    Folder(&'a Path),
    /// `claude attach <id>`.
    Attach { cli: &'a Path, id: &'a str, config_dir: Option<&'a Path> },
    /// Interactive `claude` in a folder (workspace-trust prompt).
    Trust { dir: &'a Path, cli: &'a Path, config_dir: Option<&'a Path> },
    /// `claude auth login <flag>`; the flag is one of two fixed strings.
    Login { cli: &'a Path, flag: &'static str, config_dir: Option<&'a Path> },
}

/// The PowerShell text for `target` (`None` = nothing to run, or a value failed the terminal rule). The default
/// profile removes `CLAUDE_CONFIG_DIR` so a user value cannot sign in to another profile than the app's.
pub fn script(target: &Target<'_>) -> Option<Option<String>> {
    let env = |cfg: Option<&Path>| -> Option<String> {
        match cfg {
            Some(d) => {
                let s = d.to_str().filter(|s| terminal_value_ok(s))?;
                Some(format!("$env:CLAUDE_CONFIG_DIR = {}; ", ps_quote(s)?))
            }
            None => Some("Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; ".into()),
        }
    };
    let exe = |p: &Path| p.to_str().filter(|s| terminal_value_ok(s)).and_then(ps_quote);
    match target {
        Target::Folder(dir) => terminal_dir_ok(dir).then_some(None),
        Target::Attach { cli, id, config_dir } => {
            let id_ok = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
            id_ok.then_some(())?;
            Some(Some(format!("{}& {} attach {id}", env(*config_dir)?, exe(cli)?)))
        }
        Target::Trust { dir, cli, config_dir } => {
            terminal_dir_ok(dir).then_some(())?;
            Some(Some(format!("{}& {}", env(*config_dir)?, exe(cli)?)))
        }
        Target::Login { cli, flag, config_dir } => Some(Some(format!("{}& {} auth login {flag}", env(*config_dir)?, exe(cli)?))),
    }
}

/// `-EncodedCommand` value: base64 of the UTF-16LE text.
pub fn encoded(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The copyable PowerShell line shown when no terminal could be opened. Nothing runs it, so values only need
/// PowerShell quoting (all five single quotes doubled), not the stricter terminal rule.
pub fn copy_line(target: &Target<'_>) -> Option<String> {
    let q = |p: &Path| p.to_str().and_then(ps_quote);
    let env = |cfg: Option<&Path>| -> Option<String> {
        Some(match cfg {
            Some(d) => format!("$env:CLAUDE_CONFIG_DIR = {}; ", q(d)?),
            None => "Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; ".into(),
        })
    };
    match target {
        Target::Folder(dir) => Some(format!("Set-Location -LiteralPath {}", q(dir)?)),
        Target::Trust { dir, cli, config_dir } => Some(format!("Set-Location -LiteralPath {}; {}& {}", q(dir)?, env(*config_dir)?, q(cli)?)),
        Target::Attach { cli, id, config_dir } => {
            let id_ok = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
            id_ok.then_some(())?;
            Some(format!("{}& {} attach {id}", env(*config_dir)?, q(cli)?))
        }
        Target::Login { cli, flag, config_dir } => Some(format!("{}& {} auth login {flag}", env(*config_dir)?, q(cli)?)),
    }
}

/// Windows PowerShell 5.1 from System32 (always present).
fn powershell() -> Option<PathBuf> {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).filter(|r| r.is_absolute())?;
    Some(root.join("System32").join("WindowsPowerShell").join("v1.0").join("powershell.exe"))
}

/// Windows Terminal's app execution alias, when installed.
fn windows_terminal() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).filter(|l| l.is_absolute())?;
    Some(local.join("Microsoft").join("WindowsApps").join("wt.exe")).filter(|p| p.is_file())
}

/// The folder a target opens in.
fn start_dir<'a>(target: &Target<'a>) -> Option<&'a Path> {
    match target {
        Target::Folder(d) | Target::Trust { dir: d, .. } => Some(d),
        _ => None,
    }
}

/// Opens Windows Terminal (`wt -d <dir> powershell …`) or, without it, a PowerShell window started in the folder.
/// Values go in as argv items; the script only as `-EncodedCommand`. `false` when a value fails the rules or
/// nothing could start. Off Windows always `false`.
pub fn launch(target: &Target<'_>) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let (Some(run), Some(ps)) = (script(target), powershell()) else { return false };
    let mut ps_args: Vec<String> = vec!["-NoLogo".into(), "-NoExit".into()];
    if let Some(text) = &run {
        ps_args.extend(["-EncodedCommand".into(), encoded(text)]);
    }
    let dir = start_dir(target);
    let mut cmd = match windows_terminal() {
        Some(wt) => {
            let mut c = std::process::Command::new(wt);
            if let Some(d) = dir {
                c.arg("-d").arg(d);
            }
            c.arg(&ps).args(&ps_args);
            c
        }
        None => {
            let mut c = std::process::Command::new(&ps);
            c.args(&ps_args);
            if let Some(d) = dir {
                c.current_dir(d);
            }
            c
        }
    };
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().is_ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn all_five_single_quotes_are_doubled() {
        assert_eq!(ps_quote("it's").unwrap(), "'it''s'");
        assert_eq!(ps_quote("a\u{2018}b\u{2019}c\u{201A}d\u{201B}e").unwrap(), "'a\u{2018}\u{2018}b\u{2019}\u{2019}c\u{201A}\u{201A}d\u{201B}\u{201B}e'");
        assert_eq!(ps_quote("$x `y` \"z\" \u{201C}w\u{201D}").unwrap(), "'$x `y` \"z\" \u{201C}w\u{201D}'", "literal inside single quotes");
        assert_eq!(ps_quote("a\nb"), None);
    }

    #[test]
    fn terminal_values_refuse_separators_and_quotes() {
        assert!(terminal_value_ok(r"C:\Users\me\work app"));
        for bad in ["a;b", "a`b", "a$b", "it's", "a\u{2019}b", "a\rb", ""] {
            assert!(!terminal_value_ok(bad), "{bad:?}");
        }
        assert!(terminal_dir_ok(Path::new(r"C:\Users\me\work app")));
        for bad in [r"-d C:\x", r"\\host\share", r"C:relative", r"C:\x;wt", r"C:\a’b", "/Users/me"] {
            assert!(!terminal_dir_ok(Path::new(bad)), "{bad}");
        }
    }

    #[test]
    fn attach_runs_only_validated_parts() {
        let cli = Path::new(r"C:\Users\me\.local\bin\claude.exe");
        let t = Target::Attach { cli, id: "ab12cd34", config_dir: None };
        assert_eq!(script(&t).unwrap().unwrap(), r"Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; & 'C:\Users\me\.local\bin\claude.exe' attach ab12cd34");
        let cfg = Path::new(r"C:\Users\me\claude work");
        let t = Target::Attach { cli, id: "ab12cd34", config_dir: Some(cfg) };
        assert!(script(&t).unwrap().unwrap().starts_with(r"$env:CLAUDE_CONFIG_DIR = 'C:\Users\me\claude work'; & "));
        assert_eq!(script(&Target::Attach { cli, id: "x; rm", config_dir: None }), None);
        assert_eq!(script(&Target::Attach { cli: Path::new(r"C:\a;b\claude.exe"), id: "ab", config_dir: None }), None);
        assert_eq!(script(&Target::Folder(Path::new(r"C:\x;calc"))), None, "folder rule");
        assert_eq!(script(&Target::Folder(Path::new(r"C:\work"))), Some(None), "a folder runs nothing");
    }

    #[test]
    fn encoded_command_is_utf16le_base64() {
        assert_eq!(encoded("a"), "YQA=");
        assert_eq!(encoded("가"), "AKw=");
    }

    #[test]
    fn copy_lines_are_powershell() {
        assert_eq!(copy_line(&Target::Folder(Path::new(r"C:\Users\me\it's"))).unwrap(), r"Set-Location -LiteralPath 'C:\Users\me\it''s'");
        let cli = Path::new(r"C:\c\claude.exe");
        assert_eq!(copy_line(&Target::Login { cli, flag: "--claudeai", config_dir: None }).unwrap(), r"Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; & 'C:\c\claude.exe' auth login --claudeai");
        let odd = Path::new(r"C:\it's\claude.exe");
        assert_eq!(copy_line(&Target::Attach { cli: odd, id: "f3e5ff71", config_dir: None }).unwrap(), r"Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue; & 'C:\it''s\claude.exe' attach f3e5ff71");
        assert_eq!(script(&Target::Attach { cli: odd, id: "f3e5ff71", config_dir: None }), None, "but a terminal never gets it");
    }
}
