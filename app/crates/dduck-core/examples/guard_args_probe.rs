//! Spike 1.1 helper (D-163): prints the exact scope arguments a `local_only` start gets, one per line, so a real CLI
//! run can be measured with them. Usage: `cargo run -p dduck-core --example guard_args_probe -- <app exe> [default branch]`
// Dev-only probe run by hand on synthetic or copied data, never shipped; a panic with a short label is the
// intended failure report here. The crate-wide unwrap/expect deny (D-021) guards ingestion code, not this.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dduck_core::delivery_guard::{Guard, GuardCtx, guard_args, hook_command};
use std::path::Path;

fn main() {
    let exe = std::env::args().nth(1).expect("app executable");
    let default_branch = std::env::args().nth(2);
    let ctx = GuardCtx { guard: Guard::LocalOnly, start_on_default: false, default_branch, hook_cmd: Some(hook_command(Path::new(&exe))), global_config: None };
    for a in guard_args(&ctx).expect("args") {
        println!("{}", a.to_string_lossy());
    }
}
