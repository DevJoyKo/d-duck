//! App settings stored in the DB `settings` table.
//!
//! The generic setter accepts only UI preferences on an allowlist. Security-relevant values (CLI path, config dir)
//! have dedicated setters that the shell calls only after a backend-owned native picker and validation (D-018, D-030).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

pub const NOTIFY_WAITING: &str = "notify_waiting";
pub const CLI_PATH: &str = "cli_path";
pub const CONFIG_DIR: &str = "config_dir";
/// How many code-changing tasks the user can genuinely review (AG-08).
pub const REVIEW_CAPACITY: &str = "review_capacity";
/// macOS notification when a task finishes or fails (D-067).
pub const NOTIFY_FINISHED: &str = "notify_finished";
/// Remote Control for D.Duck tasks. Set only through the shell's native confirmation, never the generic setter.
pub const REMOTE_ALERTS: &str = "remote_alerts";
/// Keep what the user typed into the task composer (D-069). Turning off may delete existing records (shell asks).
pub const STORE_TASK_PROMPTS: &str = "store_task_prompts";
/// "작업 보고서" (decision-report, D-108): local tasks D.Duck starts leave a report.json. On by default.
pub const TASK_REPORT: &str = "task_report";

/// "사용량 한도 알림" (usage-limit-resume). On by default.
pub const LIMIT_NOTIFY: &str = "limit_notify";
/// "리셋되면 자동으로 이어서 하기" (usage-limit-resume). On by default; only acts once the feature flag is on.
pub const LIMIT_AUTO_RESUME: &str = "limit_auto_resume";
/// Set once the first full ingest pass marked every limit stop it found as already announced. Never user-set.
pub const LIMIT_BACKFILLED: &str = "limit_backfilled";

/// App language (app-language, D-171): `system` (default; ko OS → Korean, else English), `ko` or `en`.
pub const UI_LANGUAGE: &str = "ui_language";
const UI_LANGUAGES: &[&str] = &["system", "ko", "en"];

/// Keys the webview may set through the generic setter, with their allowed values.
const USER_PREFS: &[(&str, &[&str])] =
    &[(UI_LANGUAGE, UI_LANGUAGES), (LIMIT_NOTIFY, &["0", "1"]), (LIMIT_AUTO_RESUME, &["0", "1"]), (NOTIFY_WAITING, &["0", "1"]), (NOTIFY_FINISHED, &["0", "1"]), (STORE_TASK_PROMPTS, &["0", "1"]), (TASK_REPORT, &["0", "1"]), (REVIEW_CAPACITY, &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", REVIEW_UNLIMITED])];

/// Stored value for "제한 없음": its own value, not a large number, so no warning ever fires.
pub const REVIEW_UNLIMITED: &str = "unlimited";

/// Review capacity as the webview sees it: a number, or the string `"unlimited"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum ReviewCapacity {
    Limit(u32),
    Unlimited(Unlimited),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unlimited {
    Unlimited,
}

/// The stored choice; anything but the three values (missing, junk) reads as `system`.
pub fn ui_language(conn: &Connection) -> Result<&'static str, Error> {
    let v = get(conn, UI_LANGUAGE)?;
    Ok(UI_LANGUAGES.iter().copied().find(|l| Some(*l) == v.as_deref()).unwrap_or("system"))
}

/// The language to use now: the stored choice, with `system` resolved through the OS locale.
pub fn resolved_language(conn: &Connection) -> Result<crate::i18n::Lang, Error> {
    Ok(resolve_language(ui_language(conn)?, crate::i18n::Lang::system))
}

fn resolve_language(choice: &str, system: impl FnOnce() -> crate::i18n::Lang) -> crate::i18n::Lang {
    match choice {
        "ko" => crate::i18n::Lang::Ko,
        "en" => crate::i18n::Lang::En,
        _ => system(),
    }
}

pub fn notify_finished(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, NOTIFY_FINISHED)?.as_deref() != Some("0"))
}

pub fn limit_notify(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, LIMIT_NOTIFY)?.as_deref() != Some("0"))
}

pub fn limit_auto_resume(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, LIMIT_AUTO_RESUME)?.as_deref() != Some("0"))
}

pub fn limit_backfilled(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, LIMIT_BACKFILLED)?.as_deref() == Some("1"))
}

