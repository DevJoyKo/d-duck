//! "추가 ▾" on the skills and plugins tabs: install from a GitHub link (install-from-link, D-085, D-087) and make or
//! edit skills, slash commands and workflows (skill-authoring, D-086). The webview sends tokens, ids, names and a
//! choice of project from Rust's own list — never a path. Every write is confirmed natively where it can run code.

use dduck_core::platform::CanonExt;
use dduck_core::i18n::Fail;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dduck_core::authoring::{self, Kind, Target};
use dduck_core::claude_cli::CliError;
use dduck_core::i18n::{Code as C, Params as P, ReasonCode, tr, trp};
use dduck_core::install_link::{self as link, Link, Preview};
use dduck_core::plugins::Outcome;
use dduck_core::text::display_safe;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, Res, catalog_roots, chrono_now_ms, err, home_dir};

const PENDING_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_PENDING: usize = 5;

enum Pending {
    Marketplace { owner: String, repo: String, market: String, plugins: Vec<String>, already_added: bool, sha: String },
    Plugin { id: String },
    Skill { stage: PathBuf, name: String, source: String, sha: String, files: usize, executables: usize, risk: link::SkillRisk },
}

pub struct AddState {
    data_dir: PathBuf,
    pending: Mutex<HashMap<String, (Instant, Pending)>>,
    /// One GitHub preview at a time (the unauthenticated API allows 60 requests an hour).
    inspecting: AtomicBool,
}

impl AddState {
    /// Staging from a previous run is cleared at startup (tokens do not survive a restart).
    pub fn new(data_dir: PathBuf) -> Self {
        let s = AddState { data_dir, pending: Mutex::new(HashMap::new()), inspecting: AtomicBool::new(false) };
        let _ = std::fs::remove_dir_all(s.staging());
        s
    }

    fn staging(&self) -> PathBuf {
        self.data_dir.join("link-staging")
    }

    fn jobs_root(&self) -> PathBuf {
        self.data_dir.join("authoring")
    }

    fn made_file(&self) -> PathBuf {
        self.data_dir.join("authoring-made.json")
    }

    fn dduck_new_hash(&self) -> PathBuf {
        self.data_dir.join("dduck-new.sha256")
    }

    fn drop_pending(p: Pending) {
        if let Pending::Skill { stage, .. } = p {
            let _ = std::fs::remove_dir_all(stage);
        }
    }

    /// Expired previews are removed (with their staged files); at most `MAX_PENDING` are kept.
    fn prune(&self) {
        if let Ok(mut map) = self.pending.lock() {
            let old: Vec<String> = map.iter().filter(|(_, (t, _))| t.elapsed() > PENDING_TTL).map(|(k, _)| k.clone()).collect();
            for k in old {
                if let Some((_, p)) = map.remove(&k) {
                    Self::drop_pending(p);
                }
            }
            while map.len() >= MAX_PENDING {
                let Some(oldest) = map.iter().min_by_key(|(_, (t, _))| *t).map(|(k, _)| k.clone()) else { break };
                if let Some((_, p)) = map.remove(&oldest) {
                    Self::drop_pending(p);
                }
            }
        }
    }

    fn take(&self, token: &str) -> Res<Pending> {
        if !link::valid_token(token) {
            return Err(Fail::PreviewExpired.now());
        }
        let mut map = self.pending.lock().map_err(err)?;
        match map.remove(token) {
            Some((t, p)) if t.elapsed() <= PENDING_TTL => Ok(p),
            Some((_, p)) => {
                Self::drop_pending(p);
                Err(Fail::PreviewExpired.now())
            }
            None => Err(Fail::PreviewExpired.now()),
        }
    }

    fn put_back(&self, token: String, p: Pending) {
        if let Ok(mut map) = self.pending.lock() {
            map.insert(token, (Instant::now(), p));
        }
    }
}

fn config_dir(state: &AppState) -> Res<PathBuf> {
    Ok(state.config.read().map_err(err)?.path.clone())
}

