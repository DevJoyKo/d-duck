//! Model choice per agent and per task (model-select, D-153·D-154). The list of models is built from local sources
//! every time (CLI help aliases, the model keys of settings, models seen in transcripts, typed ids); nothing here
//! holds a model table, calls a model or touches the network.

use std::collections::BTreeSet;
use std::path::Path;

use regex::Regex;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;

use crate::pricing::PriceTable;

/// `--agent=<file agent with model:>` + `--model=` — which one wins is not documented. Stays `false` until spike 1.1
/// measures it; until then that combination is refused (design 4).
pub const FILE_AGENT_MODEL_OVERRIDE_VERIFIED: bool = false;

/// The only `env` names read from settings and the app's own environment (design 1). Everything else in `env`
/// (tokens, keys) is never looked at.
pub const ENV_NAMES: [&str; 7] = [
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_DEFAULT_FABLE_MODEL",
    "ANTHROPIC_CUSTOM_MODEL_OPTION",
];

pub const FAMILIES: [&str; 4] = ["opus", "sonnet", "haiku", "fable"];

use crate::i18n::ReasonCode as RC;

pub const BAD_FORMAT: RC = RC::ModelNameFormat;
pub const BAD_STORED: RC = RC::ModelStoredBad;
pub const LOCKED: RC = RC::ModelAgentFixed;
pub const BLOCKED_MANAGED: RC = RC::ModelManagedBlock;
pub const BLOCKED_SETTINGS: RC = RC::ModelNotAllowed;
pub const BLOCKED_UNKNOWN: RC = RC::ModelAllowUnknown;
pub const NO_FALLBACK: RC = RC::ModelNoFallback;
pub const AGENT_UNSUPPORTED: RC = RC::ModelAgentUnsupported;

static MODEL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._:@-]{0,99}(\[1m\])?$").unwrap_or_else(|_| unreachable!("tested")));
static HELP_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'([a-z][a-z0-9-]{0,30}(\[1m\])?)'").unwrap_or_else(|_| unreachable!("tested")));
static OPTION_LINE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s{2}-").unwrap_or_else(|_| unreachable!("tested")));

const YAML_WORDS: [&str; 9] = ["null", "true", "false", "yes", "no", "on", "off", "y", "n"];

/// A model value D.Duck may pass as `--model=<v>` or write as `model: <v>` (design 2). Same table test as
/// `src/lib/model.ts`.
pub fn valid_model(s: &str) -> bool {
    if !MODEL_RE.is_match(s) {
        return false;
    }
    let bare = s.strip_suffix("[1m]").unwrap_or(s);
    if !bare.bytes().any(|b| b.is_ascii_lowercase()) {
        return false; // `123`, `1.5`: YAML would read a number
    }
    !YAML_WORDS.contains(&bare)
}

/// A value the user may choose for a task or a type: `default` and `inherit` mean "no model" and are refused.
pub fn valid_choice(s: &str) -> bool {
    valid_model(s) && !matches!(s, "default" | "inherit")
}

/// The family an alias names (`opus[1m]` → `opus`), if it is one of the four.
pub fn family_of_alias(v: &str) -> Option<&'static str> {
    let bare = v.strip_suffix("[1m]").unwrap_or(v);
    FAMILIES.iter().copied().find(|f| *f == bare)
}

// ---------- CLI help ----------

/// Aliases quoted in the `--model` paragraph of `claude --help`. `None` when the paragraph or its quotes are gone.
pub fn parse_help(text: &str) -> Option<Vec<String>> {
    let mut lines = text.lines().skip_while(|l| !l.trim_start().starts_with("--model "));
    let first = lines.next()?;
    let mut para = first.to_string();
    for l in lines {
        if OPTION_LINE_RE.is_match(l) {
            break;
        }
        para.push(' ');
        para.push_str(l.trim());
    }
    let mut out: Vec<String> = Vec::new();
    for c in HELP_ALIAS_RE.captures_iter(&para) {
        let v = c.get(1).map_or("", |m| m.as_str()).to_string();
        if valid_model(&v) && !out.contains(&v) {
            out.push(v);
        }
    }
    (!out.is_empty()).then_some(out)
}

// ---------- settings ----------

/// The model keys of one settings file. Built by picking named keys out of the JSON; nothing else is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsKeys {
    pub model: Option<String>,
    /// `Some(list)` when the file has `availableModels`.
    pub available: Option<Vec<String>>,
    pub enforce: bool,
    pub overrides: Vec<String>,
    /// Allowed `env` names only, with their values.
    pub env: Vec<(String, String)>,
    /// `availableModels` that is not a string list, or another `availableModels…` matching key the app cannot judge.
    pub unknown_match: bool,
}

pub fn extract(v: &Value) -> SettingsKeys {
    let mut k = SettingsKeys::default();
    let Some(obj) = v.as_object() else { return k };
    k.model = obj.get("model").and_then(Value::as_str).map(str::to_string);
    match obj.get("availableModels") {
        None => {}
        Some(Value::Array(a)) => {
            let list: Option<Vec<String>> = a.iter().map(|x| x.as_str().map(str::to_string)).collect();
            match list {
                Some(l) => k.available = Some(l),
                None => k.unknown_match = true,
            }
        }
        Some(_) => k.unknown_match = true,
    }
    if obj.keys().any(|key| key.starts_with("availableModels") && key != "availableModels") {
        k.unknown_match = true;
    }
    k.enforce = obj.get("enforceAvailableModels").and_then(Value::as_bool).unwrap_or(false);
    if let Some(Value::Object(o)) = obj.get("modelOverrides") {
        k.overrides = o.keys().take(50).cloned().collect();
    }
    if let Some(env) = obj.get("env") {
        for name in ENV_NAMES {
            if let Some(val) = env.get(name).and_then(Value::as_str) {
                k.env.push((name.to_string(), val.to_string()));
            }
        }
    }
    k
}

const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;

/// One user/project/local settings file. Missing or unreadable = nothing (only managed sources fail closed).
pub fn read_settings(path: &Path) -> Option<SettingsKeys> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > MAX_SETTINGS_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok().map(|v| extract(&v))
}

/// Managed sources (system file and drop-ins, server-managed cache, MDM plist). `unreadable` when one exists but
/// cannot be read or parsed, or when an MDM plist exists (its contents are not read).
#[derive(Debug, Clone, Default)]
pub struct ManagedRead {
    pub keys: Vec<SettingsKeys>,
    pub unreadable: bool,
}

