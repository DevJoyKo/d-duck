//! SQLite storage (D3). Holds only numbers, identifiers, names, timestamps and paths — never transcript text.

use std::path::Path;

use rusqlite::Connection;

use crate::Error;

#[cfg(test)]
const SCHEMA_VERSION: i64 = 17;

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS ingest_files (
    path          TEXT PRIMARY KEY,
    inode         INTEGER NOT NULL DEFAULT 0,
    size          INTEGER NOT NULL DEFAULT 0,
    offset        INTEGER NOT NULL DEFAULT 0,
    mtime         INTEGER NOT NULL DEFAULT 0,
    skipped_lines INTEGER NOT NULL DEFAULT 0,
    line_no       INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sessions (
    session_id   TEXT PRIMARY KEY,
    project_path TEXT,
    project_name TEXT,
    git_branch   TEXT,
    title        TEXT,
    version      TEXT,
    entrypoint   TEXT,
    first_ts     INTEGER NOT NULL DEFAULT 0,
    last_ts      INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS cost_segments (
    session_id     TEXT NOT NULL,
    start_time     INTEGER NOT NULL,
    total_cost_usd REAL NOT NULL,
    PRIMARY KEY (session_id, start_time)
);
CREATE TABLE IF NOT EXISTS api_calls (
    message_id     TEXT NOT NULL,
    request_id     TEXT NOT NULL DEFAULT '',
    session_id     TEXT NOT NULL,
    agent_id       TEXT,
    ts             INTEGER NOT NULL,
    model          TEXT NOT NULL,
    input          INTEGER NOT NULL DEFAULT 0,
    output         INTEGER NOT NULL DEFAULT 0,
    thinking       INTEGER NOT NULL DEFAULT 0,
    cache_write_5m INTEGER NOT NULL DEFAULT 0,
    cache_write_1h INTEGER NOT NULL DEFAULT 0,
    cache_read     INTEGER NOT NULL DEFAULT 0,
    web_search     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (message_id, request_id)
);
CREATE INDEX IF NOT EXISTS api_calls_ts ON api_calls(ts);
CREATE INDEX IF NOT EXISTS api_calls_session ON api_calls(session_id);
CREATE INDEX IF NOT EXISTS api_calls_model ON api_calls(model);
CREATE TABLE IF NOT EXISTS tool_calls (
    tool_use_id   TEXT PRIMARY KEY,
    message_id    TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    agent_id      TEXT,
    ts            INTEGER NOT NULL,
    tool_name     TEXT NOT NULL,
    mcp_server    TEXT,
    skill_name    TEXT,
    subagent_type TEXT
);
CREATE INDEX IF NOT EXISTS tool_calls_ts ON tool_calls(ts);
CREATE INDEX IF NOT EXISTS tool_calls_session ON tool_calls(session_id);
CREATE TABLE IF NOT EXISTS command_invocations (
    record_uuid TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    ts          INTEGER NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('builtin', 'skill')),
    via         TEXT NOT NULL CHECK (via IN ('tool', 'slash'))
);
CREATE INDEX IF NOT EXISTS command_invocations_ts ON command_invocations(ts);
CREATE TABLE IF NOT EXISTS subagents (
    agent_id   TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    agent_type TEXT
);
CREATE TABLE IF NOT EXISTS session_observations (
    session_id        TEXT PRIMARY KEY,
    ended_observed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Opens (creating if needed) the D.Duck database with owner-only permissions (D-023).
/// The app was called DevDeck before; its database file keeps users' history (D-084). Renamed once, only when the
/// new file does not exist yet. On failure the old file is used as is, so nothing is lost.
pub const LEGACY_FILE: &str = "devdeck.db";

/// Picks the database path inside `dir`: `file`, after moving a legacy `devdeck.db` (+ `-wal`, `-shm`) into place.
pub fn adopt_legacy(dir: &Path, file: &str) -> std::path::PathBuf {
    let new = dir.join(file);
    let old = dir.join(LEGACY_FILE);
    if new.exists() || !old.exists() {
        return new;
    }
    // Main file first, then its WAL/SHM; on any failure everything moved so far goes back, so the old database is
    // always opened together with its own WAL (QA MEDIUM).
    if std::fs::rename(&old, &new).is_err() {
        return old;
    }
    let mut moved: Vec<(std::path::PathBuf, std::path::PathBuf)> = vec![(old.clone(), new.clone())];
    for suffix in ["-wal", "-shm"] {
        let from = dir.join(format!("{LEGACY_FILE}{suffix}"));
        let to = dir.join(format!("{file}{suffix}"));
        if std::fs::symlink_metadata(&from).is_err() {
            continue;
        }
        if std::fs::rename(&from, &to).is_err() {
            for (f, t) in moved.iter().rev() {
                let _ = std::fs::rename(t, f);
            }
            return old;
        }
        moved.push((from, to));
    }
    new
}

pub fn open(path: &Path) -> Result<Connection, Error> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        restrict(dir, 0o700)?;
    }
    let conn = Connection::open(path)?;
    restrict(path, 0o600)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // Deleted rows (e.g. titles after the user turns them off) are zeroed on disk, not left in free pages.
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(conn)
}

