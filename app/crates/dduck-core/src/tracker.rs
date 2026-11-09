//! Turns successive `claude agents` snapshots into state transitions, keeps ended sessions visible for 24 hours
//! after their end was first observed (D-028 ④), and builds notification text (D-016).

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;
use crate::claude_cli::{AgentSession, CliError, SessionStatus};

pub const ENDED_VISIBLE_MS: i64 = 24 * 3600 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub key: String,
    pub name: String,
    pub from: Option<SessionStatus>,
    pub to: SessionStatus,
}

impl Transition {
    pub fn became_waiting(&self) -> bool {
        self.to == SessionStatus::WaitingInput && self.from != Some(SessionStatus::WaitingInput)
    }

    /// A running or waiting session ended (done or failed). First sightings never count (D-067).
    pub fn became_finished(&self) -> bool {
        matches!(self.to, SessionStatus::Done | SessionStatus::Failed)
            && matches!(self.from, Some(SessionStatus::Working | SessionStatus::WaitingInput))
    }
}

#[derive(Default)]
pub struct Tracker {
    last: HashMap<String, SessionStatus>,
    primed: bool,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compares `snapshot` with the previous one. The first call only primes the state, so sessions that were
    /// already waiting when the app started do not all notify at once.
    pub fn update(&mut self, snapshot: &[AgentSession]) -> Vec<Transition> {
        let mut out = Vec::new();
        let mut next = HashMap::with_capacity(snapshot.len());
        for s in snapshot {
            let prev = self.last.get(&s.key).copied();
            if self.primed && prev != Some(s.status) {
                out.push(Transition { key: s.key.clone(), name: s.name.clone(), from: prev, to: s.status });
            }
            next.insert(s.key.clone(), s.status);
        }
        self.last = next;
        self.primed = true;
        out
    }
}

/// One poll cycle's view for the UI and tray.
#[derive(Debug, Clone, PartialEq)]
pub struct PollView {
    /// Last successful snapshot (kept unchanged through errors so the list and tray do not flicker).
    pub sessions: Vec<AgentSession>,
    pub error: Option<CliError>,
    pub transitions: Vec<Transition>,
}

/// Wraps [`Tracker`] for a polling loop: a failed poll keeps the previous snapshot and produces no transitions, so a
/// session that stays waiting through a CLI hiccup is not announced again (local-agent-tasks: 입력 대기 알림).
#[derive(Default)]
pub struct PollState {
    tracker: Tracker,
    last_good: Vec<AgentSession>,
}

impl PollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, result: Result<Vec<AgentSession>, CliError>) -> PollView {
        match result {
            Ok(sessions) => {
                let transitions = self.tracker.update(&sessions);
                self.last_good = sessions.clone();
                PollView { sessions, error: None, transitions }
            }
            Err(e) => PollView { sessions: self.last_good.clone(), error: Some(e), transitions: Vec::new() },
        }
    }
}

/// Records the first time each ended session was observed and drops ended sessions older than 24 hours.
pub fn visible_sessions(conn: &Connection, snapshot: Vec<AgentSession>, now_ms: i64) -> Result<Vec<AgentSession>, Error> {
    let mut out = Vec::with_capacity(snapshot.len());
    for s in snapshot {
        if !s.status.is_ended() {
            out.push(s);
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO session_observations(session_id, ended_observed_at) VALUES (?1, ?2)",
            params![s.key, now_ms],
        )?;
        let seen: Option<i64> = conn
            .query_row("SELECT ended_observed_at FROM session_observations WHERE session_id = ?1", [&s.key], |r| r.get(0))
            .optional()?;
        if now_ms - seen.unwrap_or(now_ms) < ENDED_VISIBLE_MS {
            out.push(s);
        }
    }
    Ok(out)
}

/// Group order for the task list: waiting first, then working, then everything else (local-agent-tasks spec).
pub fn group_rank(status: SessionStatus) -> u8 {
    match status {
        SessionStatus::WaitingInput => 0,
        SessionStatus::Working => 1,
        _ => 2,
    }
}

