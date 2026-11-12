//! What D.Duck knows about the tasks it started: persona and task type, keyed by background id (D-057).

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::Error;
use crate::claude_cli::AgentSession;

/// Task types are editable keys now (D-081): only the key format is checked, so records of a type the user later
/// removed stay readable ("삭제된 유형").
pub fn valid_type(t: &str) -> bool {
    crate::team_config::valid_key(t)
}

/// Distinct task type keys in the records. Those types can be hidden but not deleted.
pub fn used_types(conn: &Connection) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare("SELECT DISTINCT task_type FROM task_meta WHERE task_type IS NOT NULL")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn record(conn: &Connection, bg_id: &str, agent: Option<&str>, task_type: Option<&str>, now_ms: i64) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO task_meta(bg_id, agent, task_type, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(bg_id) DO UPDATE SET agent = excluded.agent, task_type = excluded.task_type",
        params![bg_id, agent, task_type.filter(|t| valid_type(t)), now_ms],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub agent: Option<String>,
    pub task_type: Option<String>,
    /// "리뷰함" was pressed (AG-14).
    pub reviewed: bool,
    /// 작업 보고서 state for the list chip (decision-report); only for tasks that were given a report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<crate::task_report::Presence>,
    /// Draft source (`insight-local`/`insight-claude`), restored on "다시 맡기기".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Loadout skills left out at start ("장비 스킬이 바뀌어 빼고 시작했어요", agent-loadout).
    #[serde(skip_serializing_if = "is_zero")]
    pub loadout_dropped: u32,
    /// Board lane of an ended task (task-lifecycle); `None` = still running, or 확인 필요.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane: Option<crate::task_lifecycle::Lane>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn clean_origin(o: Option<String>) -> Option<String> {
    o.and_then(|o| crate::claude_cli::Origin::parse(&o)).map(|o| o.as_str().to_string())
}

/// Marks a D.Duck task as reviewed. Unknown ids are ignored (returns false).
pub fn mark_reviewed(conn: &Connection, bg_id: &str, now_ms: i64) -> Result<bool, Error> {
    Ok(conn.execute("UPDATE task_meta SET reviewed_at = ?2 WHERE bg_id = ?1", params![bg_id, now_ms])? > 0)
}

/// Tags per session key: D.Duck's own record first, else the transcript's `agentSetting`.
pub fn tags(conn: &Connection, sessions: &[AgentSession]) -> Result<HashMap<String, Tag>, Error> {
    let mut out = HashMap::new();
    for s in sessions {
        let mut tag = Tag::default();
        if let Some(id) = &s.id {
            if let Some(a) = conn
                .query_row("SELECT agent, task_type, reviewed_at, origin, loadout_dropped FROM task_meta WHERE bg_id = ?1", [id], |r| {
                    Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, i64>(4)?))
                })
                .optional()?
            {
                tag = Tag { agent: a.0, task_type: a.1, reviewed: a.2.is_some(), report: None, origin: clean_origin(a.3), loadout_dropped: u32::try_from(a.4.clamp(0, 99)).unwrap_or(0), lane: None };
            }
        }
        if tag.agent.is_none() {
            if let Some(sid) = &s.session_id {
                tag.agent = conn.query_row("SELECT agent_setting FROM sessions WHERE session_id = ?1", [sid], |r| r.get(0)).optional()?.flatten();
            }
        }
        if tag != Tag::default() {
            out.insert(s.key.clone(), tag);
        }
    }
    Ok(out)
}

// ---------- task history (D-069) ----------

pub const MAX_PROMPT_BYTES: usize = 100 * 1024;
pub const KEEP_RECORDS: i64 = 200;
pub const KEEP_DAYS: i64 = 90;
static BG_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[0-9a-f]{8}\z").unwrap_or_else(|_| unreachable!()));

pub fn valid_bg_id(id: &str) -> bool {
    BG_ID_RE.is_match(id)
}

/// What a successful start leaves behind. `prompt`/`brief` are `None` when storing instructions is off.
pub struct NewRecord<'a> {
    pub bg_id: &'a str,
    pub name: Option<&'a str>,
    pub cwd: &'a str,
    pub agent: Option<&'a str>,
    pub task_type: Option<&'a str>,
    pub prompt: Option<&'a str>,
    pub brief: Option<&'a crate::team::Brief>,
    pub effort: Option<&'a str>,
    pub worktree: bool,
    /// Shown in the report's "위임 내용" (decision-report).
    pub permission_mode: Option<&'a str>,
    /// Checked with `task_report::valid_id`; anything else is not stored.
    pub report_id: Option<&'a str>,
    pub origin: Option<crate::claude_cli::Origin>,
    pub loadout_dropped: usize,
    pub now_ms: i64,
}

