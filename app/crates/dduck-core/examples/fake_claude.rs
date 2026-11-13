//! Test stand-in for `claude.exe` on Windows CI (windows-support 5.2). Never ships; `cargo test` builds it next to
//! the test binaries (`target/<profile>/examples/`), where `tests` find it.
//!
//! - `--version`: prints a Claude Code version line.
//! - `--bg …`: writes its arguments to `argv.log` beside itself and prints a background-start line.
//! - `sleep <secs>`: sleeps.
//! - `spawn <pidfile> <secs> [exit]`: starts `sleep <secs>` as a child, writes the child's pid to `<pidfile>`, then
//!   sleeps as long itself, or exits at once with `exit` (a parent that leaves a background child behind).

use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => println!("2.1.330 (Claude Code)"),
        // A background start: every argument, one per line, to `argv.log` next to this executable.
        Some("--bg") => {
            let Some(log) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("argv.log"))) else { std::process::exit(5) };
            if std::fs::write(&log, args.join("\n")).is_err() {
                std::process::exit(6);
            }
            println!("backgrounded · ab12cd34 · job");
        }
        Some("sleep") => std::thread::sleep(Duration::from_secs(secs(args.get(1)))),
        Some("spawn") => {
            let (Some(pidfile), Ok(me)) = (args.get(1), std::env::current_exe()) else { std::process::exit(2) };
            let wait = secs(args.get(2));
            let Ok(child) = std::process::Command::new(me).arg("sleep").arg(wait.to_string()).spawn() else { std::process::exit(3) };
            if std::fs::write(pidfile, child.id().to_string()).is_err() {
                std::process::exit(4);
            }
            if args.get(3).map(String::as_str) != Some("exit") {
                std::thread::sleep(Duration::from_secs(wait));
            }
        }
        _ => std::process::exit(1),
    }
}

fn secs(a: Option<&String>) -> u64 {
    a.and_then(|s| s.parse().ok()).unwrap_or(30)
}
