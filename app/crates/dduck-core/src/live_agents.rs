//! Who is working right now, per agent card (D-056). Computed from the last `claude agents` snapshot and the
//! small `subagents/*.meta.json` files; transcripts themselves are never opened (only their mtime is read).
//! The short task descriptions returned here go to the UI only: never to the DB, logs, tray or notifications.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::claude_cli::{AgentSession, SessionStatus};
use crate::text::display_safe;

const MAX_META_BYTES: u64 = 16 * 1024;
const MAX_META_PER_SESSION: usize = 100;
const RUNNING_FRESH_MS: i64 = 5 * 60 * 1000;
pub const RECENT_DONE_MS: i64 = 10 * 60 * 1000;
/// Card key for sessions without a persona.
pub const DEFAULT_AGENT: &str = "";

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z").unwrap_or_else(|_| unreachable!())
});

/// Priority order: earlier = shown first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActState {
    Waiting,
    Failed,
    Working,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Agent name, or "" for 기본 Claude.
    pub agent: String,
    pub state: ActState,
    /// How many items are in `state`.
    pub count: usize,
    pub total: usize,
    pub detail: Option<String>,
    /// Task board key to jump to (main sessions only).
    pub session_key: Option<String>,
    pub subagent: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    agent_type: Option<String>,
    description: Option<String>,
    finished: Option<bool>,
}

struct Event {
    agent: String,
    state: ActState,
    at: i64,
    detail: Option<String>,
    session_key: Option<String>,
    subagent: bool,
}

fn mtime_ms(p: &Path) -> Option<i64> {
    let m = std::fs::symlink_metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    i64::try_from(t.as_millis()).ok()
}

fn session_dirs(projects: &Path, session_id: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(projects) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| std::fs::symlink_metadata(p.join(session_id)).is_ok_and(|m| m.file_type().is_dir()))
        .collect()
}

fn transcript_mtime(projects: &Path, session_id: &str) -> Option<i64> {
    let Ok(entries) = std::fs::read_dir(projects) else { return None };
    entries.filter_map(Result::ok).filter_map(|e| mtime_ms(&e.path().join(format!("{session_id}.jsonl")))).max()
}

fn subagent_events(projects: &Path, s: &AgentSession, sid: &str, now: i64, out: &mut Vec<Event>) {
    for dir in session_dirs(projects, sid) {
        let sub = dir.join(sid).join("subagents");
        if !std::fs::symlink_metadata(&sub).is_ok_and(|m| m.file_type().is_dir()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&sub) else { continue };
        // Newest first, so a long session's live subagents are never cut off by the cap.
        let mut metas: Vec<(i64, PathBuf)> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".meta.json"))
            .map(|p| (mtime_ms(&p).unwrap_or(0), p))
            .collect();
        metas.sort_by_key(|m| std::cmp::Reverse(m.0));
        for (_, path) in metas.into_iter().take(MAX_META_PER_SESSION) {
            let Ok(m) = std::fs::symlink_metadata(&path) else { continue };
            if !m.file_type().is_file() || m.len() > MAX_META_BYTES {
                continue;
            }
            let Some(meta) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Meta>(&b).ok()) else { continue };
            let Some(agent) = meta.agent_type.filter(|a| crate::agents_catalog::valid_name(a)) else { continue };
            let detail = meta.description.as_deref().map(|d| display_safe(d, 60)).filter(|d| !d.is_empty());
            let meta_at = mtime_ms(&path).unwrap_or(0);
            let jsonl = PathBuf::from(path.to_string_lossy().trim_end_matches(".meta.json").to_string() + ".jsonl");
            let last = mtime_ms(&jsonl).unwrap_or(meta_at).max(meta_at);
            let state = if meta.finished == Some(true) {
                // Finished is shown whatever the session does now (interactive sessions go idle right after).
                (now - meta_at <= RECENT_DONE_MS).then_some(ActState::Done)
            } else if now - last > RUNNING_FRESH_MS {
                None // no `finished` and nothing written for a while: an interrupted trace
            } else {
                match s.status {
                    SessionStatus::Working => Some(ActState::Working),
                    // A subagent asking for permission puts its whole session into "waiting".
                    SessionStatus::WaitingInput => Some(ActState::Waiting),
                    _ => None,
                }
            };
            if let Some(state) = state {
                out.push(Event { agent, state, at: last, detail, session_key: Some(s.key.clone()), subagent: true });
            }
        }
    }
}