pub fn read_managed(src: &crate::delivery_guard::ManagedSources) -> ManagedRead {
    let mut out = ManagedRead::default();
    let file = |p: &Path, out: &mut ManagedRead| match std::fs::read_to_string(p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => out.unreadable = true,
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(v) if v.is_object() => out.keys.push(extract(&v)),
            _ => out.unreadable = true,
        },
    };
    for f in &src.files {
        file(f, &mut out);
    }
    for d in &src.dropin_dirs {
        match std::fs::read_dir(d) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => out.unreadable = true,
            Ok(entries) => {
                let mut files: Vec<std::path::PathBuf> = Vec::new();
                for e in entries {
                    match e {
                        Ok(e) if e.path().extension().is_some_and(|x| x == "json") => files.push(e.path()),
                        Ok(_) => {}
                        Err(_) => out.unreadable = true,
                    }
                }
                files.sort();
                for f in files.iter().take(50) {
                    file(f, &mut out);
                }
            }
        }
    }
    for p in &src.plists {
        match std::fs::symlink_metadata(p) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => out.unreadable = true,
        }
    }
    out
}

// ---------- allowlist ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AllowKind {
    None,
    Managed,
    Settings,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowlist {
    pub kind: AllowKind,
    pub list: Vec<String>,
}

