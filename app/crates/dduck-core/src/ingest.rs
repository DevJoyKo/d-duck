//! Incremental transcript ingestion (D4).
//!
//! Each file is read from its stored byte offset. Only complete (newline-terminated) lines are processed, lines
//! over [`MAX_LINE_BYTES`] are skipped without being buffered, and a file that shrank is re-read from the start.
//! All events of one pass over a file are applied in a single transaction, and the offset is committed with them,
//! so a crash mid-file simply re-reads the same lines; every write is an idempotent upsert.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Error;
use crate::commands;
use crate::discover::{TranscriptFile, TranscriptKind};
use crate::parse::{self, Event};

pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
const MAX_META_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct IngestOptions {
    /// Store Claude-generated session titles (D-023). When false, titles are neither stored nor kept.
    pub store_titles: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self { store_titles: true }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FileStats {
    pub lines: u64,
    pub skipped: u64,
    pub events: u64,
    pub restarted: bool,
}

/// A skipped line, reported by position and category only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipReport {
    pub line_no: u64,
    pub category: &'static str,
}

pub struct Ingestor<'a> {
    conn: &'a mut Connection,
    options: IngestOptions,
}

struct FileState {
    inode: i64,
    offset: u64,
    line_no: u64,
    skipped: u64,
}

impl<'a> Ingestor<'a> {
    pub fn new(conn: &'a mut Connection, options: IngestOptions) -> Self {
        Self { conn, options }
    }

    pub fn ingest_all(&mut self, files: &[TranscriptFile]) -> Result<FileStats, Error> {
        let mut total = FileStats::default();
        for f in files {
            let s = self.ingest_file(f, |_| {})?;
            total.lines += s.lines;
            total.skipped += s.skipped;
            total.events += s.events;
        }
        Ok(total)
    }

    /// Ingests new lines of one file. `on_skip` receives the position and category of each skipped line.
    pub fn ingest_file(&mut self, file: &TranscriptFile, mut on_skip: impl FnMut(SkipReport)) -> Result<FileStats, Error> {
        let key = file.rel_path.clone();
        let handle = open_regular(&file.path)?;
        let meta = handle.metadata()?;
        let size = meta.len();
        let inode = inode_of(&meta);
        let mtime = mtime_of(&meta);

        // Fast path without the write lock: nothing changed since the last pass.
        let unchanged: Option<bool> = self
            .conn
            .query_row(
                "SELECT size = ?2 AND inode = ?3 AND mtime = ?4 AND offset = ?2 FROM ingest_files WHERE path = ?1",
                params![key, size as i64, inode, mtime],
                |r| r.get(0),
            )
            .optional()?;
        if unchanged == Some(true) {
            return Ok(FileStats::default());
        }

        // IMMEDIATE: take the write lock before reading the stored offset, so two passes over the same file
        // (watcher + rescan) cannot both start from the same offset.
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prev: Option<FileState> = tx
            .query_row(
                "SELECT inode, offset, skipped_lines, line_no FROM ingest_files WHERE path = ?1",
                params![key],
                |r| {
                    Ok(FileState {
                        inode: r.get(0)?,
                        offset: r.get::<_, i64>(1)?.max(0) as u64,
                        skipped: r.get::<_, i64>(2)?.max(0) as u64,
                        line_no: r.get::<_, i64>(3)?.max(0) as u64,
                    })
                },
            )
            .optional()?;

        let mut stats = FileStats::default();
        let had_row = prev.is_some();
        let mut state = match prev {
            Some(p) if p.offset <= size && p.inode == inode => p,
            Some(_) => {
                stats.restarted = true;
                FileState { inode, offset: 0, line_no: 0, skipped: 0 }
            }
            None => FileState { inode, offset: 0, line_no: 0, skipped: 0 },
        };
        if had_row && state.offset == size && !stats.restarted {
            return Ok(stats); // nothing new; dropping `tx` rolls back the empty transaction
        }

        let mut reader = BufReader::with_capacity(64 * 1024, handle);
        reader.seek(SeekFrom::Start(state.offset))?;
        let mut events: Vec<Event> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            match read_line_capped(&mut reader, &mut buf, MAX_LINE_BYTES)? {
                LineRead::Eof => break,
                LineRead::Complete(consumed) => {
                    state.offset += consumed;
                    state.line_no += 1;
                    stats.lines += 1;
                    let text = String::from_utf8_lossy(&buf);
                    let text = text.trim_end_matches(['\n', '\r']);
                    if text.trim().is_empty() {
                        continue;
                    }
                    match parse::parse_line(text, &file.context) {
                        Ok(evs) => events.extend(evs),
                        Err(e) => {
                            state.skipped += 1;
                            stats.skipped += 1;
                            on_skip(SkipReport { line_no: state.line_no, category: e.category() });
                        }
                    }
                }
                LineRead::TooLong(consumed) => {
                    state.offset += consumed;
                    state.line_no += 1;
                    stats.lines += 1;
                    state.skipped += 1;
                    stats.skipped += 1;
                    on_skip(SkipReport { line_no: state.line_no, category: "line_too_long" });
                }
            }
        }
        stats.events = events.len() as u64;

