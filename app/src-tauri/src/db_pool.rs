//! The app's SQLite connections: a small pool instead of one connection behind a `Mutex` (D-161).
//!
//! With a single `std::sync::Mutex<Connection>`, a thread that locked it and then called code that locked it again
//! waited on itself forever, and every command, the poller and the scheduler queued behind it (the "시작하는 중" hang:
//! `delivery_guard_preview` held the connection while `projects_cmds::ctx` asked for it again). Here every caller gets
//! its own connection — an idle one, or a newly opened one — so nothing waits on the app side: a nested call simply
//! gets a second connection. Ordering between writers is SQLite's (WAL + `busy_timeout`), and the read-check-write
//! claims already run in `BEGIN IMMEDIATE` transactions, as they did when the ingest worker and the poller had their
//! own connections.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Mutex;

use dduck_core::db;
use rusqlite::Connection;

/// Idle connections kept for reuse; more may be open at once (one per concurrent caller), the extra ones are closed
/// when returned.
const MAX_IDLE: usize = 4;

pub(crate) struct DbPool {
    path: PathBuf,
    /// Guards only the idle list (push/pop), never a connection in use.
    idle: Mutex<Vec<Connection>>,
}

impl DbPool {
    /// `first` is the connection the app opened at startup (migrations already applied).
    pub(crate) fn new(path: PathBuf, first: Connection) -> Self {
        DbPool { path, idle: Mutex::new(vec![first]) }
    }

    /// A connection for this caller: an idle one or a new one. Never waits for another caller.
    pub(crate) fn get(&self) -> Result<DbConn<'_>, dduck_core::Error> {
        let reused = self.idle.lock().ok().and_then(|mut v| v.pop());
        let conn = match reused {
            Some(c) => c,
            None => db::open(&self.path)?,
        };
        Ok(DbConn { pool: self, conn: Some(conn) })
    }

    fn put_back(&self, conn: Connection) {
        // A connection left inside a transaction (a caller returned early) is not reused: closing it rolls back.
        if !conn.is_autocommit() {
            return;
        }
        if let Ok(mut v) = self.idle.lock() {
            if v.len() < MAX_IDLE {
                v.push(conn);
            }
        }
    }

    #[cfg(test)]
    fn idle_len(&self) -> usize {
        self.idle.lock().map(|v| v.len()).unwrap_or(0)
    }
}

/// A pooled connection; returned to the pool when dropped.
pub(crate) struct DbConn<'a> {
    pool: &'a DbPool,
    conn: Option<Connection>,
}

impl Drop for DbConn<'_> {
    fn drop(&mut self) {
        if let Some(c) = self.conn.take() {
            self.pool.put_back(c);
        }
    }
}

impl Deref for DbConn<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn.as_ref().unwrap_or_else(|| unreachable!("taken only in drop"))
    }
}

impl DerefMut for DbConn<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap_or_else(|| unreachable!("taken only in drop"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn pool() -> (tempfile::TempDir, DbPool) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("d.db");
        let first = db::open(&path).unwrap();
        (tmp, DbPool::new(path, first))
    }

    #[test]
    fn a_nested_request_on_the_same_thread_gets_its_own_connection() {
        // The D-161 hang: hold one, ask again on the same thread. Must return at once, not wait forever.
        let (_t, p) = pool();
        let outer = p.get().unwrap();
        let inner = p.get().unwrap();
        outer.execute("INSERT INTO settings(key, value) VALUES ('a', '1')", []).unwrap();
        let v: String = inner.query_row("SELECT value FROM settings WHERE key = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "1", "both see the same database");
    }

    #[test]
    fn connections_are_reused_and_the_idle_list_is_bounded() {
        let (_t, p) = pool();
        {
            let held: Vec<_> = (0..MAX_IDLE + 3).map(|_| p.get().unwrap()).collect();
            assert_eq!(held.len(), MAX_IDLE + 3);
        }
        assert_eq!(p.idle_len(), MAX_IDLE);
        let _one = p.get().unwrap();
        assert_eq!(p.idle_len(), MAX_IDLE - 1, "taken from the idle list, not opened");
    }

    #[test]
    fn a_connection_left_in_a_transaction_is_not_reused() {
        let (_t, p) = pool();
        {
            let c = p.get().unwrap();
            c.execute_batch("BEGIN IMMEDIATE; INSERT INTO settings(key, value) VALUES ('x', '1');").unwrap();
        }
        assert_eq!(p.idle_len(), 0, "closed, which rolls the transaction back");
        let c = p.get().unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM settings WHERE key = 'x'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn writers_on_other_threads_are_ordered_by_sqlite_not_by_the_app() {
        let (_t, p) = pool();
        let p = Arc::new(p);
        let held = p.get().unwrap();
        held.execute_batch("BEGIN IMMEDIATE; INSERT INTO settings(key, value) VALUES ('w', '1');").unwrap();
        let p2 = Arc::clone(&p);
        let t = std::thread::spawn(move || {
            // Gets a connection at once; its write waits for the other transaction (busy_timeout), then succeeds.
            let c = p2.get().unwrap();
            c.execute("INSERT INTO settings(key, value) VALUES ('w2', '1')", []).is_ok()
        });
        std::thread::sleep(Duration::from_millis(100));
        held.execute_batch("COMMIT").unwrap();
        drop(held);
        assert!(t.join().unwrap());
    }
}