/// Notification body: session name and a fixed phrase only — never title, path, branch or `waitingFor`.
pub fn waiting_notification_body(name: &str, lang: crate::i18n::Lang) -> String {
    use crate::i18n::{Code, Params, msg, text};
    let clean = crate::text::display_safe(name, 64);
    let shown = if clean.trim().is_empty() { text(lang, Code::TraySessionFallback) } else { clean.trim() };
    msg(lang, Code::NotifyWaiting, &Params::new().text("name", shown, 64))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::claude_cli::SessionKind;

    fn s(key: &str, status: SessionStatus) -> AgentSession {
        AgentSession {
            key: key.into(),
            pid: Some(1),
            id: Some("ab12cd34".into()),
            session_id: Some(key.into()),
            name: format!("job-{key}"),
            cwd: "/w".into(),
            kind: SessionKind::Background,
            started_at: 0,
            status,
        }
    }

    #[test]
    fn transitions_after_priming() {
        let mut t = Tracker::new();
        assert!(t.update(&[s("a", SessionStatus::WaitingInput)]).is_empty(), "first snapshot only primes");
        let tr = t.update(&[s("a", SessionStatus::Working), s("b", SessionStatus::Working)]);
        assert_eq!(tr.len(), 2);
        let tr = t.update(&[s("a", SessionStatus::WaitingInput), s("b", SessionStatus::Working)]);
        assert_eq!(tr.len(), 1);
        assert!(tr[0].became_waiting());
        let done = Transition { key: "a".into(), name: "a".into(), from: Some(SessionStatus::Working), to: SessionStatus::Done };
        assert!(done.became_finished());
        assert!(!Transition { from: None, ..done.clone() }.became_finished(), "first sighting is not a finish");
        assert!(!Transition { from: Some(SessionStatus::Idle), ..done }.became_finished());
        // Staying in waiting does not repeat.
        assert!(t.update(&[s("a", SessionStatus::WaitingInput), s("b", SessionStatus::Working)]).is_empty());
    }

    #[test]
    fn errors_do_not_reset_state_or_repeat_notifications() {
        let mut p = PollState::new();
        // First poll fails: nothing is primed, nothing announced.
        let v = p.apply(Err(CliError::Timeout));
        assert!(v.sessions.is_empty() && v.transitions.is_empty());
        // First success primes only.
        assert!(p.apply(Ok(vec![s("a", SessionStatus::WaitingInput)])).transitions.is_empty());
        // A hiccup keeps the last snapshot.
        let v = p.apply(Err(CliError::Spawn));
        assert_eq!(v.sessions.len(), 1);
        assert!(v.transitions.is_empty());
        // Back to normal: still waiting, so no new notification.
        let v = p.apply(Ok(vec![s("a", SessionStatus::WaitingInput)]));
        assert!(v.transitions.iter().all(|t| !t.became_waiting()));
    }

    #[test]
    fn ended_sessions_visible_for_24h_from_first_observation() {
        let conn = crate::db::open_in_memory().unwrap();
        let t0 = 1_000_000_000_000;
        let snap = || vec![s("done", SessionStatus::Done), s("live", SessionStatus::Working)];
        assert_eq!(visible_sessions(&conn, snap(), t0).unwrap().len(), 2);
        assert_eq!(visible_sessions(&conn, snap(), t0 + 3_600_000).unwrap().len(), 2, "1h later still visible");
        assert_eq!(visible_sessions(&conn, snap(), t0 + ENDED_VISIBLE_MS - 1).unwrap().len(), 2, "just before 24h");
        let later = visible_sessions(&conn, snap(), t0 + ENDED_VISIBLE_MS).unwrap();
        assert_eq!(later.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), ["live"]);
    }

    #[test]
    fn notification_body_is_fixed_and_bounded() {
        use crate::i18n::Lang::{En, Ko};
        assert_eq!(waiting_notification_body("fix-tests", Ko), "fix-tests: 입력을 기다리고 있어요");
        assert_eq!(waiting_notification_body("\n\u{7}", Ko), "Claude 세션: 입력을 기다리고 있어요");
        assert!(waiting_notification_body(&"x".repeat(500), Ko).chars().count() < 90);
        assert_eq!(waiting_notification_body("fix-tests", En), "fix-tests: Waiting for your input");
        assert_eq!(waiting_notification_body("", En), "Claude session: Waiting for your input");
    }

    #[test]
    fn group_order() {
        let mut v = [SessionStatus::Done, SessionStatus::Working, SessionStatus::WaitingInput, SessionStatus::Idle];
        v.sort_by_key(|x| group_rank(*x));
        assert_eq!(v[0], SessionStatus::WaitingInput);
        assert_eq!(v[1], SessionStatus::Working);
    }
}
