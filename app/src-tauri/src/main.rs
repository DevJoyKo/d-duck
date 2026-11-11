// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // PreToolUse hook mode for the cloud-routine fetch (D-066): read the hook JSON, allow only `list`, exit.
    if std::env::args().nth(1).as_deref() == Some(dduck_core::cloud_routines::GUARD_FLAG) {
        use std::io::Read;
        let mut input = String::new();
        let _ = std::io::stdin().take(1024 * 1024).read_to_string(&mut input);
        // With arguments it is the edit guard (D-077): exactly the app's expected request, once; any problem with the
        // arguments denies everything, `list` included (fail closed).
        let rest: Vec<String> = std::env::args().skip(2).collect();
        if rest.is_empty() {
            guard_exit(dduck_core::cloud_routines::guard_allows(&input));
        }
        guard_exit(dduck_core::cloud_routines::guard_allows_expected(&input, &rest));
    }
    // Same for the usage-insights run (D-078): web tools only, allowed hosts, counted in the app-owned dir (arg 2).
    if std::env::args().nth(1).as_deref() == Some(dduck_core::insights::GUARD_FLAG) {
        use std::io::Read;
        let mut input = String::new();
        let _ = std::io::stdin().take(1024 * 1024).read_to_string(&mut input);
        let dir = std::env::args().nth(2).map(std::path::PathBuf::from);
        guard_exit(dir.is_some_and(|d| d.is_absolute() && dduck_core::insights::guard_allows(&input, &d)));
    }
    // The news run (news-tab, D-204): WebFetch only, exact official pages, counted in the app-owned dir (arg 2).
    if std::env::args().nth(1).as_deref() == Some(dduck_core::news::GUARD_FLAG) {
        use std::io::Read;
        let mut input = String::new();
        let _ = std::io::stdin().take(1024 * 1024).read_to_string(&mut input);
        let dir = std::env::args().nth(2).map(std::path::PathBuf::from);
        guard_exit(dir.is_some_and(|d| d.is_absolute() && dduck_core::news::guard_allows(&input, &d)));
    }
    // The brief-refine run (brief-grilling): no tool exists there, and this hook denies any call anyway.
    if std::env::args().nth(1).as_deref() == Some(dduck_core::brief_refine::GUARD_FLAG) {
        use std::io::Read;
        let mut input = String::new();
        let _ = std::io::stdin().take(1024 * 1024).read_to_string(&mut input);
        guard_exit(false);
    }
    // "로컬만" tasks (delivery-guard): PreToolUse hook that denies any remote step. Allow = no output and exit 0, so
    // the normal permission checks still run; anything unreadable is denied (exit 2).
    if std::env::args().nth(1).as_deref() == Some(dduck_core::delivery_guard::LOCAL_ONLY_FLAG) {
        use std::io::{Read, Write};
        let mut input = String::new();
        let read = std::io::stdin().take(1024 * 1024).read_to_string(&mut input).is_ok();
        let cwd = serde_json::from_str::<serde_json::Value>(&input)
            .ok()
            .and_then(|v| v.get("cwd").and_then(|c| c.as_str()).map(std::path::PathBuf::from))
            .filter(|p| p.is_absolute());
        let decision = if read {
            dduck_core::delivery_guard::local_only_hook(&input, &dduck_core::delivery_guard::git_alias_lookup(cwd))
        } else {
            dduck_core::delivery_guard::HookDecision::Deny
        };
        let (out, code) = dduck_core::delivery_guard::hook_output(decision);
        let _ = std::io::stdout().write_all(out.as_bytes());
        let _ = std::io::stdout().flush();
        std::process::exit(code);
    }
    // Report folder only (report-evidence 6.1): `<exe> --dduck-guard-report-dir <dir> <0|1>`. Bad arguments deny.
    if std::env::args().nth(1).as_deref() == Some(dduck_core::report_guard::FLAG) {
        use std::io::{Read, Write};
        let mut input = String::new();
        let read = std::io::stdin().take(1024 * 1024).read_to_string(&mut input).is_ok();
        let dir = std::env::args().nth(2).map(std::path::PathBuf::from).filter(|d| d.is_absolute());
        let strict = std::env::args().nth(3).as_deref() != Some("0");
        let decision = match (read, dir) {
            (true, Some(d)) => dduck_core::report_guard::decide(&input, &d, strict),
            _ => dduck_core::report_guard::Decision::Deny,
        };
        let (out, code) = dduck_core::report_guard::output(decision);
        let _ = std::io::stdout().write_all(out.as_bytes());
        let _ = std::io::stdout().flush();
        std::process::exit(code);
    }
    dduck_lib::run()
}

/// Allow = explicit PreToolUse allow JSON + exit 0; deny = exit 2. Without the JSON nothing runs (no --allowedTools).
fn guard_exit(allowed: bool) -> ! {
    use std::io::Write;
    // Fixed constants only (no transcript data, D-021), written directly rather than with print macros.
    let (out, err, code) = dduck_core::cloud_routines::hook_result(allowed);
    let _ = std::io::stdout().write_all(out.as_bytes()).and_then(|_| std::io::stdout().flush());
    let _ = std::io::stderr().write_all(err.as_bytes());
    std::process::exit(code)
}
