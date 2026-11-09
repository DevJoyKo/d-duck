//! The `--dduck-guard-local-only` hook decision without the app shell, for the security probes (same calls as
//! src-tauri/src/main.rs): reads PreToolUse JSON on stdin, prints the decision, exits 2 on deny.
use std::io::{Read, Write};

fn main() {
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
    std::process::exit(code);
}
