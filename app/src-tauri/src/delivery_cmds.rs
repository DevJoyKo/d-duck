//! "원격 push 막기" (delivery-guard, D-142·D-143): per-project local-only list and the Composer scope line.
//! The webview sends opaque project ids (or the Composer's folder for an unregistered one, ticking only); unticking
//! goes through a backend-owned native confirmation. A list that cannot be read is an error, never "draft PR".

use dduck_core::i18n::Fail;
use std::path::{Path, PathBuf};

use dduck_core::delivery_guard::{self as dg, Guard};
use dduck_core::projects;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, Res, err, home_dir, projects_cmds};
use dduck_core::i18n::{Code as C, ReasonCode, tr};

fn home() -> Option<PathBuf> {
    Some(home_dir()).filter(|h| !h.as_os_str().is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalOnlyRow {
    pub id: String,
    pub name: String,
    pub parent_dir: Option<String>,
    pub checked: bool,
    /// The ticked folder no longer exists (the tick stays).
    pub missing: bool,
    /// Worktrees follow their repository and cannot be ticked on their own.
    pub worktree_of: Option<String>,
    /// Why it cannot be ticked (home folder, no folder).
    pub off_reason: Option<String>,
    /// Fixed code of `off_reason`: `homeNotAllowed`, or a `projects::off_code` value.
    pub off_code: Option<&'static str>,
}

fn name_of(p: &Path) -> (String, Option<String>) {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned());
    (name, p.parent().map(|d| d.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn local_only_projects(app: AppHandle) -> Res<Vec<LocalOnlyRow>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let items = dg::load_items(&*state.conn()?).map_err(|_| dg::UNREADABLE.to_string())?;
        let known = projects_cmds::known(&state)?;
        let mut rows: Vec<LocalOnlyRow> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for p in known.iter().filter(|p| !p.empty || p.path.is_some()) {
            let Some(path) = p.path.as_deref().map(PathBuf::from) else { continue };
            let checked = dg::is_ticked(&items, &path);
            seen.push(dg::norm_key(&path));
            rows.push(LocalOnlyRow {
                id: p.id.clone(),
                name: p.name.clone(),
                parent_dir: p.parent_dir.clone(),
                checked,
                missing: matches!(p.folder, projects::Folder::Missing),
                worktree_of: p.worktree_of.clone(),
                off_reason: if p.is_home { Some(Fail::HomeNotAllowed.now()) } else { p.off_reason.map(str::to_string).filter(|_| !checked) },
                off_code: if p.is_home { Some("homeNotAllowed") } else { p.off_code.filter(|_| !checked) },
            });
        }
        // Ticked folders the registry no longer lists stay visible (and can be unticked).
        for it in &items {
            let path = PathBuf::from(&it.folder);
            if seen.contains(&dg::norm_key(&path)) {
                continue;
            }
            let (name, parent_dir) = name_of(&path);
            rows.push(LocalOnlyRow { id: projects::path_id(&path), name, parent_dir, checked: true, missing: !path.is_dir(), worktree_of: None, off_reason: None, off_code: None });
        }
        Ok(rows)
    })
    .await
    .map_err(err)?
}

/// Ticks (directly) or unticks (after a native confirmation) a project by its opaque id.
#[tauri::command]
pub async fn set_local_only(app: AppHandle, id: String, on: bool) -> Res<bool> {
    if on && !cfg!(target_os = "macos") {
        return Err(ReasonCode::MacOnly.now());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if !projects::valid_id(&id) || id.starts_with("key:") {
            return Err(Fail::ProjectNotListed.now());
        }
        if on {
            let folder = projects_cmds::usable_folder(&state, &id)?;
            dg::add(&*state.conn()?, &folder, home().as_deref()).map_err(|_| dg::UNREADABLE.to_string())?;
            return Ok(true);
        }
        let items = dg::load_items(&*state.conn()?).map_err(|_| dg::UNREADABLE.to_string())?;
        let Some(item) = items.iter().find(|i| projects::path_id(Path::new(&i.folder)) == id).cloned() else { return Ok(false) };
        let ok = {
            let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
            app.dialog()
                .message(tr(C::DlgGuardOffBody))
                .title(tr(C::DlgGuardOffTitle))
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnAllow), tr(C::BtnCancel)))
                .blocking_show()
        };
        if !ok {
            return Ok(true);
        }
        dg::remove(&*state.conn()?, Path::new(&item.folder)).map_err(|_| dg::UNREADABLE.to_string())?;
        Ok(false)
    })
    .await
    .map_err(err)?
}

