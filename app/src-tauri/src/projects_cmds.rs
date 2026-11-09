//! Sidebar project picker (project-switcher, D-111). The webview gets opaque project ids from `projects_known` and
//! sends only those back; every scope command resolves them here again (links excluded, home refused). No path is
//! logged or put into an error.

use dduck_core::i18n::Fail;
use std::path::PathBuf;

use dduck_core::projects::{self, Candidate, Env, KnownProject};
use dduck_core::{memory, rules, settings, task_meta};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{AppState, Res, base_catalog_roots, catalog_roots, err, home_dir};

/// Everything a resolve needs, read once per command.
pub(crate) struct Ctx {
    cfg: PathBuf,
    custom: Option<PathBuf>,
    home: Option<PathBuf>,
    cands: Vec<Candidate>,
}

impl Ctx {
    pub(crate) fn env(&self) -> Env<'_> {
        Env { config_dir: &self.cfg, custom: self.custom.as_deref(), home: self.home.as_deref() }
    }

    fn raw(&self) -> Vec<PathBuf> {
        self.cands.iter().map(|c| c.path.clone()).collect()
    }
}

/// Folders the app has seen: agent catalog roots (default folder, routines, live sessions), D.Duck tasks, usage
/// history and home — as given, so links can be told apart.
pub(crate) fn ctx(state: &AppState) -> Res<Ctx> {
    ctx_with(state, true)
}

/// `catalog: false` uses the catalog roots without the current project, for `catalog_roots` itself (which adds it):
/// the default folder, routines and live sessions still count (QA LOW: a project known only from them).
fn ctx_with(state: &AppState, catalog: bool) -> Res<Ctx> {
    let cfg = state.config.read().map_err(err)?.path.clone();
    let home = Some(home_dir()).filter(|h| !h.as_os_str().is_empty());
    let custom = memory::auto_memory_directory(&cfg, &home_dir());
    let roots = if catalog { catalog_roots(state)? } else { base_catalog_roots(state)? };
    let mut cands: Vec<Candidate> = roots.project_dirs.into_iter().map(|path| Candidate { path, last_active: None }).collect();
    {
        let conn = state.conn()?;
        for r in task_meta::records(&conn, 200).map_err(err)? {
            if let Some(c) = r.cwd {
                cands.push(Candidate { path: PathBuf::from(c), last_active: Some(r.created_at) });
            }
        }
        let mut stmt = conn.prepare("SELECT project_path, MAX(last_ts) FROM sessions WHERE project_path IS NOT NULL GROUP BY project_path LIMIT 1000").map_err(err)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).map_err(err)?;
        cands.extend(rows.filter_map(Result::ok).map(|(p, t)| Candidate { path: PathBuf::from(p), last_active: Some(t) }));
    }
    if let Some(h) = &home {
        cands.push(Candidate { path: h.clone(), last_active: None });
    }
    Ok(Ctx { cfg, custom, home, cands })
}

pub(crate) fn known(state: &AppState) -> Res<Vec<KnownProject>> {
    let c = ctx(state)?;
    let home = c.home.clone();
    let mut list = projects::known(&c.env(), &c.cands, |p| rules::project_rule_count(p, home.as_deref()));
    // "로컬만" chip (delivery-guard). Display only: an unreadable list shows no chips here, the settings section shows
    // the error and every start refuses on its own.
    let items = state.conn().ok().and_then(|conn| dduck_core::delivery_guard::load_items(&conn).ok()).unwrap_or_default();
    if !items.is_empty() {
        for p in list.iter_mut() {
            if let Some(path) = p.path.as_deref().map(std::path::Path::new) {
                let repo = dduck_core::delivery_guard::repo_info(path, home.as_deref()).ok().flatten();
                p.local_only = dduck_core::delivery_guard::guard_for(path, repo.as_ref(), &items) == dduck_core::delivery_guard::Guard::LocalOnly;
            }
        }
    }
    Ok(list)
}