pub(crate) fn confirm(app: &AppHandle, title: &str, msg: String, ok: &str, warn: bool) -> Res<bool> {
    let state = app.state::<AppState>();
    let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
    Ok(app
        .dialog()
        .message(msg)
        .title(title)
        .kind(if warn { MessageDialogKind::Warning } else { MessageDialogKind::Info })
        .buttons(MessageDialogButtons::OkCancelCustom(ok.into(), tr(C::BtnCancel)))
        .blocking_show())
}

fn risk_text(r: &link::SkillRisk) -> String {
    let mut v = Vec::new();
    if let Some(t) = &r.allowed_tools {
        v.push(trp(C::RiskAllowedTools, &P::new().text("tools", t, 200)));
    }
    if r.hooks {
        v.push(tr(C::RiskSkillHooks));
    }
    if r.shell_lines > 0 {
        v.push(trp(C::RiskShellLines, &P::new().num("count", r.shell_lines as i64)));
    }
    v.join("\n")
}

/// The item kind as a word in the current language (dialogs only; `Kind::label` stays the Korean UI label).
fn kind_name(kind: Kind) -> String {
    tr(match kind {
        Kind::Skill => C::KindSkill,
        Kind::Command => C::KindCommand,
        Kind::Workflow => C::KindWorkflow,
    })
}

/// Risk lines as one multi-line parameter (each line cleaned again).
fn risk_param(p: P, rt: &str) -> P {
    p.lines("risks", rt.lines(), 260, 10)
}

// ---------- install from link ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    token: String,
    #[serde(flatten)]
    preview: Preview,
    /// For a plugin id: already installed (from the re-read CLI list).
    installed: bool,
}

#[tauri::command]
pub async fn inspect_link(app: AppHandle, input: String) -> Res<LinkPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        let parsed = link::parse_link(&input).map_err(str::to_string)?;
        let add = app.state::<AddState>();
        let state = app.state::<AppState>();
        add.prune();
        let token = link::random_token();
        let config = config_dir(&state)?;
        let (preview, pending, installed) = match parsed {
            Link::Plugin { id } => {
                let (_, market) = id.split_once('@').unwrap_or_default();
                if !link::known_marketplaces(&config).iter().any(|(n, _)| n == market) {
                    return Err(Fail::MarketFirst.now());
                }
                let plugin = link::local_plugin(&config, &id);
                if plugin.is_none() {
                    return Err(Fail::PluginNotInMarket.now());
                }
                let installed = crate::installed_plugins(&state).map(|l| l.iter().any(|p| p.id == id)).unwrap_or(false);
                (Preview::Plugin { id: id.clone(), plugin }, Pending::Plugin { id }, installed)
            }
            Link::GitHub { owner, repo, git_ref, path } => {
                if add.inspecting.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    return Err(Fail::LinkBusy.now());
                }
                let staging = add.staging();
                let stage = staging.join(&token);
                let result = link::ensure_real_dir(&add.data_dir)
                    .and_then(|_| link::ensure_real_dir(&staging))
                    .and_then(|_| link::inspect_github(&dduck_core::http::Ureq::default(), &config, &owner, &repo, git_ref.as_deref(), &path, &stage));
                add.inspecting.store(false, Ordering::SeqCst);
                let preview = result.inspect_err(|_| {
                    let _ = std::fs::remove_dir_all(&stage);
                })?;
                let pending = match &preview {
                    Preview::Marketplace { market, already_added, sha, .. } => Pending::Marketplace {
                        owner,
                        repo,
                        market: market.market.clone(),
                        plugins: market.plugins.iter().map(|p| p.name.clone()).collect(),
                        already_added: already_added.is_some(),
                        sha: sha.clone(),
                    },
                    Preview::Skill { source, sha, skill } => Pending::Skill {
                        stage,
                        name: skill.name.clone(),
                        source: source.clone(),
                        sha: sha.clone(),
                        files: skill.files.len(),
                        executables: skill.executables,
                        risk: skill.risk.clone(),
                    },
                    Preview::Plugin { .. } => return Err(Fail::UnknownLink.now()),
                };
                (preview, pending, false)
            }
        };
        add.put_back(token.clone(), pending);
        Ok(LinkPreview { token, preview, installed })
    })
    .await
    .map_err(err)?
}