pub fn set_limit_backfilled(conn: &Connection) -> Result<(), Error> {
    put(conn, LIMIT_BACKFILLED, "1")
}

pub fn store_task_prompts(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, STORE_TASK_PROMPTS)?.as_deref() != Some("0"))
}

pub fn task_report(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, TASK_REPORT)?.as_deref() != Some("0"))
}

pub fn remote_alerts(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, REMOTE_ALERTS)?.as_deref() == Some("1"))
}

/// Call only after the shell's native confirmation (turning on) or directly (turning off).
pub fn set_remote_alerts(conn: &Connection, on: bool) -> Result<(), Error> {
    put(conn, REMOTE_ALERTS, if on { "1" } else { "0" })
}

pub fn review_capacity(conn: &Connection) -> Result<ReviewCapacity, Error> {
    let v = get(conn, REVIEW_CAPACITY)?;
    if v.as_deref() == Some(REVIEW_UNLIMITED) {
        return Ok(ReviewCapacity::Unlimited(Unlimited::Unlimited));
    }
    Ok(ReviewCapacity::Limit(v.and_then(|v| v.parse().ok()).filter(|n| (1..=10).contains(n)).unwrap_or(3)))
}

/// 리뷰 탭 (review-inbox, D-140): tasks started before this moment never count as "리뷰 전", so the first run after
/// the update does not flood the badge. Written once on first read, never through the generic setter.
pub const REVIEW_SINCE: &str = "review_since";