        let agent_type = match (&file.kind, &file.meta_path) {
            (TranscriptKind::Subagent, Some(meta)) => read_agent_type(meta),
            _ => None,
        };

        if stats.restarted {
            // Rows are idempotent upserts, so re-reading the file reproduces them; only the counters reset.
            tx.execute("DELETE FROM ingest_files WHERE path = ?1", params![key])?;
        }
        let fallback_name = project_name_from_dir(&file.project_dir);
        // The setting is read inside the transaction so a concurrent "titles off" + clear cannot be undone.
        let store_titles = self.options.store_titles && titles_enabled(&tx)?;
        apply_events(&tx, &events, &fallback_name, store_titles)?;
        if let Some(agent_id) = &file.context.agent_id {
            tx.execute(
                "INSERT INTO subagents(agent_id, session_id, agent_type) VALUES (?1, ?2, ?3)
                 ON CONFLICT(agent_id) DO UPDATE SET agent_type = COALESCE(excluded.agent_type, subagents.agent_type)",
                params![agent_id, file.context.session_id, agent_type],
            )?;
        }
        tx.execute(
            "INSERT INTO ingest_files(path, inode, size, offset, mtime, skipped_lines, line_no)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET inode = excluded.inode, size = excluded.size, offset = excluded.offset,
               mtime = excluded.mtime, skipped_lines = excluded.skipped_lines, line_no = excluded.line_no",
            params![key, state.inode, size as i64, state.offset as i64, mtime, state.skipped as i64, state.line_no as i64],
        )?;
        tx.commit()?;
        Ok(stats)
    }
}

pub const SETTING_STORE_TITLES: &str = "store_titles";

fn titles_enabled(tx: &Transaction<'_>) -> Result<bool, Error> {
    let v: Option<String> =
        tx.query_row("SELECT value FROM settings WHERE key = ?1", params![SETTING_STORE_TITLES], |r| r.get(0)).optional()?;
    Ok(v.as_deref() != Some("0"))
}

/// Switches title storage on or off. Turning it off deletes every stored title and scrubs the freed pages
/// (secure_delete + WAL checkpoint + VACUUM), so the text does not linger in the database files (D-023).
pub fn set_store_titles(conn: &Connection, enabled: bool) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SETTING_STORE_TITLES, if enabled { "1" } else { "0" }],
    )?;
    if !enabled {
        clear_titles(conn)?;
    }
    Ok(())
}

/// Deletes all stored session titles and scrubs them from disk.
pub fn clear_titles(conn: &Connection) -> Result<usize, Error> {
    let n = conn.execute("UPDATE sessions SET title = NULL WHERE title IS NOT NULL", [])?
        + conn.execute("UPDATE artifacts SET title = NULL WHERE title IS NOT NULL", [])?;
    scrub(conn)?;
    Ok(n)
}

