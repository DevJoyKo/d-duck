//! Artifact list and the safe link lookup used by "open in browser" (D-052, D-053).

use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ahttps://claude\.ai/(?:code/)?artifact/[A-Za-z0-9-]{1,64}\z").unwrap_or_else(|_| unreachable!())
});

pub const CLOUD_ROUTINES_URL: &str = "https://claude.ai/code/routines";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRow {
    pub artifact_id: String,
    pub url: String,
    pub title: Option<String>,
    pub kind: String,
    pub seq: i64,
    pub first_seen_ts: i64,
    pub last_ts: i64,
    pub project_name: Option<String>,
}

/// Most recently published first; `query` filters titles (case-insensitive substring).
pub fn list(conn: &Connection, query: &str, limit: usize) -> Result<Vec<ArtifactRow>, Error> {
    let like = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(&format!(
        "SELECT a.artifact_id, a.url, a.title, a.kind, a.seq, a.first_seen_ts, a.last_ts, s.project_name
         FROM artifacts a LEFT JOIN sessions s ON s.session_id = a.session_id
         WHERE (?1 = '' OR IFNULL(a.title, '') LIKE ?2 ESCAPE '\\')
         ORDER BY a.last_ts DESC LIMIT {}",
        limit.min(1000)
    ))?;
    let rows = stmt.query_map(params![query, like], |r| {
        Ok(ArtifactRow {
            artifact_id: r.get(0)?,
            url: r.get(1)?,
            title: r.get(2)?,
            kind: r.get(3)?,
            seq: r.get(4)?,
            first_seen_ts: r.get(5)?,
            last_ts: r.get(6)?,
            project_name: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The stored link for a collected artifact, re-validated. `None` for unknown ids or anything not an artifact link.
pub fn safe_url(conn: &Connection, artifact_id: &str) -> Result<Option<String>, Error> {
    let url: Option<String> = conn.query_row("SELECT url FROM artifacts WHERE artifact_id = ?1", [artifact_id], |r| r.get(0)).optional()?;
    Ok(url.filter(|u| URL_RE.is_match(u) && u.starts_with("https://claude.ai/")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn only_collected_valid_links_are_returned() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO artifacts VALUES ('ok', 'https://claude.ai/artifact/ok', '대시보드', 'page', 2, 's', 1, 5),
                                          ('bad', 'https://evil.example/x', 'x', 'page', 1, 's', 1, 4)",
            [],
        )
        .unwrap();
        assert_eq!(safe_url(&conn, "ok").unwrap().as_deref(), Some("https://claude.ai/artifact/ok"));
        assert_eq!(safe_url(&conn, "bad").unwrap(), None, "tampered row is refused");
        assert_eq!(safe_url(&conn, "missing").unwrap(), None);
        assert_eq!(list(&conn, "대시", 10).unwrap().len(), 1);
        assert_eq!(list(&conn, "", 10).unwrap()[0].artifact_id, "ok");
        assert_eq!(list(&conn, "%", 10).unwrap().len(), 0, "wildcards are literal");
    }
}