#[tauri::command(async)]
pub fn discard_link(app: AppHandle, token: String) {
    if let Ok(p) = app.state::<AddState>().take(&token) {
        AddState::drop_pending(p);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstall {
    outcome: Outcome,
    /// The marketplace was added in this run (so it stays even if the install did not finish).
    marketplace_added: bool,
    /// Built by Rust from validated values, for the user to paste in a terminal.
    approval_command: Option<String>,
    remove_command: Option<String>,
}

const CLI_TIMEOUT: Duration = Duration::from_secs(180);

/// After a native confirmation: `marketplace add` (if needed) then `install name@market -s user`. Never auto-approves.
#[tauri::command]
pub async fn install_plugin(app: AppHandle, token: String, plugin: Option<String>) -> Res<PluginInstall> {
    tauri::async_runtime::spawn_blocking(move || {
        let add = app.state::<AddState>();
        let state = app.state::<AppState>();
        // Busy first, so a refusal here never consumes the preview (QA LOW).
        let _busy = crate::PluginBusy::take(&state.plugin_busy)?;
        let pending = add.take(&token)?;
        let (id, market_add, msg) = match &pending {
            Pending::Marketplace { owner, repo, market, plugins, already_added, sha } => {
                let Some(name) = plugin.clone().filter(|p| plugins.contains(p)) else {
                    add.put_back(token, pending);
                    return Err(Fail::PluginNotInMarket.now());
                };
                let id = format!("{name}@{market}");
                let msg = trp(
                    C::DlgPluginInstallMarket,
                    &P::new()
                        .text("name", &name, 100)
                        .text("owner", owner, 100)
                        .text("repo", repo, 100)
                        .text("sha", &sha[..12], 12)
                        .text("market", market, 100)
                        .text("added", &tr(if *already_added { C::MarketAlreadyAdded } else { C::MarketAddFirst }), 40),
                );
                (id, (!already_added).then(|| (owner.clone(), repo.clone(), market.clone())), msg)
            }
            Pending::Plugin { id } => {
                let msg = trp(C::DlgPluginInstallId, &P::new().text("name", id, 200));
                (id.clone(), None, msg)
            }
            Pending::Skill { .. } => return Err(Fail::NotPluginLink.now()),
        };
        if !confirm(&app, &tr(C::DlgPluginInstallTitle), msg, &tr(C::BtnInstall), true).unwrap_or(false) {
            add.put_back(token, pending);
            return Ok(PluginInstall { outcome: Outcome::Cancelled, marketplace_added: false, approval_command: None, remove_command: None });
        }
        let client = state.client()?;
        let config = config_dir(&state)?;
        let mut marketplace_added = false;
        let remove = |m: &str| Some(format!("claude plugin marketplace remove {m}"));
        if let Some((owner, repo, market)) = &market_add {
            let o = client.marketplace_add(&home_dir(), owner, repo, CLI_TIMEOUT).map_err(err)?;
            let now = link::market_state(&config, market, owner, repo);
            if o != Outcome::Done || now != Some(true) {
                // Added under another name, or not at all: stop before installing anything.
                let outcome = if o == Outcome::Done { Outcome::Failed } else { o };
                return Ok(PluginInstall { outcome, marketplace_added: now.is_some(), approval_command: None, remove_command: if now.is_some() { remove(market) } else { None } });
            }
            marketplace_added = true;
        }
        let market = id.split_once('@').map(|(_, m)| m.to_string()).unwrap_or_default();
        let outcome = client.plugin_install(&home_dir(), &id, CLI_TIMEOUT).map_err(err)?;
        let outcome = match outcome {
            Outcome::Done => {
                let present = crate::installed_plugins(&state).map(|l| l.iter().any(|p| p.id == id)).unwrap_or(false);
                if present { Outcome::Done } else { Outcome::Unchanged }
            }
            o => o,
        };
        Ok(PluginInstall {
            approval_command: (outcome == Outcome::NeedsApproval).then(|| format!("claude plugin install {id} -s user")),
            remove_command: if marketplace_added && outcome != Outcome::Done { remove(&market) } else { None },
            outcome,
            marketplace_added,
        })
    })
    .await
    .map_err(err)?
}

/// Adds just the marketplace (no plugin), after a native confirmation.
#[tauri::command]
pub async fn add_marketplace(app: AppHandle, token: String) -> Res<Outcome> {
    tauri::async_runtime::spawn_blocking(move || {
        let add = app.state::<AddState>();
        let state = app.state::<AppState>();
        let _busy = crate::PluginBusy::take(&state.plugin_busy)?;
        let pending = add.take(&token)?;
        let Pending::Marketplace { owner, repo, market, already_added, sha, .. } = &pending else {
            return Err(Fail::NotMarketLink.now());
        };
        if *already_added {
            return Ok(Outcome::Unchanged);
        }
        let msg = trp(C::DlgMarketAddBody, &P::new().text("market", market, 100).text("owner", owner, 100).text("repo", repo, 100).text("sha", &sha[..12], 12));
        if !confirm(&app, &tr(C::DlgMarketAddTitle), msg, &tr(C::BtnAdd), false).unwrap_or(false) {
            add.put_back(token, pending);
            return Ok(Outcome::Cancelled);
        }
        let o = state.client()?.marketplace_add(&home_dir(), owner, repo, CLI_TIMEOUT).map_err(err)?;
        let config = config_dir(&state)?;
        Ok(match o {
            Outcome::Done if link::market_state(&config, market, owner, repo) == Some(true) => Outcome::Done,
            Outcome::Done => Outcome::Unchanged,
            other => other,
        })
    })
    .await
    .map_err(err)?
}

/// Installs the staged skill (exactly the previewed bytes) into "내 스킬" after a native confirmation.
#[tauri::command]
pub async fn install_skill(app: AppHandle, token: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let add = app.state::<AddState>();
        let state = app.state::<AppState>();
        let config = config_dir(&state)?;
        let pending = add.take(&token)?;
        let Pending::Skill { stage, name, source, sha, files, executables, risk } = &pending else {
            return Err(Fail::NotSkillLink.now());
        };
        let dest = config.join("skills").join(name);
        let exec = if *executables > 0 { trp(C::SkillExecutables, &P::new().num("count", *executables as i64)) } else { String::new() };
        let mut msg = trp(
            C::DlgSkillInstallBody,
            &P::new()
                .text("name", name, 64)
                .text("source", source, 200)
                .text("sha", &sha[..12], 12)
                .text("dest", &dest.display().to_string(), 200)
                .num("files", *files as i64)
                .text("exec", &exec, 80),
        );
        let rt = risk_text(risk);
        if !rt.is_empty() {
            msg.push_str(&format!("\n\n{}\n{rt}", tr(C::DlgRiskSettings)));
        }
        msg.push_str(&format!("\n\n{}", tr(C::DlgInstallAsk)));
        if !confirm(&app, &tr(C::DlgSkillInstallTitle), msg, &tr(C::BtnInstall), true).unwrap_or(false) {
            add.put_back(token, pending);
            return Err(ReasonCode::Cancelled.now());
        }
        let result = link::install_staged(stage, &config.join("skills"), name);
        let _ = std::fs::remove_dir_all(stage);
        result?;
        authoring::record_made(&add.made_file(), authoring::Made { kind: Kind::Skill, name: name.clone(), project: None, at_ms: chrono_now_ms() });
        Ok(name.clone())
    })
    .await
    .map_err(err)?
}