/// Composer: ticks the chosen folder even when the registry does not list it yet. Ticking only.
#[tauri::command]
pub async fn set_local_only_cwd(app: AppHandle, cwd: String) -> Res<()> {
    if !cfg!(target_os = "macos") {
        return Err(ReasonCode::MacOnly.now());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let folder = dg::checked_folder(Path::new(&cwd)).ok_or_else(|| Fail::FolderUnchecked.now())?;
        let c = projects_cmds::ctx(&state)?;
        if let Some(reason) = projects::home_like(&folder, &c.env()) {
            return Err(reason.to_string());
        }
        dg::add(&*state.conn()?, &folder, home().as_deref()).map_err(|_| dg::UNREADABLE.to_string())
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardPreview {
    pub guard: Guard,
    /// The folder itself is ticked (vs. inherited from its repository or a parent).
    pub ticked: bool,
    /// The "이 프로젝트는 로컬만" button applies (draft scope, a folder that can be ticked).
    pub can_tick: bool,
    /// Starting here would move the task into a worktree (default branch checked out).
    pub forced_worktree: bool,
}

/// The Composer line: the same lookup the start uses.
#[tauri::command]
pub async fn delivery_guard_preview(app: AppHandle, cwd: String) -> Res<GuardPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let folder = dg::checked_folder(Path::new(&cwd)).ok_or_else(|| Fail::FolderUnchecked.now())?;
        // Released before `projects_cmds::ctx`, which locks the connection itself (D-161: holding it here froze the app).
        let items = dg::load_items(&*state.conn()?).map_err(|_| dg::UNREADABLE.to_string())?;
        let h = home();
        let repo = dg::repo_info(&folder, h.as_deref()).map_err(|_| dg::UNREADABLE.to_string())?;
        let guard = dg::guard_for(&folder, repo.as_ref(), &items);
        let c = projects_cmds::ctx(&state)?;
        let can_tick = guard == Guard::DraftPr && projects::home_like(&folder, &c.env()).is_none();
        // Same condition as the start: only when a worktree can actually be made here.
        let worktree_possible = dduck_core::team::repo_status(&folder, &home_dir()) == dduck_core::team::RepoStatus::Ok;
        let forced_worktree = match &repo {
            Some(r) if worktree_possible => {
                let detected = dg::detect_default_branch(&r.root).map_err(|_| dg::UNREADABLE.to_string())?;
                let start = dg::start_branch(&folder).map_err(|_| dg::UNREADABLE.to_string())?;
                dg::on_default(start.as_deref(), detected.as_deref())
            }
            _ => false,
        };
        Ok(GuardPreview { guard, ticked: dg::is_ticked(&items, &folder), can_tick, forced_worktree })
    })
    .await
    .map_err(err)?
}

/// The scope for a start or resume in `cwd`, with the app executable as the local-only hook (self-tested).
/// Any error refuses the start; nothing falls back to `draft_pr`.
pub(crate) fn start_guard(state: &AppState, cwd: &Path, worktree_possible: bool) -> Result<dg::StartCtx, &'static str> {
    let exe = std::env::current_exe().map_err(|_| dg::HOOK_MISSING)?;
    // The DB only for the list; git reads and the hook self-test run without holding it (D-161).
    let items = dg::load_items(&*state.conn().map_err(|_| dg::UNREADABLE)?).map_err(|_| dg::UNREADABLE)?;
    let h = home();
    let global = h.as_deref().map(dg::global_config_path);
    dg::start_ctx_for_items(&items, cwd, h.as_deref(), worktree_possible, Some(&exe), global.as_deref())
}

/// The cloud-routine check: `host/owner/repo` of every ticked project's `origin` (offline). A list that cannot be
/// read blocks every cloud routine (fail closed).
pub(crate) fn cloud_guard(state: &AppState) -> dg::CloudGuard {
    let conn = state.conn().ok();
    dg::CloudGuard::load(conn.as_deref(), home().as_deref())
}

/// Managed sources for the hook-lock check, with the app's Claude config folder.
pub(crate) fn managed_sources(state: &AppState) -> dg::ManagedSources {
    let cfg = state.config.read().ok().map(|c| c.path.clone());
    dg::ManagedSources::from_env(cfg.as_deref())
}