impl Allowlist {
    pub fn none() -> Self {
        Allowlist { kind: AllowKind::None, list: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Allow {
    Yes,
    No,
    Unknown,
}

/// Managed lists replace the others; otherwise user, project and local lists are merged (design 5).
pub fn allowlist(managed: &ManagedRead, others: &[&SettingsKeys]) -> Allowlist {
    if managed.unreadable || managed.keys.iter().any(|k| k.unknown_match) {
        return Allowlist { kind: AllowKind::Unknown, list: Vec::new() };
    }
    let lists: Vec<&Vec<String>> = managed.keys.iter().filter_map(|k| k.available.as_ref()).collect();
    if !lists.is_empty() {
        return Allowlist { kind: AllowKind::Managed, list: lists.into_iter().flatten().cloned().collect() };
    }
    if others.iter().any(|k| k.unknown_match) {
        return Allowlist { kind: AllowKind::Unknown, list: Vec::new() };
    }
    let lists: Vec<&Vec<String>> = others.iter().filter_map(|k| k.available.as_ref()).collect();
    if lists.is_empty() {
        return Allowlist::none();
    }
    Allowlist { kind: AllowKind::Settings, list: lists.into_iter().flatten().cloned().collect() }
}

fn entry_matches(entry: &str, value: &str) -> bool {
    let e = entry.strip_suffix("[1m]").unwrap_or(entry);
    let v = value.strip_suffix("[1m]").unwrap_or(value);
    if FAMILIES.contains(&e) {
        return v == e || v.starts_with(&format!("claude-{e}-"));
    }
    v == e || v.starts_with(&format!("{e}-"))
}

/// Whether `value` passes the allowlist, and the note for an alias that the CLI narrows to an allowed version.
pub fn allowed(list: &Allowlist, value: &str) -> (Allow, Option<&'static str>) {
    match list.kind {
        AllowKind::None => (Allow::Yes, None),
        AllowKind::Unknown => (Allow::Unknown, None),
        AllowKind::Managed | AllowKind::Settings => {
            if list.list.iter().any(|e| entry_matches(e, value)) {
                return (Allow::Yes, None);
            }
            // A blocked family with allowed versions: the CLI picks the newest allowed one.
            if let Some(f) = family_of_alias(value) {
                if list.list.iter().any(|e| e.starts_with(&format!("claude-{f}-"))) {
                    return (Allow::Yes, Some(crate::i18n::Fail::ModelManagedPin.str()));
                }
            }
            (Allow::No, None)
        }
    }
}

/// `Ok` or the one-line reason for a refused start, resume or save.
pub fn check_allowed(list: &Allowlist, value: &str) -> Result<(), RC> {
    match (allowed(list, value).0, list.kind) {
        (Allow::Yes, _) => Ok(()),
        (Allow::Unknown, _) | (_, AllowKind::Unknown) => Err(BLOCKED_UNKNOWN),
        (Allow::No, AllowKind::Managed) => Err(BLOCKED_MANAGED),
        (Allow::No, _) => Err(BLOCKED_SETTINGS),
    }
}

// ---------- options ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Cli,
    Settings,
    Recent,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedId {
    pub id: String,
    /// `settings` (an `ANTHROPIC_DEFAULT_<F>_MODEL` value) or `recent` (guessed from transcripts).
    pub basis: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelItem {
    pub value: String,
    pub sources: Vec<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedId>,
    pub allow: Allow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<AllowKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
    /// Output price per million tokens from the app's table, for the id this item stands for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    /// Set only for a stored value that is no longer supported: what a start would use instead (design 11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<Replacement>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultView {
    pub value: Option<String>,
    /// `env`, `settings`, `recent` or `account` (nothing known: the account's default).
    pub basis: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOptions {
    pub items: Vec<ModelItem>,
    pub current_default: DefaultView,
    pub cli_read: bool,
    pub dropped: usize,
    pub allowlist: AllowKind,
}

/// Everything `options` needs, read by the shell. Settings in precedence order: local, project, user.
#[derive(Debug, Clone, Default)]
pub struct OptionsEnv {
    /// `None` = the help could not be run or parsed.
    pub cli: Option<Vec<String>>,
    pub managed: ManagedRead,
    pub local: Option<SettingsKeys>,
    pub project: Option<SettingsKeys>,
    pub user: Option<SettingsKeys>,
    /// The app's own environment, allowed names only.
    pub process_env: Vec<(String, String)>,
    /// Models seen in the last 30 days, newest first.
    pub recent: Vec<String>,
    /// Models seen in the last 7 days: what counts as still supported (design 11).
    pub recent_week: Vec<String>,
    /// Newest main-conversation model of a session D.Duck did not start with a model.
    pub recent_main: Option<String>,
    /// Values already stored somewhere (types, agents) or typed in.
    pub custom: Vec<String>,
}

impl OptionsEnv {
    /// Settings by CLI precedence: managed, local, project, user.
    fn scopes(&self) -> Vec<&SettingsKeys> {
        self.managed.keys.iter().chain(self.local.iter()).chain(self.project.iter()).chain(self.user.iter()).collect()
    }

    fn env_value(&self, name: &str) -> Option<String> {
        self.scopes()
            .into_iter()
            .flat_map(|k| k.env.iter())
            .chain(self.process_env.iter())
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    pub fn allowlist(&self) -> Allowlist {
        let others: Vec<&SettingsKeys> = self.local.iter().chain(self.project.iter()).chain(self.user.iter()).collect();
        allowlist(&self.managed, &others)
    }
}

/// Only these names of a process environment, for `OptionsEnv::process_env`.
pub fn pick_env(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.into_iter().filter(|(k, _)| ENV_NAMES.contains(&k.as_str())).collect()
}

pub fn options(env: &OptionsEnv, prices: &PriceTable) -> ModelOptions {
    let mut items: Vec<ModelItem> = Vec::new();
    let mut dropped = 0usize;
    let mut add = |v: &str, src: Source, items: &mut Vec<ModelItem>| {
        let v = v.trim();
        if !valid_model(v) {
            dropped += 1;
            return;
        }
        match items.iter_mut().find(|i| i.value == v) {
            Some(i) => {
                if !i.sources.contains(&src) {
                    i.sources.push(src);
                }
            }
            None => items.push(ModelItem { value: v.to_string(), sources: vec![src], resolved: None, allow: Allow::Yes, blocked_by: None, note: None, price: None, replacement: None }),
        }
    };
    for a in env.cli.iter().flatten() {
        add(a, Source::Cli, &mut items);
    }
    for k in env.scopes() {
        if let Some(m) = &k.model {
            add(m, Source::Settings, &mut items);
        }
        for a in k.available.iter().flatten() {
            add(a, Source::Settings, &mut items);
        }
        for o in &k.overrides {
            add(o, Source::Settings, &mut items);
        }
        for (_, v) in &k.env {
            add(v, Source::Settings, &mut items);
        }
    }
    for (_, v) in &env.process_env {
        add(v, Source::Settings, &mut items);
    }
    for r in &env.recent {
        add(r, Source::Recent, &mut items);
    }
    for c in &env.custom {
        add(c, Source::Custom, &mut items);
    }
    let list = env.allowlist();
    for i in &mut items {
        if let Some(f) = family_of_alias(&i.value) {
            let pinned = env.env_value(&format!("ANTHROPIC_DEFAULT_{}_MODEL", f.to_ascii_uppercase())).filter(|v| valid_model(v));
            i.resolved = match pinned {
                Some(id) => Some(ResolvedId { id, basis: "settings" }),
                None => env.recent.iter().find(|r| r.starts_with(&format!("claude-{f}-")) && valid_model(r)).map(|id| ResolvedId { id: id.clone(), basis: "recent" }),
            };
        }
        let (allow, note) = allowed(&list, &i.value);
        i.allow = allow;
        i.note = note;
        i.blocked_by = (allow != Allow::Yes).then_some(if allow == Allow::Unknown { AllowKind::Unknown } else { list.kind });
        let id = i.resolved.as_ref().map_or(i.value.as_str(), |r| r.id.as_str());
        i.price = prices.output_of(id);
    }
    let current_default = current_default(env, &items, prices);
    let known = Known::from_env(env, prices);
    let ctx = FallbackCtx { known: &known, list: &list, default_value: current_default.value.as_deref() };
    for i in items.iter_mut().filter(|i| i.sources == [Source::Custom] && i.allow == Allow::Yes) {
        if !supported(&i.value, &known) {
            i.replacement = Some(match fallback(&i.value, &ctx) {
                Ok(r) => r,
                Err(_) => Replacement::None,
            });
        }
    }
    ModelOptions { items, current_default, cli_read: env.cli.is_some(), dropped, allowlist: list.kind }
}

/// What the CLI would use with no `--model`, in its own order (design 1).
fn current_default(env: &OptionsEnv, items: &[ModelItem], prices: &PriceTable) -> DefaultView {
    let ok = |v: Option<String>| v.filter(|v| valid_model(v));
    let pick = ok(env.env_value("ANTHROPIC_MODEL"))
        .map(|v| (v, "env"))
        .or_else(|| ok(env.scopes().into_iter().find_map(|k| k.model.clone())).map(|v| (v, "settings")))
        .or_else(|| ok(env.env_value("ANTHROPIC_DEFAULT_MODEL")).map(|v| (v, "env")))
        .or_else(|| ok(env.recent_main.clone()).map(|v| (v, "recent")));
    match pick {
        Some((value, basis)) => {
            let id = items.iter().find(|i| i.value == value).and_then(|i| i.resolved.as_ref()).map_or(value.clone(), |r| r.id.clone());
            DefaultView { price: prices.output_of(&id), value: Some(value), basis }
        }
        None => DefaultView { value: None, basis: "account", price: None },
    }
}

const THIRTY_DAYS_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Models used in the last 30 days, newest first, at most 12 (`<synthetic>` and malformed values left out).
pub fn recent_models(conn: &Connection, now_ms: i64) -> rusqlite::Result<Vec<String>> {
    recent_within(conn, now_ms, THIRTY_DAYS_MS)
}

const SEVEN_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Models used in the last 7 days: the "still supported" evidence (design 11).
pub fn recent_week_models(conn: &Connection, now_ms: i64) -> rusqlite::Result<Vec<String>> {
    recent_within(conn, now_ms, SEVEN_DAYS_MS)
}

fn recent_within(conn: &Connection, now_ms: i64, window_ms: i64) -> rusqlite::Result<Vec<String>> {
    let mut st = conn.prepare(
        "SELECT model, MAX(ts) AS last FROM api_calls WHERE ts >= ?1 AND model != '<synthetic>' GROUP BY model ORDER BY last DESC LIMIT 40",
    )?;
    let rows = st.query_map(params![now_ms - window_ms], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for m in rows.flatten() {
        if valid_model(&m) && out.len() < 12 {
            out.push(m);
        }
    }
    Ok(out)
}

/// The newest main-conversation model of the last 30 days, leaving out sessions D.Duck started with a model (they do
/// not show the CLI default) and `<synthetic>` rows.
pub fn recent_main_model(conn: &Connection, now_ms: i64) -> rusqlite::Result<Option<String>> {
    let mut st = conn.prepare(
        "SELECT model FROM api_calls a WHERE a.ts >= ?1 AND a.agent_id IS NULL AND a.model != '<synthetic>'
           AND NOT EXISTS (SELECT 1 FROM task_meta t WHERE t.session_id = a.session_id AND (t.model_arg IS NOT NULL OR t.role_model IS NOT NULL))
         ORDER BY a.ts DESC LIMIT 20",
    )?;
    let rows = st.query_map(params![now_ms - THIRTY_DAYS_MS], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().find(|m| valid_model(m)))
}

// ---------- precedence ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Task,
    Agent,
    Type,
    Cli,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Task => "task",
            Layer::Agent => "agent",
            Layer::Type => "type",
            Layer::Cli => "cli",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Layer::Task, Layer::Agent, Layer::Type, Layer::Cli].into_iter().find(|l| l.as_str() == s)
    }
}

/// What a start passed: the layer that decided, the `--model=` value and the inline role's `model`. Stored in
/// `task_meta` so a resume or rework reproduces it without re-resolving (D-154 (2)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPin {
    pub layer: Layer,
    pub arg: Option<String>,
    pub role: Option<String>,
}

/// How a start runs: an inline role (bundled or added type) or a file agent (`--agent=<file>`) with the raw
/// `model` of the definition `claude --agent` will use (`None` = no usable model there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner<'a> {
    Inline,
    FileAgent(Option<&'a str>),
}

fn stored_ok(v: Option<&str>) -> Result<Option<String>, RC> {
    match v {
        Some(v) if !valid_choice(v) => Err(BAD_STORED),
        other => Ok(other.map(str::to_string)),
    }
}

/// task > agent file > type > CLI (design 4). Inline roles get the same value in the role JSON and `--model`; a file
/// agent with no task value gets no `--model` when its file sets one.
pub fn resolve(task: Option<&str>, runner: Runner<'_>, type_default: Option<&str>) -> Result<ModelPin, RC> {
    if task.is_some_and(|t| !valid_choice(t)) {
        return Err(BAD_FORMAT);
    }
    let type_default = stored_ok(type_default)?;
    match runner {
        Runner::Inline => {
            let (value, layer) = match (task, type_default) {
                (Some(t), _) => (Some(t.to_string()), Layer::Task),
                (None, Some(d)) => (Some(d), Layer::Type),
                (None, None) => (None, Layer::Cli),
            };
            Ok(ModelPin { layer, arg: value.clone(), role: value })
        }
        Runner::FileAgent(file) => {
            let file = file.filter(|f| valid_choice(f));
            match (task, file, type_default) {
                (Some(_), Some(_), _) if !FILE_AGENT_MODEL_OVERRIDE_VERIFIED => Err(LOCKED),
                (Some(t), _, _) => Ok(ModelPin { layer: Layer::Task, arg: Some(t.to_string()), role: None }),
                (None, Some(_), _) => Ok(ModelPin { layer: Layer::Agent, arg: None, role: None }),
                (None, None, Some(d)) => Ok(ModelPin { layer: Layer::Type, arg: Some(d), role: None }),
                (None, None, None) => Ok(ModelPin { layer: Layer::Cli, arg: None, role: None }),
            }
        }
    }
}

/// A stored pin, re-checked right before the arguments are built. The lock is checked again only for task-layer
/// pins on a file agent whose file now sets a model.
pub fn check_pin(pin: &ModelPin, runner: Runner<'_>) -> Result<(), RC> {
    stored_ok(pin.arg.as_deref())?;
    stored_ok(pin.role.as_deref())?;
    if pin.layer == Layer::Task && !FILE_AGENT_MODEL_OVERRIDE_VERIFIED {
        if let Runner::FileAgent(Some(f)) = runner {
            if valid_choice(f) {
                return Err(LOCKED);
            }
        }
    }
    Ok(())
}

/// The raw `model:` of an agent definition, as `claude --agent` reads it. Invalid values and `inherit` = none.
pub fn file_model(content: &str) -> Option<String> {
    let fm = crate::agents_catalog::parse_frontmatter(content)?;
    let v = fm.get("model")?.trim().to_string();
    valid_choice(&v).then_some(v)
}

// ---------- after the start ----------

/// Strips what does not name the model: `[1m]`, provider prefix, `@date`, `-v1:0` (same as the price key).
pub fn normalize_id(id: &str) -> String {
    crate::pricing::price_key(id)
}

/// `Some(true)` when the main conversation ran on what was asked, `Some(false)` when not, `None` when the request
/// cannot be compared (`best`, `opusplan`, `default`, …) (design 9).
pub fn same_model(requested: &str, actual: &str) -> Option<bool> {
    if actual == "<synthetic>" || !valid_model(requested) {
        return None;
    }
    let actual = normalize_id(actual);
    if let Some(f) = family_of_alias(requested) {
        return Some(actual.starts_with(&format!("claude-{f}-")));
    }
    if !requested.starts_with("claude-") && !requested.contains(".anthropic.") && !requested.starts_with("anthropic.") {
        return None;
    }
    let want = normalize_id(requested);
    Some(actual == want || actual.starts_with(&format!("{want}-")))
}

/// "요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요" when the first main-conversation model differs.
pub fn mismatch_line(requested: Option<&str>, actual: Option<&str>) -> Option<String> {
    let (r, a) = (requested?, actual?);
    (same_model(r, a) == Some(false)).then(|| crate::tr_fmt!(crate::i18n::Fail::ModelRanOther, r = r, a0 = crate::text::display_safe(a, 80)))
}

/// The first main-conversation model of a session (`<synthetic>` left out).
pub fn first_main_model(conn: &Connection, session_id: &str) -> rusqlite::Result<Option<String>> {
    let mut st = conn.prepare("SELECT model FROM api_calls WHERE session_id = ?1 AND agent_id IS NULL AND model != '<synthetic>' ORDER BY ts ASC LIMIT 1")?;
    let mut rows = st.query_map([session_id], |r| r.get::<_, String>(0))?;
    Ok(rows.next().and_then(Result::ok))
}


// ---------- unsupported models: automatic fallback (design 11, D-197) ----------

/// Documented aliases that name no single family but that the CLI accepts (model-config docs).
pub const DOC_ALIASES: [&str; 2] = ["best", "opusplan"];

/// A start the CLI refused because of its model is not retried until spike 1.2 measures what a failed `--bg` leaves
/// behind and how the refusal reads; until then the failure only gets one hint line (design 11).
pub const CLI_REJECT_RETRY_MEASURED: bool = false;

/// What counts as "still supported", as plain data (design 11). `availableModels` entries are not evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    pub cli: Vec<String>,
    /// `model`, `modelOverrides` keys and the named env values of every settings scope and the app environment.
    pub settings: Vec<String>,
    /// Models seen in the last 7 days.
    pub recent: Vec<String>,
    /// Price table prefixes.
    pub priced: Vec<String>,
}

impl Known {
    pub fn from_env(env: &OptionsEnv, prices: &PriceTable) -> Self {
        let mut settings: Vec<String> = Vec::new();
        for k in env.scopes() {
            settings.extend(k.model.iter().cloned());
            settings.extend(k.overrides.iter().cloned());
            settings.extend(k.env.iter().map(|(_, v)| v.clone()));
        }
        settings.extend(env.process_env.iter().map(|(_, v)| v.clone()));
        Known {
            cli: env.cli.clone().unwrap_or_default(),
            settings: settings.into_iter().filter(|v| valid_model(v)).collect(),
            recent: env.recent_week.clone(),
            priced: prices.models.iter().map(|m| m.prefix.clone()).collect(),
        }
    }
}

fn dated(rest: &str) -> bool {
    rest.len() == 9 && rest.starts_with('-') && rest[1..].bytes().all(|b| b.is_ascii_digit())
}

/// Whether the CLI can still be expected to take `value` (design 11).
pub fn supported(value: &str, k: &Known) -> bool {
    if family_of_alias(value).is_some() || DOC_ALIASES.contains(&value) {
        return true;
    }
    if k.cli.iter().chain(&k.settings).any(|c| c == value) {
        return true;
    }
    let key = normalize_id(value);
    if k.recent.iter().any(|r| r == value || normalize_id(r) == key) {
        return true;
    }
    k.priced.iter().any(|p| key == *p || key.strip_prefix(p.as_str()).is_some_and(dated))
}

/// The family a value belongs to: an alias, `opusplan`, or a `claude-…` id with exactly one family word in it.
pub fn family_of(value: &str) -> Option<&'static str> {
    if let Some(f) = family_of_alias(value) {
        return Some(f);
    }
    if value == "opusplan" {
        return Some("opus");
    }
    let key = normalize_id(value);
    if !key.starts_with("claude-") {
        return None;
    }
    let found: Vec<&'static str> = FAMILIES.iter().copied().filter(|f| key.split('-').any(|w| w == *f)).collect();
    (found.len() == 1).then(|| found[0])
}

fn rank(family: &str) -> usize {
    ["haiku", "sonnet", "opus", "fable"].iter().position(|f| *f == family).unwrap_or(usize::MAX)
}

/// What an unsupported value is replaced with. `None` = nothing allowed and not pricier (the start is refused).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Replacement {
    Alias { to: String },
    Default,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct FallbackCtx<'a> {
    pub known: &'a Known,
    pub list: &'a Allowlist,
    /// What the CLI uses with no `--model` (`current_default`), if known.
    pub default_value: Option<&'a str>,
}

