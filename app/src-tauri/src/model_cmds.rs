//! Model choice (model-select, D-153·D-154·D-197): the model list from local sources, the agent-file model line, and
//! what a start or resume needs to decide and replace a model. Reads only the named settings keys, the CLI help and
//! the usage DB; never credentials, `~/.claude.json` or transcript text, and never calls a model.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use dduck_core::claude_cli::is_safe_cwd;
use dduck_core::i18n::{Fail, ReasonCode};
use dduck_core::model_select::{self as ms, Allowlist, Known, ModelOptions, OptionsEnv};
use dduck_core::{agents_catalog, settings};
use tauri::{AppHandle, Manager};

use crate::{AppState, Res, app_data_dir, chrono_now_ms, err, fresh_catalog_for, permission_project};

/// Aliases from `claude --help`, keyed by the CLI path and its file time (a CLI update changes it).
type HelpCache = Option<(PathBuf, Option<SystemTime>, Option<Vec<String>>)>;
static HELP: Mutex<HelpCache> = Mutex::new(None);

fn help_aliases(app: &AppHandle, refresh: bool) -> Option<Vec<String>> {
    let state = app.state::<AppState>();
    let client = state.client().ok()?;
    let stamp = std::fs::metadata(&client.path).and_then(|m| m.modified()).ok();
    if let Ok(cache) = HELP.lock() {
        if let Some((p, t, v)) = cache.as_ref() {
            if !refresh && *p == client.path && *t == stamp {
                return v.clone();
            }
        }
    }
    // Fixed cwd: the app data folder, never a project folder (design 1).
    let dir = app_data_dir(app).ok()?;
    let _ = std::fs::create_dir_all(&dir);
    let parsed = client.help_text(&dir).ok().and_then(|t| ms::parse_help(&t));
    if let Ok(mut cache) = HELP.lock() {
        *cache = Some((client.path.clone(), stamp, parsed.clone()));
    }
    parsed
}

/// Everything `model_select::options` reads, for an optional project folder.
pub(crate) fn options_env(app: &AppHandle, cwd: Option<&Path>, refresh: bool) -> OptionsEnv {
    let state = app.state::<AppState>();
    let cwd = cwd.filter(|c| is_safe_cwd(c));
    let config_dir = state.config.read().ok().map(|c| c.path.clone());
    let user = config_dir.as_ref().and_then(|d| ms::read_settings(&d.join("settings.json")));
    let project = cwd.and_then(|c| ms::read_settings(&c.join(".claude/settings.json")));
    let local = cwd.and_then(|c| ms::read_settings(&c.join(".claude/settings.local.json")));
    let managed = ms::read_managed(&crate::delivery_cmds::managed_sources(&state));
    let now = chrono_now_ms();
    let (recent, recent_week, recent_main) = match state.conn() {
        Ok(c) => (ms::recent_models(&c, now).unwrap_or_default(), ms::recent_week_models(&c, now).unwrap_or_default(), ms::recent_main_model(&c, now).ok().flatten()),
        Err(_) => (Vec::new(), Vec::new(), None),
    };
    // Stored values: type models of the team config and the agent files usable here.
    let mut custom: Vec<String> = Vec::new();
    let team = state.conn().ok().and_then(|c| settings::team_config(&c).ok().flatten());
    custom.extend(dduck_core::team_config::load(team.as_deref()).types.iter().filter_map(|t| t.def.model.clone()));
    if let Ok(defs) = fresh_catalog_for(&state, cwd) {
        for d in &defs {
            if let Some(m) = d.file.as_deref().and_then(read_file_model) {
                custom.push(m);
            }
        }
    }
    OptionsEnv {
        cli: help_aliases(app, refresh),
        managed,
        local,
        project,
        user,
        process_env: ms::pick_env(std::env::vars()),
        recent,
        recent_week,
        recent_main,
        custom: ms::dedup(custom),
    }
}

/// The raw `model:` of one definition file (256 KB cap). Invalid values and `inherit` = none.
fn read_file_model(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > 256 * 1024 {
        return None;
    }
    ms::file_model(&std::fs::read_to_string(path).ok()?)
}

/// The model of the definition `claude --agent=<name>` will use in `cwd` (design 4), read from the file itself.
pub(crate) fn agent_file_model(state: &AppState, name: &str, cwd: &Path) -> Option<String> {
    let defs = fresh_catalog_for(state, Some(cwd)).ok()?;
    agents_catalog::resolve_for_cwd(&defs, name, cwd).and_then(|d| d.file.as_deref()).and_then(read_file_model)
}

/// What a start or resume needs to check and replace a model (design 11).
pub(crate) struct FallbackParts {
    pub known: Known,
    pub list: Allowlist,
    pub default_value: Option<String>,
}

impl FallbackParts {
    pub fn ctx(&self) -> ms::FallbackCtx<'_> {
        ms::FallbackCtx { known: &self.known, list: &self.list, default_value: self.default_value.as_deref() }
    }
}