pub fn review_since(conn: &Connection, now_ms: i64) -> Result<i64, Error> {
    if let Some(v) = get(conn, REVIEW_SINCE)?.and_then(|v| v.parse::<i64>().ok()) {
        return Ok(v);
    }
    put(conn, REVIEW_SINCE, &now_ms.to_string())?;
    Ok(now_ms)
}

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, Error> {
    Ok(conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn put(conn: &Connection, key: &str, value: &str) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Writer for keys owned by a dedicated module (e.g. `delivery_guard::LOCAL_ONLY_PROJECTS`); never reachable from the
/// generic setter, which keeps its allowlist.
pub(crate) fn put_guarded(conn: &Connection, key: &str, value: &str) -> Result<(), Error> {
    put(conn, key, value)
}

fn delete(conn: &Connection, key: &str) -> Result<(), Error> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    Ok(())
}

/// Generic setter for UI preferences. Rejects any key or value not on the allowlist.
pub fn set_user_pref(conn: &Connection, key: &str, value: &str) -> Result<(), Error> {
    let allowed = USER_PREFS.iter().find(|(k, _)| *k == key).ok_or(Error::Config("setting not allowed"))?;
    if !allowed.1.contains(&value) {
        return Err(Error::Config("value not allowed"));
    }
    put(conn, key, value)
}

pub fn notify_waiting(conn: &Connection) -> Result<bool, Error> {
    Ok(get(conn, NOTIFY_WAITING)?.as_deref() != Some("0"))
}

/// Store a CLI path that already passed `cli_path::validate`.
pub fn set_cli_path(conn: &Connection, validated: &Path) -> Result<(), Error> {
    put(conn, CLI_PATH, &validated.to_string_lossy())
}

pub fn clear_cli_path(conn: &Connection) -> Result<(), Error> {
    delete(conn, CLI_PATH)
}

pub fn cli_path(conn: &Connection) -> Result<Option<PathBuf>, Error> {
    Ok(get(conn, CLI_PATH)?.map(PathBuf::from))
}

/// Store a config dir chosen in the native folder picker. It must be an existing absolute directory.
pub fn set_config_dir(conn: &Connection, dir: &Path) -> Result<(), Error> {
    if !dir.is_absolute() || !dir.is_dir() {
        return Err(Error::Config("config dir must be an existing absolute directory"));
    }
    if !crate::cli_path::is_shell_safe_path(dir) {
        return Err(Error::Config("config dir contains unsupported characters"));
    }
    put(conn, CONFIG_DIR, &dir.to_string_lossy())
}

pub fn clear_config_dir(conn: &Connection) -> Result<(), Error> {
    delete(conn, CONFIG_DIR)
}

pub fn config_dir(conn: &Connection) -> Result<Option<PathBuf>, Error> {
    Ok(get(conn, CONFIG_DIR)?.map(PathBuf::from))
}

/// Team rules and task types: only the differences from the bundle (`team_config::Stored`, D-081). Written by the
/// shell after `team_config::diff` validated it, never by the generic setter.
pub const TEAM_CONFIG: &str = "team_config";
/// Agent names hidden from the agents page (D.Duck only; Claude Code is unaffected).
pub const HIDDEN_AGENTS: &str = "hidden_agents";
pub const MAX_HIDDEN_AGENTS: usize = 200;

pub fn team_config(conn: &Connection) -> Result<Option<String>, Error> {
    get(conn, TEAM_CONFIG)
}

pub fn set_team_config(conn: &Connection, stored: &crate::team_config::Stored) -> Result<(), Error> {
    let json = serde_json::to_string(stored).map_err(|_| Error::Config("team config"))?;
    put(conn, TEAM_CONFIG, &json)
}

pub fn clear_team_config(conn: &Connection) -> Result<(), Error> {
    delete(conn, TEAM_CONFIG)
}

pub fn hidden_agents(conn: &Connection) -> Result<Vec<String>, Error> {
    let raw = get(conn, HIDDEN_AGENTS)?.unwrap_or_default();
    let list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(list.into_iter().filter(|n| crate::agents_catalog::valid_name(n)).take(MAX_HIDDEN_AGENTS).collect())
}

pub fn set_agent_hidden(conn: &Connection, name: &str, hidden: bool) -> Result<(), Error> {
    if !crate::agents_catalog::valid_name(name) {
        return Err(Error::Config("agent name"));
    }
    let mut list = hidden_agents(conn)?;
    list.retain(|n| n != name);
    if hidden {
        if list.len() >= MAX_HIDDEN_AGENTS {
            return Err(Error::Config("too many hidden agents"));
        }
        list.push(name.to_string());
    }
    let json = serde_json::to_string(&list).map_err(|_| Error::Config("hidden agents"))?;
    put(conn, HIDDEN_AGENTS, &json)
}

pub const DEFAULT_CWD: &str = "default_cwd";

/// Folder pre-filled in the new-task form. Chosen in the native picker; must be an existing absolute directory.
pub fn set_default_cwd(conn: &Connection, dir: &Path) -> Result<(), Error> {
    if !dir.is_absolute() || !dir.is_dir() {
        return Err(Error::Config("default folder must be an existing absolute directory"));
    }
    put(conn, DEFAULT_CWD, &dir.to_string_lossy())
}

pub fn clear_default_cwd(conn: &Connection) -> Result<(), Error> {
    delete(conn, DEFAULT_CWD)
}

/// The stored default folder, or `None` if unset or no longer a directory.
pub fn default_cwd(conn: &Connection) -> Result<Option<PathBuf>, Error> {
    Ok(get(conn, DEFAULT_CWD)?.map(PathBuf::from).filter(|p| p.is_dir()))
}

/// Current project of the sidebar picker (project-switcher, D-111): an opaque id from `projects::known`, plus the
/// recently picked ids. Own getter/setter; never on the generic allowlist. Both re-filter against the known ids.
/// Legacy app-wide pick (D-111, superseded by per-tab picks D-127). Read nowhere now; kept so old rows are harmless.
pub const CURRENT_PROJECT: &str = "current_project";
pub const RECENT_PROJECTS: &str = "recent_projects";
/// Per-tab project picks (D-127): a JSON object `{page: id | null}`; null = "프로젝트 없음(전역만)" chosen on purpose.
pub const PAGE_PROJECTS: &str = "page_projects";
/// Tabs that pick their own project. Anything else is refused by the setter and dropped on read.
/// `review` (D-140) starts on 모든 프로젝트 and is left out of `picked_folders`.
pub const PROJECT_PAGES: &[&str] = &["permissions", "memory", "extensions", "mcp", "review"];

/// Tabs whose project adds skills and agents to the catalogs (`picked_folders`): every project tab but 리뷰, which only
/// narrows a list of past tasks.
pub fn catalog_pages() -> impl Iterator<Item = &'static str> {
    PROJECT_PAGES.iter().copied().filter(|p| *p != "review")
}
const MAX_RECENT: usize = 8;
const MAX_PAGE_PROJECTS_BYTES: usize = 1024;

fn known_id(id: &str, known: &[String]) -> bool {
    crate::projects::valid_id(id) && known.iter().any(|k| k == id)
}