/// Removes freed pages from the database and its WAL (secure_delete zeroes them; checkpoint + VACUUM drops the
/// WAL copies). Used after deleting anything the user typed or asked to forget.
pub fn scrub(conn: &Connection) -> Result<(), Error> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    conn.execute_batch("VACUUM")?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
}

/// Opens a transcript without following a symlink planted after discovery, without blocking on a FIFO, and only if
/// it is still a single-link regular file.
fn open_regular(path: &Path) -> Result<File, Error> {
    let file = crate::platform::open_nofollow(path)?;
    let meta = file.metadata()?;
    let single_link = crate::platform::handle_ids(&file, &meta).2 == 1;
    if !meta.is_file() || !single_link {
        return Err(Error::Config("transcript is not a single-link regular file"));
    }
    Ok(file)
}

fn apply_events(tx: &Transaction<'_>, events: &[Event], fallback_name: &str, store_titles: bool) -> Result<(), Error> {
    for ev in events {
        match ev {
            Event::Session(m) => {
                let project_name = m.project_path.as_ref().and_then(|p| basename(p.as_str())).unwrap_or(fallback_name);
                tx.execute(
                    "INSERT INTO sessions(session_id, project_path, project_name, git_branch, version, entrypoint, first_ts, last_ts)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                     ON CONFLICT(session_id) DO UPDATE SET
                       project_path = COALESCE(excluded.project_path, sessions.project_path),
                       project_name = CASE WHEN excluded.project_path IS NOT NULL THEN excluded.project_name
                                           ELSE COALESCE(sessions.project_name, excluded.project_name) END,
                       git_branch   = COALESCE(excluded.git_branch, sessions.git_branch),
                       version      = COALESCE(excluded.version, sessions.version),
                       entrypoint   = COALESCE(sessions.entrypoint, excluded.entrypoint),
                       first_ts     = CASE WHEN sessions.first_ts = 0 THEN excluded.first_ts
                                           WHEN excluded.first_ts = 0 THEN sessions.first_ts
                                           ELSE MIN(sessions.first_ts, excluded.first_ts) END,
                       last_ts      = MAX(sessions.last_ts, excluded.last_ts)",
                    params![
                        m.session_id,
                        m.project_path.as_ref().map(|p| p.as_str()),
                        project_name,
                        m.git_branch.as_ref().map(|b| b.as_str()),
                        m.version,
                        m.entrypoint,
                        m.ts
                    ],
                )?;
            }
            Event::Api(a) => {
                tx.execute(
                    "INSERT INTO api_calls(message_id, request_id, session_id, agent_id, ts, model, input, output, thinking,
                                           cache_write_5m, cache_write_1h, cache_read, web_search)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT(message_id, request_id) DO UPDATE SET
                       ts = CASE WHEN api_calls.ts = 0 THEN excluded.ts ELSE MIN(api_calls.ts, excluded.ts) END,
                       input = MAX(api_calls.input, excluded.input),
                       output = MAX(api_calls.output, excluded.output),
                       thinking = MAX(api_calls.thinking, excluded.thinking),
                       cache_write_5m = MAX(api_calls.cache_write_5m, excluded.cache_write_5m),
                       cache_write_1h = MAX(api_calls.cache_write_1h, excluded.cache_write_1h),
                       cache_read = MAX(api_calls.cache_read, excluded.cache_read),
                       web_search = MAX(api_calls.web_search, excluded.web_search)",
                    params![
                        a.message_id, a.request_id, a.session_id, a.agent_id, a.ts, a.model, a.input, a.output,
                        a.thinking, a.cache_write_5m, a.cache_write_1h, a.cache_read, a.web_search
                    ],
                )?;
            }
            Event::Tool(t) => {
                tx.execute(
                    "INSERT OR IGNORE INTO tool_calls(tool_use_id, message_id, session_id, agent_id, ts, tool_name, mcp_server,
                                                      skill_name, subagent_type)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        t.tool_use_id, t.message_id, t.session_id, t.agent_id, t.ts, t.tool_name, t.mcp_server,
                        t.skill_name, t.subagent_type
                    ],
                )?;
            }
            Event::Command(c) => {
                let kind = match c.via {
                    parse::Via::Tool => commands::CommandKind::Skill,
                    parse::Via::Slash => commands::classify(&c.name),
                };
                tx.execute(
                    "INSERT OR IGNORE INTO command_invocations(record_uuid, session_id, ts, name, kind, via)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![c.record_uuid, c.session_id, c.ts, c.name, kind.as_str(), c.via.as_str()],
                )?;
            }
            Event::CostSegment { session_id, start_time, total_cost_usd } => {
                tx.execute(
                    "INSERT INTO cost_segments(session_id, start_time, total_cost_usd) VALUES (?1, ?2, ?3)
                     ON CONFLICT(session_id, start_time) DO UPDATE SET total_cost_usd = excluded.total_cost_usd",
                    params![session_id, start_time, total_cost_usd],
                )?;
            }
            Event::Artifact(a) => {
                let title = if store_titles { a.title.as_ref().map(|t| t.as_str()) } else { None };
                tx.execute(
                    "INSERT INTO artifacts(artifact_id, url, title, kind, seq, session_id, first_seen_ts, last_ts)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                     ON CONFLICT(artifact_id) DO UPDATE SET
                       url = excluded.url,
                       title = COALESCE(excluded.title, artifacts.title),
                       kind = excluded.kind,
                       seq = MAX(artifacts.seq, excluded.seq),
                       first_seen_ts = MIN(artifacts.first_seen_ts, excluded.first_seen_ts),
                       last_ts = MAX(artifacts.last_ts, excluded.last_ts),
                       session_id = CASE WHEN excluded.last_ts >= artifacts.last_ts THEN excluded.session_id ELSE artifacts.session_id END",
                    params![a.artifact_id, a.url, title, if a.typed { "typed" } else { "page" }, a.seq, a.session_id, a.ts],
                )?;
            }
            Event::AgentSetting { session_id, name } => {
                tx.execute(
                    "INSERT INTO sessions(session_id, agent_setting) VALUES (?1, ?2)
                     ON CONFLICT(session_id) DO UPDATE SET agent_setting = excluded.agent_setting",
                    params![session_id, name],
                )?;
            }
            Event::ToolError { tool_use_id, session_id, ts } => {
                tx.execute("INSERT OR IGNORE INTO tool_errors(tool_use_id, session_id, ts) VALUES (?1, ?2, ?3)", params![tool_use_id, session_id, ts])?;
            }
            Event::LimitHit(h) => {
                tx.execute(
                    "INSERT OR IGNORE INTO limit_hits(record_uuid, session_id, is_subagent, ts, resets_at, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![h.record_uuid, h.session_id, h.is_subagent, h.ts, h.resets_at, h.kind],
                )?;
            }
            Event::Compaction { record_uuid, session_id, ts } => {
                tx.execute("INSERT OR IGNORE INTO compactions(record_uuid, session_id, ts) VALUES (?1, ?2, ?3)", params![record_uuid, session_id, ts])?;
            }
            Event::Title { session_id, title } => {
                if store_titles {
                    tx.execute(
                        "INSERT INTO sessions(session_id, title) VALUES (?1, ?2)
                         ON CONFLICT(session_id) DO UPDATE SET title = excluded.title",
                        params![session_id, title.as_str()],
                    )?;
                }
            }
        }
    }
    Ok(())
}