// ---------- skill authoring ----------

fn target(state: &AppState, project: Option<&str>) -> Res<Target> {
    match project {
        None => {
            // Links are followed for the user's own config (dotfile managers); projects stay strict.
            let base = config_dir(state)?.canon().map_err(|_| Fail::ConfigDirMissing.now())?;
            Ok(Target { base, project: false })
        }
        // An opaque id from `projects_known` (project-switcher); home and folder-less projects are refused.
        Some(id) => Ok(Target { base: crate::projects_cmds::usable_folder(state, id)?, project: true }),
    }
}

fn place_label(t: &Target) -> String {
    if t.project { trp(C::PlaceProject, &P::new().text("path", &t.base.display().to_string(), 200)) } else { tr(C::PlaceGlobal) }
}

fn workflows_disabled(config: &Path) -> bool {
    let p = config.join("settings.json");
    let ok = std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file() && m.len() < 1024 * 1024);
    ok && std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| v.get("disableWorkflows")?.as_bool()).unwrap_or(false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringInfo {
    workflows_disabled: bool,
    /// `off` (no file), `ours` (installed by D.Duck, unchanged), `other` (a file D.Duck did not write or that changed).
    dduck_new: &'static str,
    made: Vec<authoring::Made>,
    templates: HashMap<&'static str, String>,
}

fn dduck_new_state(add: &AddState, config: &Path) -> &'static str {
    let recorded = std::fs::read_to_string(add.dduck_new_hash()).ok();
    authoring::dduck_new_state(&config.join("commands").join("dduck-new.md"), recorded.as_deref())
}

