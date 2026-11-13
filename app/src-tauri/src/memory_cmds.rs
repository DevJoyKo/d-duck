//! Memory tab (memory-tab, D-091): Claude Code auto memory and CLAUDE.md files. The webview sends a project id from
//! `memory_projects`, a file name from `memory_entries` and hash tokens — never a path. Memory text is returned for
//! display only; nothing is stored, cached or logged (the "seen" set below holds hashes only, in memory).

use dduck_core::i18n::Fail;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use dduck_core::i18n::{Code as C, Params as P, ReasonCode, tr, trp};
use dduck_core::memory::{self, AutoMemory, Doc, Edit, InstructionFile, MemoryList, ProjectRow};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, Res, chrono_now_ms, err, home_dir, permission_projects};

/// Hashes of memory files the user opened or the app wrote in this run: not "새로 바뀜".
static SEEN: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
const FRESH_MS: i64 = 24 * 3_600_000;

fn seen(token: &str) {
    if let Ok(mut s) = SEEN.lock() {
        if s.len() > 5000 {
            s.clear();
        }
        s.insert(token.to_string());
    }
}

fn config_dir(state: &AppState) -> Res<PathBuf> {
    Ok(state.config.read().map_err(err)?.path.clone())
}

/// Folders whose Claude Code project key we can name: usage history, known projects and home.
fn candidates(state: &AppState) -> Res<Vec<PathBuf>> {
    let mut out = permission_projects(state)?;
    {
        let conn = state.conn()?;
        let mut stmt = conn.prepare("SELECT DISTINCT project_path FROM sessions WHERE project_path IS NOT NULL LIMIT 1000").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        out.extend(rows.filter_map(Result::ok).map(PathBuf::from));
    }
    out.push(home_dir());
    Ok(out)
}

/// Memory folder of a project id from `projects_known` (project-switcher: ids only, never a key or path).
fn dir_for(state: &AppState, project: &str) -> Res<PathBuf> {
    let key = crate::projects_cmds::memory_key(state, project)?;
    let cfg = config_dir(state)?;
    let custom = memory::auto_memory_directory(&cfg, &home_dir());
    memory::memory_dir(&cfg, custom.as_deref(), &key).map_err(|e| e.to_string())
}

async fn blocking<T: Send + 'static>(app: AppHandle, f: impl FnOnce(&AppState) -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>())).await.map_err(err)?
}

/// A memory project row plus how many rules its folder has, so projects with rules but no memory are listed too.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPayload {
    #[serde(flatten)]
    row: ProjectRow,
    rules: usize,
}

/// `None` when the home folder is unknown: project-scope rules are then refused (rules-editor).
fn home_opt() -> Option<PathBuf> {
    Some(home_dir()).filter(|h| !h.as_os_str().is_empty())
}