/// In-memory database for tests and tools.
pub fn open_in_memory() -> Result<Connection, Error> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

/// Version 2: artifacts, routines and their runs (artifacts-and-routines change).
const SCHEMA_V2: &str = r#"
CREATE TABLE IF NOT EXISTS artifacts (
    artifact_id   TEXT PRIMARY KEY,
    url           TEXT NOT NULL,
    title         TEXT,
    kind          TEXT NOT NULL CHECK (kind IN ('page', 'typed')),
    seq           INTEGER NOT NULL DEFAULT 1,
    session_id    TEXT NOT NULL,
    first_seen_ts INTEGER NOT NULL,
    last_ts       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS artifacts_last ON artifacts(last_ts);
CREATE TABLE IF NOT EXISTS routines (
    id               INTEGER PRIMARY KEY,
    name             TEXT NOT NULL,
    cwd              TEXT NOT NULL,
    prompt           TEXT NOT NULL,
    permission_mode  TEXT NOT NULL CHECK (permission_mode IN ('manual', 'plan', 'acceptEdits', 'dontAsk')),
    schedule_kind    TEXT NOT NULL CHECK (schedule_kind IN ('daily', 'weekly', 'interval')),
    time_hm          TEXT,
    weekdays         INTEGER,
    interval_hours   INTEGER,
    enabled          INTEGER NOT NULL DEFAULT 1,
    created_at       INTEGER NOT NULL,
    anchor_ms        INTEGER NOT NULL,
    next_run_at      INTEGER,
    approved_digest  BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS routine_runs (
    id            INTEGER PRIMARY KEY,
    routine_id    INTEGER NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
    scheduled_for INTEGER NOT NULL,
    started_at    INTEGER,
    ended_at      INTEGER,
    last_seen_at  INTEGER,
    status        TEXT NOT NULL CHECK (status IN ('starting','start_failed','running','waiting','done','failed','stopped','missed','skipped','lost')),
    bg_id         TEXT,
    error         TEXT
);
CREATE INDEX IF NOT EXISTS routine_runs_routine ON routine_runs(routine_id, scheduled_for);
CREATE TABLE IF NOT EXISTS session_labels (
    session_id TEXT PRIMARY KEY,
    label      TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
"#;

/// v3: persona per D.Duck task and the `--agent` setting seen in a transcript (D-055).
const SCHEMA_V3: &str = r#"
CREATE TABLE IF NOT EXISTS task_meta (
    bg_id      TEXT PRIMARY KEY,
    agent      TEXT,
    task_type  TEXT,
    created_at INTEGER NOT NULL
);
ALTER TABLE sessions ADD COLUMN agent_setting TEXT;
"#;

/// v4: "리뷰함" on D.Duck tasks (AG-14).
const SCHEMA_V4: &str = "ALTER TABLE task_meta ADD COLUMN reviewed_at INTEGER;";

/// v5: what the user typed into the D.Duck task composer (D-069). User input, not transcript text.
const SCHEMA_V5: &str = "ALTER TABLE task_meta ADD COLUMN name TEXT;
ALTER TABLE task_meta ADD COLUMN cwd TEXT;
ALTER TABLE task_meta ADD COLUMN prompt TEXT;
ALTER TABLE task_meta ADD COLUMN brief_json TEXT;
ALTER TABLE task_meta ADD COLUMN effort TEXT;
ALTER TABLE task_meta ADD COLUMN worktree INTEGER NOT NULL DEFAULT 0;";

/// v6: failed tool results and context compactions, ids and times only (usage-insights).
const SCHEMA_V6: &str = r#"
CREATE TABLE IF NOT EXISTS tool_errors (
    tool_use_id TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    ts          INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS compactions (
    record_uuid TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    ts          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS compactions_session ON compactions(session_id);
"#;

/// v7: task report id (decision-report, D-108). Ids only, never report content. `permission_mode` is shown in the
/// report's "위임 내용"; routine runs keep their own report id (the newest 20 per routine).
const SCHEMA_V7: &str = "ALTER TABLE task_meta ADD COLUMN report_id TEXT;
ALTER TABLE task_meta ADD COLUMN permission_mode TEXT;
ALTER TABLE routine_runs ADD COLUMN report_id TEXT;";

/// v8: where a task's draft came from (`insight-local`/`insight-claude`), restored on "다시 맡기기" (brief-grilling
/// security review MEDIUM).
const SCHEMA_V8: &str = "ALTER TABLE task_meta ADD COLUMN origin TEXT;";

/// v9: how many loadout skills a start left out because their file changed (agent-loadout). A count, no names.
const SCHEMA_V9: &str = "ALTER TABLE task_meta ADD COLUMN loadout_dropped INTEGER NOT NULL DEFAULT 0;";

/// v10: usage-limit stops and their resume schedule (usage-limit-resume, D-139·D-141). Ids, times, fixed codes only —
/// never message text or CLI output.
const SCHEMA_V10: &str = r#"
CREATE TABLE IF NOT EXISTS limit_hits (
    record_uuid TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    is_subagent INTEGER NOT NULL DEFAULT 0,
    ts          INTEGER NOT NULL,
    resets_at   INTEGER,
    kind        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS limit_hits_session ON limit_hits(session_id);
CREATE TABLE IF NOT EXISTS limit_resumes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL,
    incident        TEXT NOT NULL,
    resets_at       INTEGER,
    hit_ts          INTEGER NOT NULL,
    origin_bg_id    TEXT,
    bg_id           TEXT,
    permission_mode TEXT,
    task_type       TEXT,
    due_at          INTEGER,
    state           TEXT NOT NULL,
    reason          TEXT,
    notified        INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    UNIQUE(session_id, incident)
);
ALTER TABLE task_meta ADD COLUMN session_id TEXT;
ALTER TABLE task_meta ADD COLUMN limit_opt_out INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_meta ADD COLUMN limit_auto_ok INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_meta ADD COLUMN origin_bg_id TEXT;
ALTER TABLE task_meta ADD COLUMN remote INTEGER;
ALTER TABLE limit_hits ADD COLUMN seen INTEGER NOT NULL DEFAULT 0;
"#;

/// delivery-guard (D-143): the scope a task started with and the default-branch head for the post-check.
const SCHEMA_V11: &str = "ALTER TABLE task_meta ADD COLUMN delivery_guard TEXT;
ALTER TABLE task_meta ADD COLUMN default_branch TEXT;
ALTER TABLE task_meta ADD COLUMN default_head TEXT;
ALTER TABLE task_meta ADD COLUMN delivery_alert INTEGER;";

/// task-lifecycle (D-152): "정리 끝" time. NULL = not wrapped up (every older row).
const SCHEMA_V12: &str = "ALTER TABLE task_meta ADD COLUMN wrapped_at INTEGER;";

/// model-select (D-154): the model layer and the values a start passed (`--model=`, inline role `model`). NULL = no
/// record (older rows) and no model argument.
const SCHEMA_V13: &str = "ALTER TABLE task_meta ADD COLUMN model_layer TEXT;
ALTER TABLE task_meta ADD COLUMN model_arg TEXT;
ALTER TABLE task_meta ADD COLUMN role_model TEXT;";

/// app-language (D-171): the language a task started in (`ko`/`en`), reused by its resume and rework. NULL = older
/// rows, which follow the current app language.
const SCHEMA_V14: &str = "ALTER TABLE task_meta ADD COLUMN lang TEXT;";

/// task-lifecycle 5.2: how the task last ended while the CLI still listed it (`done`/`failed`/`stopped`/`unknown`),
/// so a record card keeps it after the task leaves the listing. NULL = never seen ended (older rows, still running).
const SCHEMA_V15: &str = "ALTER TABLE task_meta ADD COLUMN ended_status TEXT;";

/// review-routine-runs: a routine run with a report is a review target like a task record, so it keeps its own
/// 리뷰했어요 and 정리 끝 times. The index serves the review rows' per-bg lookups.
const SCHEMA_V16: &str = "ALTER TABLE routine_runs ADD COLUMN reviewed_at INTEGER;
ALTER TABLE routine_runs ADD COLUMN wrapped_at INTEGER;
CREATE INDEX IF NOT EXISTS routine_runs_bg ON routine_runs(bg_id);";

/// news-tab (D-204): checked Claude Code feature explainers per language and which ones were opened. Only the checked
/// fields are kept: no fetched page, prompt or raw answer.
const SCHEMA_V17: &str = "CREATE TABLE IF NOT EXISTS news_items (
    id TEXT NOT NULL, lang TEXT NOT NULL, title TEXT NOT NULL, version TEXT, date TEXT NOT NULL,
    category TEXT NOT NULL, summary TEXT NOT NULL, what TEXT NOT NULL, when_to_use TEXT NOT NULL,
    best_practice TEXT NOT NULL, source_url TEXT NOT NULL, first_seen_ms INTEGER NOT NULL,
    PRIMARY KEY (id, lang)
);
CREATE TABLE IF NOT EXISTS news_seen (id TEXT NOT NULL, lang TEXT NOT NULL, seen_ms INTEGER NOT NULL, PRIMARY KEY (id, lang));";

pub const SETTING_ARTIFACTS_BACKFILL: &str = "artifacts_backfill";

/// Staged migrations, each in one transaction with its version bump. Upgrading an existing v1 DB marks a one-time
/// full re-read so artifacts in already-ingested files are found (design R1b); a fresh DB needs none.
fn migrate(conn: &Connection) -> Result<(), Error> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V1} PRAGMA user_version = 1; COMMIT;"))?;
    }
    if version < 2 {
        let backfill = if version == 0 { "done" } else { "pending" };
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {SCHEMA_V2}
             INSERT INTO settings(key, value) VALUES ('{SETTING_ARTIFACTS_BACKFILL}', '{backfill}')
               ON CONFLICT(key) DO UPDATE SET value = excluded.value;
             PRAGMA user_version = 2; COMMIT;"
        ))?;
    }
    if version < 3 {
        // Existing DBs re-read every file once (same flag as the artifact backfill) so `agentSetting` records near
        // the start of already-ingested transcripts are picked up.
        let backfill = if version == 0 { "done" } else { "pending" };
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {SCHEMA_V3}
             INSERT INTO settings(key, value) VALUES ('{SETTING_ARTIFACTS_BACKFILL}', '{backfill}')
               ON CONFLICT(key) DO UPDATE SET value = excluded.value;
             PRAGMA user_version = 3; COMMIT;"
        ))?;
    }
    if version < 4 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V4} PRAGMA user_version = 4; COMMIT;"))?;
    }
    if version < 5 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V5} PRAGMA user_version = 5; COMMIT;"))?;
    }
    if version < 6 {
        // Existing DBs re-read every file once so errors and compactions in old transcripts are counted.
        let backfill = if version == 0 { "done" } else { "pending" };
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {SCHEMA_V6}
             INSERT INTO settings(key, value) VALUES ('{SETTING_ARTIFACTS_BACKFILL}', '{backfill}')
               ON CONFLICT(key) DO UPDATE SET value = excluded.value;
             PRAGMA user_version = 6; COMMIT;"
        ))?;
    }
    if version < 7 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V7} PRAGMA user_version = 7; COMMIT;"))?;
    }
    if version < 8 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V8} PRAGMA user_version = 8; COMMIT;"))?;
    }
    if version < 9 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V9} PRAGMA user_version = 9; COMMIT;"))?;
    }
    if version < 10 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V10} PRAGMA user_version = 10; COMMIT;"))?;
    }
    if version < 11 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V11} PRAGMA user_version = 11; COMMIT;"))?;
    }
    if version < 12 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V12} PRAGMA user_version = 12; COMMIT;"))?;
    }
    if version < 13 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V13} PRAGMA user_version = 13; COMMIT;"))?;
    }
    if version < 14 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V14} PRAGMA user_version = 14; COMMIT;"))?;
    }
    if version < 15 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V15} PRAGMA user_version = 15; COMMIT;"))?;
    }
    if version < 16 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V16} PRAGMA user_version = 16; COMMIT;"))?;
    }
    if version < 17 {
        conn.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA_V17} PRAGMA user_version = 17; COMMIT;"))?;
    }
    ensure_columns(conn)?;
    Ok(())
}