#[tauri::command(async)]
pub fn authoring_info(app: AppHandle) -> Res<AuthoringInfo> {
    let state = app.state::<AppState>();
    let add = app.state::<AddState>();
    let config = config_dir(&state)?;
    Ok(AuthoringInfo {
        workflows_disabled: workflows_disabled(&config),
        dduck_new: dduck_new_state(&add, &config),
        made: authoring::made_list(&add.made_file()),
        templates: Kind::ALL.iter().map(|k| (k.as_str(), authoring::template(*k))).collect(),
    })
}

fn record(add: &AddState, kind: Kind, name: &str, t: &Target) {
    let project = t.project.then(|| t.base.display().to_string());
    authoring::record_made(&add.made_file(), authoring::Made { kind, name: name.to_string(), project, at_ms: chrono_now_ms() });
}

/// Creates a new skill/command/workflow (never overwriting). Content that can run commands is confirmed natively.
#[tauri::command]
pub async fn create_authoring_item(app: AppHandle, kind: Kind, project: Option<String>, name: String, content: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let t = target(&state, project.as_deref())?;
        if !authoring::valid_name(&name) {
            return Err(Fail::ItemNameFormat.now());
        }
        authoring::check_content(kind, &name, &content)?;
        let rt = risk_text(&link::skill_risk(&content));
        if !rt.is_empty() {
            let msg = trp(C::DlgCreateBody, &risk_param(P::new().text("kind", &kind_name(kind), 20).text("name", &name, 64).text("place", &place_label(&t), 220), &rt));
            if !confirm(&app, &tr(C::DlgCreateTitle), msg, &tr(C::BtnCreate), true)? {
                return Err(ReasonCode::Cancelled.now());
            }
        }
        authoring::create_item(&t, kind, &name, &content)?;
        record(&add, kind, &name, &t);
        Ok(())
    })
    .await
    .map_err(err)?
}

pub(crate) fn find_item(state: &AppState, id: &str) -> Res<dduck_core::skills_catalog::Skill> {
    let roots = catalog_roots(state)?;
    dduck_core::skills_catalog::discover(&roots.config_dir, &roots.project_dirs)
        .into_iter()
        .find(|s| s.id.as_deref() == Some(id))
        .ok_or_else(|| Fail::ItemNotListed.now())
}

fn kind_of(s: &dduck_core::skills_catalog::Skill) -> Kind {
    match s.kind {
        "command" => Kind::Command,
        "workflow" => Kind::Workflow,
        _ => Kind::Skill,
    }
}