/// Recent picks (shared by every tab), limited to ids that are still known.
pub fn recent_projects(conn: &Connection, known: &[String]) -> Result<Vec<String>, Error> {
    Ok(get(conn, RECENT_PROJECTS)?.unwrap_or_default().split('\n').filter(|id| known_id(id, known)).take(MAX_RECENT).map(str::to_string).collect())
}

/// Each tab's pick, re-filtered on read: unknown pages and ids that are no longer known are dropped (the tab then
/// falls back to the most recent project). An explicit "none" stays `None`.
pub fn page_projects(conn: &Connection, known: &[String]) -> Result<std::collections::BTreeMap<String, Option<String>>, Error> {
    let raw = get(conn, PAGE_PROJECTS)?.filter(|r| r.len() <= MAX_PAGE_PROJECTS_BYTES).unwrap_or_default();
    let map: std::collections::BTreeMap<String, Option<String>> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(map
        .into_iter()
        .filter(|(page, id)| PROJECT_PAGES.contains(&page.as_str()) && id.as_deref().is_none_or(|id| known_id(id, known)))
        .collect())
}

/// The tab's own setter. `page` must be a project tab and `id` (when given) one of `known`; a picked id moves to the
/// front of the shared recent list.
pub fn set_page_project(conn: &Connection, page: &str, id: Option<&str>, known: &[String]) -> Result<(), Error> {
    if !PROJECT_PAGES.contains(&page) {
        return Err(Error::Config("unknown page"));
    }
    if id.is_some_and(|id| !known_id(id, known)) {
        return Err(Error::Config("unknown project"));
    }
    let mut map = page_projects(conn, known)?;
    map.insert(page.to_string(), id.map(str::to_string));
    put(conn, PAGE_PROJECTS, &serde_json::to_string(&map).map_err(|_| Error::Config("page projects"))?)?;
    if let Some(id) = id {
        let mut list = vec![id.to_string()];
        list.extend(recent_projects(conn, known)?.into_iter().filter(|r| r != id));
        list.truncate(MAX_RECENT);
        put(conn, RECENT_PROJECTS, &list.join("\n"))?;
    }
    Ok(())
}

/// The project a tab uses: its own pick, else the most recently used one.
pub fn page_project(conn: &Connection, page: &str, known: &[String]) -> Result<Option<String>, Error> {
    let map = page_projects(conn, known)?;
    Ok(match map.get(page) {
        Some(pick) => pick.clone(),
        None => recent_projects(conn, known)?.into_iter().next(),
    })
}

/// First-run tour state (onboarding-tour, D-089; tour-v2, D-113). Own getter/setter; never on the generic allowlist.
pub const ONBOARDING_STATE: &str = "onboarding_state";
/// The tour version this build shows. A stored state below it with cards seen gets a one-time toast, not the deck.
pub const TOUR_VERSION: u32 = 2;
/// Version 1 card ids (tab by tab). Still accepted so older stored states stay valid.
pub const TOUR_CARDS_V1: &[&str] = &["welcome", "agents", "tasks", "outputs", "extend", "permissions", "memory", "usage", "finish"];
/// Version 2 card ids (the story, then the "first task" action), in tour order.
pub const TOUR_CARDS: &[&str] = &["solo", "roles", "brief", "board", "report", "start"];
const MAX_ONBOARDING_BYTES: usize = 2048;
const MAX_SEEN: usize = 32;

fn known_card(id: &str) -> bool {
    TOUR_CARDS.contains(&id) || TOUR_CARDS_V1.contains(&id)
}

/// Version 1 kept `timelineOpen`·`notifyOn`; version 2 adds `refineUsed`·`reportOpened`. Missing keys read as false,
/// so a version 1 value loads as is and keeps what it had.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnboardingChecklist {
    #[serde(default)]
    pub first_task: bool,
    #[serde(default)]
    pub timeline_open: bool,
    #[serde(default)]
    pub notify_on: bool,
    #[serde(default)]
    pub refine_used: bool,
    #[serde(default)]
    pub report_opened: bool,
}

/// Booleans and known card ids only: no paths, no content, no timestamps.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnboardingState {
    pub version: u32,
    pub seen: Vec<String>,
    #[serde(default)]
    pub dismissed: bool,
    #[serde(default)]
    pub checklist: OnboardingChecklist,
}

