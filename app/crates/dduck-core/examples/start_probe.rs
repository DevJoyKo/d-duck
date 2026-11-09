//! D-164 reproduction: a `local_only` start in a real folder through `ClaudeCli::start_background`, as start_task does.
//! Usage: start_probe <cwd> <app exe> <mode: plan|auto>
// Dev-only probe run by hand on synthetic or copied data, never shipped; a panic with a short label is the
// intended failure report here. The crate-wide unwrap/expect deny (D-021) guards ingestion code, not this.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dduck_core::claude_cli::{ClaudeCli, DangerApproval, PermissionMode, TaskSpec};
use dduck_core::delivery_guard as dg;
use std::path::{Path, PathBuf};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cwd = PathBuf::from(&a[1]);
    let exe = PathBuf::from(&a[2]);
    let mode = if a[3] == "auto" { PermissionMode::Auto } else { PermissionMode::Plan };
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let items = vec![]; // guard is forced below
    let mut start = dg::start_ctx_for_items(&items, &cwd, Some(&home), false, Some(&exe), None).expect("ctx");
    start.ctx.guard = dg::Guard::LocalOnly;
    start.ctx.hook_cmd = Some(dg::hook_command(&exe));
    let mut spec = TaskSpec::basic(cwd.clone(), "Reply with OK only.".into(), Some("dduck-probe".into()), Some(mode));
    spec.guard_ctx = Some(start.ctx.clone());
    spec.prepare_with(&home, 0, None, &dduck_core::loadout::Env::default()).expect("prepare");
    let cli = ClaudeCli::new(PathBuf::from(std::env::var("CLAUDE_BIN").unwrap_or_else(|_| format!("{}/.local/bin/claude", home.display()))), None);
    let t = std::time::Instant::now();
    let approval = matches!(mode, PermissionMode::Auto).then(DangerApproval::after_native_confirmation);
    let r = cli.start_background(&spec, approval);
    println!("{:?} after {:?}", r, t.elapsed());
    let _ = Path::new("");
}
