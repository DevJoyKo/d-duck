//! IPC contract: the JSON field names Rust sends must match the TypeScript interfaces in `src/lib/api.ts`.
//! When a field is renamed on either side, this test fails instead of the UI silently showing `undefined`.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use dduck_core::analytics::{self, Query};
use dduck_core::claude_cli::{AgentSession, CliError, PermissionMode, SessionKind, SessionStatus, TaskSpec};
use dduck_core::cli_path::CliInfo;
use dduck_core::pricing::PriceTable;
use dduck_core::{db, discover, ingest};

fn keys(v: &serde_json::Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn ingested() -> rusqlite::Connection {
    let mut conn = db::open_in_memory().unwrap();
    let root = discover::projects_root(&dduck_core::testkit::ensure_basic()).unwrap();
    ingest::Ingestor::new(&mut conn, Default::default()).ingest_all(&discover::discover(&root)).unwrap();
    conn
}

#[test]
fn analytics_payloads_match_typescript() {
    let conn = ingested();
    let p = PriceTable::bundled();
    let q = Query { from_ms: 0, to_ms: i64::MAX / 2, tz_offset_minutes: 540, ..Default::default() };

    let summary = serde_json::to_value(analytics::summary(&conn, &q, &p).unwrap()).unwrap();
    assert_eq!(keys(&summary), set(&["tokens", "estCostUsd", "unpricedModels", "sessions", "responses", "toolCalls", "skillCalls", "commandCalls"]));
    assert_eq!(keys(&summary["tokens"]), set(&["input", "output", "thinking", "cacheWrite", "cacheRead"]));

    let point = serde_json::to_value(&analytics::timeseries(&conn, &q, &p, analytics::Bucket::Day).unwrap()[0]).unwrap();
    assert_eq!(keys(&point), set(&["bucket", "tokens", "estCostUsd"]));

    let item = serde_json::to_value(&analytics::distribution(&conn, &q, &p, analytics::Dimension::Model, 0).unwrap()[0]).unwrap();
    assert_eq!(keys(&item), set(&["key", "count", "share", "viaTool", "viaSlash", "tokens", "estCostUsd", "priced"]));

    let row = serde_json::to_value(&analytics::sessions(&conn, &q, &p, 10).unwrap()[0]).unwrap();
    assert_eq!(
        keys(&row),
        set(&["sessionId", "title", "label", "projectName", "gitBranch", "firstTs", "lastTs", "responses", "tokens", "estCostUsd", "reportedCostUsd", "topTools", "skills", "subagents"])
    );
    let id = row["sessionId"].as_str().unwrap();
    let detail = serde_json::to_value(analytics::session_detail(&conn, &p, id).unwrap()).unwrap();
    assert_eq!(keys(&detail), set(&["session", "models", "tools", "skills"]));
}

#[test]
fn query_and_dimension_accept_typescript_shapes() {
    let q: Query = serde_json::from_str(r#"{"fromMs":1,"toMs":2,"tzOffsetMinutes":540,"projects":["a"],"models":[]}"#).unwrap();
    assert_eq!((q.from_ms, q.to_ms, q.tz_offset_minutes, q.projects.len()), (1, 2, 540, 1));
    for d in ["model", "project", "tool", "skill", "command", "mcpServer", "subagent"] {
        serde_json::from_value::<analytics::Dimension>(serde_json::json!(d)).unwrap();
    }
}

#[test]
fn task_payloads_match_typescript() {
    let s = AgentSession {
        key: "k".into(),
        pid: Some(1),
        id: Some("ab12cd34".into()),
        session_id: Some("k".into()),
        name: "n".into(),
        cwd: "/w".into(),
        kind: SessionKind::Background,
        started_at: 1,
        status: SessionStatus::WaitingInput,
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(keys(&v), set(&["key", "pid", "id", "sessionId", "name", "cwd", "kind", "startedAt", "status"]));
    assert_eq!(v["status"], "waitingInput");
    assert_eq!(v["kind"], "background");

    let e = serde_json::to_value(CliError::Failed { code: 2, stderr_tail: "x".into() }).unwrap();
    assert_eq!(keys(&e), set(&["kind", "code", "stderr_tail"]));
    assert_eq!(e["kind"], "failed");
    assert_eq!(serde_json::to_value(CliError::UntrustedWorkspace { stderr_tail: String::new() }).unwrap()["kind"], "untrustedWorkspace");
    assert_eq!(serde_json::to_value(CliError::DangerNotConfirmed).unwrap()["kind"], "dangerNotConfirmed");

    // The form sends permission modes by their CLI names.
    let spec: TaskSpec = serde_json::from_str(r#"{"cwd":"/w","prompt":"p","name":null,"permissionMode":"bypassPermissions"}"#).unwrap();
    assert_eq!(spec.permission_mode, Some(PermissionMode::BypassPermissions));
    for m in PermissionMode::ALL {
        let parsed: PermissionMode = serde_json::from_value(serde_json::json!(m.as_cli())).unwrap();
        assert_eq!(parsed, m);
    }

    let cli = serde_json::to_value(CliInfo { path: "/x/claude".into(), version: "2.1.319".into(), supported: true }).unwrap();
    assert_eq!(keys(&cli), set(&["path", "version", "supported"]));
}

#[test]
fn routine_and_artifact_payloads_match_typescript() {
    use dduck_core::{artifacts, routines};
    let conn = db::open_in_memory().unwrap();
    conn.execute("INSERT INTO artifacts VALUES ('a', 'https://claude.ai/artifact/a', 't', 'page', 1, 's', 1, 2)", []).unwrap();
    let a = serde_json::to_value(&artifacts::list(&conn, "", 10).unwrap()[0]).unwrap();
    assert_eq!(keys(&a), set(&["artifactId", "url", "title", "kind", "seq", "firstSeenTs", "lastTs", "projectName"]));

    let dir = std::env::temp_dir();
    let input: routines::RoutineInput = serde_json::from_value(serde_json::json!({
        "name": "r", "cwd": dir, "prompt": "p", "permissionMode": "plan",
        "schedule": {"kind": "weekly", "weekdays": 3, "hour": 9, "minute": 0}
    }))
    .unwrap();
    let id = routines::insert(&conn, &input.validate().unwrap(), 0, &chrono::Utc).unwrap();
    let r = serde_json::to_value(&routines::list(&conn).unwrap()[0]).unwrap();
    assert_eq!(
        keys(&r),
        set(&["id", "name", "cwd", "promptLen", "permissionMode", "schedule", "scheduleText", "enabled", "nextRunAt", "lastStatus", "active"])
    );
    assert_eq!(r["schedule"]["kind"], "weekly");
    for s in [r#"{"kind":"daily","hour":1,"minute":2}"#, r#"{"kind":"interval","hours":6}"#] {
        serde_json::from_str::<routines::Schedule>(s).unwrap();
    }
    conn.execute("INSERT INTO routine_runs (routine_id, scheduled_for, status) VALUES (?1, 1, 'missed')", [id]).unwrap();
    let run = serde_json::to_value(&routines::runs(&conn, id, 10).unwrap()[0]).unwrap();
    assert_eq!(keys(&run), set(&["id", "scheduledFor", "startedAt", "endedAt", "status", "bgId", "error"]));
}

/// Task types are string keys from the editable team config (D-081): bundled keys, user keys, unknown keys.
#[test]
fn task_type_keys_and_team_config_payloads() {
    use dduck_core::team_config::{self, TeamInput};
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("nohome");
    let spec = |t: &str| -> TaskSpec {
        serde_json::from_str(&format!(r#"{{"cwd":{:?},"prompt":"p","name":null,"permissionMode":null,"taskType":{t:?}}}"#, dir.path())).unwrap()
    };
    let mut learn = spec("learn");
    learn.prepare(&home, 0, None).unwrap();
    assert_eq!(learn.inline_role.as_deref(), Some("dduck-tutor"));
    let mut unknown = spec("nope");
    assert!(unknown.prepare(&home, 0, None).is_err(), "a key missing from the config is refused");
    assert!(spec("Bad Key").validate().is_err());

    // A user-added type, effort changed on learn: the stored config drives the arguments.
    let d = team_config::defaults();
    let mut input = TeamInput {
        rules: d.rules.iter().map(|r| team_config::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: String::new() }).collect(),
        types: d.types.iter().map(|t| t.def.clone()).collect(),
        refine: None,
        preferred: None,
    };
    input.types.iter_mut().find(|t| t.key == "learn").unwrap().effort = dduck_core::team::Effort::High;
    let mut ops = d.get("qa").unwrap().clone();
    ops.key = "ops".into();
    ops.label = "운영".into();
    ops.role_tools = Some(vec!["Read".into()]);
    input.types.push(ops);
    let stored = serde_json::to_string(&team_config::diff(&input, &[]).unwrap()).unwrap();
    let mut learn = spec("learn");
    learn.prepare(&home, 0, Some(&stored)).unwrap();
    assert!(learn.bg_args().iter().any(|a| a == "--effort=high"));
    let mut ops = spec("ops");
    ops.prepare(&home, 0, Some(&stored)).unwrap();
    assert_eq!(ops.inline_role.as_deref(), Some("dduck-ops"));
    assert!(ops.strict_type);

    // TeamConfig payload keys (src/lib/api.ts TeamType / TeamRule).
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(keys(&v), set(&["header", "rules", "types", "refine", "preferred", "errors"]));
    assert_eq!(keys(&v["refine"]), set(&["guidance", "maxQuestions", "rounds"]));
    assert_eq!(keys(&v["rules"][0]), set(&["id", "text", "extra", "locked", "bundled", "defaultText", "changed", "defaultChanged"]));
    assert_eq!(
        keys(&v["types"][0]),
        set(&["key", "label", "role", "roleLabel", "agent", "rolePrompt", "roleTools", "mode", "modes", "effort", "review", "strict", "hidden", "autoAllowed", "note", "refineFocus", "skills", "mcpAllow", "bundled", "changed", "defaultChanged", "fixedStrict", "lockedRule", "reportShape"])
    );
    // What the webview sends back parses into TeamInput.
    let back: TeamInput = serde_json::from_value(serde_json::json!({ "rules": v["rules"], "types": v["types"] })).unwrap();
    assert!(team_config::diff(&back, &[]).is_ok());
    let back: TeamInput = serde_json::from_value(serde_json::json!({ "rules": v["rules"], "types": v["types"], "refine": v["refine"] })).unwrap();
    assert!(team_config::diff(&back, &[]).unwrap().refine.is_none());
}