#[tauri::command]
pub async fn memory_projects(app: AppHandle) -> Res<Vec<ProjectPayload>> {
    blocking(app, |state| {
        let cfg = config_dir(state)?;
        let custom = memory::auto_memory_directory(&cfg, &home_dir());
        let home = home_opt();
        Ok(memory::projects(&cfg, custom.as_deref(), &candidates(state)?)
            .into_iter()
            .map(|row| {
                let rules = row.path.as_deref().map_or(0, |p| rules::project_rule_count(std::path::Path::new(p), home.as_deref()));
                ProjectPayload { row, rules }
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntriesPayload {
    #[serde(flatten)]
    list: MemoryList,
    /// `autoMemoryEnabled` in this project's own settings files (read-only): (file label, value).
    project_settings: Vec<(String, bool)>,
}

#[tauri::command]
pub async fn memory_entries(app: AppHandle, project: String) -> Res<EntriesPayload> {
    blocking(app, move |state| {
        let dir = dir_for(state, &project)?;
        let mut list = memory::list(&dir);
        let now = chrono_now_ms();
        if let Ok(s) = SEEN.lock() {
            for e in &mut list.entries {
                e.fresh = now - e.mtime_ms < FRESH_MS && !e.token.is_empty() && !s.contains(&e.token);
            }
        }
        let mut project_settings = Vec::new();
        // Home's settings are the global ones: not repeated as project settings.
        if let Ok(p) = crate::projects_cmds::usable_folder(state, &project) {
            for f in [".claude/settings.json", ".claude/settings.local.json"] {
                if let Some(v) = memory::auto_memory_value(&p.join(f)) {
                    project_settings.push((f.to_string(), v));
                }
            }
        }
        Ok(EntriesPayload { list, project_settings })
    })
    .await
}

#[tauri::command]
pub async fn memory_read(app: AppHandle, project: String, file: String) -> Res<Doc> {
    blocking(app, move |state| {
        let doc = memory::read(&dir_for(state, &project)?, &file).map_err(|e| e.to_string())?;
        seen(&doc.token);
        Ok(doc)
    })
    .await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEdit {
    name: String,
    description: String,
    kind: String,
    body: String,
}

fn remember_written(state: &AppState, project: &str, file: &str) {
    if let Ok(dir) = dir_for(state, project) {
        if let Ok(d) = memory::read(&dir, file) {
            seen(&d.token);
        }
    }
}

#[tauri::command]
pub async fn save_memory(app: AppHandle, project: String, file: String, token: String, index_token: String, edit: MemoryEdit) -> Res<Option<String>> {
    blocking(app, move |state| {
        let dir = dir_for(state, &project)?;
        let e = Edit { name: edit.name, description: edit.description, kind: edit.kind, body: edit.body };
        let note = memory::save(&dir, &file, &token, &index_token, &e).map_err(|e| e.to_string())?;
        remember_written(state, &project, &file);
        Ok(note)
    })
    .await
}

#[tauri::command]
pub async fn set_memory_enabled(app: AppHandle, project: String, file: String, enabled: bool, token: String, index_token: String) -> Res<Option<String>> {
    blocking(app, move |state| {
        let dir = dir_for(state, &project)?;
        let note = memory::set_enabled(&dir, &file, enabled, &token, &index_token).map_err(|e| e.to_string())?;
        remember_written(state, &project, &file);
        Ok(note)
    })
    .await
}

#[tauri::command]
pub async fn trash_memory(app: AppHandle, project: String, file: String, token: String, index_token: String) -> Res<bool> {
    let handle = app.clone();
    blocking(app, move |state| {
        let dir = dir_for(state, &project)?;
        let plan = memory::plan_trash(&dir, &file, &token, &index_token).map_err(|e| e.to_string())?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = handle
            .dialog()
            .message(trp(C::DlgMemTrashBody, &P::new().text("name", &plan.file, 100).text("path", &plan.path.display().to_string(), 300)))
            .title(tr(C::DlgMemTrashTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTrash), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        memory::recheck_trash(&dir, &plan).map_err(|_| Fail::ChangedWhileChecking.now())?;
        let in_bin = crate::move_to_trash(&handle, &plan.path)?;
        memory::forget_after_trash(&dir, &plan.file, plan.enabled).map_err(|_| Fail::MemoryLineLeft.now())?;
        crate::trash_unconfirmed(in_bin)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
pub async fn auto_memory_setting(app: AppHandle) -> Res<AutoMemory> {
    blocking(app, |state| Ok(memory::auto_memory(&config_dir(state)?, &home_dir()))).await
}

/// Global `autoMemoryEnabled`: only that key's value changes, after a native confirmation.
#[tauri::command]
pub async fn set_auto_memory(app: AppHandle, enabled: bool, token: String) -> Res<bool> {
    let handle = app.clone();
    blocking(app, move |state| {
        let cfg = config_dir(state)?;
        let plan = memory::plan_auto_memory(&cfg, &home_dir(), enabled, &token).map_err(|e| e.to_string())?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let text = tr(if enabled { C::DlgAutoMemOn } else { C::DlgAutoMemOff });
        let ok = handle
            .dialog()
            .message(text)
            .title(tr(C::DlgAutoMemTitle))
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(if enabled { C::BtnTurnOn } else { C::BtnTurnOff }), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        memory::commit_auto_memory(&plan).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
}

/// `None` = global only (the user's CLAUDE.md). Home and folder-less projects add nothing on the project side.
fn instruction_list(state: &AppState, project: Option<&str>) -> Res<Vec<(InstructionFile, PathBuf)>> {
    let cfg = config_dir(state)?;
    let path = match project {
        Some(id) => crate::projects_cmds::usable_folder(state, id).ok(),
        None => None,
    };
    Ok(memory::instruction_files(&cfg, path.as_deref()))
}

#[tauri::command]
pub async fn instruction_files(app: AppHandle, project: Option<String>) -> Res<Vec<InstructionFile>> {
    blocking(app, move |state| Ok(instruction_list(state, project.as_deref())?.into_iter().map(|(f, _)| f).collect())).await
}

static LAST_OPEN: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Opens a file in the default text editor: `/usr/bin/open -t <absolute path>`, at most once a second.
fn open_in_editor(path: &std::path::Path) -> Res<()> {
    {
        let mut last = LAST_OPEN.lock().map_err(err)?;
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return Err(Fail::TryLater.now());
        }
        *last = Some(Instant::now());
    }
    if !path.is_absolute() {
        return Err(Fail::BadPathValue.now());
    }
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/open")
            .arg("-t")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if status.is_ok_and(|s| s.success()) { Ok(()) } else { Err(Fail::EditorFailed.now()) }
    }
    // Windows: Notepad from System32, `.md`/`.txt` only (decision 6).
    #[cfg(windows)]
    {
        if dduck_core::platform::shell::open_in_editor(path) { Ok(()) } else { Err(Fail::EditorFailed.now()) }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(Fail::UnsupportedOs.now().into())
    }
}

#[tauri::command]
pub async fn open_instruction_file(app: AppHandle, project: Option<String>, id: String) -> Res<()> {
    blocking(app, move |state| {
        let (f, path) = instruction_list(state, project.as_deref())?.into_iter().find(|(f, _)| f.id == id).ok_or_else(|| Fail::FileNotListed.now())?;
        // A linked CLAUDE.md could point anywhere; it is listed but not opened from here.
        if f.link || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            return Err(Fail::LinkedInstruction.now());
        }
        open_in_editor(&path)
    })
    .await
}

/// MEMORY.md itself is edited outside the app (to tidy duplicate lines, for example).
#[tauri::command]
pub async fn open_memory_index(app: AppHandle, project: String) -> Res<()> {
    blocking(app, move |state| {
        let p = dir_for(state, &project)?.join("MEMORY.md");
        if !std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_file()) {
            return Err(Fail::MemoryIndexMissing.now());
        }
        open_in_editor(&p)
    })
    .await
}

// ---------- rules (rules-editor, D-106) ----------
// The webview sends a memory project id (resolved to a folder the app already knows), an opaque rule id from
// `rules_list`, hash tokens and the new text. Rule text is never logged, stored or put into an error.

use dduck_core::rules::{self, RuleDoc, RuleGroup, RuleTemplate, Roots, Scope};

fn rule_roots(state: &AppState, project: Option<&str>) -> Res<Roots> {
    // A project id from `projects_known`; home stays in (rules.rs lists it as "홈 폴더는 전역과 같아요").
    let project = match project.filter(|p| !p.is_empty()) {
        Some(id) => crate::projects_cmds::folder(state, id)?,
        None => None,
    };
    Ok(Roots { config_dir: config_dir(state)?, project, home: home_opt() })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesPayload {
    global: Option<RuleGroup>,
    /// `None` when no project is picked or its folder is unknown.
    project: Option<RuleGroup>,
    templates: Vec<RuleTemplate>,
}

#[tauri::command]
pub async fn rules_list(app: AppHandle, project: Option<String>) -> Res<RulesPayload> {
    blocking(app, move |state| {
        let roots = rule_roots(state, project.as_deref())?;
        Ok(RulesPayload { global: rules::list_group(&roots, Scope::Global), project: rules::list_group(&roots, Scope::Project), templates: rules::templates() })
    })
    .await
}

#[tauri::command]
pub async fn rule_read(app: AppHandle, project: Option<String>, id: String) -> Res<RuleDoc> {
    blocking(app, move |state| rules::read(&rule_roots(state, project.as_deref())?, &id).map_err(|e| e.to_string())).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSave {
    /// Existing rule: id and token. New rule: scope and name instead.
    id: Option<String>,
    token: Option<String>,
    scope: Option<Scope>,
    name: Option<String>,
    body: String,
    /// `None` keeps the frontmatter as it is (complex frontmatter: body only).
    paths: Option<Vec<String>>,
}

/// Saves an existing rule or creates a new one. Returns the rule id.
#[tauri::command]
pub async fn save_rule(app: AppHandle, project: Option<String>, rule: RuleSave) -> Res<String> {
    blocking(app, move |state| {
        let roots = rule_roots(state, project.as_deref())?;
        match (rule.id, rule.scope, rule.name) {
            (Some(id), _, _) => {
                rules::save(&roots, &id, rule.token.as_deref().unwrap_or(""), &rule.body, rule.paths.as_deref()).map_err(|e| e.to_string())?;
                Ok(id)
            }
            (None, Some(scope), Some(name)) => rules::create(&roots, scope, &name, &rule.body, rule.paths.as_deref().unwrap_or(&[])).map_err(|e| e.to_string()),
            _ => Err(Fail::UnknownRule.now()),
        }
    })
    .await
}

#[tauri::command]
pub async fn set_rule_enabled(app: AppHandle, project: Option<String>, id: String, enabled: bool, settings_token: String) -> Res<()> {
    blocking(app, move |state| rules::set_enabled(&rule_roots(state, project.as_deref())?, &id, enabled, &settings_token).map_err(|e| e.to_string())).await
}

#[tauri::command]
pub async fn trash_rule(app: AppHandle, project: Option<String>, id: String, token: String) -> Res<bool> {
    let handle = app.clone();
    blocking(app, move |state| {
        let roots = rule_roots(state, project.as_deref())?;
        let plan = rules::plan_trash(&roots, &id, &token).map_err(|e| e.to_string())?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = handle
            .dialog()
            .message(trp(C::DlgRuleTrashBody, &P::new().text("name", &plan.rel, 120).text("path", &plan.path.display().to_string(), 300)))
            .title(tr(C::DlgRuleTrashTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTrash), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        rules::recheck_trash(&plan).map_err(|_| Fail::ChangedWhileChecking.now())?;
        let in_bin = crate::move_to_trash(&handle, &plan.path)?;
        crate::trash_unconfirmed(in_bin)?;
        Ok(true)
    })
    .await
}