/// Records every D.Duck start (with or without type/persona) and prunes old history.
pub fn record_start(conn: &Connection, r: &NewRecord<'_>) -> Result<(), Error> {
    let prompt = r.prompt.filter(|p| p.len() <= MAX_PROMPT_BYTES);
    let brief = r.brief.and_then(|b| serde_json::to_string(b).ok()).filter(|b| b.len() <= MAX_PROMPT_BYTES);
    conn.execute(
        "INSERT INTO task_meta(bg_id, agent, task_type, created_at, name, cwd, prompt, brief_json, effort, worktree, permission_mode, report_id, origin, loadout_dropped)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(bg_id) DO UPDATE SET agent = excluded.agent, task_type = excluded.task_type, name = excluded.name,
           cwd = excluded.cwd, prompt = excluded.prompt, brief_json = excluded.brief_json, effort = excluded.effort,
           worktree = excluded.worktree, permission_mode = excluded.permission_mode, report_id = excluded.report_id,
           origin = excluded.origin, loadout_dropped = excluded.loadout_dropped",
        params![
            r.bg_id,
            r.agent,
            r.task_type.filter(|t| valid_type(t)),
            r.now_ms,
            r.name.map(|n| crate::text::display_safe(n, 64)),
            crate::text::display_safe(r.cwd, 500),
            prompt,
            brief,
            r.effort,
            r.worktree,
            r.permission_mode.filter(|m| m.len() <= 32 && m.chars().all(|c| c.is_ascii_alphabetic())),
            r.report_id.filter(|id| crate::task_report::valid_id(id)),
            r.origin.map(|o| o.as_str()),
            i64::try_from(r.loadout_dropped.min(99)).unwrap_or(0),
        ],
    )?;
    prune(conn, r.now_ms)?;
    Ok(())
}

/// The delivery scope a task started with (delivery-guard). Fixed codes and validated values only.
pub fn record_guard(conn: &Connection, bg_id: &str, guard: crate::delivery_guard::Guard, default_branch: Option<&str>, default_head: Option<&str>) -> Result<(), Error> {
    let branch = default_branch.filter(|b| crate::delivery_guard::valid_branch(b));
    let head = default_head.filter(|h| (h.len() == 40 || h.len() == 64) && h.bytes().all(|b| b.is_ascii_hexdigit()));
    conn.execute(
        "UPDATE task_meta SET delivery_guard = ?2, default_branch = ?3, default_head = ?4 WHERE bg_id = ?1",
        params![bg_id, guard.code(), branch, head],
    )?;
    Ok(())
}

/// The language a task started in (app-language design 6). Only `ko`/`en` are written.
pub fn record_lang(conn: &Connection, bg_id: &str, lang: crate::i18n::Lang) -> Result<(), Error> {
    conn.execute("UPDATE task_meta SET lang = ?2 WHERE bg_id = ?1", params![bg_id, lang.as_str()])?;
    Ok(())
}

/// What a start passed for the model (model-select design 3·11): the layer, the `--model=` value, the role `model`
/// and, when an unsupported model was replaced, the value asked for. Only valid model names are written.
pub fn record_model(conn: &Connection, bg_id: &str, pin: &crate::model_select::ModelPin, requested: Option<&str>) -> Result<(), Error> {
    use crate::model_select::valid_model;
    let ok = |v: Option<&str>| v.filter(|m| valid_model(m)).map(str::to_string);
    let requested = ok(requested);
    conn.execute(
        "UPDATE task_meta SET model_layer = ?2, model_arg = ?3, role_model = ?4,
           model_requested = COALESCE(model_requested, ?5), model_fallback = CASE WHEN ?5 IS NULL THEN model_fallback ELSE 'unsupported' END
         WHERE bg_id = ?1",
        params![bg_id, pin.layer.as_str(), ok(pin.arg.as_deref()), ok(pin.role.as_deref()), requested],
    )?;
    Ok(())
}

/// A recorded pin as a resume reads it (design 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinRead {
    /// An older task (no layer): no `--model` and no role `model`, as before model-select.
    None,
    /// Values as stored; `ModelPin` checks are done by the caller (`check_pin` refuses a bad value).
    Pin(crate::model_select::ModelPin),
    /// A layer that is not one of the four: never guessed, the resume is refused.
    Corrupt,
}

