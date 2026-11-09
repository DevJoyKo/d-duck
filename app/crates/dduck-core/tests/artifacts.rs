//! Artifact collection against record shapes measured on Claude Code 2.1.x (D-052).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use dduck_core::{db, discover, ingest};

const S: &str = "33333333-3333-4333-8333-333333333333";

fn publish(seq: i64, ts: &str, title: &str) -> String {
    serde_json::json!({
        "type": "user", "uuid": format!("00000000-0000-4000-8000-0000000000{seq:02}"), "timestamp": ts, "sessionId": S,
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_x", "content": "CANARY_RESULTTEXT"}]},
        "toolUseResult": {
            "artifact_id": "0c3e4a52-1111-4222-8333-444455556666",
            "url": "https://claude.ai/artifact/AbCdEfGhIjKlMnOpQrStUv",
            "title": title,
            "version": format!("17944{seq:05}-a1b2"),
            "seq": seq,
            "updated": true,
            "path": "/Users/me/CANARY_LOCALPATH/page.html",
            "own_files": [{"path": "CANARY_OWNFILE"}],
            "type": {"url": "https://claude.ai/artifact/TypeTypeTypeTypeTypeTy", "release": "CANARY_TYPERELEASE"}
        }
    })
    .to_string()
}

fn lines() -> Vec<String> {
    let mut v = vec![
        publish(3, "2026-11-01T10:03:00Z", "TITLEMARK_dash"),
        publish(2, "2026-11-01T10:02:00Z", "TITLEMARK_dash"),
        publish(4, "2026-11-01T10:04:00Z", "TITLEMARK_dashboard"),
    ];
    // A typed-create result: no seq, no artifact_id.
    v.push(serde_json::json!({
        "type": "user", "uuid": "00000000-0000-4000-8000-000000000099", "timestamp": "2026-11-01T09:00:00Z", "sessionId": S,
        "message": {"content": [{"type": "tool_result", "content": "x"}]},
        "toolUseResult": {"url": "https://claude.ai/code/artifact/Typed1234567890abcdefg", "title": "TITLEMARK_deck",
            "version": "1794400000-ffff", "type": {"url": "https://claude.ai/artifact/X"}, "instructions": "CANARY_INSTRUCTIONS"}
    }).to_string());
    // A web fetch of an artifact link: url but no version marker → not an artifact.
    v.push(serde_json::json!({
        "type": "user", "uuid": "00000000-0000-4000-8000-000000000098", "timestamp": "2026-11-01T11:00:00Z", "sessionId": S,
        "message": {"content": [{"type": "tool_result", "content": "x"}]},
        "toolUseResult": {"url": "https://claude.ai/artifact/FetchedFetchedFetched1", "bytes": 10, "code": 200, "result": "CANARY_FETCH"}
    }).to_string());
    // Wrong host / trailing junk → rejected.
    v.push(serde_json::json!({
        "type": "user", "uuid": "00000000-0000-4000-8000-000000000097", "timestamp": "2026-11-01T11:00:00Z", "sessionId": S,
        "message": {"content": [{"type": "tool_result", "content": "x"}]},
        "toolUseResult": {"url": "https://evil.example/artifact/Abc", "title": "t", "version": "1794400000-ffff"}
    }).to_string());
    v
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("claude/projects/-tmp-proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join(format!("{S}.jsonl")), lines().join("\n") + "\n").unwrap();
    let cfg = tmp.path().join("claude");
    (tmp, cfg)
}

fn ingest_all(conn: &mut rusqlite::Connection, cfg: &std::path::Path) {
    let root = discover::projects_root(cfg).unwrap();
    ingest::Ingestor::new(conn, Default::default()).ingest_all(&discover::discover(&root)).unwrap();
}

#[test]
fn artifacts_are_merged_by_link_id_and_filtered() {
    let (_tmp, cfg) = setup();
    let mut conn = db::open_in_memory().unwrap();
    ingest_all(&mut conn, &cfg);
    let rows: Vec<(String, String, Option<String>, String, i64)> = conn
        .prepare("SELECT artifact_id, url, title, kind, seq FROM artifacts ORDER BY artifact_id").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap()
        .map(Result::unwrap).collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let (id, url, title, kind, seq) = &rows[0];
    assert_eq!(id, "AbCdEfGhIjKlMnOpQrStUv");
    assert_eq!(url, "https://claude.ai/artifact/AbCdEfGhIjKlMnOpQrStUv");
    assert_eq!(title.as_deref(), Some("TITLEMARK_dashboard"));
    assert_eq!((kind.as_str(), *seq), ("typed", 4));
    assert_eq!((rows[1].0.as_str(), rows[1].4), ("Typed1234567890abcdefg", 1));
    let last: i64 = conn.query_row("SELECT last_ts FROM artifacts WHERE seq = 4", [], |r| r.get(0)).unwrap();
    assert_eq!(last, chrono::DateTime::parse_from_rfc3339("2026-11-01T10:04:00Z").unwrap().timestamp_millis());
}

#[test]
fn no_other_result_fields_are_stored() {
    let (tmp, cfg) = setup();
    let db_path = tmp.path().join("data/dduck.db");
    {
        let mut conn = db::open(&db_path).unwrap();
        ingest_all(&mut conn, &cfg);
    }
    for e in fs::read_dir(db_path.parent().unwrap()).unwrap() {
        let bytes = fs::read(e.unwrap().path()).unwrap();
        assert!(!bytes.windows(7).any(|w| w == b"CANARY_"), "result field leaked into the database files");
    }
}

#[test]
fn titles_off_keeps_artifact_titles_out() {
    let (_tmp, cfg) = setup();
    let mut conn = db::open_in_memory().unwrap();
    ingest::set_store_titles(&conn, false).unwrap();
    ingest_all(&mut conn, &cfg);
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM artifacts WHERE title IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0)).unwrap();
    assert_eq!(total, 2);
}
