//! D-164 one-off: backfill task records for starts that ran without one (unnamed `--bg` output read as a failure).
//! Usage: backfill_records <db> <bg_id>|<name>|<cwd>|<created_ms>|<report_id>|<mode> ...
// Dev-only probe run by hand on synthetic or copied data, never shipped; a panic with a short label is the
// intended failure report here. The crate-wide unwrap/expect deny (D-021) guards ingestion code, not this.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dduck_core::{db, task_meta};
fn main() {
    let mut args = std::env::args().skip(1);
    let conn = db::open(std::path::Path::new(&args.next().expect("db"))).expect("open");
    for a in args {
        let f: Vec<&str> = a.split('|').collect();
        let rec = task_meta::NewRecord { bg_id: f[0], name: Some(f[1]), cwd: f[2], agent: None, task_type: None, prompt: None, brief: None, effort: None, worktree: false, permission_mode: Some(f[5]), report_id: Some(f[4]), origin: None, loadout_dropped: 0, now_ms: f[3].parse().unwrap() };
        let r = task_meta::record_start(&conn, &rec).and_then(|()| task_meta::record_guard(&conn, f[0], dduck_core::delivery_guard::Guard::LocalOnly, None, None));
        println!("{} {:?}", f[0], r);
    }
}
