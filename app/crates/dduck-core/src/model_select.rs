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

pub const BAD_FORMAT: crate::i18n::Fail = crate::i18n::Fail::ModelNameFormat;
pub const BAD_STORED: crate::i18n::Fail = crate::i18n::Fail::ModelStoredBad;
pub const LOCKED: crate::i18n::Fail = crate::i18n::Fail::ModelAgentFixed;
pub const BLOCKED_MANAGED: crate::i18n::Fail = crate::i18n::Fail::ModelManagedBlock;
pub const BLOCKED_SETTINGS: crate::i18n::Fail = crate::i18n::Fail::ModelNotAllowed;
pub const BLOCKED_UNKNOWN: crate::i18n::Fail = crate::i18n::Fail::ModelAllowUnknown;

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
pub fn check_allowed(list: &Allowlist, value: &str) -> Result<(), &'static str> {
    match (allowed(list, value).0, list.kind) {
        (Allow::Yes, _) => Ok(()),
        (Allow::Unknown, _) | (_, AllowKind::Unknown) => Err(BLOCKED_UNKNOWN.str()),
        (Allow::No, AllowKind::Managed) => Err(BLOCKED_MANAGED.str()),
        (Allow::No, _) => Err(BLOCKED_SETTINGS.str()),
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
            None => items.push(ModelItem { value: v.to_string(), sources: vec![src], resolved: None, allow: Allow::Yes, blocked_by: None, note: None, price: None }),
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
    let mut st = conn.prepare(
        "SELECT model, MAX(ts) AS last FROM api_calls WHERE ts >= ?1 AND model != '<synthetic>' GROUP BY model ORDER BY last DESC LIMIT 40",
    )?;
    let rows = st.query_map(params![now_ms - THIRTY_DAYS_MS], |r| r.get::<_, String>(0))?;
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

fn stored_ok(v: Option<&str>) -> Result<Option<String>, &'static str> {
    match v {
        Some(v) if !valid_choice(v) => Err(BAD_STORED.str()),
        other => Ok(other.map(str::to_string)),
    }
}

/// task > agent file > type > CLI (design 4). Inline roles get the same value in the role JSON and `--model`; a file
/// agent with no task value gets no `--model` when its file sets one.
pub fn resolve(task: Option<&str>, runner: Runner<'_>, type_default: Option<&str>) -> Result<ModelPin, &'static str> {
    if task.is_some_and(|t| !valid_choice(t)) {
        return Err(BAD_FORMAT.str());
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
                (Some(_), Some(_), _) if !FILE_AGENT_MODEL_OVERRIDE_VERIFIED => Err(LOCKED.str()),
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
pub fn check_pin(pin: &ModelPin, runner: Runner<'_>) -> Result<(), &'static str> {
    stored_ok(pin.arg.as_deref())?;
    stored_ok(pin.role.as_deref())?;
    if pin.layer == Layer::Task && !FILE_AGENT_MODEL_OVERRIDE_VERIFIED {
        if let Runner::FileAgent(Some(f)) = runner {
            if valid_choice(f) {
                return Err(LOCKED.str());
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
        assert_eq!(check_allowed(&l, "haiku"), Err(BLOCKED_SETTINGS.str()));
        let m = allowlist(&managed(r#"{"availableModels":["sonnet","haiku"]}"#), &[&user]);
        assert_eq!(m.kind, AllowKind::Managed, "managed list alone");
        assert_eq!(check_allowed(&m, "claude-opus-5-5"), Err(BLOCKED_MANAGED.str()));
        assert_eq!(check_allowed(&m, "claude-opus-4-6"), Err(BLOCKED_MANAGED.str()), "user list cannot widen it");
        let empty = allowlist(&managed(r#"{"availableModels":[]}"#), &[]);
        assert_eq!(allowed(&empty, "sonnet").0, Allow::No);
        assert_eq!(allowlist(&none, &[&keys(r#"{"availableModelsMatch":"regex","availableModels":["sonnet"]}"#)]).kind, AllowKind::Unknown);
        assert_eq!(allowlist(&none, &[&keys(r#"{"availableModels":"sonnet"}"#)]).kind, AllowKind::Unknown);
        assert_eq!(allowlist(&none, &[&keys(r#"{"model":"x"}"#)]).kind, AllowKind::None);
        let broken = ManagedRead { keys: Vec::new(), unreadable: true };
        let u = allowlist(&broken, &[]);
        assert_eq!(check_allowed(&u, "sonnet"), Err(BLOCKED_UNKNOWN.str()));
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
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(Some("opus")), None), Err(LOCKED.str()));
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(None), None), Ok(ModelPin { layer: Layer::Task, arg: Some("sonnet".into()), role: None }));
        assert_eq!(resolve(Some("sonnet"), Runner::FileAgent(Some("inherit")), None).map(|p| p.layer), Ok(Layer::Task), "inherit = no model");
        // Stored values are re-checked.
        assert_eq!(resolve(None, Runner::Inline, Some("opus x")), Err(BAD_STORED.str()));
        assert_eq!(resolve(Some("-p"), Runner::Inline, None), Err(BAD_FORMAT.str()));
        let pin = ModelPin { layer: Layer::Type, arg: Some("sonnet".into()), role: Some("sonnet".into()) };
        assert_eq!(check_pin(&pin, Runner::FileAgent(Some("opus"))), Ok(()), "only task pins hit the lock");
        assert_eq!(check_pin(&ModelPin { layer: Layer::Task, ..pin.clone() }, Runner::FileAgent(Some("opus"))), Err(LOCKED.str()));
        assert_eq!(check_pin(&ModelPin { layer: Layer::Task, arg: Some("opus x".into()), role: None }, Runner::Inline), Err(BAD_STORED.str()));
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
}
