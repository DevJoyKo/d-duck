//! usage-limit-resume end to end below the shell: a transcript with a limit stop is ingested, announced once, waits
//! for its reset, is claimed once, resumed through a fake `claude` with the fixed prompt, and the task shows as
//! resumed. Also: the first ingest stays quiet, a restart mid-start never runs twice, a late wake-up asks, and the
//! shipped flag off runs nothing. Message text never reaches the DB.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dduck_core::claude_cli::{ClaudeCli, PermissionMode, ResumeOutput, ResumeSpec, TaskSpec};
use dduck_core::limit_resume::{self as lr, Policy, State};
use dduck_core::{db, discover, ingest, settings, task_meta};

const SID: &str = "0f3b2a8e-1111-4222-8333-444455556666";
const H: i64 = 3_600_000;
const ON: Policy = Policy { auto_on: true, shipped: true };

struct Env {
    tmp: tempfile::TempDir,
    config: PathBuf,
    work: PathBuf,
}

/// Hit at 2026-11-09T03:00:00Z, reset one hour later.
fn hit_ts() -> i64 {
    chrono::DateTime::parse_from_rfc3339("2026-11-09T03:00:00Z").unwrap().timestamp_millis()
}

fn setup() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude");
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let proj = config.join("projects/-tmp-work");
    std::fs::create_dir_all(&proj).unwrap();
    let reset = hit_ts() / 1000 + 3600;
    let lines = [
        format!(r#"{{"type":"user","uuid":"u1","timestamp":"2026-11-09T02:59:00.000Z","sessionId":"{SID}","message":{{"content":"CANARY_PROMPT"}}}}"#),
        format!(
            r#"{{"type":"assistant","uuid":"lim1","timestamp":"2026-11-09T03:00:00.000Z","sessionId":"{SID}","isApiErrorMessage":true,"error":"rate_limit","apiErrorStatus":429,"quotaLimits":{{"status":"rejected","resetsAt":{reset},"rateLimitType":"five_hour","upgradePaths":["CANARY_UP"]}},"message":{{"model":"<synthetic>","content":[{{"type":"text","text":"CANARY_LIMIT_TEXT"}}]}}}}"#
        ),
    ];
    std::fs::write(proj.join(format!("{SID}.jsonl")), lines.join("\n") + "\n").unwrap();
    Env { tmp, config, work }
}

fn ingest_all(conn: &mut rusqlite::Connection, config: &Path) {
    let root = discover::projects_root(config).unwrap();
    ingest::Ingestor::new(conn, Default::default()).ingest_all(&discover::discover(&root)).unwrap();
}

fn fake_cli(dir: &Path) -> (ClaudeCli, PathBuf) {
    let bin = dir.join("bin/claude");
    let log = dir.join("argv.log");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, format!("#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{}'; done\necho 'backgrounded · ab12cd34 · job'\n", log.display())).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    (ClaudeCli::new(bin, None), log)
}

fn record_task(conn: &rusqlite::Connection, cwd: &Path) {
    let rec = task_meta::NewRecord {
        bg_id: "ab12cd34",
        name: Some("job"),
        cwd: &cwd.to_string_lossy(),
        agent: None,
        task_type: None,
        prompt: None,
        brief: None,
        effort: None,
        worktree: false,
        permission_mode: Some("acceptEdits"),
        report_id: None,
        origin: None,
        loadout_dropped: 0,
        now_ms: hit_ts() - 10 * 60_000,
    };
    task_meta::record_start(conn, &rec).unwrap();
    task_meta::set_limit_fields(conn, "ab12cd34", false, false).unwrap();
    conn.execute("UPDATE task_meta SET session_id = ?1", [SID]).unwrap();
}

