//! Ingests a Claude config directory into a D.Duck database and prints totals (numbers only).
//! Usage: cargo run -p dduck-core --example ingest_dir -- <claude-config-dir> <db-path>

use dduck_core::{db, discover, ingest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(config), Some(db_path)) = (args.next(), args.next()) else {
        eprintln!("usage: ingest_dir <claude-config-dir> <db-path>");
        std::process::exit(2);
    };
    let root = discover::projects_root(config.as_ref()).ok_or("no projects/ directory")?;
    let files = discover::discover(&root);
    let mut conn = db::open(db_path.as_ref())?;
    let started = std::time::Instant::now();
    let stats = ingest::Ingestor::new(&mut conn, Default::default()).ingest_all(&files)?;
    println!("files={} lines={} skipped={} events={} elapsed_ms={}", files.len(), stats.lines, stats.skipped, stats.events, started.elapsed().as_millis());
    let (n, i, o, cr, cw): (i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), IFNULL(SUM(input),0), IFNULL(SUM(output),0), IFNULL(SUM(cache_read),0), IFNULL(SUM(cache_write_5m+cache_write_1h),0) FROM api_calls",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    println!("responses={n} input={i} output={o} cache_read={cr} cache_write={cw}");
    let sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
    let cost: f64 = conn.query_row("SELECT IFNULL(SUM(total_cost_usd),0) FROM cost_segments", [], |r| r.get(0))?;
    println!("sessions={sessions} reported_cost_usd={cost:.2}");
    Ok(())
}
