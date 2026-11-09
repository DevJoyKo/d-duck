//! Ingests `tests/fixtures/basic` and checks every expectation listed in `tests/fixtures/README.md`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use dduck_core::{db, discover, ingest};
use rusqlite::Connection;

const S: &str = "11111111-1111-4111-8111-111111111111";

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &to) } else { fs::copy(e.path(), &to).unwrap(); }
    }
}

struct Env {
    _tmp: tempfile::TempDir,
    config: PathBuf,
    db_path: PathBuf,
}

fn setup() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude");
    copy_dir(&dduck_core::testkit::ensure_basic(), &config);
    let db_path = tmp.path().join("appdata/dduck.db");
    Env { _tmp: tmp, config, db_path }
}

fn run(env: &Env, conn: &mut Connection, opts: ingest::IngestOptions) -> Vec<ingest::SkipReport> {
    let root = discover::projects_root(&env.config).unwrap();
    let files = discover::discover(&root);
    assert_eq!(files.len(), 2, "main + subagent transcript");
    let mut skips = Vec::new();
    let mut ing = ingest::Ingestor::new(conn, opts);
    for f in &files {
        ing.ingest_file(f, |s| skips.push(s)).unwrap();
    }
    skips
}

fn one<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str) -> T {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn token_totals(conn: &Connection) -> (i64, i64, i64, i64, i64, i64, i64) {
    conn.query_row(
        "SELECT COUNT(*), SUM(input), SUM(output), SUM(cache_write_5m), SUM(cache_write_1h), SUM(cache_read), SUM(thinking)
         FROM api_calls",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    )
    .unwrap()
}

#[test]
fn fixture_expectations() {
    let env = setup();
    let mut conn = db::open(&env.db_path).unwrap();
    let skips = run(&env, &mut conn, Default::default());

    // Responses deduplicated, max-merged; synthetic, API error and the unterminated last line excluded.
    assert_eq!(token_totals(&conn), (3, 16, 175, 30, 100, 3000, 27));
    let ids: Vec<String> = conn
        .prepare("SELECT message_id FROM api_calls ORDER BY message_id").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(ids, ["msg_A", "msg_B", "msg_S"]);

    // Tool calls.
    let tools: Vec<(String, Option<String>, Option<String>, Option<String>)> = conn
        .prepare("SELECT tool_name, mcp_server, skill_name, subagent_type FROM tool_calls ORDER BY tool_use_id").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(
        tools,
        vec![
            ("Bash".into(), None, None, None),
            ("Skill".into(), None, Some("openspec-apply-change".into()), None),
            ("Agent".into(), None, None, Some("Explore".into())),
            ("mcp__notion__search".into(), Some("notion".into()), None, None),
        ]
    );

    // Commands: only typed slash commands and the Skill tool; builtin separated from skills.
    let cmds: Vec<(String, String, String)> = conn
        .prepare("SELECT name, kind, via FROM command_invocations ORDER BY name").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(
        cmds,
        vec![
            ("openspec-apply-change".into(), "skill".into(), "tool".into()),
            ("opsx:propose".into(), "skill".into(), "slash".into()),
            ("upgrade".into(), "builtin".into(), "slash".into()),
        ]
    );

    // Reported cost = sum of the last value of each process segment.
    let cost: f64 = one(&conn, &format!("SELECT SUM(total_cost_usd) FROM cost_segments WHERE session_id = '{S}'"));
    assert!((cost - 5.0).abs() < 1e-9, "cost {cost}");

    // Session metadata.
    let (title, path, name, branch): (Option<String>, String, String, String) = conn
        .query_row("SELECT title, project_path, project_name, git_branch FROM sessions WHERE session_id = ?1", [S], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .unwrap();
    assert_eq!(title.as_deref(), Some("TITLEMARK_alpha"));
    assert_eq!(path, "/tmp/PATHMARK_proj");
    assert_eq!(name, "PATHMARK_proj");
    assert_eq!(branch, "BRANCHMARK_main");
    assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM sessions"), 1);

    // Subagent linked to the main session; only agentType read from meta.
    let sub: (String, String) =
        conn.query_row("SELECT session_id, agent_type FROM subagents WHERE agent_id = 'a1b2c3'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        }).unwrap();
    assert_eq!(sub, (S.to_string(), "Explore".to_string()));

    // Exactly one skipped line, reported by category and position only.
    assert_eq!(skips.len(), 1);
    assert_eq!(skips[0].category, "invalid_json");
    assert_eq!(one::<i64>(&conn, "SELECT SUM(skipped_lines) FROM ingest_files"), 1);
}

#[test]
fn incremental_partial_line_and_idempotency() {
    let env = setup();
    let mut conn = db::open(&env.db_path).unwrap();
    run(&env, &mut conn, Default::default());
    let before = token_totals(&conn);

    // Re-running without changes adds nothing.
    run(&env, &mut conn, Default::default());
    assert_eq!(token_totals(&conn), before);

    // Completing the partial last line makes msg_C count.
    let main = discover::projects_root(&env.config).unwrap().join("-tmp-PATHMARK-proj").join(format!("{S}.jsonl"));
    fs::OpenOptions::new().append(true).open(&main).unwrap().write_all(b"\n").unwrap();
    run(&env, &mut conn, Default::default());
    let after = token_totals(&conn);
    assert_eq!((after.0, after.1, after.2), (4, before.1 + 1, before.2 + 3));

    // Full re-ingest from scratch (fresh DB) gives the same totals: upserts are idempotent.
    let mut fresh = db::open_in_memory().unwrap();
    run(&env, &mut fresh, Default::default());
    assert_eq!(token_totals(&fresh), after);
}

#[test]
fn shrunk_file_is_reprocessed() {
    let env = setup();
    let mut conn = db::open(&env.db_path).unwrap();
    run(&env, &mut conn, Default::default());
    let main = discover::projects_root(&env.config).unwrap().join("-tmp-PATHMARK-proj").join(format!("{S}.jsonl"));
    let original = fs::read(&main).unwrap();
    let first_line_end = original.iter().position(|&b| b == b'\n').unwrap() + 1;
    fs::write(&main, &original[..first_line_end]).unwrap();
    let root = discover::projects_root(&env.config).unwrap();
    let file = discover::classify(&root, &main).unwrap();
    let stats = ingest::Ingestor::new(&mut conn, Default::default()).ingest_file(&file, |_| {}).unwrap();
    assert!(stats.restarted);
    assert_eq!(stats.lines, 1);
    assert_eq!(one::<i64>(&conn, &format!("SELECT skipped_lines FROM ingest_files WHERE path = '-tmp-PATHMARK-proj/{S}.jsonl'")), 0);
}

#[test]
fn oversized_line_is_skipped_and_following_lines_parse() {
    let env = setup();
    let main = discover::projects_root(&env.config).unwrap().join("-tmp-PATHMARK-proj").join(format!("{S}.jsonl"));
    let mut data = fs::read(&main).unwrap();
    data.push(b'\n'); // complete msg_C first
    data.extend(std::iter::repeat_n(b'x', 17 * 1024 * 1024));
    data.push(b'\n');
    data.extend_from_slice(
        br#"{"type":"assistant","timestamp":"2026-11-01T11:00:00Z","requestId":"req_D","message":{"model":"claude-opus-5-5","id":"msg_D","content":[],"usage":{"input_tokens":2,"output_tokens":4}}}"#,
    );
    data.push(b'\n');
    fs::write(&main, data).unwrap();

    let mut conn = db::open(&env.db_path).unwrap();
    let skips = run(&env, &mut conn, Default::default());
    let cats: Vec<&str> = skips.iter().map(|s| s.category).collect();
    assert_eq!(cats, ["invalid_json", "line_too_long"], "exactly one extra skip for the oversized line");
    assert_eq!(one::<i64>(&conn, "SELECT SUM(skipped_lines) FROM ingest_files"), 2);
    assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM api_calls WHERE message_id = 'msg_D'"), 1);
}

#[test]
fn turning_titles_off_scrubs_them_from_disk_and_keeps_them_out() {
    let env = setup();
    let main = discover::projects_root(&env.config).unwrap().join("-tmp-PATHMARK-proj").join(format!("{S}.jsonl"));
    {
        let mut conn = db::open(&env.db_path).unwrap();
        run(&env, &mut conn, Default::default());
        ingest::set_store_titles(&conn, false).unwrap();
        assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM sessions WHERE title IS NOT NULL"), 0);
        // A new title arriving later is not stored either (setting read inside the ingest transaction).
        let mut f = fs::OpenOptions::new().append(true).open(&main).unwrap();
        f.write_all(format!("\n{{\"type\":\"ai-title\",\"aiTitle\":\"TITLEMARK_beta\",\"sessionId\":\"{S}\"}}\n").as_bytes()).unwrap();
        run(&env, &mut conn, Default::default());
        assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM sessions WHERE title IS NOT NULL"), 0);
    }
    for e in fs::read_dir(env.db_path.parent().unwrap()).unwrap() {
        let p = e.unwrap().path();
        let bytes = fs::read(&p).unwrap();
        assert!(!bytes.windows(9).any(|w| w == b"TITLEMARK"), "title bytes left in {}", p.display());
    }
}

#[test]
fn zero_byte_transcript_is_recorded() {
    let env = setup();
    let empty = discover::projects_root(&env.config).unwrap().join("-tmp-PATHMARK-proj").join("22222222-2222-4222-8222-222222222222.jsonl");
    fs::write(&empty, b"").unwrap();
    let root = discover::projects_root(&env.config).unwrap();
    let mut conn = db::open(&env.db_path).unwrap();
    let mut ing = ingest::Ingestor::new(&mut conn, Default::default());
    for f in discover::discover(&root) {
        ing.ingest_file(&f, |_| {}).unwrap();
    }
    assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM ingest_files"), 3);
}

#[test]
fn title_storage_can_be_disabled() {
    let env = setup();
    let mut conn = db::open(&env.db_path).unwrap();
    run(&env, &mut conn, Default::default());
    assert_eq!(ingest::clear_titles(&conn).unwrap(), 1);
    // Re-ingest from scratch with titles off: nothing comes back.
    let mut fresh = db::open_in_memory().unwrap();
    run(&env, &mut fresh, ingest::IngestOptions { store_titles: false });
    assert_eq!(one::<i64>(&fresh, "SELECT COUNT(*) FROM sessions WHERE title IS NOT NULL"), 0);
}

/// D-021 canary check: no forbidden free text may reach any byte of the database files or skip reports;
/// allowed metadata markers may appear only in their own columns.
#[test]
fn no_canary_reaches_storage() {
    let env = setup();
    let skips;
    {
        let mut conn = db::open(&env.db_path).unwrap();
        skips = run(&env, &mut conn, Default::default());
        // Keep WAL content on disk too: inspect before and after checkpoint.
        let dir = env.db_path.parent().unwrap();
        assert_no_canary_in_dir(dir);
        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();
    }
    assert_no_canary_in_dir(env.db_path.parent().unwrap());
    assert!(!format!("{skips:?}").contains("CANARY"));

    // Allowed markers only in their allowed columns.
    let conn = db::open(&env.db_path).unwrap();
    for (table, cols) in [
        ("api_calls", "message_id || request_id || session_id || IFNULL(agent_id,'') || model"),
        ("tool_calls", "tool_use_id || message_id || tool_name || IFNULL(mcp_server,'') || IFNULL(skill_name,'') || IFNULL(subagent_type,'')"),
        ("command_invocations", "record_uuid || name || kind || via"),
        ("subagents", "agent_id || IFNULL(agent_type,'')"),
        // ingest_files.path is the relative transcript path; its project directory name is the encoded cwd, so it
        // carries the same information as sessions.project_path and is checked by neither rule.
        ("ingest_files", "''"),
    ] {
        let hits: i64 = one(&conn, &format!(
            "SELECT COUNT(*) FROM {table} WHERE ({cols}) LIKE '%TITLEMARK%' OR ({cols}) LIKE '%PATHMARK%' OR ({cols}) LIKE '%BRANCHMARK%'"
        ));
        assert_eq!(hits, 0, "marker leaked into {table}");
    }
}

fn assert_no_canary_in_dir(dir: &Path) {
    let mut checked = 0;
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let bytes = fs::read(&p).unwrap();
        checked += 1;
        assert!(
            !bytes.windows(b"CANARY_".len()).any(|w| w == b"CANARY_"),
            "canary found in {}",
            p.display()
        );
    }
    assert!(checked >= 1);
}
