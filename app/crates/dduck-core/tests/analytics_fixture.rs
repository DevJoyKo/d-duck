//! Analytics over the ingested `basic` fixture (expected values: tests/fixtures/README.md).
#![allow(clippy::unwrap_used, clippy::expect_used)]


use dduck_core::analytics::{self, Bucket, Dimension, Query};
use dduck_core::pricing::PriceTable;
use dduck_core::{db, discover, ingest};

const S: &str = "11111111-1111-4111-8111-111111111111";

fn ingested() -> rusqlite::Connection {
    let mut conn = db::open_in_memory().unwrap();
    let config = dduck_core::testkit::ensure_basic();
    let root = discover::projects_root(&config).unwrap();
    ingest::Ingestor::new(&mut conn, Default::default()).ingest_all(&discover::discover(&root)).unwrap();
    conn
}

/// 2026-11-01 00:00 KST .. 2026-11-02 00:00 KST (fixture timestamps are 2026-11-01T10:xxZ = 19:xx KST).
fn day_kst() -> Query {
    let start = 1_793_458_800_000; // 2026-10-31T15:00:00Z
    Query { from_ms: start, to_ms: start + 86_400_000, tz_offset_minutes: 540, ..Default::default() }
}

#[test]
fn summary_matches_fixture() {
    let conn = ingested();
    let s = analytics::summary(&conn, &day_kst(), &PriceTable::bundled()).unwrap();
    assert_eq!((s.tokens.input, s.tokens.output, s.tokens.cache_write, s.tokens.cache_read, s.tokens.thinking), (16, 175, 130, 3000, 27));
    assert_eq!((s.responses, s.sessions, s.tool_calls, s.skill_calls, s.command_calls), (3, 1, 4, 2, 1));
    // opus-5-5: in 16*4 + out 175*20 + cw5m 30*5 + cw1h 100*8 + cr 3000*0.2, per million
    let expected = (16.0 * 4.0 + 175.0 * 20.0 + 30.0 * 5.0 + 100.0 * 8.0 + 3000.0 * 0.2) / 1e6;
    assert!((s.est_cost_usd - expected).abs() < 1e-12, "{} vs {expected}", s.est_cost_usd);
    assert!(s.unpriced_models.is_empty());
}

#[test]
fn empty_period_is_zero() {
    let conn = ingested();
    let q = Query { from_ms: 0, to_ms: 1000, ..Default::default() };
    let s = analytics::summary(&conn, &q, &PriceTable::bundled()).unwrap();
    assert_eq!((s.responses, s.tokens.output, s.tool_calls, s.skill_calls), (0, 0, 0, 0));
    assert_eq!(s.est_cost_usd, 0.0);
}

#[test]
fn local_timezone_buckets() {
    let conn = ingested();
    let q = day_kst();
    assert_eq!(Bucket::for_range(q.from_ms, q.to_ms), Bucket::Hour);
    let hours = analytics::timeseries(&conn, &q, &PriceTable::bundled(), Bucket::Hour).unwrap();
    assert!(hours.iter().all(|p| p.bucket.starts_with("2026-11-01 19:")), "{hours:?}");
    let days = analytics::timeseries(&conn, &q, &PriceTable::bundled(), Bucket::Day).unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].bucket, "2026-11-01");
    assert_eq!(days[0].tokens.output, 175);
    // At UTC-12 the same instants (10:xxZ) are 22:xx on 2026-10-31.
    let west = Query { tz_offset_minutes: -720, from_ms: 0, to_ms: i64::MAX / 2, ..Default::default() };
    let days = analytics::timeseries(&conn, &west, &PriceTable::bundled(), Bucket::Day).unwrap();
    assert_eq!(days[0].bucket, "2026-10-31");
}