/// The recorded pin, values unfiltered so a hand-edited one is refused rather than dropped (QA review HIGH).
pub fn model_pin(conn: &Connection, bg_id: &str) -> Result<PinRead, Error> {
    use crate::model_select::{Layer, ModelPin};
    use rusqlite::OptionalExtension;
    let row: Option<(Option<String>, Option<String>, Option<String>)> =
        conn.query_row("SELECT model_layer, model_arg, role_model FROM task_meta WHERE bg_id = ?1", [bg_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    Ok(match row {
        None | Some((None, _, _)) => PinRead::None,
        Some((Some(l), a, r)) => match Layer::parse(&l) {
            Some(layer) => PinRead::Pin(ModelPin { layer, arg: a, role: r }),
            None => PinRead::Corrupt,
        },
    })
}

/// The recorded start language; `None` for older tasks or anything but the two values.
pub fn lang(conn: &Connection, bg_id: &str) -> Result<Option<crate::i18n::Lang>, Error> {
    use rusqlite::OptionalExtension;
    let v: Option<Option<String>> = conn.query_row("SELECT lang FROM task_meta WHERE bg_id = ?1", [bg_id], |r| r.get(0)).optional()?;
    Ok(v.flatten().as_deref().and_then(crate::i18n::Lang::parse))
}

/// The recorded scope, `None` for older tasks (or an unknown code).
pub fn delivery_guard(conn: &Connection, bg_id: &str) -> Result<Option<crate::delivery_guard::Guard>, Error> {
    use rusqlite::OptionalExtension;
    let v: Option<Option<String>> = conn.query_row("SELECT delivery_guard FROM task_meta WHERE bg_id = ?1", [bg_id], |r| r.get(0)).optional()?;
    Ok(v.flatten().as_deref().and_then(crate::delivery_guard::Guard::from_code))
}

/// Keeps the newest 200 records and nothing older than 90 days. Returns how many were removed.
pub fn prune(conn: &Connection, now_ms: i64) -> Result<usize, Error> {
    let cutoff = now_ms - KEEP_DAYS * 24 * 3600 * 1000;
    let n = conn.execute(
        "DELETE FROM task_meta WHERE created_at < ?1
           OR bg_id NOT IN (SELECT bg_id FROM task_meta ORDER BY created_at DESC LIMIT ?2)",
        params![cutoff, KEEP_RECORDS],
    )?;
    Ok(n)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRow {
    pub bg_id: String,
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub agent: Option<String>,
    pub task_type: Option<String>,
    pub created_at: i64,
    /// Length only; the text itself comes from [`record_detail`] (D-050 pattern).
    pub prompt_len: usize,
    pub reviewed: bool,
    /// A 작업 보고서 was asked for (decision-report). The id itself never leaves the backend.
    pub has_report: bool,
    /// "정리 끝" (task-lifecycle): shown as "완료".
    pub wrapped: bool,
}

pub fn records(conn: &Connection, limit: usize) -> Result<Vec<RecordRow>, Error> {
    let mut stmt = conn.prepare(
        "SELECT bg_id, name, cwd, agent, task_type, created_at, LENGTH(prompt), reviewed_at, report_id IS NOT NULL, wrapped_at FROM task_meta
         ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit.min(500) as i64], |r| {
        Ok(RecordRow {
            bg_id: r.get(0)?,
            name: r.get(1)?,
            cwd: r.get(2)?,
            agent: r.get(3)?,
            task_type: r.get(4)?,
            created_at: r.get(5)?,
            prompt_len: r.get::<_, Option<i64>>(6)?.unwrap_or(0) as usize,
            reviewed: r.get::<_, Option<i64>>(7)?.is_some(),
            has_report: r.get(8)?,
            wrapped: r.get::<_, Option<i64>>(9)?.is_some(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDetail {
    pub prompt: Option<String>,
    pub brief: Option<crate::team::Brief>,
    pub effort: Option<String>,
    pub worktree: bool,
    pub origin: Option<String>,
    /// model-select: the layer that decided (`task`·`agent`·`type`·`cli`), the model passed (`--model` or the role's),
    /// and the value asked for when an unsupported one was replaced. All `None` for older tasks.
    pub model_layer: Option<String>,
    pub model_used: Option<String>,
    pub model_requested: Option<String>,
    /// The main conversation's first model when it differs from `model_used` (design 9), computed on read.
    pub model_other: Option<String>,
}

pub fn record_detail(conn: &Connection, bg_id: &str) -> Result<Option<RecordDetail>, Error> {
    Ok(conn
        .query_row("SELECT prompt, brief_json, effort, worktree, origin, model_layer, COALESCE(model_arg, role_model), model_requested, session_id FROM task_meta WHERE bg_id = ?1", [bg_id], |r| {
            let model = |v: Option<String>| v.filter(|m| crate::model_select::valid_model(m));
            Ok(RecordDetail {
                prompt: r.get(0)?,
                brief: r.get::<_, Option<String>>(1)?.and_then(|j| serde_json::from_str(&j).ok()),
                effort: r.get(2)?,
                worktree: r.get::<_, i64>(3)? != 0,
                origin: clean_origin(r.get(4)?),
                model_layer: r.get::<_, Option<String>>(5)?.filter(|l| crate::model_select::Layer::parse(l).is_some()),
                model_used: model(r.get(6)?),
                model_requested: model(r.get(7)?),
                model_other: r.get::<_, Option<String>>(8)?,
            })
        })
        .optional()?
        .map(|mut d| {
            // `model_other` held the session id until here.
            let session = d.model_other.take();
            if let (Some(used), Some(sid)) = (d.model_used.as_deref(), session.as_deref()) {
                let seen = crate::model_select::first_main_model(conn, sid).ok().flatten();
                if let Some(seen) = seen.filter(|a| crate::model_select::same_model(used, a) == Some(false)) {
                    d.model_other = Some(crate::text::display_safe(&seen, 80));
                }
            }
            d
        }))
}

/// Deletes one record (and its instruction). Callers scrub afterwards. Unknown ids → false.
pub fn delete_record(conn: &Connection, bg_id: &str) -> Result<bool, Error> {
    Ok(conn.execute("DELETE FROM task_meta WHERE bg_id = ?1", [bg_id])? > 0)
}

/// [`delete_record`] and, right away, the report folder it pointed at (evidence and attachments included), unless
/// another record still points at it (a resumed task under a new id). report-evidence 2.5.
pub fn delete_record_and_report(conn: &Connection, bg_id: &str, reports_root: Option<&std::path::Path>) -> Result<bool, Error> {
    let ids = crate::task_report::ids_of_records(conn, Some(bg_id));
    let gone = delete_record(conn, bg_id)?;
    if let Some(root) = reports_root {
        remove_unreferenced(conn, root, &ids);
    }
    Ok(gone)
}

/// [`delete_all`] and every report folder those records pointed at (routine runs keep theirs).
pub fn delete_all_and_reports(conn: &Connection, reports_root: Option<&std::path::Path>) -> Result<usize, Error> {
    let ids = crate::task_report::ids_of_records(conn, None);
    let n = delete_all(conn)?;
    if let Some(root) = reports_root {
        remove_unreferenced(conn, root, &ids);
    }
    Ok(n)
}

/// Removes each report folder that no record and no routine run points at any more (a failed count keeps it).
pub(crate) fn remove_unreferenced(conn: &Connection, root: &std::path::Path, ids: &[String]) {
    for id in ids {
        let still: i64 = conn
            .query_row("SELECT (SELECT COUNT(*) FROM task_meta WHERE report_id = ?1) + (SELECT COUNT(*) FROM routine_runs WHERE report_id = ?1)", [id], |r| r.get(0))
            .unwrap_or(1);
        if still == 0 {
            crate::task_report::remove_folder(root, id);
        }
    }
}

/// Removes every stored instruction but keeps type/persona/"리뷰함" (not user text).
pub fn clear_prompts(conn: &Connection) -> Result<usize, Error> {
    Ok(conn.execute("UPDATE task_meta SET prompt = NULL, brief_json = NULL WHERE prompt IS NOT NULL OR brief_json IS NOT NULL", [])?)
}

pub fn delete_all(conn: &Connection) -> Result<usize, Error> {
    Ok(conn.execute("DELETE FROM task_meta", [])?)
}

// ---------- usage-limit-resume ----------

/// Fills `session_id` for D.Duck tasks the first time the poller sees their bg id with a session (usage-limit-resume).
pub fn link_sessions(conn: &Connection, sessions: &[AgentSession]) -> Result<(), Error> {
    for s in sessions {
        if let (Some(id), Some(sid)) = (&s.id, &s.session_id) {
            if crate::claude_cli::check_session_id(sid).is_ok() {
                conn.execute("UPDATE task_meta SET session_id = ?2 WHERE bg_id = ?1 AND session_id IS NULL", params![id, sid])?;
            }
        }
    }
    Ok(())
}

/// Start-time facts the limit scheduler needs: phone control on or off, and the "리셋 후 자동으로 이어서" answer of the
/// dangerous-mode dialog.
pub fn set_limit_fields(conn: &Connection, bg_id: &str, remote: bool, auto_ok: bool) -> Result<(), Error> {
    conn.execute("UPDATE task_meta SET remote = ?2, limit_auto_ok = ?3 WHERE bg_id = ?1", params![bg_id, remote, auto_ok])?;
    Ok(())
}

/// A resume printed a different bg id: the same task under a new id, keeping its type, report and origin.
pub fn alias(conn: &Connection, old: &str, new: &str, now_ms: i64) -> Result<bool, Error> {
    if old == new {
        return Ok(false);
    }
    Ok(conn.execute(
        "INSERT OR IGNORE INTO task_meta(bg_id, agent, task_type, created_at, reviewed_at, name, cwd, prompt, brief_json, effort, worktree,
           report_id, permission_mode, origin, loadout_dropped, session_id, limit_opt_out, limit_auto_ok, origin_bg_id, remote,
           model_layer, model_arg, role_model, model_requested, model_fallback)
         SELECT ?2, agent, task_type, ?3, NULL, name, cwd, prompt, brief_json, effort, worktree,
           report_id, permission_mode, origin, loadout_dropped, session_id, limit_opt_out, limit_auto_ok, COALESCE(origin_bg_id, bg_id), remote,
           model_layer, model_arg, role_model, model_requested, model_fallback
         FROM task_meta WHERE bg_id = ?1",
        params![old, new, now_ms],
    )? > 0)
}

/// What a resume rebuilds its arguments from (never the webview).
#[derive(Debug, Clone, PartialEq)]
pub struct ResumeSource {
    pub bg_id: String,
    pub session_id: Option<String>,
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub agent: Option<String>,
    pub task_type: Option<String>,
    pub brief: Option<crate::team::Brief>,
    pub effort: Option<String>,
    pub worktree: bool,
    pub permission_mode: Option<String>,
    pub report_id: Option<String>,
    pub origin: Option<String>,
    pub remote: Option<bool>,
    pub auto_ok: bool,
    /// Start language; `None` = an older task (follow the current app language).
    pub lang: Option<crate::i18n::Lang>,
}

pub fn resume_source(conn: &Connection, bg_id: &str) -> Result<Option<ResumeSource>, Error> {
    Ok(conn
        .query_row(
            "SELECT bg_id, session_id, name, cwd, agent, task_type, brief_json, effort, worktree, permission_mode, report_id, origin, remote, limit_auto_ok, lang
             FROM task_meta WHERE bg_id = ?1",
            [bg_id],
            |r| {
                Ok(ResumeSource {
                    bg_id: r.get(0)?,
                    session_id: r.get(1)?,
                    name: r.get(2)?,
                    cwd: r.get(3)?,
                    agent: r.get(4)?,
                    task_type: r.get(5)?,
                    brief: r.get::<_, Option<String>>(6)?.and_then(|j| serde_json::from_str(&j).ok()),
                    effort: r.get(7)?,
                    worktree: r.get::<_, i64>(8)? != 0,
                    permission_mode: r.get(9)?,
                    report_id: r.get(10)?,
                    origin: clean_origin(r.get(11)?),
                    remote: r.get::<_, Option<i64>>(12)?.map(|v| v != 0),
                    auto_ok: r.get::<_, i64>(13)? != 0,
                    lang: r.get::<_, Option<String>>(14)?.as_deref().and_then(crate::i18n::Lang::parse),
                })
            },
        )
        .optional()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn records_the_start_language_and_reads_back_only_two_values() {
        use crate::i18n::Lang;
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at) VALUES ('ab12cd34', 1)", []).unwrap();
        assert_eq!(super::lang(&conn, "ab12cd34").unwrap(), None, "older task follows the app language");
        super::record_lang(&conn, "ab12cd34", Lang::En).unwrap();
        assert_eq!(super::lang(&conn, "ab12cd34").unwrap(), Some(Lang::En));
        assert_eq!(super::resume_source(&conn, "ab12cd34").unwrap().unwrap().lang, Some(Lang::En), "resume follows the start");
        conn.execute("UPDATE task_meta SET lang = 'fr' WHERE bg_id = 'ab12cd34'", []).unwrap();
        assert_eq!(super::lang(&conn, "ab12cd34").unwrap(), None, "junk is not trusted");
        assert_eq!(super::lang(&conn, "nope0000").unwrap(), None);
    }

    #[test]
    fn records_the_delivery_scope_with_checked_values() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at) VALUES ('ab12cd34', 1)", []).unwrap();
        assert_eq!(super::delivery_guard(&conn, "ab12cd34").unwrap(), None, "older task");
        super::record_guard(&conn, "ab12cd34", crate::delivery_guard::Guard::LocalOnly, Some("x) Edit("), Some("not-hex")).unwrap();
        assert_eq!(super::delivery_guard(&conn, "ab12cd34").unwrap(), Some(crate::delivery_guard::Guard::LocalOnly));
        let (b, h): (Option<String>, Option<String>) = conn.query_row("SELECT default_branch, default_head FROM task_meta WHERE bg_id = 'ab12cd34'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((b, h), (None, None), "unsafe values are not stored");
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert!(v >= 11, "delivery-guard columns exist from v11 on (v12 = task-lifecycle)");
    }

    use super::*;
    use crate::claude_cli::{SessionKind, SessionStatus};

    #[test]
    fn tags_prefer_own_record_then_transcript() {
        let conn = crate::db::open_in_memory().unwrap();
        record(&conn, "ab12cd34", Some("qa-verifier"), Some("qa"), 1).unwrap();
        record(&conn, "ffff0000", None, Some("Bad Key!"), 1).unwrap();
        conn.execute("INSERT INTO sessions(session_id, agent_setting) VALUES ('s2', 'design-critic')", []).unwrap();
        let s = |key: &str, id: Option<&str>, sid: &str| AgentSession { key: key.into(), pid: None, id: id.map(Into::into), session_id: Some(sid.into()), name: "n".into(), cwd: "/".into(), kind: SessionKind::Background, started_at: 0, status: SessionStatus::Working };
        let t = tags(&conn, &[s("a", Some("ab12cd34"), "s1"), s("b", None, "s2"), s("c", Some("ffff0000"), "s3")]).unwrap();
        assert_eq!(t["a"], Tag { agent: Some("qa-verifier".into()), task_type: Some("qa".into()), reviewed: false, report: None, origin: None, loadout_dropped: 0, lane: None });
        assert!(mark_reviewed(&conn, "ab12cd34", 5).unwrap());
        assert!(!mark_reviewed(&conn, "nope", 5).unwrap());
        assert!(tags(&conn, &[s("a", Some("ab12cd34"), "s1")]).unwrap()["a"].reviewed);
        assert_eq!(t["b"].agent.as_deref(), Some("design-critic"));
        assert!(!t.contains_key("c"), "invalid task type is dropped");
        // A user-defined type key (or one the user later removed) is kept as recorded.
        record(&conn, "eeee0000", None, Some("ops-check"), 1).unwrap();
        assert_eq!(tags(&conn, &[s("d", Some("eeee0000"), "s4")]).unwrap()["d"].task_type.as_deref(), Some("ops-check"));
        assert_eq!(used_types(&conn).unwrap().len(), 2);
    }

    #[test]
    fn limit_fields_session_links_and_aliases() {
        let conn = crate::db::open_in_memory().unwrap();
        record_start(&conn, &NewRecord { task_type: Some("dev"), permission_mode: Some("acceptEdits"), ..rec("ab12cd34", None, 5) }).unwrap();
        set_limit_fields(&conn, "ab12cd34", false, true).unwrap();
        let sid = "0f3b2a8e-1111-4222-8333-444455556666";
        let s = |id: &str, sid: &str| AgentSession { key: sid.into(), pid: None, id: Some(id.into()), session_id: Some(sid.into()), name: "n".into(), cwd: "/".into(), kind: SessionKind::Background, started_at: 0, status: SessionStatus::Working };
        link_sessions(&conn, &[s("ab12cd34", sid), s("ffff", "not-a-uuid")]).unwrap();
        let src = resume_source(&conn, "ab12cd34").unwrap().unwrap();
        assert_eq!((src.session_id.as_deref(), src.remote, src.auto_ok, src.task_type.as_deref()), (Some(sid), Some(false), true, Some("dev")));
        assert!(alias(&conn, "ab12cd34", "zz99", 9).unwrap());
        assert!(!alias(&conn, "ab12cd34", "ab12cd34", 9).unwrap());
        let (origin, sid2): (Option<String>, Option<String>) = conn.query_row("SELECT origin_bg_id, session_id FROM task_meta WHERE bg_id = 'zz99'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((origin.as_deref(), sid2.as_deref()), (Some("ab12cd34"), Some(sid)));
        // An alias of an alias still points at the first id.
        alias(&conn, "zz99", "yy88", 10).unwrap();
        let o: String = conn.query_row("SELECT origin_bg_id FROM task_meta WHERE bg_id = 'yy88'", [], |r| r.get(0)).unwrap();
        assert_eq!(o, "ab12cd34");
    }

    #[test]
    fn deleting_a_record_removes_its_report_folder_now() {
        let conn = crate::db::open_in_memory().unwrap();
        let t = tempfile::tempdir().unwrap();
        let root = crate::platform::canon(t.path()).unwrap().join(crate::task_report::DIR_NAME);
        let (a, b) = (crate::task_report::new_id().unwrap(), crate::task_report::new_id().unwrap());
        for id in [&a, &b] {
            let d = crate::task_report::prepare_dir(&root, id).unwrap();
            std::fs::create_dir(d.join("evidence")).unwrap();
            std::fs::write(d.join("evidence/x.png"), "x").unwrap();
        }
        record_start(&conn, &NewRecord { report_id: Some(&a), ..rec("ab12cd34", None, 1) }).unwrap();
        record_start(&conn, &NewRecord { report_id: Some(&b), ..rec("cd34ef56", None, 2) }).unwrap();
        // A resumed twin shares the report: the folder stays until the last record goes.
        alias(&conn, "ab12cd34", "ef56ab12", 3).unwrap();
        assert!(delete_record_and_report(&conn, "ab12cd34", Some(&root)).unwrap());
        assert!(root.join(&a).exists());
        assert!(delete_record_and_report(&conn, "ef56ab12", Some(&root)).unwrap());
        assert!(!root.join(&a).exists() && root.join(&b).exists());
        assert_eq!(delete_all_and_reports(&conn, Some(&root)).unwrap(), 1);
        assert!(!root.join(&b).exists());
        // A tampered id is never turned into a path.
        assert!(!crate::task_report::remove_folder(&root, "../x"));
    }

    fn rec<'a>(id: &'a str, prompt: Option<&'a str>, at: i64) -> NewRecord<'a> {
        NewRecord { bg_id: id, name: Some("job"), cwd: "/w", agent: None, task_type: None, prompt, brief: None, effort: None, worktree: false, permission_mode: None, report_id: None, origin: None, loadout_dropped: 0, now_ms: at }
    }

    #[test]
    fn history_records_prunes_and_scrubs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("t.db");
        let conn = crate::db::open(&path).unwrap();
        let day = 24 * 3600 * 1000;
        // Untyped, persona-less tasks are recorded too (design review MAJOR).
        record_start(&conn, &rec("aaaaaaaa", Some("CANARY_SECRET_PROMPT fix the tests"), 100 * day)).unwrap();
        let rows = records(&conn, 10).unwrap();
        assert_eq!((rows.len(), rows[0].prompt_len), (1, "CANARY_SECRET_PROMPT fix the tests".len()));
        assert!(!serde_json::to_string(&rows).unwrap().contains("CANARY"), "list carries the length only");
        assert_eq!(record_detail(&conn, "aaaaaaaa").unwrap().unwrap().prompt.as_deref(), Some("CANARY_SECRET_PROMPT fix the tests"));
        // 90-day window.
        record_start(&conn, &rec("bbbbbbbb", None, 100 * day + 91 * day)).unwrap();
        assert_eq!(records(&conn, 10).unwrap().len(), 1, "older than 90 days pruned");
        // Delete + scrub leaves no bytes behind.
        record_start(&conn, &rec("cccccccc", Some("CANARY_TWO"), 200 * day)).unwrap();
        assert!(delete_record(&conn, "cccccccc").unwrap());
        assert!(!delete_record(&conn, "cccccccc").unwrap());
        crate::ingest::scrub(&conn).unwrap();
        for f in ["t.db", "t.db-wal"] {
            let bytes = std::fs::read(tmp.path().join(f)).unwrap_or_default();
            assert!(!bytes.windows(10).any(|w| w == b"CANARY_TWO"), "{f}");
        }
        assert!(valid_bg_id("ab12cd34") && !valid_bg_id("../x") && !valid_bg_id("AB12CD34"));
    }

    #[test]
    fn origin_is_kept_for_retry() {
        use crate::claude_cli::{Origin, SessionKind, SessionStatus};
        let conn = crate::db::open_in_memory().unwrap();
        record_start(&conn, &NewRecord { origin: Some(Origin::InsightClaude), ..rec("abababab", Some("p"), 1) }).unwrap();
        record_start(&conn, &rec("cdcdcdcd", Some("p"), 2)).unwrap();
        assert_eq!(record_detail(&conn, "abababab").unwrap().unwrap().origin.as_deref(), Some("insight-claude"));
        assert_eq!(record_detail(&conn, "cdcdcdcd").unwrap().unwrap().origin, None);
        let s = AgentSession { key: "k".into(), pid: None, id: Some("abababab".into()), session_id: None, name: "n".into(), cwd: "/".into(), kind: SessionKind::Background, started_at: 0, status: SessionStatus::Working };
        assert_eq!(tags(&conn, std::slice::from_ref(&s)).unwrap()["k"].origin.as_deref(), Some("insight-claude"));
        record_start(&conn, &NewRecord { loadout_dropped: 2, ..rec("abababab", Some("p"), 1) }).unwrap();
        assert_eq!(tags(&conn, std::slice::from_ref(&s)).unwrap()["k"].loadout_dropped, 2, "a count of dropped loadout skills, no names");
        // A tampered value reads as no origin.
        conn.execute("UPDATE task_meta SET origin = 'evil' WHERE bg_id = 'abababab'", []).unwrap();
        assert_eq!(record_detail(&conn, "abababab").unwrap().unwrap().origin, None);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod model_record {
    use crate::model_select::{Layer, ModelPin};

    #[test]
    fn records_the_pin_and_the_replaced_request() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO task_meta(bg_id, created_at, session_id) VALUES ('ab12cd34', 1, 's1')", []).unwrap();
        assert_eq!(super::model_pin(&conn, "ab12cd34").unwrap(), super::PinRead::None, "older task: no pin");
        let d = super::record_detail(&conn, "ab12cd34").unwrap().unwrap();
        assert_eq!((d.model_layer, d.model_used, d.model_requested), (None, None, None));
        let pin = ModelPin { layer: Layer::Type, arg: Some("opus".into()), role: Some("opus".into()) };
        super::record_model(&conn, "ab12cd34", &pin, Some("claude-opus-3-9")).unwrap();
        assert_eq!(super::model_pin(&conn, "ab12cd34").unwrap(), super::PinRead::Pin(pin.clone()));
        let d = super::record_detail(&conn, "ab12cd34").unwrap().unwrap();
        assert_eq!((d.model_layer.as_deref(), d.model_used.as_deref(), d.model_requested.as_deref()), (Some("type"), Some("opus"), Some("claude-opus-3-9")));
        let reason: Option<String> = conn.query_row("SELECT model_fallback FROM task_meta WHERE bg_id = 'ab12cd34'", [], |r| r.get(0)).unwrap();
        assert_eq!(reason.as_deref(), Some("unsupported"));
        // A later resume keeps the first request.
        super::record_model(&conn, "ab12cd34", &ModelPin { arg: Some("sonnet".into()), ..pin.clone() }, Some("opus")).unwrap();
        assert_eq!(super::record_detail(&conn, "ab12cd34").unwrap().unwrap().model_requested.as_deref(), Some("claude-opus-3-9"));
        // Junk is never written; a mismatching transcript model shows up on read.
        super::record_model(&conn, "ab12cd34", &pin, Some("opus x")).unwrap();
        conn.execute("INSERT INTO api_calls(message_id, session_id, agent_id, ts, model) VALUES ('m1', 's1', NULL, 5, 'claude-sonnet-5-5')", []).unwrap();
        let d = super::record_detail(&conn, "ab12cd34").unwrap().unwrap();
        assert_eq!(d.model_other.as_deref(), Some("claude-sonnet-5-5"), "ran on another family than opus");
        // An alias (resume under a new id) keeps the pin.
        super::alias(&conn, "ab12cd34", "ef56ab78", 9).unwrap();
        assert_eq!(super::model_pin(&conn, "ef56ab78").unwrap(), super::PinRead::Pin(pin));
        // Hand-edited values reach the caller as they are (check_pin refuses them); a junk layer is Corrupt.
        conn.execute("UPDATE task_meta SET model_arg = 'opus x' WHERE bg_id = 'ef56ab78'", []).unwrap();
        let super::PinRead::Pin(bad) = super::model_pin(&conn, "ef56ab78").unwrap() else { panic!() };
        assert!(crate::model_select::check_pin(&bad, crate::model_select::Runner::Inline).is_err());
        conn.execute("UPDATE task_meta SET model_layer = 'zzz' WHERE bg_id = 'ef56ab78'", []).unwrap();
        assert_eq!(super::model_pin(&conn, "ef56ab78").unwrap(), super::PinRead::Corrupt);
    }

    #[test]
    fn unversioned_columns_are_added_once() {
        let conn = crate::db::open_in_memory().unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, 17, "no version number taken (v16 is review-routine-runs, v17 news-tab)");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_table_info('task_meta') WHERE name IN ('model_requested','model_fallback')", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }
}