/// Item name as the file system knows it (folder for a skill, file stem otherwise), for checks against content.
fn file_name_of(s: &dduck_core::skills_catalog::Skill) -> String {
    let p = s.path.clone().unwrap_or_default();
    let base = if s.kind == "skill" { p.parent().and_then(|d| d.file_name()) } else { p.file_stem() };
    base.map(|b| b.to_string_lossy().to_string()).unwrap_or_default()
}

#[tauri::command(async)]
pub fn read_authoring_item(app: AppHandle, id: String) -> Res<authoring::ItemText> {
    let state = app.state::<AppState>();
    let s = find_item(&state, &id)?;
    let path = s.path.clone().ok_or_else(|| Fail::NoDefinitionFile.now())?;
    let location = match (&s.source, &s.scope) {
        (dduck_core::skills_catalog::SkillSource::Project, Some(p)) => trp(C::PlaceProject, &P::new().text("path", p, 200)),
        (dduck_core::skills_catalog::SkillSource::Plugin, _) => tr(C::PlacePlugin),
        _ => tr(C::PlaceGlobalShort),
    };
    let mut item = authoring::read_item(&path, s.kind, &file_name_of(&s), location, s.readonly)?;
    item.name = s.name.clone();
    Ok(item)
}

#[tauri::command]
pub async fn save_authoring_item(app: AppHandle, id: String, content: String, token: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let s = find_item(&state, &id)?;
        if let Some(r) = s.readonly {
            return Err(r.to_string());
        }
        let path = s.path.clone().ok_or_else(|| Fail::NoDefinitionFile.now())?;
        let kind = kind_of(&s);
        // An existing workflow may keep a different meta.name (a warning in the editor, design 8).
        authoring::check_existing(kind, &file_name_of(&s), &content)?;
        let plan = dduck_core::agents_catalog::plan_save_path(path, content, &token).map_err(|e| match e {
            dduck_core::agents_catalog::SaveError::NotFound => Fail::FileNotFound.now(),
            other => other.to_string(),
        })?;
        let rt = risk_text(&link::skill_risk(&plan.content));
        if !rt.is_empty() {
            let msg = trp(C::DlgItemSaveBody, &risk_param(P::new().text("kind", &kind_name(kind), 20).text("name", &s.name, 100), &rt));
            if !confirm(&app, &tr(C::DlgItemSaveTitle), msg, &tr(C::BtnSave), true)? {
                return Err(ReasonCode::Cancelled.now());
            }
        }
        dduck_core::agents_catalog::commit_save(&plan).map_err(|e| e.to_string())
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedJob {
    token: String,
    bg_id: String,
}

/// "Claude에게 부탁하기": a background task with the Rust-fixed preset in an empty app-data folder (D-086).
#[tauri::command]
pub async fn start_authoring_task(app: AppHandle, kind: Kind, request: String) -> Result<StartedJob, CliError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::login_cmds::gate_cli(&app, dduck_core::auth_status::Need::Model)?;
        let bad = CliError::invalid;
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let client = state.client().map_err(|_| CliError::Spawn)?;
        let token = link::random_token();
        let root = add.jobs_root();
        let title = display_safe(request.trim().lines().next().unwrap_or(""), 80);
        let mut meta = authoring::JobMeta { kind, created_ms: chrono_now_ms(), bg_id: None, title };
        link::ensure_real_dir(&add.data_dir).map_err(|_| bad(ReasonCode::AppDataFolder))?;
        let work = authoring::create_job(&root, &token, &meta).map_err(|_| bad(ReasonCode::JobFolder))?;
        let spec = match authoring::job_spec(&work, kind, &request) {
            Ok(s) => s,
            Err(_) => {
                let _ = std::fs::remove_dir_all(root.join(&token));
                return Err(bad(ReasonCode::RequestLength));
            }
        };
        let started = match client.start_background(&spec, None) {
            Err(CliError::UntrustedWorkspace { stderr_tail }) => crate::trust_then_retry(&app, &client, &spec, stderr_tail),
            other => other,
        };
        let bg_id = match started {
            Ok(id) => id,
            Err(e) => {
                let _ = std::fs::remove_dir_all(root.join(&token));
                return Err(e);
            }
        };
        meta.bg_id = Some(bg_id.clone());
        let _ = authoring::save_job(&root, &token, &meta);
        if let Ok(conn) = state.conn() {
            let keep = dduck_core::settings::store_task_prompts(&conn).unwrap_or(true);
            let rec = dduck_core::task_meta::NewRecord {
                bg_id: &bg_id,
                name: spec.name.as_deref(),
                cwd: &spec.cwd.to_string_lossy(),
                agent: None,
                task_type: None,
                prompt: keep.then_some(spec.prompt.as_str()),
                brief: None,
                effort: spec.effort.map(|e| e.as_cli()),
                worktree: false,
                permission_mode: spec.permission_mode.map(|m| m.as_cli()),
                // Authoring jobs have their own result screen; no 작업 보고서 (decision-report scope).
                report_id: None,
                origin: None,
                loadout_dropped: 0,
                now_ms: chrono_now_ms(),
            };
            let _ = dduck_core::task_meta::record_start(&conn, &rec);
        }
        Ok(StartedJob { token, bg_id })
    })
    .await
    .map_err(|_| CliError::Spawn)?
}