#[test]
fn distributions() {
    let conn = ingested();
    let p = PriceTable::bundled();
    let q = day_kst();
    let tools = analytics::distribution(&conn, &q, &p, Dimension::Tool, 10).unwrap();
    assert_eq!(tools.len(), 4);
    assert!((tools.iter().map(|t| t.share).sum::<f64>() - 100.0).abs() < 1e-9);

    let top2 = analytics::distribution(&conn, &q, &p, Dimension::Tool, 2).unwrap();
    assert_eq!(top2.len(), 3);
    assert_eq!((top2[2].key.as_str(), top2[2].count), ("기타", 2));

    let skills = analytics::distribution(&conn, &q, &p, Dimension::Skill, 0).unwrap();
    let get = |k: &str| skills.iter().find(|i| i.key == k).unwrap().clone();
    assert_eq!((get("opsx:propose").via_slash, get("opsx:propose").via_tool), (1, 0));
    assert_eq!((get("openspec-apply-change").via_tool, get("openspec-apply-change").via_slash), (1, 0));
    assert!(skills.iter().all(|i| i.key != "upgrade"), "builtin must not be a skill");

    let cmds = analytics::distribution(&conn, &q, &p, Dimension::Command, 0).unwrap();
    assert_eq!(cmds.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["upgrade"]);

    let mcp = analytics::distribution(&conn, &q, &p, Dimension::McpServer, 0).unwrap();
    assert_eq!((mcp[0].key.as_str(), mcp[0].count), ("notion", 1));
    let sub = analytics::distribution(&conn, &q, &p, Dimension::Subagent, 0).unwrap();
    assert_eq!(sub[0].key, "Explore");

    let models = analytics::distribution(&conn, &q, &p, Dimension::Model, 0).unwrap();
    assert_eq!((models[0].key.as_str(), models[0].count, models[0].priced), ("claude-opus-5-5", 3, true));
    let projects = analytics::distribution(&conn, &q, &p, Dimension::Project, 0).unwrap();
    assert_eq!((projects[0].key.as_str(), projects[0].count), ("PATHMARK_proj", 3));
}

#[test]
fn filters_apply_everywhere() {
    let conn = ingested();
    let p = PriceTable::bundled();
    let other_project = Query { projects: vec!["nope".into()], ..day_kst() };
    let s = analytics::summary(&conn, &other_project, &p).unwrap();
    assert_eq!((s.responses, s.tool_calls, s.skill_calls, s.command_calls), (0, 0, 0, 0));
    let mine = Query { projects: vec!["PATHMARK_proj".into()], models: vec!["claude-opus-5-5".into()], ..day_kst() };
    assert_eq!(analytics::summary(&conn, &mine, &p).unwrap().responses, 3);
    let other_model = Query { models: vec!["claude-haiku-5-5".into()], ..day_kst() };
    let s = analytics::summary(&conn, &other_model, &p).unwrap();
    assert_eq!((s.responses, s.tool_calls, s.skill_calls), (0, 0, 0));
}

#[test]
fn unpriced_model_reported() {
    let conn = ingested();
    let empty = PriceTable { models: vec![], checked: None };
    let s = analytics::summary(&conn, &day_kst(), &empty).unwrap();
    assert_eq!(s.unpriced_models, ["claude-opus-5-5"]);
    assert_eq!(s.est_cost_usd, 0.0);
}

#[test]
fn sessions_and_detail() {
    let conn = ingested();
    let p = PriceTable::bundled();
    let rows = analytics::sessions(&conn, &day_kst(), &p, 50).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, S);
    assert_eq!(rows[0].reported_cost_usd, Some(5.0));
    let d = analytics::session_detail(&conn, &p, S).unwrap();
    assert_eq!(d.session.responses, 3);
    assert_eq!(d.tools.len(), 4);
    assert_eq!(d.skills.len(), 2);
    let (projects, models) = analytics::filter_options(&conn).unwrap();
    assert_eq!((projects, models), (vec!["PATHMARK_proj".to_string()], vec!["claude-opus-5-5".to_string()]));
}