/// Folder of a path id, home included (rules list home as "홈 폴더는 전역과 같아요"). `None` for `key:` ids.
pub(crate) fn folder(state: &AppState, id: &str) -> Res<Option<PathBuf>> {
    if !projects::valid_id(id) {
        return Err(Fail::ProjectNotListed.now());
    }
    if id.starts_with("key:") {
        return Ok(None);
    }
    let c = ctx(state)?;
    projects::resolve_path(id, &c.raw()).map(Some).ok_or_else(|| Fail::ProjectNotListed.now())
}

/// Folder for folder-based scopes (permissions, instruction files, adding items): never home, never unknown.
pub(crate) fn usable_folder(state: &AppState, id: &str) -> Res<PathBuf> {
    let p = folder(state, id)?.ok_or(projects::UNKNOWN_REASON)?;
    let c = ctx(state)?;
    match projects::home_like(&p, &c.env()) {
        Some(reason) => Err(reason.into()),
        None => Ok(p),
    }
}

/// Memory key of any project id (home is allowed here: its memory is its own).
pub(crate) fn memory_key(state: &AppState, id: &str) -> Res<String> {
    let c = ctx(state)?;
    projects::resolve_memory_key(id, &c.env(), &c.raw()).ok_or_else(|| Fail::NoMemoryFolder.now())
}

/// Folders of the projects the tabs use (each tab's pick or the most recent one), when real and not home: added to
/// the skill and agent catalogs so a tab's project items are listed (D-127).
pub(crate) fn picked_folders(state: &AppState) -> Vec<PathBuf> {
    let Ok(c) = ctx_with(state, false) else { return Vec::new() };
    let Ok(conn) = state.conn() else { return Vec::new() };
    let ids: Vec<String> = projects::known(&c.env(), &c.cands, |_| 0).into_iter().map(|p| p.id).collect();
    let mut out: Vec<PathBuf> = Vec::new();
    for page in settings::catalog_pages() {
        let Ok(Some(id)) = settings::page_project(&conn, page, &ids) else { continue };
        let Some(p) = projects::resolve_path(&id, &c.raw()) else { continue };
        if projects::home_like(&p, &c.env()).is_none() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// MCP servers from the settings files only (nav-ia, D-114): nothing is started. `project` is an opaque id.
#[tauri::command]
pub async fn mcp_servers(app: AppHandle, project: Option<String>) -> Res<dduck_core::mcp_config::McpList> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let cfg = state.config.read().map_err(err)?.clone();
        let folder = match project {
            Some(id) => Some(usable_folder(&state, &id)?),
            None => None,
        };
        let file = dduck_core::mcp_config::user_file(&home_dir(), cfg.explicit.then_some(cfg.path.as_path()));
        Ok(dduck_core::mcp_config::list(&file, folder.as_deref()))
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsPayload {
    projects: Vec<KnownProject>,
    /// Each project tab's own pick (D-127); a missing tab uses `recent[0]`, `null` = none on purpose.
    pages: std::collections::BTreeMap<String, Option<String>>,
    recent: Vec<String>,
}

#[tauri::command]
pub async fn projects_known(app: AppHandle) -> Res<ProjectsPayload> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let projects = known(&state)?;
        let ids: Vec<String> = projects.iter().map(|p| p.id.clone()).collect();
        let conn = state.conn()?;
        let pages = settings::page_projects(&conn, &ids).map_err(err)?;
        let recent = settings::recent_projects(&conn, &ids).map_err(err)?;
        Ok(ProjectsPayload { projects, pages, recent })
    })
    .await
    .map_err(err)?
}

/// A tab's own setter: a project tab and an id from `projects_known` (or none = global only).
#[tauri::command]
pub async fn set_page_project(app: AppHandle, page: String, id: Option<String>) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let ids: Vec<String> = known(&state)?.into_iter().map(|p| p.id).collect();
        settings::set_page_project(&*state.conn()?, &page, id.as_deref(), &ids).map_err(|_| Fail::ProjectNotListed.now())
    })
    .await
    .map_err(err)?
}