enum LineRead {
    Eof,
    /// A full line (including its newline) is in the buffer; value = bytes consumed.
    Complete(u64),
    /// A line longer than the cap was consumed and discarded; value = bytes consumed.
    TooLong(u64),
}

/// Reads one newline-terminated line, never buffering more than `cap` bytes. An unterminated tail at EOF is left
/// unconsumed (reported as `Eof`) so it is retried once the writer finishes the line.
fn read_line_capped<R: BufRead + Seek>(reader: &mut R, buf: &mut Vec<u8>, cap: usize) -> std::io::Result<LineRead> {
    buf.clear();
    let start = reader.stream_position()?;
    let mut consumed: u64 = 0;
    let mut overflow = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            // EOF without newline: rewind so the partial line is read again next time.
            reader.seek(SeekFrom::Start(start))?;
            return Ok(LineRead::Eof);
        }
        let (take, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        if !overflow {
            if buf.len() + take > cap {
                overflow = true;
                buf.clear();
                buf.shrink_to_fit();
            } else {
                buf.extend_from_slice(&chunk[..take]);
            }
        }
        reader.consume(take);
        consumed += take as u64;
        if done {
            return Ok(if overflow { LineRead::TooLong(consumed) } else { LineRead::Complete(consumed) });
        }
    }
}