/// Replacement for an unsupported value. The requested value must pass the allowlist first: a blocked value is
/// refused with its reason and never replaced (design 11).
pub fn fallback(requested: &str, ctx: &FallbackCtx<'_>) -> Result<Replacement, RC> {
    check_allowed(ctx.list, requested)?;
    let Some(fam) = family_of(requested) else { return Ok(Replacement::Default) };
    // An alias the allowlist narrows to listed versions could land on the gone version again: not a target.
    if allowed(ctx.list, fam) == (Allow::Yes, None) {
        return Ok(Replacement::Alias { to: fam.to_string() });
    }
    let default_family = ctx.default_value.and_then(family_of);
    match default_family {
        Some(d) if rank(d) <= rank(fam) => Ok(Replacement::Default),
        _ => Err(NO_FALLBACK),
    }
}

/// One model value after the support check: what to pass and, when it was replaced, what was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub value: Option<String>,
    pub requested: Option<String>,
}

/// Allowlist check, then the fallback for an unsupported value. `None` stays `None`.
pub fn fix(value: Option<&str>, ctx: &FallbackCtx<'_>) -> Result<Fixed, RC> {
    let Some(v) = value else { return Ok(Fixed { value: None, requested: None }) };
    check_allowed(ctx.list, v)?;
    if supported(v, ctx.known) {
        return Ok(Fixed { value: Some(v.to_string()), requested: None });
    }
    let to = match fallback(v, ctx)? {
        Replacement::Alias { to } => Some(to),
        _ => None,
    };
    Ok(Fixed { value: to, requested: Some(v.to_string()) })
}