/// `persona` maps a session to its `--agent` (D.Duck task meta or the transcript's `agentSetting`).
pub fn collect(config_dir: &Path, sessions: &[AgentSession], persona: &dyn Fn(&AgentSession) -> Option<String>, now: i64) -> Vec<Activity> {
    let projects = config_dir.join("projects");
    let mut events = Vec::new();
    for s in sessions {
        let sid = s.session_id.as_deref().filter(|id| UUID_RE.is_match(id));
        let main_state = match s.status {
            SessionStatus::Working => Some(ActState::Working),
            SessionStatus::WaitingInput => Some(ActState::Waiting),
            SessionStatus::Failed => Some(ActState::Failed),
            SessionStatus::Done => sid.and_then(|id| transcript_mtime(&projects, id)).filter(|t| now - t <= RECENT_DONE_MS).map(|_| ActState::Done),
            _ => None,
        };
        if let Some(state) = main_state {
            events.push(Event {
                agent: persona(s).unwrap_or_else(|| DEFAULT_AGENT.to_string()),
                state,
                at: sid.and_then(|id| transcript_mtime(&projects, id)).unwrap_or(s.started_at),
                detail: Some(display_safe(&s.name, 60)),
                session_key: Some(s.key.clone()),
                subagent: false,
            });
        }
        if let Some(id) = sid {
            subagent_events(&projects, s, id, now, &mut events);
        }
    }
    let mut agents: Vec<String> = events.iter().map(|e| e.agent.clone()).collect();
    agents.sort();
    agents.dedup();
    agents
        .into_iter()
        .filter_map(|agent| {
            let mine: Vec<&Event> = events.iter().filter(|e| e.agent == agent).collect();
            let state = mine.iter().map(|e| e.state).min()?;
            let top: Vec<&&Event> = mine.iter().filter(|e| e.state == state).collect();
            let latest = top.iter().max_by_key(|e| e.at)?;
            Some(Activity {
                agent,
                state,
                count: top.len(),
                total: mine.len(),
                detail: latest.detail.clone(),
                session_key: latest.session_key.clone(),
                subagent: latest.subagent,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::claude_cli::SessionKind;

    const SID: &str = "5f3c2a10-7d4e-4b8a-9c21-6e0f1a2b3c4d";

    fn session(status: SessionStatus, sid: &str) -> AgentSession {
        AgentSession { key: sid.into(), pid: Some(1), id: Some("ab12cd34".into()), session_id: Some(sid.into()), name: "flaky-test-fix".into(), cwd: "/x".into(), kind: SessionKind::Background, started_at: 1, status }
    }

    fn now() -> i64 {
        i64::try_from(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis()).unwrap()
    }

    fn set_old(p: &Path) {
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - std::time::Duration::from_secs(3600)).unwrap();
    }

    #[test]
    fn subagent_states_follow_measured_format() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("projects/-x").join(SID).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        // Running: no `finished` key, transcript fresh.
        std::fs::write(sub.join("agent-a1.meta.json"), r#"{"agentType":"design-critic","description":"설계 비판 검토\u202e","isBuiltIn":false}"#).unwrap();
        std::fs::write(sub.join("agent-a1.jsonl"), "{}").unwrap();
        // Stale trace: no `finished`, transcript untouched for an hour.
        std::fs::write(sub.join("agent-a2.meta.json"), r#"{"agentType":"qa-verifier"}"#).unwrap();
        std::fs::write(sub.join("agent-a2.jsonl"), "{}").unwrap();
        set_old(&sub.join("agent-a2.jsonl"));
        set_old(&sub.join("agent-a2.meta.json"));
        // Recently finished, unknown builtin type.
        std::fs::write(sub.join("agent-a3.meta.json"), r#"{"agentType":"web-fetch","finished":true}"#).unwrap();
        // Garbage.
        std::fs::write(sub.join("agent-a4.meta.json"), "not json").unwrap();

        let acts = collect(tmp.path(), &[session(SessionStatus::Working, SID)], &|_| None, now());
        let find = |a: &str| acts.iter().find(|x| x.agent == a);
        let dc = find("design-critic").unwrap();
        assert_eq!((dc.state, dc.detail.as_deref(), dc.subagent), (ActState::Working, Some("설계 비판 검토"), true));
        assert!(find("qa-verifier").is_none(), "stale trace ignored");
        assert_eq!(find("web-fetch").unwrap().state, ActState::Done);
        assert_eq!(find(DEFAULT_AGENT).unwrap().state, ActState::Working);

        let idle = collect(tmp.path(), &[session(SessionStatus::Idle, SID)], &|_| None, now());
        assert!(idle.iter().all(|a| a.agent != "design-critic"), "idle session: unfinished meta is not work");
        assert_eq!(idle.iter().find(|a| a.agent == "web-fetch").map(|a| a.state), Some(ActState::Done), "done shows even after the session went idle");
        let waiting = collect(tmp.path(), &[session(SessionStatus::WaitingInput, SID)], &|_| None, now());
        assert_eq!(waiting.iter().find(|a| a.agent == "design-critic").map(|a| a.state), Some(ActState::Waiting));
    }

    #[test]
    fn done_window_is_ten_minutes() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("projects/-x").join(SID).join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let at = |name: &str, mins: u64| {
            let p = sub.join(format!("{name}.meta.json"));
            std::fs::write(&p, format!(r#"{{"agentType":"{name}","finished":true}}"#)).unwrap();
            let f = std::fs::File::options().write(true).open(&p).unwrap();
            f.set_modified(SystemTime::now() - std::time::Duration::from_secs(mins * 60)).unwrap();
        };
        at("nine", 9);
        at("eleven", 11);
        let acts = collect(tmp.path(), &[session(SessionStatus::Idle, SID)], &|_| None, now());
        assert!(acts.iter().any(|a| a.agent == "nine"));
        assert!(acts.iter().all(|a| a.agent != "eleven"));
    }

    #[test]
    fn persona_sessions_and_priority() {
        let tmp = tempfile::tempdir().unwrap();
        let a = session(SessionStatus::WaitingInput, "11111111-1111-4111-8111-111111111111");
        let mut b = session(SessionStatus::Working, "22222222-2222-4222-8222-222222222222");
        b.key = "b".into();
        let acts = collect(tmp.path(), &[a, b], &|_| Some("qa-verifier".into()), now());
        assert_eq!(acts.len(), 1);
        assert_eq!((acts[0].state, acts[0].count, acts[0].total), (ActState::Waiting, 1, 2));
    }

    #[test]
    fn hostile_session_ids_are_not_paths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("projects/p/subagents")).unwrap();
        std::fs::write(tmp.path().join("projects/p/subagents/agent-x.meta.json"), r#"{"agentType":"evil"}"#).unwrap();
        let acts = collect(tmp.path(), &[session(SessionStatus::Working, "../p")], &|_| None, now());
        assert!(acts.iter().all(|a| a.agent != "evil"));
    }
}