#[test]
fn stop_is_announced_once_waits_and_resumes_once() {
    let env = setup();
    let db_path = env.tmp.path().join("t.db");
    let mut conn = db::open(&db_path).unwrap();
    record_task(&conn, &env.work);
    ingest_all(&mut conn, &env.config);
    lr::finish_backfill(&conn).unwrap();
    // Re-recording as new after the backfill: a fresh stop (the backfill marks what it saw as old news).
    conn.execute("UPDATE limit_hits SET seen = 0", []).unwrap();
    let now = hit_ts() + 60_000;
    let notices = lr::sync(&mut conn, now, &[], ON, true).unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!((notices[0].auto, notices[0].manual), (1, 0));
    assert!(lr::sync(&mut conn, now + 5_000, &[], ON, true).unwrap().is_empty(), "announced once");

    let reset = hit_ts() + H;
    assert!(lr::tick(&mut conn, reset - 1, ON).unwrap().is_none(), "waits for the reset");
    let claim = lr::tick(&mut conn, reset + 181_000, ON).unwrap().expect("due");
    assert!(lr::tick(&mut conn, reset + 400_000, ON).unwrap().is_none(), "claimed once");

    let (cli, log) = fake_cli(env.tmp.path());
    let mut spec = TaskSpec::basic(env.work.clone(), lr::RESUME_PROMPT.into(), Some("job".into()), Some(PermissionMode::AcceptEdits));
    spec.prepare(env.tmp.path(), reset, None).unwrap();
    let mut r = ResumeSpec::from_prepared(&spec, &claim.session_id, env.work.clone()).unwrap();
    r.guard = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    assert_eq!(cli.resume_background(&r, None).unwrap(), ResumeOutput::Started("ab12cd34".into()));
    lr::finish(&conn, claim.id, Ok(()), reset + 182_000).unwrap();
    let argv = std::fs::read_to_string(&log).unwrap();
    assert!(argv.contains(&format!("--resume={SID}")) && argv.contains(lr::RESUME_PROMPT) && !argv.contains("--worktree"));
    let st = lr::status(&conn, reset + 183_000).unwrap();
    assert_eq!((st[0].state, st[0].attempts), (State::Resumed, 1));

    drop(conn);
    for f in std::fs::read_dir(env.tmp.path()).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e != "log") && p.is_file() && p.to_string_lossy().contains("t.db") {
            let bytes = std::fs::read(&p).unwrap();
            assert!(!bytes.windows(6).any(|w| w == b"CANARY"), "{}", p.display());
        }
    }
}

#[test]
fn first_ingest_is_quiet_and_restarts_never_run_twice() {
    let env = setup();
    let mut conn = db::open_in_memory().unwrap();
    record_task(&conn, &env.work);
    ingest_all(&mut conn, &env.config);
    // App start: the first full pass finishes before any announcement.
    assert!(!settings::limit_backfilled(&conn).unwrap());
    lr::finish_backfill(&conn).unwrap();
    assert!(lr::sync(&mut conn, hit_ts() + 1000, &[], ON, true).unwrap().is_empty(), "history, not news");

    // Quit while starting: the row is never run again after the restart.
    let reset = hit_ts() + H;
    let c = lr::tick(&mut conn, reset + 181_000, ON).unwrap().expect("due");
    assert_eq!(lr::close_interrupted(&conn, reset + 200_000).unwrap(), 1);
    assert!(lr::tick(&mut conn, reset + 300_000, ON).unwrap().is_none());
    assert!(lr::status(&conn, reset + 300_000).unwrap().iter().any(|e| e.id == c.id && e.state == State::Manual));
}

#[test]
fn late_wake_up_asks_and_unshipped_flag_runs_nothing() {
    let env = setup();
    let mut conn = db::open_in_memory().unwrap();
    record_task(&conn, &env.work);
    ingest_all(&mut conn, &env.config);
    lr::sync(&mut conn, hit_ts() + 1000, &[], ON, true).unwrap();
    // The app was closed from the stop until 6 hours after the reset.
    assert!(lr::tick(&mut conn, hit_ts() + 7 * H, ON).unwrap().is_none());
    assert_eq!(lr::status(&conn, hit_ts() + 7 * H).unwrap()[0].state, State::NeedsChoice);

    let env2 = setup();
    let mut c2 = db::open_in_memory().unwrap();
    record_task(&c2, &env2.work);
    ingest_all(&mut c2, &env2.config);
    let off = Policy { auto_on: true, shipped: lr::AUTO_RESUME_SHIPPED };
    lr::sync(&mut c2, hit_ts() + 1000, &[], off, true).unwrap();
    assert!(lr::tick(&mut c2, hit_ts() + H + 200_000, off).unwrap().is_none(), "nothing runs while the flag is off");
}