impl OnboardingState {
    /// What a damaged value becomes: everything seen, checklist done, so nothing pops up again.
    fn all_seen() -> Self {
        OnboardingState {
            version: TOUR_VERSION,
            seen: TOUR_CARDS_V1.iter().chain(TOUR_CARDS).map(|s| s.to_string()).collect(),
            dismissed: true,
            checklist: OnboardingChecklist { first_task: true, timeline_open: true, notify_on: true, refine_used: true, report_opened: true },
        }
    }

    fn validate(&self) -> Result<(), Error> {
        if self.seen.len() > MAX_SEEN {
            return Err(Error::Config("too many tour cards"));
        }
        if self.seen.iter().any(|id| !known_card(id)) {
            return Err(Error::Config("unknown tour card"));
        }
        Ok(())
    }

    /// Current cards not seen yet, in tour order.
    pub fn unseen(&self) -> Vec<&'static str> {
        TOUR_CARDS.iter().copied().filter(|c| !self.seen.iter().any(|s| s == c)).collect()
    }

    /// Someone who went through an older tour: offer the new one once with a toast instead of opening the deck.
    /// The webview mirrors this (tour.ts `wantsTourToast`) and stores `version = TOUR_VERSION` once shown.
    pub fn wants_tour_toast(&self) -> bool {
        self.version < TOUR_VERSION && !self.seen.is_empty() && !self.unseen().is_empty()
    }
}

fn parse_onboarding(raw: &str) -> Result<OnboardingState, Error> {
    if raw.len() > MAX_ONBOARDING_BYTES {
        return Err(Error::Config("onboarding state too large"));
    }
    let state: OnboardingState = serde_json::from_str(raw).map_err(|_| Error::Config("onboarding state"))?;
    state.validate()?;
    Ok(state)
}

/// `None` only on a fresh install (no value). A damaged value reads as "all seen" so the tour does not reappear.
pub fn onboarding_state(conn: &Connection) -> Result<Option<OnboardingState>, Error> {
    Ok(get(conn, ONBOARDING_STATE)?.map(|raw| parse_onboarding(&raw).unwrap_or_else(|_| OnboardingState::all_seen())))
}