pub(crate) fn fallback_parts(app: &AppHandle, cwd: &Path) -> FallbackParts {
    let state = app.state::<AppState>();
    let env = options_env(app, Some(cwd), false);
    let o = ms::options(&env, &state.prices);
    FallbackParts { known: Known::from_env(&env, &state.prices), list: env.allowlist(), default_value: o.current_default.value }
}

/// The model list for the pickers (design 7). `refresh` re-runs `claude --help`.
#[tauri::command]
pub(crate) async fn model_options(app: AppHandle, cwd: Option<String>, refresh: bool) -> Res<ModelOptions> {
    tauri::async_runtime::spawn_blocking(move || {
        let cwd = cwd.map(PathBuf::from).filter(|c| is_safe_cwd(c));
        let env = options_env(&app, cwd.as_deref(), refresh);
        Ok(ms::options(&env, &app.state::<AppState>().prices))
    })
    .await
    .map_err(err)?
}

/// Writes only the `model:` line of an editable agent file (design 3). Same guarded save as the other agent edits
/// (catalog id, token, links, owner, atomic write). A value the allowlist blocks is refused (fail closed).
#[tauri::command]
pub(crate) async fn save_agent_model(app: AppHandle, id: String, project: Option<String>, model: Option<String>, token: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if let Some(m) = model.as_deref() {
            if !ms::valid_choice(m) {
                return Err(ReasonCode::ModelNameFormat.now());
            }
        }
        let proj = permission_project(&state, project.as_deref())?;
        if let Some(m) = model.as_deref() {
            let env = options_env(&app, proj.as_deref(), false);
            ms::check_allowed(&env.allowlist(), m).map_err(|r| r.now())?;
        }
        let cached = state.catalog.read().map_err(err)?.iter().find(|d| d.id == id).and_then(|d| d.file.clone());
        let defs = fresh_catalog_for(&state, proj.as_deref())?;
        let def = defs.iter().find(|d| d.id == id).cloned();
        if cached.is_none() || cached != def.as_ref().and_then(|d| d.file.clone()) {
            return Err(ReasonCode::ListChanged.now());
        }
        def.ok_or_else(|| Fail::AgentNotListed.now())?;
        let (content, current) = agents_catalog::load(&defs, &id).map_err(|e| e.to_string())?;
        if current != token {
            return Err(ReasonCode::ChangedOutside.now());
        }
        let new = agents_catalog::with_model(&content, model.as_deref()).map_err(|e| e.to_string())?;
        if new == content {
            return Ok(());
        }
        let plan = agents_catalog::plan_save(&defs, &id, new, &token).map_err(|e| e.to_string())?;
        agents_catalog::commit_save(&plan).map_err(|e| e.to_string())?;
        let _ = fresh_catalog_for(&state, proj.as_deref());
        Ok(())
    })
    .await
    .map_err(err)?
}

/// Type models in a team config about to be saved: each one must pass the allowlist (fail closed, design 5).
pub(crate) fn check_team_models(app: &AppHandle, merged: &dduck_core::team_config::Merged) -> Res<()> {
    let env = options_env(app, None, false);
    let list = env.allowlist();
    for t in &merged.types {
        if let Some(m) = &t.def.model {
            ms::check_allowed(&list, m).map_err(|r| format!("{} · {}", dduck_core::text::display_safe(&t.def.label, 20), r.now()))?;
        }
    }
    Ok(())
}

/// Adds "이 모델을 쓸 수 없는 것 같아요 · 다른 모델을 골라 주세요" to a failed start whose stderr reads like a refusal
/// of `model` (never for policy wording). The stderr text is only matched here, never stored (design 11).
pub(crate) fn hint_rejected(e: dduck_core::claude_cli::CliError, model: Option<&str>) -> dduck_core::claude_cli::CliError {
    use dduck_core::claude_cli::CliError;
    match (e, model) {
        (CliError::Failed { code, stderr_tail }, Some(m)) if ms::rejects_model(&stderr_tail, m) => {
            CliError::Failed { code, stderr_tail: format!("{stderr_tail}\n{}", Fail::ModelCliRejected.now()) }
        }
        (e, _) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::hint_rejected;
    use dduck_core::claude_cli::CliError;

    #[test]
    fn rejection_gets_one_hint_line_and_policy_wording_none() {
        let fail = |t: &str| CliError::Failed { code: 1, stderr_tail: t.into() };
        let tail = |e: CliError| match e {
            CliError::Failed { stderr_tail, .. } => stderr_tail,
            _ => String::new(),
        };
        let hinted = tail(hint_rejected(fail("Error: model claude-opus-3-9 not found"), Some("claude-opus-3-9")));
        assert_eq!(hinted.lines().count(), 2, "{hinted}");
        assert_eq!(tail(hint_rejected(fail("model claude-opus-3-9 not available for your organization"), Some("claude-opus-3-9"))).lines().count(), 1);
        assert_eq!(tail(hint_rejected(fail("model claude-opus-3-9 not found"), None)).lines().count(), 1, "no model passed");
        assert!(matches!(hint_rejected(CliError::Timeout, Some("opus")), CliError::Timeout));
    }
}
