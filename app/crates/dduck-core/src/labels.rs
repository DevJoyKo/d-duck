//! Session titles the user typed in D.Duck (pencil icon). User input, so stored; never transcript text.
//! Independent of "세션 제목 저장" (which only governs Claude's own titles) and kept across a usage rebuild.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;
use crate::text::is_unsafe_char;

pub const MAX_CHARS: usize = 80;
const MAX_LABELS: i64 = 10_000;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    #[error("{}", crate::i18n::Fail::TitleFormat.str())]
    Invalid,
    #[error("{}", crate::i18n::Fail::SessionNotFound.str())]
    UnknownSession,
    #[error("{}", crate::i18n::Fail::SaveFailed.str())]
    Db,
}

pub fn get(conn: &Connection, session_id: &str) -> Result<Option<String>, Error> {
    Ok(conn.query_row("SELECT label FROM session_labels WHERE session_id = ?1", [session_id], |r| r.get(0)).optional()?)
}

/// Sets (or, with an empty/None label, clears) a session's title. Only for sessions D.Duck has ingested.
/// Returns true when an old label was replaced or removed (callers scrub then).
pub fn set(conn: &Connection, session_id: &str, label: Option<&str>, now_ms: i64) -> Result<bool, LabelError> {
    let known: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = ?1)", [session_id], |r| r.get(0))
        .map_err(|_| LabelError::Db)?;
    if !known {
        return Err(LabelError::UnknownSession);
    }
    let had = get(conn, session_id).map_err(|_| LabelError::Db)?.is_some();
    match label.map(str::trim).filter(|l| !l.is_empty()) {
        None => {
            conn.execute("DELETE FROM session_labels WHERE session_id = ?1", [session_id]).map_err(|_| LabelError::Db)?;
        }
        Some(l) => {
            if l.chars().count() > MAX_CHARS || l.chars().any(|c| c.is_control() || is_unsafe_char(c)) {
                return Err(LabelError::Invalid);
            }
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM session_labels", [], |r| r.get(0)).map_err(|_| LabelError::Db)?;
            if !had && n >= MAX_LABELS {
                return Err(LabelError::Invalid);
            }
            conn.execute(
                "INSERT INTO session_labels(session_id, label, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(session_id) DO UPDATE SET label = excluded.label, updated_at = excluded.updated_at",
                params![session_id, l, now_ms],
            )
            .map_err(|_| LabelError::Db)?;
        }
    }
    Ok(had)
}

/// Labels for many sessions (task board).
pub fn for_sessions(conn: &Connection, ids: &[String]) -> Result<std::collections::HashMap<String, String>, Error> {
    let mut out = std::collections::HashMap::new();
    for id in ids.iter().take(500) {
        if let Some(l) = get(conn, id)? {
            out.insert(id.clone(), l);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn labels_validate_and_survive_title_toggle() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO sessions(session_id, title) VALUES ('s1', 'claude title')", []).unwrap();
        assert_eq!(set(&conn, "nope", Some("x"), 1).unwrap_err(), LabelError::UnknownSession);
        assert_eq!(set(&conn, "s1", Some("a\u{202e}b"), 1).unwrap_err(), LabelError::Invalid);
        assert_eq!(set(&conn, "s1", Some(&"가".repeat(81)), 1).unwrap_err(), LabelError::Invalid);
        assert!(!set(&conn, "s1", Some("  노트북 정리 "), 1).unwrap());
        assert_eq!(get(&conn, "s1").unwrap().as_deref(), Some("노트북 정리"));
        // "세션 제목 저장" off clears Claude titles only.
        conn.execute("UPDATE sessions SET title = NULL", []).unwrap();
        assert_eq!(get(&conn, "s1").unwrap().as_deref(), Some("노트북 정리"));
        assert!(set(&conn, "s1", Some(""), 2).unwrap(), "clearing reports the old label");
        assert_eq!(get(&conn, "s1").unwrap(), None);
    }
}