/// Validates the webview's JSON (size, shape, known ids) before storing it.
pub fn set_onboarding_state(conn: &Connection, raw: &str) -> Result<(), Error> {
    let mut state = parse_onboarding(raw)?;
    state.seen.sort();
    state.seen.dedup();
    let json = serde_json::to_string(&state).map_err(|_| Error::Config("onboarding state"))?;
    put(conn, ONBOARDING_STATE, &json)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn limit_settings_default_on_and_backfill_flag_is_not_user_set() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(limit_notify(&conn).unwrap() && limit_auto_resume(&conn).unwrap());
        assert!(!limit_backfilled(&conn).unwrap());
        set_user_pref(&conn, LIMIT_AUTO_RESUME, "0").unwrap();
        set_user_pref(&conn, LIMIT_NOTIFY, "0").unwrap();
        assert!(!limit_notify(&conn).unwrap() && !limit_auto_resume(&conn).unwrap());
        assert!(set_user_pref(&conn, LIMIT_AUTO_RESUME, "yes").is_err());
        assert!(set_user_pref(&conn, LIMIT_BACKFILLED, "1").is_err(), "not on the generic allowlist");
        set_limit_backfilled(&conn).unwrap();
        assert!(limit_backfilled(&conn).unwrap());
    }

    #[test]
    fn page_projects_take_known_pages_and_ids_only() {
        let conn = crate::db::open_in_memory().unwrap();
        let known: Vec<String> = (0..10).map(|i| format!("{i:016x}")).collect();
        assert!(set_page_project(&conn, "memory", Some("/Users/me/dev"), &known).is_err());
        assert!(set_page_project(&conn, "memory", Some("ffffffffffffffff"), &known).is_err());
        assert!(set_page_project(&conn, "agents", Some(&known[0]), &known).is_err(), "not a project tab");
        assert!(set_user_pref(&conn, PAGE_PROJECTS, "{}").is_err(), "not on the generic allowlist");
        assert_eq!(page_project(&conn, "mcp", &known).unwrap(), None, "nothing picked, nothing recent");
        for k in &known {
            set_page_project(&conn, "memory", Some(k), &known).unwrap();
        }
        set_page_project(&conn, "permissions", Some(&known[3]), &known).unwrap();
        set_page_project(&conn, "extensions", None, &known).unwrap();
        // Each tab keeps its own; a tab with no pick follows the most recent one; an explicit none stays none.
        assert_eq!(page_project(&conn, "memory", &known).unwrap().as_deref(), Some(known[9].as_str()));
        assert_eq!(page_project(&conn, "permissions", &known).unwrap().as_deref(), Some(known[3].as_str()));
        assert_eq!(page_project(&conn, "mcp", &known).unwrap().as_deref(), Some(known[3].as_str()));
        assert_eq!(page_project(&conn, "extensions", &known).unwrap(), None);
        let recent = recent_projects(&conn, &known).unwrap();
        assert_eq!((recent.len(), recent[0].as_str()), (8, known[3].as_str()));
        // Re-filtered on read: a project that disappeared is dropped, and the tab falls back.
        let fewer: Vec<String> = known.iter().filter(|k| **k != known[3]).cloned().collect();
        assert!(!page_projects(&conn, &fewer).unwrap().contains_key("permissions"));
        assert_eq!(page_project(&conn, "permissions", &fewer).unwrap().as_deref(), Some(known[9].as_str()));
        assert!(!recent_projects(&conn, &fewer).unwrap().contains(&known[3]));
        // Junk or oversized stored values read as nothing.
        put(&conn, PAGE_PROJECTS, &format!("{{\"memory\":\"{}\",\"x\":\"y\"}}", known[1])).unwrap();
        assert_eq!(page_projects(&conn, &known).unwrap().len(), 1);
        put(&conn, PAGE_PROJECTS, &"x".repeat(2000)).unwrap();
        assert!(page_projects(&conn, &known).unwrap().is_empty());
    }

    #[test]
    fn review_tab_picks_a_project_but_adds_nothing_to_catalogs() {
        let conn = crate::db::open_in_memory().unwrap();
        let known = vec!["0000000000000001".to_string()];
        set_page_project(&conn, "review", Some(&known[0]), &known).unwrap();
        assert_eq!(page_project(&conn, "review", &known).unwrap().as_deref(), Some(known[0].as_str()));
        assert!(!catalog_pages().any(|p| p == "review"));
        assert_eq!(catalog_pages().count(), PROJECT_PAGES.len() - 1);
    }

    #[test]
    fn review_since_is_written_once() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(review_since(&conn, 1000).unwrap(), 1000);
        assert_eq!(review_since(&conn, 5000).unwrap(), 1000, "never moves");
        assert!(set_user_pref(&conn, REVIEW_SINCE, "0").is_err(), "not on the generic allowlist");
        put(&conn, REVIEW_SINCE, "junk").unwrap();
        assert_eq!(review_since(&conn, 7000).unwrap(), 7000, "junk is replaced once");
        assert_eq!(review_since(&conn, 9000).unwrap(), 7000);
    }

    #[test]
    fn generic_setter_rejects_security_keys_and_bad_values() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(set_user_pref(&conn, CLI_PATH, "/tmp/evil").is_err());
        assert!(set_user_pref(&conn, CONFIG_DIR, "/tmp").is_err());
        assert!(set_user_pref(&conn, "store_titles", "1").is_err(), "titles use the scrubbing setter");
        assert!(set_user_pref(&conn, NOTIFY_WAITING, "yes").is_err());
        assert_eq!(cli_path(&conn).unwrap(), None);
        set_user_pref(&conn, NOTIFY_WAITING, "0").unwrap();
        assert!(!notify_waiting(&conn).unwrap());
        assert!(task_report(&conn).unwrap(), "task report is on by default");
        assert!(set_user_pref(&conn, TASK_REPORT, "off").is_err());
        set_user_pref(&conn, TASK_REPORT, "0").unwrap();
        assert!(!task_report(&conn).unwrap());
    }

    #[test]
    fn ui_language_takes_three_values_and_junk_reads_as_system() {
        use crate::i18n::Lang;
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(ui_language(&conn).unwrap(), "system", "default");
        for v in ["ko", "en", "system"] {
            set_user_pref(&conn, UI_LANGUAGE, v).unwrap();
            assert_eq!(ui_language(&conn).unwrap(), v);
        }
        for bad in ["ja", "KO", "en-US", ""] {
            assert!(set_user_pref(&conn, UI_LANGUAGE, bad).is_err(), "{bad}");
        }
        put(&conn, UI_LANGUAGE, "fr").unwrap();
        assert_eq!(ui_language(&conn).unwrap(), "system", "junk in the DB is not trusted");
        assert_eq!(resolve_language("ko", || Lang::En), Lang::Ko);
        assert_eq!(resolve_language("en", || Lang::Ko), Lang::En);
        assert_eq!(resolve_language("system", || Lang::Ko), Lang::Ko);
        assert_eq!(resolve_language("system", || Lang::En), Lang::En);
    }

    #[test]
    fn review_capacity_can_be_unlimited() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(review_capacity(&conn).unwrap(), ReviewCapacity::Limit(3), "default 3");
        set_user_pref(&conn, REVIEW_CAPACITY, "7").unwrap();
        assert_eq!(review_capacity(&conn).unwrap(), ReviewCapacity::Limit(7));
        set_user_pref(&conn, REVIEW_CAPACITY, REVIEW_UNLIMITED).unwrap();
        assert_eq!(review_capacity(&conn).unwrap(), ReviewCapacity::Unlimited(Unlimited::Unlimited));
        assert_eq!(serde_json::to_string(&review_capacity(&conn).unwrap()).unwrap(), r#""unlimited""#);
        assert_eq!(serde_json::to_string(&ReviewCapacity::Limit(4)).unwrap(), "4");
        for bad in ["0", "11", "9999", "Unlimited", "none"] {
            assert!(set_user_pref(&conn, REVIEW_CAPACITY, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hidden_agents_and_team_config_have_own_setters() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(set_user_pref(&conn, TEAM_CONFIG, "{}").is_err());
        assert!(set_user_pref(&conn, HIDDEN_AGENTS, "[]").is_err());
        set_agent_hidden(&conn, "statusline-setup", true).unwrap();
        set_agent_hidden(&conn, "statusline-setup", true).unwrap();
        assert_eq!(hidden_agents(&conn).unwrap(), ["statusline-setup"]);
        assert!(set_agent_hidden(&conn, "--evil", true).is_err());
        set_agent_hidden(&conn, "statusline-setup", false).unwrap();
        assert!(hidden_agents(&conn).unwrap().is_empty());
    }

    #[test]
    fn default_cwd_roundtrip() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(default_cwd(&conn).unwrap(), None);
        assert!(set_default_cwd(&conn, Path::new("/definitely/not/here")).is_err());
        assert!(set_user_pref(&conn, DEFAULT_CWD, "/tmp").is_err(), "only the picker may set it");
        let d = tempfile::tempdir().unwrap();
        set_default_cwd(&conn, d.path()).unwrap();
        assert_eq!(default_cwd(&conn).unwrap().as_deref(), Some(d.path()));
        clear_default_cwd(&conn).unwrap();
        assert_eq!(default_cwd(&conn).unwrap(), None);
    }

    #[test]
    fn config_dir_must_exist() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(set_config_dir(&conn, Path::new("relative")).is_err());
        assert!(set_config_dir(&conn, Path::new("/definitely/not/here")).is_err());
        let d = tempfile::tempdir().unwrap();
        set_config_dir(&conn, d.path()).unwrap();
        assert_eq!(config_dir(&conn).unwrap().as_deref(), Some(d.path()));
    }

    #[test]
    fn onboarding_state_roundtrip_and_validation() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(onboarding_state(&conn).unwrap(), None, "fresh install");
        let raw = r#"{"version":1,"seen":["welcome","agents","agents"],"dismissed":true,"checklist":{"firstTask":true,"timelineOpen":false,"notifyOn":false}}"#;
        // A version 1 value (old checklist keys only) is still accepted.
        set_onboarding_state(&conn, raw).unwrap();
        let s = onboarding_state(&conn).unwrap().unwrap();
        assert_eq!(s.seen, ["agents", "welcome"]);
        assert!(s.dismissed && s.checklist.first_task && !s.checklist.notify_on);
        // Oversize, unknown keys, unknown ids, wrong types and broken JSON are all refused; the stored value stays.
        let big = format!(r#"{{"version":1,"seen":[],"dismissed":false{}}}"#, " ".repeat(3000));
        for bad in [
            big.as_str(),
            r#"{"version":1,"seen":[],"path":"/Users/x"}"#,
            r#"{"version":1,"seen":[],"checklist":{"firstTask":true,"note":true}}"#,
            r#"{"version":1,"seen":["../etc"]}"#,
            r#"{"version":"1","seen":[]}"#,
            r#"{"version":1,"seen":[],"checklist":{"firstTask":"yes"}}"#,
            r#"{"version":1,"seen":"#,
        ] {
            assert!(set_onboarding_state(&conn, bad).is_err(), "{bad}");
        }
        let many = format!(r#"{{"version":1,"seen":[{}]}}"#, vec![r#""agents""#; 33].join(","));
        assert!(set_onboarding_state(&conn, &many).is_err());
        assert_eq!(onboarding_state(&conn).unwrap().unwrap(), s);
    }

    #[test]
    fn onboarding_state_has_no_generic_path_and_corruption_reads_as_seen() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(set_user_pref(&conn, ONBOARDING_STATE, "{}").is_err());
        put(&conn, ONBOARDING_STATE, "{not json").unwrap();
        let s = onboarding_state(&conn).unwrap().unwrap();
        assert!(s.unseen().is_empty() && s.checklist.notify_on && s.checklist.report_opened);
        assert!(!s.wants_tour_toast());
    }

    #[test]
    fn onboarding_version_bump_leaves_only_new_cards() {
        let conn = crate::db::open_in_memory().unwrap();
        // A build that knew every v2 card but report.
        let older: Vec<String> = TOUR_CARDS.iter().filter(|c| **c != "report").map(|c| format!("\"{c}\"")).collect();
        set_onboarding_state(&conn, &format!(r#"{{"version":2,"seen":[{}]}}"#, older.join(","))).unwrap();
        assert_eq!(onboarding_state(&conn).unwrap().unwrap().unseen(), ["report"]);
        let all: Vec<String> = TOUR_CARDS.iter().map(|c| format!("\"{c}\"")).collect();
        set_onboarding_state(&conn, &format!(r#"{{"version":2,"seen":[{}]}}"#, all.join(","))).unwrap();
        assert!(onboarding_state(&conn).unwrap().unwrap().unseen().is_empty(), "update without new cards shows nothing");
    }

    #[test]
    fn onboarding_v1_state_migrates_and_gets_one_toast() {
        let conn = crate::db::open_in_memory().unwrap();
        // Exactly what a version 1 build stored.
        let v1: Vec<String> = TOUR_CARDS_V1.iter().map(|c| format!("\"{c}\"")).collect();
        let raw = format!(r#"{{"version":1,"seen":[{}],"dismissed":false,"checklist":{{"firstTask":true,"timelineOpen":true,"notifyOn":false}}}}"#, v1.join(","));
        set_onboarding_state(&conn, &raw).unwrap();
        let s = onboarding_state(&conn).unwrap().unwrap();
        // Old keys kept, new keys start false; every v2 card is unseen, so the toast is offered (not the deck).
        assert!(s.checklist.first_task && s.checklist.timeline_open && !s.checklist.notify_on);
        assert!(!s.checklist.refine_used && !s.checklist.report_opened);
        assert_eq!(s.unseen(), TOUR_CARDS);
        assert!(s.wants_tour_toast());
        // Once the toast was shown the webview stores the new version with the v2 cards seen: no second toast, no deck.
        let both: Vec<String> = TOUR_CARDS_V1.iter().chain(TOUR_CARDS).map(|c| format!("\"{c}\"")).collect();
        let shown = format!(r#"{{"version":2,"seen":[{}],"checklist":{{"firstTask":true,"timelineOpen":true,"refineUsed":true}}}}"#, both.join(","));
        set_onboarding_state(&conn, &shown).unwrap();
        let s = onboarding_state(&conn).unwrap().unwrap();
        assert!(!s.wants_tour_toast() && s.unseen().is_empty() && s.checklist.refine_used);
        // A fresh install (nothing seen) never gets the toast.
        assert!(!OnboardingState { version: 1, ..Default::default() }.wants_tour_toast());
        assert!(set_onboarding_state(&conn, r#"{"version":2,"seen":["tour-v3"]}"#).is_err());
    }
}
