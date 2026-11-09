//! D-164 reproduction: run the start record on a copy of the app DB and print each step's result.
// Dev-only probe run by hand on synthetic or copied data, never shipped; a panic with a short label is the
// intended failure report here. The crate-wide unwrap/expect deny (D-021) guards ingestion code, not this.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dduck_core::{db, task_meta};
fn main() {
    let path = std::env::args().nth(1).expect("db copy");
    let conn = db::open(std::path::Path::new(&path)).expect("open");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let rec = task_meta::NewRecord { bg_id: "c14db183", name: Some("weekly priority tasks"), cwd: "/Users/me/Agents/side-project", agent: None, task_type: None, prompt: Some("p"), brief: None, effort: None, worktree: false, permission_mode: Some("auto"), report_id: Some("a948675b6a96388d4121452dc83186c4"), origin: None, loadout_dropped: 0, now_ms: now };
    println!("record_start: {:?}", task_meta::record_start(&conn, &rec));
    println!("record_guard: {:?}", task_meta::record_guard(&conn, "c14db183", dduck_core::delivery_guard::Guard::LocalOnly, None, None));
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM task_meta", [], |r| r.get(0)).unwrap();
    println!("rows after: {n}");
}