/// The pin a start or resume really passes, with the value asked for when something was replaced. Inline roles keep
/// the role `model` equal to `--model`. A file agent whose own `model:` is unsupported is refused until spike 1.1
/// shows that `--model` wins over `--agent` (design 11).
pub fn apply_fallback(pin: &ModelPin, runner: Runner<'_>, ctx: &FallbackCtx<'_>) -> Result<(ModelPin, Option<String>), RC> {
    let arg = fix(pin.arg.as_deref(), ctx)?;
    match runner {
        Runner::Inline => {
            let role = if pin.role == pin.arg { arg.clone() } else { fix(pin.role.as_deref(), ctx)? };
            let requested = arg.requested.clone().or(role.requested.clone());
            Ok((ModelPin { layer: pin.layer, arg: arg.value, role: role.value }, requested))
        }
        Runner::FileAgent(file) => {
            let file = file.filter(|f| valid_choice(f));
            if let (None, Some(f)) = (&pin.arg, file) {
                check_allowed(ctx.list, f)?;
                if !supported(f, ctx.known) {
                    if !FILE_AGENT_MODEL_OVERRIDE_VERIFIED {
                        return Err(AGENT_UNSUPPORTED);
                    }
                    let to = match fallback(f, ctx)? {
                        Replacement::Alias { to } => to,
                        _ => ctx.default_value.filter(|d| valid_choice(d)).map(str::to_string).ok_or(NO_FALLBACK)?,
                    };
                    return Ok((ModelPin { layer: pin.layer, arg: Some(to), role: None }, Some(f.to_string())));
                }
            }
            Ok((ModelPin { layer: pin.layer, arg: arg.value, role: None }, arg.requested))
        }
    }
}

const REJECT_WORDS: [&str; 10] = ["not found", "not_found", "invalid model", "unknown model", "not available", "not supported", "unsupported", "does not exist", "retired", "deprecated"];
const POLICY_WORDS: [&str; 6] = ["availablemodels", "policy", "organization", "managed", "permission", "access"];

/// Whether a failed `claude --bg` reads like a refusal of `value` (unmeasured wording, spike 1.2). Policy wording
/// never counts. The text is only looked at here, never kept.
pub fn rejects_model(stderr_tail: &str, value: &str) -> bool {
    let t = stderr_tail.to_ascii_lowercase();
    !value.is_empty() && t.contains(&value.to_ascii_lowercase()) && t.contains("model") && REJECT_WORDS.iter().any(|w| t.contains(w)) && !POLICY_WORDS.iter().any(|w| t.contains(w))
}

