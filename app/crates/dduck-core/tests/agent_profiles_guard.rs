//! Subagent task descriptions reach the UI only (D-056); payload field names match `src/lib/api.ts`.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use dduck_core::{agents_catalog, live_agents};

fn src(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn descriptions_are_never_logged_or_stored() {
    let live = src("src/live_agents.rs");
    for bad in ["println!", "eprintln!", "dbg!", " log::", "info!(", "warn!(", "tracing::", "execute(", "Connection"] {
        assert!(!live.contains(bad), "live_agents.rs must not use {bad}");
    }
    let shell = src("../../src-tauri/src/lib.rs");
    assert!(!shell.contains(".detail"), "the shell only forwards activity, it never reads the description");
}

#[test]
fn payload_keys_match_typescript() {
    let keys = |v: serde_json::Value| v.as_object().unwrap().keys().cloned().collect::<BTreeSet<_>>();
    let set = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
    let tmp = tempfile::tempdir().unwrap();
    let defs = agents_catalog::discover(&agents_catalog::Roots { config_dir: tmp.path().into(), project_dirs: vec![] });
    let d = serde_json::to_value(&defs[0]).unwrap();
    assert_eq!(
        keys(d.clone()),
        set(&["id", "name", "source", "scope", "path", "description", "model", "tools", "disallowedTools", "skills", "malformed", "editable", "readonlyReason", "primary", "usable", "risk"])
    );
    assert_eq!(keys(d["risk"].clone()), set(&["hooks", "mcpServers", "dangerMode", "unreadable"]));
    let a = live_agents::Activity { agent: "a".into(), state: live_agents::ActState::Working, count: 1, total: 1, detail: None, session_key: None, subagent: false };
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(keys(v.clone()), set(&["agent", "state", "count", "total", "detail", "sessionKey", "subagent"]));
    assert_eq!(v["state"], "working");
}