#[tauri::command(async)]
pub fn authoring_jobs(app: AppHandle) -> Vec<authoring::Job> {
    authoring::jobs(&app.state::<AddState>().jobs_root())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copied {
    /// Editor id of the copy, when the catalog lists it (it opens in the editor right away).
    id: Option<String>,
}

/// "복사해서 내 것으로" (nav-ia, D-117): a locked plugin or claude.ai-synced skill/command is copied — whole folder,
/// helper files and LICENSE included — to mine (`dest` = "user") or the current project ("project", opaque id).
/// The source is re-resolved from the catalog id and must sit in the plugin cache or the synced folder; it is never
/// changed and nothing is overwritten. No path goes back to the webview or into a log.
#[tauri::command]
pub async fn copy_item(app: AppHandle, item_id: String, dest: String, name: String, project: Option<String>) -> Res<Copied> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let s = find_item(&state, &item_id)?;
        if s.readonly.is_none() {
            return Err(Fail::AlreadyEditable.now());
        }
        let src = s.path.clone().ok_or_else(|| Fail::NoDefinitionFile.now())?;
        let cfg = config_dir(&state)?;
        let allowed = [cfg.join("plugins").join("cache"), cfg.join("skills").join("synced")];
        let real = src.canon().map_err(|_| Fail::SourceMissing.now())?;
        if !allowed.iter().filter_map(|a| a.canon().ok()).any(|a| real.starts_with(&a)) {
            return Err(Fail::CopyPluginOnly.now());
        }
        let t = match dest.as_str() {
            "user" => target(&state, None)?,
            "project" => target(&state, Some(project.as_deref().ok_or_else(|| Fail::PickProjectFirst.now())?))?,
            _ => return Err(Fail::BadSavePlace.now()),
        };
        let kind = kind_of(&s);
        // Same rule as create/install: content that can run commands is confirmed natively first.
        let rt = risk_text(&authoring::copy_risk(&real)?);
        if !rt.is_empty() {
            let msg = trp(
                C::DlgCopyBody,
                &risk_param(P::new().text("kind", &kind_name(kind), 20).text("name", &s.name, 100).text("place", &place_label(&t), 220).text("new", &name, 64), &rt),
            );
            if !confirm(&app, &tr(C::DlgCopyTitle), msg, &tr(C::BtnCopy), true)? {
                return Err(ReasonCode::Cancelled.now());
            }
        }
        authoring::copy_item(&real, kind, &s.name, &name, &t)?;
        record(&add, kind, &name, &t);
        let roots = catalog_roots(&state)?;
        let source = if t.project { dduck_core::skills_catalog::SkillSource::Project } else { dduck_core::skills_catalog::SkillSource::User };
        let id = dduck_core::skills_catalog::discover(&roots.config_dir, &roots.project_dirs)
            .into_iter()
            .find(|x| x.kind == s.kind && x.name == name && x.source == source && (!t.project || x.scope.as_deref().map(Path::new) == Some(t.base.as_path())))
            .and_then(|x| x.id);
        Ok(Copied { id })
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn install_authoring_result(app: AppHandle, token: String, kind: Kind, name: String, project: Option<String>) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let root = add.jobs_root();
        let job = authoring::job(&root, &token).ok_or_else(|| Fail::JobNotFound.now())?;
        let item = job.produced.iter().find(|p| p.kind == kind && p.name == name).ok_or_else(|| Fail::ProducedNotFound.now())?;
        if let Some(p) = &item.problem {
            return Err(p.clone());
        }
        let t = target(&state, project.as_deref())?;
        let mut msg = trp(
            C::DlgResultBody,
            &P::new()
                .text("kind", &kind_name(kind), 20)
                .text("name", &name, 64)
                .text("place", &place_label(&t), 220)
                .text("path", &t.path_of(kind, &name).display().to_string(), 200)
                .num("files", item.files.len().max(1) as i64),
        );
        let rt = risk_text(&item.risk);
        if !rt.is_empty() {
            msg.push_str(&format!("\n\n{}\n{rt}", tr(C::DlgResultRisks)));
        }
        msg.push_str(&format!("\n\n{}", tr(C::DlgInstallAsk)));
        if !confirm(&app, &tr(C::DlgResultTitle), msg, &tr(C::BtnInstall), !rt.is_empty())? {
            return Err(ReasonCode::Cancelled.now());
        }
        authoring::install_produced(&root, &token, kind, &name, &t)?;
        record(&add, kind, &name, &t);
        Ok(())
    })
    .await
    .map_err(err)?
}