fn read_agent_type(meta: &Path) -> Option<String> {
    let file = open_regular(meta).ok()?;
    let mut raw = String::new();
    file.take(MAX_META_BYTES).read_to_string(&mut raw).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("agentType").and_then(|t| t.as_str()).and_then(parse::clean_name)
}

/// Last path component of a record's `cwd`, e.g. `/Users/me/dev_team` → `dev_team`.
fn basename(path: &str) -> Option<&str> {
    path.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|s| !s.is_empty())
}

/// Fallback when no record carries a `cwd`: the encoded directory name loses `/` and `_`, so this is approximate.
fn project_name_from_dir(dir: &str) -> String {
    dir.rsplit('-').find(|s| !s.is_empty()).unwrap_or(dir).to_string()
}

#[cfg(unix)]
fn inode_of(meta: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino() as i64
}

#[cfg(not(unix))]
fn inode_of(_meta: &std::fs::Metadata) -> i64 {
    0
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn capped_reader_handles_partial_and_long_lines() {
        let data = b"short\nthis-line-is-way-too-long\nok\npartial".to_vec();
        let mut r = BufReader::with_capacity(4, Cursor::new(data));
        let mut buf = Vec::new();
        assert!(matches!(read_line_capped(&mut r, &mut buf, 10).unwrap(), LineRead::Complete(6)));
        assert_eq!(buf, b"short\n");
        assert!(matches!(read_line_capped(&mut r, &mut buf, 10).unwrap(), LineRead::TooLong(26)));
        assert!(buf.is_empty());
        assert!(matches!(read_line_capped(&mut r, &mut buf, 10).unwrap(), LineRead::Complete(3)));
        let pos = r.stream_position().unwrap();
        assert!(matches!(read_line_capped(&mut r, &mut buf, 10).unwrap(), LineRead::Eof));
        assert_eq!(r.stream_position().unwrap(), pos, "partial tail must stay unconsumed");
    }

    /// Many titles spread over many pages, as in a real DB: after switching titles off no title byte may remain in
    /// the database or WAL files (secure_delete + VACUUM).
    #[test]
    fn titles_off_leaves_no_bytes_with_many_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data/dduck.db");
        {
            let conn = crate::db::open(&path).unwrap();
            for i in 0..300 {
                conn.execute(
                    "INSERT INTO sessions(session_id, title, project_path) VALUES (?1, ?2, '/p')",
                    params![format!("s{i:04}"), format!("SECRETTITLE number {i} with some padding text")],
                )
                .unwrap();
            }
            for i in (0..300).step_by(3) {
                conn.execute("DELETE FROM sessions WHERE session_id = ?1", params![format!("s{i:04}")]).unwrap();
            }
            set_store_titles(&conn, false).unwrap();
        }
        for e in std::fs::read_dir(path.parent().unwrap()).unwrap() {
            let p = e.unwrap().path();
            let bytes = std::fs::read(&p).unwrap();
            assert!(!bytes.windows(11).any(|w| w == b"SECRETTITLE"), "title bytes left in {}", p.display());
        }
    }

    #[test]
    fn project_names() {
        assert_eq!(basename("/Users/me/Agents/dev_team"), Some("dev_team"));
        assert_eq!(basename("/Users/me/app/"), Some("app"));
        assert_eq!(basename("C:\\work\\proj"), Some("proj"));
        assert_eq!(project_name_from_dir("-Users-me-app"), "app");
    }
}