/// Unique values, order kept, for `OptionsEnv::custom`.
pub fn dedup(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values.into_iter().filter(|v| seen.insert(v.clone())).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const HELP_2_1_319: &str = "Usage: claude [options] [command] [prompt]\n\nOptions:\n  --mcp-config <configs...>             Load MCP servers from JSON files or\n                                        strings (space-separated)\n  --model <model>                       Model for the current session. Provide\n                                        an alias for the latest model (e.g.\n                                        'fable', 'opus', or 'sonnet') or a\n                                        model's full name.\n  -n, --name <name>                     Set a display name for this session\n                                        ('x' is not an alias)\n";

    #[test]
    fn model_value_table() {
        for ok in ["opus", "sonnet[1m]", "claude-opus-4-6[1m]", "us.anthropic.claude-opus-4-8", "claude-sonnet-4-5@20250929", "anthropic.claude-haiku-4-5-v1:0", "claude-opus-5-5", "best", "opusplan"] {
            assert!(valid_model(ok), "{ok}");
        }
        let long = format!("a{}", "b".repeat(100));
        for bad in ["-p", "opus --dangerously-skip-permissions", "Opus", "a/b", "null", "TRUE", "true", "yes", "no", "on", "off", "y", "n", "123", "1.5", "", " opus", "<synthetic>", "opus\n", "~", long.as_str()] {
            assert!(!valid_model(bad), "{bad:?}");
        }
        assert_eq!("a".repeat(100).len(), 100);
        assert!(valid_model(&"a".repeat(100)), "100 chars is fine");
        assert!(!valid_choice("inherit") && !valid_choice("default") && valid_choice("haiku"));
    }

    #[test]
    fn help_parser_reads_the_model_paragraph_only() {
        assert_eq!(parse_help(HELP_2_1_319).unwrap(), vec!["fable", "opus", "sonnet"]);
        assert_eq!(parse_help("Usage: claude\n  --model <model>  Model name.\n  --x  y\n"), None, "no quoted aliases");
        assert_eq!(parse_help("Usage: claude\n  --name <n>  'opus'\n"), None, "no --model line");
    }

    fn keys(v: &str) -> SettingsKeys {
        extract(&serde_json::from_str(v).unwrap())
    }

    #[test]
    fn settings_keep_only_model_keys_and_named_env() {
        let k = keys(r#"{"model":"sonnet","availableModels":["sonnet","claude-opus-4-6"],"modelOverrides":{"claude-opus-4-8":"arn:x"},
            "env":{"ANTHROPIC_DEFAULT_OPUS_MODEL":"claude-opus-5-5","ANTHROPIC_AUTH_TOKEN":"sk-canary-123","ANTHROPIC_API_KEY":"sk-canary-456"},"apiKeyHelper":"x"}"#);
        assert_eq!(k.model.as_deref(), Some("sonnet"));
        assert_eq!(k.overrides, vec!["claude-opus-4-8"]);
        assert_eq!(k.env, vec![("ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(), "claude-opus-5-5".to_string())]);
        let env = OptionsEnv { cli: Some(vec!["opus".into()]), user: Some(k), ..Default::default() };
        let out = serde_json::to_string(&options(&env, &PriceTable::bundled())).unwrap();
        for canary in ["sk-canary", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY", "arn:x", "apiKeyHelper"] {
            assert!(!out.contains(canary), "{canary} leaked: {out}");
        }
        assert!(out.contains(r#""resolved":{"id":"claude-opus-5-5","basis":"settings"}"#), "{out}");
    }

    #[test]
    fn sources_merge_and_sort() {
        let env = OptionsEnv {
            cli: parse_help(HELP_2_1_319),
            user: Some(keys(r#"{"model":"sonnet"}"#)),
            recent: vec!["claude-opus-5-5".into(), "claude-haiku-5-5".into(), "<synthetic>".into(), "has space".into(), "a".repeat(101)],
            ..Default::default()
        };
        let o = options(&env, &PriceTable::bundled());
        let got: Vec<(&str, Vec<Source>)> = o.items.iter().map(|i| (i.value.as_str(), i.sources.clone())).collect();
        assert_eq!(
            got,
            vec![
                ("fable", vec![Source::Cli]),
                ("opus", vec![Source::Cli]),
                ("sonnet", vec![Source::Cli, Source::Settings]),
                ("claude-opus-5-5", vec![Source::Recent]),
                ("claude-haiku-5-5", vec![Source::Recent]),
            ]
        );
        assert_eq!(o.dropped, 3);
        let opus = &o.items[1];
        assert_eq!(opus.resolved, Some(ResolvedId { id: "claude-opus-5-5".into(), basis: "recent" }));
        assert!(opus.price.is_some());
        assert_eq!(o.current_default.value.as_deref(), Some("sonnet"));
        assert_eq!(o.current_default.basis, "settings");
        assert!(o.cli_read);
        let none = options(&OptionsEnv { recent_main: Some("claude-opus-5-5".into()), ..Default::default() }, &PriceTable::bundled());
        assert!(!none.cli_read);
        assert_eq!((none.current_default.value.as_deref(), none.current_default.basis), (Some("claude-opus-5-5"), "recent"));
        let account = options(&OptionsEnv::default(), &PriceTable::bundled());
        assert_eq!((account.current_default.value, account.current_default.basis), (None, "account"));
        let env_first = OptionsEnv { process_env: pick_env([("ANTHROPIC_MODEL".into(), "haiku".into()), ("ANTHROPIC_API_KEY".into(), "k".into())]), user: Some(keys(r#"{"model":"sonnet"}"#)), ..Default::default() };
        assert_eq!(env_first.process_env.len(), 1);
        assert_eq!(options(&env_first, &PriceTable::bundled()).current_default.value.as_deref(), Some("haiku"));
    }

    #[test]
    fn recent_queries_skip_synthetic_and_dduck_model_sessions() {
        let conn = crate::db::open_in_memory().unwrap();
        let now = 100 * 24 * 3600 * 1000_i64;
        let ins = |id: &str, sid: &str, agent: Option<&str>, ts: i64, model: &str| {
            conn.execute("INSERT INTO api_calls(message_id, session_id, agent_id, ts, model) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, sid, agent, ts, model]).unwrap();
        };
        ins("m1", "s-cli", None, now - 1000, "claude-opus-5-5");
        ins("m2", "s-dduck", None, now - 10, "claude-haiku-5-5");
        ins("m3", "s-cli", None, now - 5, "<synthetic>");
        ins("m4", "s-cli", Some("a1"), now - 3, "claude-sonnet-5-5");
        ins("m5", "s-old", None, now - 31 * 24 * 3600 * 1000, "claude-old-1");
        conn.execute("INSERT INTO task_meta(bg_id, created_at, session_id, model_arg) VALUES ('ab12cd34', 1, 's-dduck', 'haiku')", []).unwrap();
        assert_eq!(recent_models(&conn, now).unwrap(), vec!["claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-5-5"]);
        assert_eq!(recent_main_model(&conn, now).unwrap().as_deref(), Some("claude-opus-5-5"), "D.Duck --model session left out");
        assert_eq!(first_main_model(&conn, "s-cli").unwrap().as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn allowlist_table() {
        let managed = |v: &str| ManagedRead { keys: vec![keys(v)], unreadable: false };
        let none = ManagedRead::default();
        let user = keys(r#"{"availableModels":["sonnet","claude-opus-4-6"]}"#);
        let l = allowlist(&none, &[&user]);
        assert_eq!(l.kind, AllowKind::Settings);
        for (v, want) in [("sonnet", Allow::Yes), ("sonnet[1m]", Allow::Yes), ("claude-sonnet-5-5", Allow::Yes), ("claude-opus-4-6", Allow::Yes), ("claude-opus-4-6[1m]", Allow::Yes), ("claude-opus-4-6-20250101", Allow::Yes), ("claude-opus-5-5", Allow::No), ("haiku", Allow::No)] {
            assert_eq!(allowed(&l, v).0, want, "{v}");
        }
        assert_eq!(allowed(&l, "opus"), (Allow::Yes, Some("관리 설정 범위의 버전으로 바뀌어요")), "family narrowed to an allowed version");
        assert_eq!(check_allowed(&l, "haiku"), Err(BLOCKED_SETTINGS));
        let m = allowlist(&managed(r#"{"availableModels":["sonnet","haiku"]}"#), &[&user]);
        assert_eq!(m.kind, AllowKind::Managed, "managed list alone");
        assert_eq!(check_allowed(&m, "claude-opus-5-5"), Err(BLOCKED_MANAGED));
        assert_eq!(check_allowed(&m, "claude-opus-4-6"), Err(BLOCKED_MANAGED), "user list cannot widen it");
        let empty = allowlist(&managed(r#"{"availableModels":[]}"#), &[]);
        assert_eq!(allowed(&empty, "sonnet").0, Allow::No);
        assert_eq!(allowlist(&none, &[&keys(r#"{"availableModelsMatch":"regex","availableModels":["sonnet"]}"#)]).kind, AllowKind::Unknown);
        assert_eq!(allowlist(&none, &[&keys(r#"{"availableModels":"sonnet"}"#)]).kind, AllowKind::Unknown);
        assert_eq!(allowlist(&none, &[&keys(r#"{"model":"x"}"#)]).kind, AllowKind::None);
        let broken = ManagedRead { keys: Vec::new(), unreadable: true };
        let u = allowlist(&broken, &[]);
        assert_eq!(check_allowed(&u, "sonnet"), Err(BLOCKED_UNKNOWN));
    }

    #[test]
    fn broken_managed_file_or_plist_blocks_everything_but_the_default() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("managed-settings.json");
        std::fs::write(&bad, "{not json").unwrap();
        let src = crate::delivery_guard::ManagedSources { files: vec![bad.clone()], dropin_dirs: vec![tmp.path().join("none.d")], plists: vec![tmp.path().join("none.plist")] };
        let r = read_managed(&src);
        assert!(r.unreadable);
        let env = OptionsEnv { cli: Some(vec!["opus".into(), "sonnet".into()]), managed: r, custom: vec!["claude-opus-5-5".into()], ..Default::default() };
        let o = options(&env, &PriceTable::bundled());
        assert_eq!(o.allowlist, AllowKind::Unknown);
        assert!(o.items.iter().all(|i| i.allow == Allow::Unknown && i.blocked_by == Some(AllowKind::Unknown)));
        // An MDM plist is never read: its presence alone blocks.
        let plist = tmp.path().join("com.anthropic.claudecode.plist");
        std::fs::write(&plist, "x").unwrap();
        std::fs::write(&bad, r#"{"availableModels":["sonnet"]}"#).unwrap();
        let src = crate::delivery_guard::ManagedSources { files: vec![bad.clone()], dropin_dirs: Vec::new(), plists: vec![plist.clone()] };
        assert!(read_managed(&src).unreadable);
        std::fs::remove_file(&plist).unwrap();
        let ok = read_managed(&src);
        assert!(!ok.unreadable);
        assert_eq!(ok.keys[0].available.as_deref(), Some(&["sonnet".to_string()][..]));
    }

    #[test]
    fn precedence_and_lock() {
        // Inline roles: same value in the role JSON and --model.
        assert_eq!(resolve(Some("opus"), Runner::Inline, Some("sonnet")), Ok(ModelPin { layer: Layer::Task, arg: Some("opus".into()), role: Some("opus".into()) }));
        assert_eq!(resolve(None, Runner::Inline, Some("sonnet")), Ok(ModelPin { layer: Layer::Type, arg: Some("sonnet".into()), role: Some("sonnet".into()) }));
        assert_eq!(resolve(None, Runner::Inline, None), Ok(ModelPin { layer: Layer::Cli, arg: None, role: None }));
        // File agents: the file wins over the type, and a task value on top is locked until spike 1.1.
        assert_eq!(resolve(None, Runner::FileAgent(Some("opus")), Some("haiku")), Ok(ModelPin { layer: Layer::Agent, arg: None, role: None }));
        assert_eq!(resolve(None, Runner::FileAgent(None), Some("haiku")), Ok(ModelPin { layer: Layer::Type, arg: Some("haiku".into()), role: None }));
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(Some("opus")), None), Err(LOCKED));
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(None), None), Ok(ModelPin { layer: Layer::Task, arg: Some("sonnet".into()), role: None }));
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(Some("inherit")), None).map(|p| p.layer), Ok(Layer::Task), "inherit = no model");
        // Stored values are re-checked.
        assert_eq!(resolve(None, Runner::Inline, Some("opus x")), Err(BAD_STORED));
        assert_eq!(resolve(Some("-p"), Runner::Inline, None), Err(BAD_FORMAT));
        let pin = ModelPin { layer: Layer::Type, arg: Some("sonnet".into()), role: Some("sonnet".into()) };
        assert_eq!(check_pin(&pin, Runner::FileAgent(Some("opus"))), Ok(()), "only task pins hit the lock");
        assert_eq!(check_pin(&ModelPin { layer: Layer::Task, ..pin.clone() }, Runner::FileAgent(Some("opus"))), Err(LOCKED));
        assert_eq!(check_pin(&ModelPin { layer: Layer::Task, arg: Some("opus x".into()), role: None }, Runner::Inline), Err(BAD_STORED));
        assert_eq!(Layer::parse("agent"), Some(Layer::Agent));
    }

    #[test]
    fn file_model_reads_the_raw_line() {
        assert_eq!(file_model("---\nname: a\nmodel: claude-opus-4-6[1m]\n---\nbody"), Some("claude-opus-4-6[1m]".into()));
        assert_eq!(file_model("---\nname: a\nmodel: inherit\n---\n"), None);
        assert_eq!(file_model("---\nname: a\nmodel: Opus X\n---\n"), None);
        assert_eq!(file_model("no frontmatter"), None);
    }

    #[test]
    fn mismatch_compares_family_or_prefix() {
        assert_eq!(mismatch_line(Some("opus"), Some("claude-sonnet-5-5")).as_deref(), Some("요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요"));
        assert_eq!(mismatch_line(Some("opus"), Some("claude-opus-5-5")), None);
        assert_eq!(same_model("us.anthropic.claude-opus-4-8", "claude-opus-4-8-20260101"), Some(true));
        assert_eq!(same_model("claude-opus-4-6[1m]", "claude-opus-4-6"), Some(true));
        assert_eq!(same_model("best", "claude-opus-5-5"), None);
        assert_eq!(same_model("opusplan", "claude-opus-5-5"), None);
        assert_eq!(same_model("opus", "<synthetic>"), None);
        assert_eq!(mismatch_line(None, Some("x")), None);
    }

    fn known() -> Known {
        Known::from_env(&OptionsEnv { cli: parse_help(HELP_2_1_319), recent_week: vec!["claude-sonnet-5-5".into()], ..Default::default() }, &PriceTable::bundled())
    }

    #[test]
    fn support_table() {
        let k = known();
        for ok in ["opus", "haiku", "sonnet[1m]", "best", "opusplan", "fable", "claude-sonnet-5-5", "claude-haiku-4-5-20251001", "claude-opus-4-6[1m]", "us.anthropic.claude-opus-4-8", "claude-sonnet-4-6@20250929"] {
            assert!(supported(ok, &k), "{ok}");
        }
        for gone in ["claude-opus-3-9", "claude-opus-5-9", "acme-large-2", "claude-haiku-4-5-2025", "claude-haiku-3-1"] {
            assert!(!supported(gone, &k), "{gone}");
        }
        // Settings values count, availableModels entries do not (design review BLOCKER).
        let env = OptionsEnv { user: Some(keys(r#"{"model":"acme-large-2","availableModels":["claude-haiku-3-1"]}"#)), ..Default::default() };
        let k2 = Known::from_env(&env, &PriceTable::bundled());
        assert!(supported("acme-large-2", &k2));
        assert!(!supported("claude-haiku-3-1", &k2));
    }

    #[test]
    fn family_words() {
        for (v, f) in [("opus[1m]", Some("opus")), ("opusplan", Some("opus")), ("claude-opus-3-9", Some("opus")), ("claude-3-5-sonnet-20241022", Some("sonnet")), ("us.anthropic.claude-haiku-3-1-v1:0", Some("haiku")), ("acme-large-2", None), ("best", None), ("claude-opus-sonnet-1", None)] {
            assert_eq!(family_of(v), f, "{v}");
        }
    }

    #[test]
    fn fallback_rules() {
        let k = known();
        let none = Allowlist::none();
        let ctx = FallbackCtx { known: &k, list: &none, default_value: Some("opus") };
        // Same family alias, without [1m].
        assert_eq!(fix(Some("claude-opus-3-9[1m]"), &ctx), Ok(Fixed { value: Some("opus".into()), requested: Some("claude-opus-3-9[1m]".into()) }));
        // Unknown family: the default.
        assert_eq!(fix(Some("acme-large-2"), &ctx), Ok(Fixed { value: None, requested: Some("acme-large-2".into()) }));
        // Supported values stay.
        assert_eq!(fix(Some("claude-haiku-4-5-20251001"), &ctx), Ok(Fixed { value: Some("claude-haiku-4-5-20251001".into()), requested: None }));
        assert_eq!(fix(None, &ctx), Ok(Fixed { value: None, requested: None }));
        // Policy-blocked values are refused, never replaced.
        let managed = allowlist(&ManagedRead { keys: vec![keys(r#"{"availableModels":["sonnet"]}"#)], unreadable: false }, &[]);
        let mctx = FallbackCtx { list: &managed, ..ctx };
        assert_eq!(fix(Some("claude-opus-3-9"), &mctx), Err(BLOCKED_MANAGED));
        let unknown = Allowlist { kind: AllowKind::Unknown, list: Vec::new() };
        assert_eq!(fix(Some("claude-opus-3-9"), &FallbackCtx { list: &unknown, ..ctx }), Err(BLOCKED_UNKNOWN));
        // A narrowed alias is no target, and the default may not be a pricier family.
        let user = allowlist(&ManagedRead::default(), &[&keys(r#"{"availableModels":["claude-haiku-3-1","opus"]}"#)]);
        let uctx = FallbackCtx { list: &user, ..ctx };
        assert_eq!(fix(Some("claude-haiku-3-1"), &uctx), Err(NO_FALLBACK));
        assert_eq!(fallback("claude-haiku-3-1", &FallbackCtx { default_value: None, ..uctx }), Err(NO_FALLBACK), "unknown default family");
        let cheap = allowlist(&ManagedRead::default(), &[&keys(r#"{"availableModels":["claude-opus-3-9","claude-opus-4-6","haiku"]}"#)]);
        assert_eq!(fallback("claude-opus-3-9", &FallbackCtx { list: &cheap, default_value: Some("haiku"), ..ctx }), Ok(Replacement::Default), "a cheaper default is fine");
    }

    #[test]
    fn fallback_by_layer() {
        let k = known();
        let none = Allowlist::none();
        let ctx = FallbackCtx { known: &k, list: &none, default_value: Some("sonnet") };
        let inline = ModelPin { layer: Layer::Type, arg: Some("claude-opus-3-9".into()), role: Some("claude-opus-3-9".into()) };
        assert_eq!(apply_fallback(&inline, Runner::Inline, &ctx), Ok((ModelPin { layer: Layer::Type, arg: Some("opus".into()), role: Some("opus".into()) }, Some("claude-opus-3-9".into()))));
        let to_default = ModelPin { layer: Layer::Task, arg: Some("acme-large-2".into()), role: Some("acme-large-2".into()) };
        assert_eq!(apply_fallback(&to_default, Runner::Inline, &ctx).unwrap().0, ModelPin { layer: Layer::Task, arg: None, role: None });
        // File agent: a task/type value on top is replaced in --model only.
        let on_file = ModelPin { layer: Layer::Type, arg: Some("claude-sonnet-3-9".into()), role: None };
        assert_eq!(apply_fallback(&on_file, Runner::FileAgent(None), &ctx).unwrap().0.arg.as_deref(), Some("sonnet"));
        // The file's own model: refused until spike 1.1.
        let agent = ModelPin { layer: Layer::Agent, arg: None, role: None };
        assert_eq!(apply_fallback(&agent, Runner::FileAgent(Some("claude-sonnet-3-9")), &ctx), Err(AGENT_UNSUPPORTED));
        assert_eq!(apply_fallback(&agent, Runner::FileAgent(Some("opus")), &ctx), Ok((agent.clone(), None)));
        let blocked = allowlist(&ManagedRead { keys: vec![keys(r#"{"availableModels":["sonnet"]}"#)], unreadable: false }, &[]);
        assert_eq!(apply_fallback(&agent, Runner::FileAgent(Some("opus")), &FallbackCtx { list: &blocked, ..ctx }), Err(BLOCKED_MANAGED));
    }

    #[test]
    fn cli_rejection_words() {
        assert!(rejects_model("Error: model claude-opus-3-9 not found", "claude-opus-3-9"));
        assert!(rejects_model("API Error: 404 {\"type\":\"not_found_error\",\"message\":\"model: claude-opus-3-9\"}", "claude-opus-3-9"));
        assert!(!rejects_model("Error: model claude-opus-3-9 is not available for your organization", "claude-opus-3-9"), "policy wording");
        assert!(!rejects_model("Error: model blocked by availableModels", "opus"));
        assert!(!rejects_model("Workspace not trusted", "opus"));
        assert!(!rejects_model("model sonnet not found", "opus"), "another model");
        const { assert!(!CLI_REJECT_RETRY_MEASURED) };
    }

    #[test]
    fn options_mark_stored_unsupported_values() {
        let env = OptionsEnv { cli: parse_help(HELP_2_1_319), custom: vec!["claude-opus-3-9".into(), "acme-large-2".into(), "claude-opus-5-5".into()], ..Default::default() };
        let o = options(&env, &PriceTable::bundled());
        let r = |v: &str| o.items.iter().find(|i| i.value == v).unwrap().replacement.clone();
        assert_eq!(r("claude-opus-3-9"), Some(Replacement::Alias { to: "opus".into() }));
        assert_eq!(r("acme-large-2"), Some(Replacement::Default));
        assert_eq!(r("claude-opus-5-5"), None);
        assert_eq!(r("opus"), None);
        let json = serde_json::to_string(&o).unwrap();
        assert!(json.contains(r#""replacement":{"kind":"alias","to":"opus"}"#), "{json}");
    }
}