/// Columns added without a version number (model-select design 11): each is added only when missing, so this step
/// never collides with a numbered migration landing from another branch.
const UNVERSIONED_COLUMNS: [(&str, &str, &str); 2] = [("task_meta", "model_requested", "TEXT"), ("task_meta", "model_fallback", "TEXT")];

fn ensure_columns(conn: &Connection) -> Result<(), Error> {
    for (table, column, kind) in UNVERSIONED_COLUMNS {
        let has: bool = conn.query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"), [column], |r| r.get::<_, i64>(0))? > 0;
        if !has {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind};"))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> Result<(), Error> {
    // Windows: the per-user app data directory is already private to the user.
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn creates_all_tables_and_indexes() {
        let conn = open_in_memory().unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','index') AND name NOT LIKE 'sqlite_%'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for t in [
            "ingest_files", "sessions", "cost_segments", "api_calls", "tool_calls", "command_invocations",
            "subagents", "session_observations", "settings", "api_calls_ts", "tool_calls_ts", "artifacts", "routines",
            "routine_runs", "tool_errors", "compactions", "news_items", "news_seen",
        ] {
            assert!(names.iter().any(|n| n == t), "missing {t}");
        }
    }

    #[test]
    fn upgrading_v1_keeps_data_and_marks_backfill() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO sessions(session_id) VALUES ('s1')", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "v1 data kept");
        let flag: String = conn.query_row("SELECT value FROM settings WHERE key = 'artifacts_backfill'", [], |r| r.get(0)).unwrap();
        assert_eq!(flag, "pending");
        let fresh = open_in_memory().unwrap();
        let flag: String = fresh.query_row("SELECT value FROM settings WHERE key = 'artifacts_backfill'", [], |r| r.get(0)).unwrap();
        assert_eq!(flag, "done", "a new DB has nothing to backfill");
    }

    #[test]
    fn upgrading_v6_adds_report_columns_and_keeps_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6}")).unwrap();
        conn.pragma_update(None, "user_version", 6).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at) VALUES ('ab12cd34', 1)", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let row: (String, Option<String>, Option<String>) =
            conn.query_row("SELECT bg_id, report_id, permission_mode FROM task_meta", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(row, ("ab12cd34".into(), None, None));
        assert!(conn.prepare("SELECT report_id FROM routine_runs").is_ok());
    }

    #[test]
    fn upgrading_v15_adds_routine_review_columns_and_keeps_runs() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9} {SCHEMA_V10} {SCHEMA_V11} {SCHEMA_V12} {SCHEMA_V13} {SCHEMA_V14} {SCHEMA_V15}"
        ))
        .unwrap();
        conn.pragma_update(None, "user_version", 15).unwrap();
        conn.execute(
            "INSERT INTO routines(id, name, cwd, prompt, permission_mode, schedule_kind, created_at, anchor_ms, approved_digest) VALUES (1, 'r', '/w', 'p', 'manual', 'daily', 0, 0, x'00')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO routine_runs(routine_id, scheduled_for, status, bg_id, report_id) VALUES (1, 5, 'done', 'ab12cd34', ?1)", [format!("{:032x}", 7)]).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let row: (String, Option<String>, Option<i64>, Option<i64>) =
            conn.query_row("SELECT bg_id, report_id, reviewed_at, wrapped_at FROM routine_runs", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
        assert_eq!(row, ("ab12cd34".into(), Some(format!("{:032x}", 7)), None, None), "runs kept, nothing marked reviewed for the user");
        let idx: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'routine_runs_bg'", [], |r| r.get(0)).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn upgrading_v7_adds_origin_and_keeps_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7}")).unwrap();
        conn.pragma_update(None, "user_version", 7).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, permission_mode) VALUES ('ab12cd34', 1, 'plan')", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let row: (Option<String>, Option<String>, i64) = conn.query_row("SELECT permission_mode, origin, loadout_dropped FROM task_meta", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(row, (Some("plan".into()), None, 0));
    }

    #[test]
    fn upgrading_v9_adds_limit_tables_and_keeps_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9}")).unwrap();
        conn.pragma_update(None, "user_version", 9).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, permission_mode) VALUES ('ab12cd34', 1, 'plan')", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let row: (Option<String>, i64, i64, Option<String>) = conn
            .query_row("SELECT session_id, limit_opt_out, limit_auto_ok, origin_bg_id FROM task_meta", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap();
        assert_eq!(row, (None, 0, 0, None));
        assert!(conn.prepare("SELECT record_uuid, resets_at FROM limit_hits").is_ok());
        assert!(conn.prepare("SELECT incident, state, reason, notified FROM limit_resumes").is_ok());
    }

    #[test]
    fn upgrading_v10_adds_the_delivery_columns_and_keeps_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9} {SCHEMA_V10}")).unwrap();
        conn.pragma_update(None, "user_version", 10).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at) VALUES ('ab12cd34', 1)", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let row: (Option<String>, Option<String>, Option<String>, Option<i64>) = conn
            .query_row("SELECT delivery_guard, default_branch, default_head, delivery_alert FROM task_meta", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap();
        assert_eq!(row, (None, None, None, None));
    }

    #[test]
    fn upgrading_v11_adds_wrapped_at_null_for_old_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9} {SCHEMA_V10} {SCHEMA_V11}")).unwrap();
        conn.pragma_update(None, "user_version", 11).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, reviewed_at) VALUES ('ab12cd34', 1, 2)", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let w: Option<i64> = conn.query_row("SELECT wrapped_at FROM task_meta", [], |r| r.get(0)).unwrap();
        assert_eq!(w, None);
    }

    #[test]
    fn upgrading_v14_adds_ended_status_null_for_old_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{SCHEMA_V1} {SCHEMA_V2} {SCHEMA_V3} {SCHEMA_V4} {SCHEMA_V5} {SCHEMA_V6} {SCHEMA_V7} {SCHEMA_V8} {SCHEMA_V9} {SCHEMA_V10} {SCHEMA_V11} {SCHEMA_V12} {SCHEMA_V13} {SCHEMA_V14}")).unwrap();
        conn.pragma_update(None, "user_version", 14).unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at) VALUES ('ab12cd34', 1)", []).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let e: Option<String> = conn.query_row("SELECT ended_status FROM task_meta", [], |r| r.get(0)).unwrap();
        assert_eq!(e, None);
    }

    #[test]
    fn upgrading_v16_adds_the_news_tables() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("DROP TABLE news_items; DROP TABLE news_seen;").unwrap();
        conn.pragma_update(None, "user_version", 16).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, 17);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM news_items", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn legacy_database_is_adopted_once() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("devdeck.db"), b"old").unwrap();
        std::fs::write(d.join("devdeck.db-wal"), b"wal").unwrap();
        let p = adopt_legacy(d, "dduck.db");
        assert_eq!(p, d.join("dduck.db"));
        assert_eq!(std::fs::read(d.join("dduck.db")).unwrap(), b"old");
        assert_eq!(std::fs::read(d.join("dduck.db-wal")).unwrap(), b"wal");
        assert!(!d.join("devdeck.db").exists());
        // A second legacy file appearing later is left alone once the new one exists.
        std::fs::write(d.join("devdeck.db"), b"other").unwrap();
        assert_eq!(adopt_legacy(d, "dduck.db"), d.join("dduck.db"));
        assert_eq!(std::fs::read(d.join("dduck.db")).unwrap(), b"old");
        // A sidecar that cannot be moved rolls everything back (a directory in the way makes rename fail).
        let r = tempfile::tempdir().unwrap();
        let d = r.path();
        std::fs::write(d.join("devdeck.db"), b"old").unwrap();
        std::fs::write(d.join("devdeck.db-wal"), b"wal").unwrap();
        std::fs::create_dir(d.join("dduck.db-wal")).unwrap();
        std::fs::write(d.join("dduck.db-wal").join("x"), b"").unwrap();
        assert_eq!(adopt_legacy(d, "dduck.db"), d.join("devdeck.db"));
        assert_eq!(std::fs::read(d.join("devdeck.db")).unwrap(), b"old");
        assert_eq!(std::fs::read(d.join("devdeck.db-wal")).unwrap(), b"wal");
        assert!(!d.join("dduck.db").exists());
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(adopt_legacy(empty.path(), "dduck.db"), empty.path().join("dduck.db"));
    }

    #[cfg(unix)]
    #[test]
    fn file_permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("appdata");
        let db = data.join("dduck.db");
        open(&db).unwrap();
        assert_eq!(std::fs::metadata(&data).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&db).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
