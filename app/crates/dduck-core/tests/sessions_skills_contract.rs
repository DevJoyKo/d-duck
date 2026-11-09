//! Field names for the sessions page, timeline, skills and connectors match `src/lib/api.ts`; timeline text never
//! reaches logs or the DB (D-071).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

fn keys(v: &serde_json::Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}
fn set(n: &[&str]) -> BTreeSet<String> {
    n.iter().map(|s| s.to_string()).collect()
}

#[test]
fn payload_shapes() {
    use dduck_core::{mcp_list, skills_catalog, timeline};
    let item = serde_json::to_value(timeline::Item::Tool { ts: 1, id: "t".into(), name: "Bash".into(), summary: "x".into(), ok: Some(true), error: None, raw: vec![[0, 1]] }).unwrap();
    assert_eq!(keys(&item), set(&["kind", "ts", "id", "name", "summary", "ok", "error", "raw"]));
    assert_eq!(item["kind"], "tool");
    assert_eq!(item["raw"], serde_json::json!([[0, 1]]));
    let agent = serde_json::to_value(timeline::Item::Agent {
        ts: 1,
        id: "t".into(),
        subagent_type: "Explore".into(),
        description: "d".into(),
        ok: None,
        error: None,
        agent_id: Some("a1".into()),
        raw: vec![],
    })
    .unwrap();
    assert_eq!(keys(&agent), set(&["kind", "ts", "id", "subagentType", "description", "ok", "error", "agentId", "raw"]));
    let skill = serde_json::to_value(timeline::Item::Skill { ts: 1, id: "t".into(), name: "s".into(), ok: None, error: None, raw: vec![] }).unwrap();
    assert_eq!((keys(&skill), &skill["kind"]), (set(&["kind", "ts", "id", "name", "ok", "error", "raw"]), &serde_json::json!("skill")));
    let cmd = serde_json::to_value(timeline::Item::Command { ts: 1, name: "c".into(), raw: vec![] }).unwrap();
    assert_eq!((keys(&cmd), &cmd["kind"]), (set(&["kind", "ts", "name", "raw"]), &serde_json::json!("command")));
    let page = serde_json::to_value(dduck_core::raw::RawPage {
        records: vec![dduck_core::raw::RawRecord { offset: 0, kind: None, ts: None, minor: false, text: String::new(), json: true, truncated: false }],
        start: 0,
        next: None,
        file_len: 0,
        stopped: false,
    })
    .unwrap();
    assert_eq!(keys(&page), set(&["records", "start", "next", "fileLen", "stopped"]));
    assert_eq!(keys(&page["records"][0]), set(&["offset", "kind", "ts", "minor", "text", "json", "truncated"]));
    let t = serde_json::to_value(timeline::Timeline { items: vec![], truncated_front: true, skipped_lines: 0 }).unwrap();
    assert_eq!(keys(&t), set(&["items", "truncatedFront", "skippedLines"]));
    let list = mcp_list::parse("x: https://a.example/y - ✔ Connected");
    assert_eq!(keys(&serde_json::to_value(&list).unwrap()), set(&["connectors", "skipped"]));
    assert_eq!(keys(&serde_json::to_value(&list.connectors[0]).unwrap()), set(&["name", "kind", "host", "state"]));
    let cat = skills_catalog::catalog(vec![], [("a".to_string(), (1, 2))].into_iter().collect());
    assert_eq!(keys(&serde_json::to_value(&cat[0]).unwrap()), set(&["name", "source", "scope", "kind", "description", "uses", "lastUsed", "defined", "id", "readonly"]));
    let conn = dduck_core::db::open_in_memory().unwrap();
    let q: dduck_core::analytics::Query = serde_json::from_str(r#"{"fromMs":0,"toMs":1,"tzOffsetMinutes":0,"projects":[],"models":[]}"#).unwrap();
    let page = dduck_core::analytics::sessions_page(&conn, &q, &dduck_core::pricing::PriceTable::bundled(), 0, 10).unwrap();
    assert_eq!(keys(&serde_json::to_value(&page).unwrap()), set(&["rows", "total"]));
}

#[test]
fn session_rows_carry_tool_skill_and_subagent_counts() {
    let conn = dduck_core::db::open_in_memory().unwrap();
    conn.execute("INSERT INTO api_calls(message_id, session_id, ts, model, output) VALUES ('m', 's1', 10, 'claude-opus-5-5', 1)", []).unwrap();
    let tools = [("Bash", None), ("Bash", None), ("Bash", None), ("Edit", None), ("Edit", None), ("Read", None), ("Grep", None), ("Agent", Some("Explore")), ("Agent", Some("Explore"))];
    for (i, (name, sub)) in tools.iter().enumerate() {
        conn.execute(
            "INSERT INTO tool_calls(tool_use_id, message_id, session_id, ts, tool_name, subagent_type) VALUES (?1, 'm', 's1', 10, ?2, ?3)",
            rusqlite::params![format!("t{i}"), name, sub],
        )
        .unwrap();
    }
    conn.execute("INSERT INTO command_invocations(record_uuid, session_id, ts, name, kind, via) VALUES ('u1', 's1', 10, 'opsx:propose', 'skill', 'slash')", []).unwrap();
    conn.execute("INSERT INTO command_invocations(record_uuid, session_id, ts, name, kind, via) VALUES ('u2', 's1', 10, 'upgrade', 'builtin', 'slash')", []).unwrap();
    let q: dduck_core::analytics::Query = serde_json::from_str(r#"{"fromMs":0,"toMs":100,"tzOffsetMinutes":0,"projects":[],"models":[]}"#).unwrap();
    let page = dduck_core::analytics::sessions_page(&conn, &q, &dduck_core::pricing::PriceTable::bundled(), 0, 10).unwrap();
    let r = serde_json::to_value(&page.rows[0]).unwrap();
    assert_eq!(r["topTools"], serde_json::json!([{"name":"Bash","count":3},{"name":"Agent","count":2},{"name":"Edit","count":2}]));
    assert_eq!(r["skills"], serde_json::json!([{"name":"opsx:propose","count":1}]));
    assert_eq!(r["subagents"], serde_json::json!([{"name":"Explore","count":2}]));
}

#[test]
fn timeline_is_never_logged_or_stored() {
    for file in ["timeline.rs", "raw.rs"] {
        let src = std::fs::read_to_string(format!("{}/src/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        for bad in ["println!", "eprintln!", "dbg!", "log::", "tracing::", "execute(", "Connection", "{:?}", "fs::write", "File::create", "OpenOptions"] {
            assert!(!src[..src.find("#[cfg(test)]").unwrap_or(src.len())].contains(bad), "{file} must not use {bad}");
        }
    }
    // The shell command that returns raw records (D-074).
    let shell = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/src/lib.rs")).unwrap();
    let start = shell.find("fn session_raw(").unwrap();
    let body = &shell[start..start + shell[start..].find("#[tauri::command").unwrap()];
    for bad in ["println!", "eprintln!", "dbg!", "log::", "{:?}", "execute(", "fs::write"] {
        assert!(!body.contains(bad), "session_raw must not use {bad}");
    }
    // The UI that shows raw records: no console output, no browser storage.
    for file in ["src/components/SessionBits.tsx", "src/components/RawView.tsx"] {
        let src = std::fs::read_to_string(format!("{}/../../{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        for bad in ["console.", "localStorage", "sessionStorage", "indexedDB", "dangerouslySetInnerHTML", "<textarea"] {
            assert!(!src.contains(bad), "{file} must not use {bad}");
        }
    }
}

#[test]
fn sessions_page_counts_and_offsets() {
    let conn = dduck_core::db::open_in_memory().unwrap();
    for i in 0..34 {
        conn.execute(
            "INSERT INTO api_calls(message_id, session_id, ts, model, output) VALUES (?1, ?2, ?3, 'claude-opus-5-5', 1)",
            rusqlite::params![format!("m{i}"), format!("s{i:02}"), 1000 + (i % 5)],
        )
        .unwrap();
    }
    let q: dduck_core::analytics::Query = serde_json::from_str(r#"{"fromMs":0,"toMs":100000,"tzOffsetMinutes":0,"projects":[],"models":[]}"#).unwrap();
    let p = dduck_core::pricing::PriceTable::bundled();
    let a = dduck_core::analytics::sessions_page(&conn, &q, &p, 0, 10).unwrap();
    let b = dduck_core::analytics::sessions_page(&conn, &q, &p, 10, 10).unwrap();
    let last = dduck_core::analytics::sessions_page(&conn, &q, &p, 30, 10).unwrap();
    assert_eq!((a.total, a.rows.len(), last.rows.len()), (34, 10, 4));
    let ids = |pg: &dduck_core::analytics::SessionPage| pg.rows.iter().map(|r| r.session_id.clone()).collect::<Vec<_>>();
    assert!(ids(&a).iter().all(|x| !ids(&b).contains(x)), "ties are ordered: no duplicates across pages");
}