#[tauri::command(async)]
pub fn discard_authoring_job(app: AppHandle, token: String) -> Res<()> {
    let add = app.state::<AddState>();
    let root = add.jobs_root();
    let sessions = app.state::<AppState>().agents.read().map(|a| a.clone()).unwrap_or_default();
    authoring::discard_job(&root, &token, |bg| sessions.iter().any(|s| s.id.as_deref() == Some(bg) && !s.status.is_ended()))
}

/// Installs `/dduck-new` into the user's commands after a native confirmation; the hash is kept for removal.
#[tauri::command]
pub async fn install_authoring_command(app: AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let t = target(&state, None)?;
        let path = t.path_of(Kind::Command, authoring::DDUCK_NEW);
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err(Fail::DduckNewExists.now());
        }
        let msg = trp(C::DlgDduckNewBody, &P::new().text("path", &path.display().to_string(), 200));
        if !confirm(&app, &tr(C::DlgDduckNewTitle), msg, &tr(C::BtnInstall), false)? {
            return Err(ReasonCode::Cancelled.now());
        }
        let content = authoring::dduck_new_command();
        link::ensure_real_dir(&add.data_dir)?;
        authoring::create_item(&t, Kind::Command, authoring::DDUCK_NEW, &content)?;
        std::fs::write(add.dduck_new_hash(), dduck_core::agents_catalog::token_of(content.as_bytes())).map_err(|_| Fail::InstallRecordFailed.now())
    })
    .await
    .map_err(err)?
}

/// Moves `/dduck-new` to the Trash only if it is still exactly what D.Duck installed.
#[tauri::command]
pub async fn remove_authoring_command(app: AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let add = app.state::<AddState>();
        let config = config_dir(&state)?;
        if dduck_new_state(&add, &config) != "ours" {
            return Err(Fail::NotPristine.now());
        }
        let path = config.join("commands").join("dduck-new.md");
        let in_bin = crate::move_to_trash(&app, &path)?;
        let _ = std::fs::remove_file(add.dduck_new_hash());
        crate::trash_unconfirmed(in_bin)
    })
    .await
    .map_err(err)?
}
