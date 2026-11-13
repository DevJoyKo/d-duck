//! Adapter around the `claude` CLI (D6, D-014, D-015, D-025). All parsing of CLI output lives here.
//!
//! Commands are spawned directly with argument vectors (never through a shell), stdin closed, and the effective
//! `CLAUDE_CONFIG_DIR` set on every child. Output shown to the user is kept in memory only.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use crate::i18n::ReasonCode;
use crate::process::Keep;
use serde::{Deserialize, Serialize};

pub const MAX_PROMPT_BYTES: usize = 100 * 1024;
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
const STDERR_TAIL_BYTES: usize = 2048;

/// Background short ids. Never starting with `-`, so an id cannot be read as an option by `stop`/`logs`/`attach`.
static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_][A-Za-z0-9_-]{0,63}$").unwrap_or_else(|_| unreachable!()));
/// Session ids for `--resume=`: lowercase UUIDs, the same as the transcript file name (usage-limit-resume).
static SESSION_UUID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap_or_else(|_| unreachable!()));
static BG_RE: LazyLock<Regex> =
    // The name part is present only when `--name` was given (CLI 2.1.319 prints `backgrounded · <id>` alone otherwise;
    // D-164: an unnamed start was read as a failure while the task ran).
    LazyLock::new(|| Regex::new(r"^backgrounded · ([A-Za-z0-9_-]{1,64})(?: · |$)").unwrap_or_else(|_| unreachable!()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    AcceptEdits,
    Auto,
    BypassPermissions,
    Manual,
    DontAsk,
    Plan,
}

impl PermissionMode {
    pub const ALL: [PermissionMode; 6] = [
        PermissionMode::AcceptEdits,
        PermissionMode::Auto,
        PermissionMode::BypassPermissions,
        PermissionMode::Manual,
        PermissionMode::DontAsk,
        PermissionMode::Plan,
    ];

    pub fn as_cli(self) -> &'static str {
        match self {
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Auto => "auto",
            PermissionMode::BypassPermissions => "bypassPermissions",
            PermissionMode::Manual => "manual",
            PermissionMode::DontAsk => "dontAsk",
            PermissionMode::Plan => "plan",
        }
    }

    /// Modes that let the agent act without per-action approval (D-015).
    pub fn is_dangerous(self) -> bool {
        matches!(self, PermissionMode::BypassPermissions | PermissionMode::Auto)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    Interactive,
    Background,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Working,
    WaitingInput,
    Idle,
    Done,
    Failed,
    Stopped,
    Unknown,
}

impl SessionStatus {
    pub fn is_ended(self) -> bool {
        matches!(self, SessionStatus::Done | SessionStatus::Failed | SessionStatus::Stopped)
    }
}

/// One entry of `claude agents --json --all`. `waitingFor` is deliberately not kept (it may hold tool details).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    /// Stable key: `sessionId`, or `pid:<n>` when absent.
    pub key: String,
    pub pid: Option<i64>,
    /// Background short id; only sessions with one support logs/stop/attach.
    pub id: Option<String>,
    pub session_id: Option<String>,
    pub name: String,
    pub cwd: String,
    pub kind: SessionKind,
    pub started_at: i64,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CliError {
    #[error("Claude CLI를 실행하지 못했어요")]
    Spawn,
    #[error("Claude CLI 응답 시간이 초과되었어요")]
    Timeout,
    #[error("이 디렉터리는 아직 Claude에서 신뢰되지 않았어요")]
    UntrustedWorkspace { stderr_tail: String },
    #[error("Claude CLI가 종료 코드 {code}로 실패했어요")]
    Failed { code: i32, stderr_tail: String },
    #[error("지원되지 않는 CLI 출력이에요")]
    UnsupportedOutput,
    #[error("잘못된 세션 ID이에요")]
    InvalidId,
    /// A refused input: a fixed reason code with typed params and a fallback sentence (app-language, D-171).
    #[error("{reason}")]
    InvalidInput {
        #[serde(flatten)]
        reason: crate::i18n::Reason,
    },
    #[error("위험 권한 모드는 확인이 필요해요")]
    DangerNotConfirmed,
    #[error("세션 프로세스를 종료하지 못했어요")]
    KillFailed,
    /// The user cancelled a start while it was being prepared (D-161). Nothing keeps running.
    #[error("작업 시작을 취소했어요")]
    Cancelled,
    /// Logged out (claude-login gate). Fixed code; checked before any CLI run, claim or confirmation.
    #[error("{}", crate::auth_status::LOGIN_REQUIRED_TEXT)]
    #[serde(rename = "login_required")]
    LoginRequired,
    /// Signed in another way than a claude.ai login, for a claude.ai-only feature.
    #[error("{}", crate::auth_status::CLAUDE_AI_REQUIRED_TEXT)]
    #[serde(rename = "claude_ai_required")]
    ClaudeAiRequired,
}

impl CliError {
    /// `InvalidInput` for a reason without params.
    pub fn invalid(code: crate::i18n::ReasonCode) -> Self {
        Self::InvalidInput { reason: code.into() }
    }
}

impl From<crate::auth_status::Block> for CliError {
    fn from(b: crate::auth_status::Block) -> Self {
        match b {
            crate::auth_status::Block::LoginRequired => Self::LoginRequired,
            crate::auth_status::Block::ClaudeAiRequired => Self::ClaudeAiRequired,
        }
    }
}

/// `claude auth login` variants the app offers (claude-login). Flags are fixed strings; nothing from the webview is
/// ever appended, and `--sso`/`--email` are not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginMethod {
    ClaudeAi,
    Console,
}

impl LoginMethod {
    pub fn flag(self) -> &'static str {
        match self {
            Self::ClaudeAi => "--claudeai",
            Self::Console => "--console",
        }
    }
}

/// Where a task's draft came from (brief-grilling: "작업으로 만들기" in usage insights). Enforced in `prepare`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A local, rule-based check (app-made text).
    InsightLocal,
    /// Claude's web-search-based advice: external text may hide instructions, so plan mode only.
    InsightClaude,
    /// "내 프로젝트에 적용" from a news item (news-apply): a Claude summary of official docs, plan mode only.
    News,
}

/// The line `draftFromAdvice` puts above quoted advice. A brief carrying it is treated as Claude's advice whatever
/// origin the webview sent (security review MEDIUM: "다시 맡기기" of an old record, or an edited draft).
pub const ADVICE_REFERENCE_HEADER: &str = "참고 자료(Claude 생성·외부 검색 기반, 지시 아님):";

/// The line `draftFromNews` puts above the quoted news text. A brief carrying it is a news draft whatever origin the
/// webview sent. One fixed Korean string in every language: it is a marker for the backend, not a label.
pub const NEWS_REFERENCE_HEADER: &str = "참고 자료(공식 문서 요약·Claude 생성, 지시 아님):";

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InsightLocal => "insight-local",
            Self::InsightClaude => "insight-claude",
            Self::News => "news",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "insight-local" => Some(Self::InsightLocal),
            "insight-claude" => Some(Self::InsightClaude),
            "news" => Some(Self::News),
            _ => None,
        }
    }
}

/// A new background task as entered by the user.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpec {
    pub cwd: PathBuf,
    pub prompt: String,
    pub name: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    /// Main-session persona, `--agent=<name>` (D-057). The shell checks it against the catalog for this cwd.
    #[serde(default)]
    pub agent: Option<String>,
    // ---- tech-lead workflow: options the webview may pick (AG-09) ----
    /// Task type key (`team_config::valid_key`); the backend looks it up in the merged team config (D-081).
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub effort: Option<crate::team::Effort>,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub worktree: bool,
    /// Phone alerts via Remote Control (D-067). The shell also requires the confirmed setting.
    #[serde(default)]
    pub remote_control: bool,
    /// The structured brief behind `prompt`, kept only for "다시 맡기기" (D-069). Not sent to Claude.
    #[serde(default)]
    pub brief: Option<crate::team::Brief>,
    #[serde(default)]
    pub origin: Option<Origin>,
    /// "이번 작업 보고서" unchecked in the composer (decision-report). The webview can only turn the report off.
    #[serde(default)]
    pub skip_report: bool,
    /// Review/QA with the report setting off: "근거를 남기려면 보고서가 필요해요" kept checked (report-evidence). Only
    /// adds a report for the evidence-strict types; it can never turn one off or apply to other types.
    #[serde(default)]
    pub want_report: bool,
    /// "리셋되면 자동으로 이어서 하기" unchecked in the composer (usage-limit-resume). The webview can only turn it off.
    #[serde(default)]
    pub limit_opt_out: bool,
    /// This task's model (model-select). Checked with `model_select::valid_choice` in `validate`.
    #[serde(default)]
    pub model: Option<String>,
    // ---- backend-only, filled by `prepare` from the merged team config; never deserialized ----
    #[serde(skip)]
    pub inline_role: Option<String>,
    #[serde(skip)]
    pub inline_roles_json: Option<String>,
    #[serde(skip)]
    pub auto_allowed: Vec<String>,
    #[serde(skip)]
    pub team_policy: bool,
    /// Team rules text (Composer and routine paths). `None` with `team_policy` = bundled rules.
    #[serde(skip)]
    pub policy_text: Option<String>,
    /// The task type's risk class (Rust-owned for deploy/security/qa) and label, for the shell's checks and dialogs.
    #[serde(skip)]
    pub strict_type: bool,
    #[serde(skip)]
    pub type_label: Option<String>,
    #[serde(skip)]
    pub worktree_name: Option<String>,
    #[serde(skip)]
    pub remote_name: Option<String>,
    /// Built-in tool set (`--tools=`), backend-only: the skill-authoring preset fixes it to file tools (D-086).
    #[serde(skip)]
    pub tools: Option<String>,
    /// App-owned report folder (`--add-dir`) and its instruction, set by `task_report::attach` only (D-108).
    #[serde(skip)]
    pub report_dir: Option<PathBuf>,
    #[serde(skip)]
    pub report_text: Option<String>,
    /// `report-dir` hook fragment (report-evidence 6.1), merged into delivery-guard's one `--settings`. Only set by
    /// `report_guard::grant`, which stays off until spike 1.1 (`report_guard::MEASURED`).
    #[serde(skip)]
    pub report_settings: Option<serde_json::Value>,
    /// Loadout skills left out at start because their file changed or disappeared (agent-loadout). A count only.
    #[serde(skip)]
    pub loadout_dropped: usize,
    /// 전달 범위 (delivery-guard), computed by the backend right before the start. `None` = the start is refused.
    #[serde(skip)]
    pub guard_ctx: Option<crate::delivery_guard::GuardCtx>,
    /// The language the task started in (app-language design 6), set by the shell and recorded in `task_meta`; a
    /// resume or rework reuses the recorded one. Adds one reply-language line at the end of the app's fixed text.
    #[serde(skip)]
    pub lang: Option<crate::i18n::Lang>,
    /// The task type's default model, from the merged team config (`prepare_with`).
    #[serde(skip)]
    pub type_model: Option<String>,
    /// A resume's recorded pin: reproduced instead of resolving the layers again (design 10).
    #[serde(skip)]
    pub model_pin_in: Option<crate::model_select::ModelPin>,
    /// What `apply_model` decided: the layer, the `--model=` value and the role's `model`.
    #[serde(skip)]
    pub model_pin: Option<crate::model_select::ModelPin>,
    /// The value asked for when an unsupported model was replaced (design 11).
    #[serde(skip)]
    pub model_requested: Option<String>,
}

const MAX_ARGV_BYTES: usize = 512 * 1024;

/// Windows `CreateProcessW` takes at most 32,767 UTF-16 units for the whole command line; this leaves room for the
/// quoted program path (decision 4).
pub const MAX_CMDLINE_UTF16: usize = 30_000;

/// UTF-16 length of `args` as std quotes them on Windows (`make_command_line`: quotes around an empty argument or one
/// with a space or tab, backslashes doubled before a quote, `"` escaped), plus one separator each. The program path
/// is not included (see [`MAX_CMDLINE_UTF16`]).
pub fn windows_cmdline_len(args: &[OsString]) -> usize {
    let mut total = 0;
    for a in args {
        let s = a.to_string_lossy();
        let quote = s.is_empty() || s.contains([' ', '\t']);
        let mut n = 1 + usize::from(quote) * 2; // separator and the quotes
        let mut backslashes = 0;
        for c in s.chars() {
            if c == '\\' {
                backslashes += 1;
            } else {
                if c == '"' {
                    n += backslashes + 1; // each backslash doubled, then `\"`
                }
                backslashes = 0;
            }
            n += c.len_utf16();
        }
        if quote {
            n += backslashes; // trailing backslashes before the closing quote are doubled
        }
        total += n;
    }
    total
}

/// `[a-z0-9-]`, starts with a letter, at most 40 chars, always prefixed `dduck-` (D-067).
pub fn remote_name(task_name: Option<&str>, now_ms: i64) -> String {
    let slug: String = task_name
        .unwrap_or("")
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let slug: String = slug.chars().take(28).collect::<String>().trim_end_matches('-').to_string();
    if slug.is_empty() {
        format!("dduck-{}", (now_ms / 1000) % 1_000_000)
    } else {
        format!("dduck-{slug}")
    }
}

impl TaskSpec {
    /// Plain spec (routines, tests): no tech-lead extras.
    pub fn basic(cwd: PathBuf, prompt: String, name: Option<String>, permission_mode: Option<PermissionMode>) -> Self {
        TaskSpec {
            cwd,
            prompt,
            name,
            permission_mode,
            agent: None,
            task_type: None,
            effort: None,
            sensitive: false,
            worktree: false,
            remote_control: false,
            brief: None,
            origin: None,
            inline_role: None,
            inline_roles_json: None,
            auto_allowed: Vec::new(),
            team_policy: false,
            policy_text: None,
            strict_type: false,
            type_label: None,
            worktree_name: None,
            remote_name: None,
            tools: None,
            skip_report: false,
            want_report: false,
            limit_opt_out: false,
            report_dir: None,
            report_text: None,
            report_settings: None,
            loadout_dropped: 0,
            guard_ctx: None,
            lang: None,
            model: None,
            type_model: None,
            model_pin_in: None,
            model_pin: None,
            model_requested: None,
        }
    }

    /// Applies the task type and the tech-lead guards (AG-02, AG-09, AG-11 … AG-13). `team` is the stored team
    /// config (DB `team_config`); it is merged and re-validated here on every start, falling back to bundled values.
    /// Everything injected is built here; the webview only chose the options. Not used for routines.
    pub fn prepare(&mut self, home: &std::path::Path, now_ms: i64, team: Option<&str>) -> Result<(), CliError> {
        self.prepare_with(home, now_ms, team, &crate::loadout::Env::default())
    }

    /// `prepare` with what the task's folder offers for the role's loadout (skills found there, static MCP server
    /// names). With an empty `env` every pinned skill counts as changed and no server is withheld.
    pub fn prepare_with(&mut self, home: &std::path::Path, now_ms: i64, team: Option<&str>, env: &crate::loadout::Env) -> Result<(), CliError> {
        use crate::loadout;
        use crate::team::{self, Effort, RepoStatus};
        use crate::team_config as tc;
        // The start language (a resume passes the task's recorded one), so bundled rules and fixed lines follow it.
        let config = tc::load_in(team, self.lang.unwrap_or_else(crate::i18n::current));
        self.team_policy = true;
        self.apply_origin(home)?;
        if let Some(key) = &self.task_type {
            if !tc::valid_key(key) {
                return Err(CliError::invalid(ReasonCode::TaskTypeFormat));
            }
            let t = config.get(key).ok_or(CliError::invalid(ReasonCode::TaskTypeUnknown))?.clone();
            let mode = self.permission_mode.unwrap_or(t.mode);
            if !t.modes.contains(&mode) || mode.is_dangerous() {
                return Err(CliError::invalid(ReasonCode::PermissionNotAllowed));
            }
            self.permission_mode = Some(mode);
            self.effort = Some(self.effort.unwrap_or(t.effort));
            self.type_model = t.model.clone();
            self.strict_type = t.strict;
            self.type_label = Some(t.label.clone());
            if self.agent.is_none() {
                if let Some(a) = &t.agent {
                    // "내 에이전트" role: the shell checks it like a persona (catalog, risk, deny).
                    self.agent = Some(a.clone());
                } else {
                    // A worktree checks project skills out of git, so they are not the files hashed here.
                    let verified = loadout::verify_in(&t.skills, env, self.worktree);
                    if verified.dropped > 0 && t.strict {
                        return Err(CliError::invalid(ReasonCode::LoadoutChanged));
                    }
                    // Without the full server list "only the chosen servers" would quietly allow everything.
                    if env.mcp_unreadable && !t.mcp_allow.is_empty() && (t.strict || t.role_tools.is_none()) {
                        return Err(CliError::invalid(ReasonCode::McpUnreadable));
                    }
                    self.loadout_dropped = verified.dropped;
                    let mut tools = t.role_tools.clone();
                    if let Some(list) = tools.as_mut() {
                        list.extend(loadout::mcp_allow_tools(&t.mcp_allow));
                    }
                    let disallowed = loadout::mcp_disallow(&t.mcp_allow, &env.mcp_servers);
                    let role = tc::RoleDef {
                        description: tc::role_description_in(&t, config.lang),
                        prompt: t.role_prompt.clone(),
                        tools,
                        skills: (!verified.skills.is_empty()).then_some(verified.skills),
                        disallowed_tools: (!disallowed.is_empty()).then_some(disallowed),
                        model: None,
                    };
                    let mut one = serde_json::Map::new();
                    one.insert(t.role.clone(), serde_json::to_value(&role).map_err(|_| CliError::invalid(ReasonCode::RoleFailed))?);
                    self.inline_roles_json = Some(serde_json::Value::Object(one).to_string());
                    self.inline_role = Some(t.role.clone());
                    self.auto_allowed = tc::auto_allowed_rules(&t);
                }
            }
        }
        if self.sensitive {
            if !matches!(self.permission_mode, Some(PermissionMode::Plan | PermissionMode::Manual) | None) {
                return Err(CliError::invalid(ReasonCode::SensitiveModes));
            }
            self.permission_mode = Some(self.permission_mode.unwrap_or(PermissionMode::Manual));
            self.effort = Some(self.effort.unwrap_or(Effort::Xhigh).max(Effort::Xhigh));
        }
        if self.worktree {
            if team::repo_status(&self.cwd, home) != RepoStatus::Ok {
                return Err(CliError::invalid(ReasonCode::WorktreeUnavailable));
            }
            self.worktree_name = Some(team::worktree_name(self.name.as_deref(), self.task_type.as_deref(), now_ms));
        }
        if self.remote_control {
            // Remote Control lets the phone drive the session: never for risky work (D-067, D-007).
            if self.sensitive || self.strict_type || self.permission_mode.is_some_and(PermissionMode::is_dangerous) {
                return Err(CliError::invalid(ReasonCode::RemoteRisky));
            }
            self.remote_name = Some(remote_name(self.name.as_deref(), now_ms));
        }
        let guard = self.guard_ctx.as_ref().map_or(crate::delivery_guard::Guard::DraftPr, |g| g.guard);
        // "먼저 살펴볼 스킬·에이전트": installed ones only, without writing agents for strict types or project items in a
        // worktree (preferred-skills).
        let ctx = crate::preferred::Ctx { strict: self.strict_type, worktree: self.worktree };
        let preferred = crate::preferred::line(&config.preferred, &env.preferred, config.lang, ctx);
        self.policy_text = Some(tc::policy_text_with(&config, self.remote_name.is_some(), guard, self.task_type.as_deref(), preferred.as_deref()));
        // Last: every injected value is in place.
        self.check_fit()
    }

    /// [`Self::args_fit`] and, on Windows, the command-line limit (its own reason, so the composer can say so).
    fn check_fit(&self) -> Result<(), CliError> {
        if !self.args_fit() {
            return Err(CliError::invalid(ReasonCode::TooLong));
        }
        if cfg!(windows) && !self.fits_windows_cmdline() {
            return Err(CliError::invalid(ReasonCode::WindowsCommandTooLong));
        }
        Ok(())
    }

    /// The quoted command line fits the Windows limit (checked on every OS by tests, enforced on Windows).
    pub fn fits_windows_cmdline(&self) -> bool {
        windows_cmdline_len(&self.bg_args()) <= MAX_CMDLINE_UTF16
    }

    /// Decides the model (design 4) and replaces an unsupported one (design 11). Called by the shell after
    /// `prepare_with`, once it knows the raw `model:` of the file agent `claude --agent` will use. Inline roles get
    /// the same value in the role JSON and `--model`.
    pub fn apply_model(&mut self, file_model: Option<&str>, ctx: &crate::model_select::FallbackCtx<'_>) -> Result<(), CliError> {
        use crate::model_select as ms;
        let runner = if self.inline_role.is_none() && self.agent.is_some() { ms::Runner::FileAgent(file_model) } else { ms::Runner::Inline };
        let pin = match &self.model_pin_in {
            Some(p) => {
                ms::check_pin(p, runner).map_err(CliError::invalid)?;
                p.clone()
            }
            None => ms::resolve(self.model.as_deref(), runner, self.type_model.as_deref()).map_err(CliError::invalid)?,
        };
        let (pin, requested) = ms::apply_fallback(&pin, runner, ctx).map_err(CliError::invalid)?;
        if let (Some(json), Some(role)) = (&self.inline_roles_json, &self.inline_role) {
            let mut v: serde_json::Value = serde_json::from_str(json).map_err(|_| CliError::invalid(ReasonCode::RoleFailed))?;
            let obj = v.get_mut(role).and_then(serde_json::Value::as_object_mut).ok_or(CliError::invalid(ReasonCode::RoleFailed))?;
            match &pin.role {
                Some(m) => {
                    obj.insert("model".into(), serde_json::Value::String(m.clone()));
                }
                None => {
                    obj.remove("model");
                }
            }
            self.inline_roles_json = Some(v.to_string());
        }
        self.model_pin = Some(pin);
        self.model_requested = requested;
        self.check_fit()
    }

    /// `--model=<v>` as one token, if a value is passed (design 2).
    pub fn model_args(&self) -> Vec<OsString> {
        self.model_pin.as_ref().and_then(|p| p.arg.as_ref()).map(|m| vec![format!("--model={m}").into()]).unwrap_or_default()
    }

    /// The whole argument vector stays under the CLI size cap.
    pub fn args_fit(&self) -> bool {
        self.bg_args().iter().map(|a| a.len()).sum::<usize>() <= MAX_ARGV_BYTES
    }

    /// Drafts from usage insights and news: Claude's advice (external search), a news summary or a task in the whole
    /// home folder start in plan mode only, with no phone control; the home folder also gets no worktree. Decided here,
    /// not by the webview's type or mode.
    fn apply_origin(&mut self, home: &std::path::Path) -> Result<(), CliError> {
        let carries = |header: &str| self.prompt.contains(header) || self.brief.as_ref().is_some_and(|b| b.constraints.contains(header));
        // Both headers: Claude's advice wins (the same rules, its own wording).
        if carries(ADVICE_REFERENCE_HEADER) {
            self.origin = Some(Origin::InsightClaude);
        } else if carries(NEWS_REFERENCE_HEADER) {
            self.origin = Some(Origin::News);
        }
        let Some(origin) = self.origin else { return Ok(()) };
        let canon = |p: &std::path::Path| crate::platform::canon(p).unwrap_or_else(|_| p.to_path_buf());
        let at_home = canon(&self.cwd) == canon(home);
        if !matches!(origin, Origin::InsightClaude | Origin::News) && !at_home {
            return Ok(());
        }
        let (no_remote, plan_only) = match origin {
            Origin::News => (ReasonCode::NewsNoRemote, ReasonCode::NewsPlanOnly),
            _ => (ReasonCode::AdviceNoRemote, ReasonCode::AdvicePlanOnly),
        };
        if self.remote_control {
            return Err(CliError::invalid(no_remote));
        }
        if self.permission_mode.is_some_and(|m| m != PermissionMode::Plan) {
            return Err(CliError::invalid(plan_only));
        }
        self.permission_mode = Some(PermissionMode::Plan);
        if at_home {
            self.worktree = false;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), CliError> {
        if !self.cwd.is_absolute() || !self.cwd.is_dir() {
            return Err(CliError::invalid(ReasonCode::DirNotFound));
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(CliError::invalid(ReasonCode::PromptSize));
        }
        if let Some(name) = &self.name {
            let ok = (1..=64).contains(&name.chars().count())
                && !name.starts_with('-')
                && !name.chars().any(char::is_control);
            if !ok {
                return Err(CliError::invalid(ReasonCode::NameFormat));
            }
        }
        if let Some(a) = &self.agent {
            if !crate::agents_catalog::valid_name(a) {
                return Err(CliError::invalid(ReasonCode::AgentNameFormat));
            }
        }
        if self.task_type.as_deref().is_some_and(|k| !crate::team_config::valid_key(k)) {
            return Err(CliError::invalid(ReasonCode::TaskTypeFormat));
        }
        if self.model.as_deref().is_some_and(|m| !crate::model_select::valid_choice(m)) {
            return Err(CliError::invalid(ReasonCode::ModelNameFormat));
        }
        Ok(())
    }

    /// Team rules and the report instruction as one `--append-system-prompt` value (also reused by resumes).
    pub fn system_text(&self) -> Option<String> {
        let local_only = self.guard_ctx.as_ref().is_some_and(|g| g.guard == crate::delivery_guard::Guard::LocalOnly);
        let lang = self.lang.unwrap_or_else(crate::i18n::current);
        let policy = if self.team_policy {
            Some(self.policy_text.clone().unwrap_or_else(|| {
                let guard = if local_only { crate::delivery_guard::Guard::LocalOnly } else { crate::delivery_guard::Guard::DraftPr };
                crate::team_config::policy_text_for(&crate::team_config::defaults_in(lang), self.remote_name.is_some(), guard, self.task_type.as_deref())
            }))
        } else {
            // The local-only line is a safety rule: it is added even where the team rules are not (delivery-guard).
            local_only.then(|| format!("{} {}", crate::team_config::safety_tag(lang), crate::delivery_guard::local_only_rule(lang)))
        };
        let report = self.report_text.as_deref().filter(|_| self.report_dir.is_some());
        let text = match (policy, report) {
            (Some(p), Some(r)) => Some(format!("{p}\n\n{r}")),
            (Some(p), None) => Some(p),
            (None, Some(r)) => Some(r.to_string()),
            (None, None) => None,
        };
        // Only after text the app adds: a task with no fixed instructions gets no system prompt for it.
        match (text, self.lang) {
            (Some(t), Some(lang)) => Some(format!("{t}\n\n{}", lang.reply_line())),
            (t, _) => t,
        }
    }

    /// Argument vector for `claude --bg`. Values use `--opt=value` and the prompt follows `--`, so nothing the user
    /// types can be parsed as an option (D-015).
    pub fn bg_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["--bg".into()];
        if let Some(name) = &self.name {
            args.push(format!("--name={name}").into());
        }
        // Always explicit: otherwise a `defaultMode` in the user's or project's settings could silently raise the
        // permission level without the native confirmation (D-037).
        let mode = self.permission_mode.unwrap_or(PermissionMode::Manual);
        args.push(format!("--permission-mode={}", mode.as_cli()).into());
        if let Some(effort) = self.effort {
            args.push(format!("--effort={}", effort.as_cli()).into());
        }
        args.extend(self.model_args());
        if let Some(w) = &self.worktree_name {
            args.push(format!("--worktree={w}").into());
        }
        if let Some(r) = &self.remote_name {
            args.push(format!("--remote-control={r}").into());
        }
        if let (Some(json), Some(role)) = (&self.inline_roles_json, &self.inline_role) {
            args.push(format!("--agents={json}").into());
            args.push(format!("--agent={role}").into());
        } else if let Some(agent) = &self.agent {
            args.push(format!("--agent={agent}").into());
        }
        if let Some(t) = &self.tools {
            args.push(format!("--tools={t}").into());
        }
        if !self.auto_allowed.is_empty() {
            args.push(format!("--allowedTools={}", self.auto_allowed.join(",")).into());
        }
        if let Some(dir) = &self.report_dir {
            let mut a = OsString::from("--add-dir=");
            a.push(dir.as_os_str());
            args.push(a);
        }
        if let Some(text) = self.system_text() {
            args.push(format!("--append-system-prompt={text}").into());
        }
        // Deny list (always) and the local-only hook; `start_background` refuses a spec without a usable scope.
        let extra: Vec<serde_json::Value> = self.report_settings.iter().cloned().collect();
        if let Some(Ok(guard)) = self.guard_ctx.as_ref().map(|g| crate::delivery_guard::guard_args_with(g, &extra)) {
            args.extend(guard);
        }
        args.push("--".into());
        args.push(self.prompt.clone().into());
        args
    }
}

/// Proof that the user approved a dangerous mode in a native dialog. Only the Tauri shell constructs it.
pub struct DangerApproval(());

impl DangerApproval {
    /// Call only after a backend-owned native confirmation dialog returned "yes".
    pub fn after_native_confirmation() -> Self {
        DangerApproval(())
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeCli {
    pub path: PathBuf,
    /// `Some` only when the user chose a non-default config dir (see `cli_path::ConfigDir::env_value`).
    pub config_dir_env: Option<PathBuf>,
    /// Login-shell PATH (`cli_path::ShellProbe::path`); `None` keeps the app's own PATH.
    pub path_env: Option<String>,
}

impl ClaudeCli {
    pub fn new(path: PathBuf, config_dir_env: Option<PathBuf>) -> Self {
        Self { path, config_dir_env, path_env: None }
    }

    pub fn with_path_env(mut self, path_env: Option<String>) -> Self {
        self.path_env = path_env;
        self
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.path);
        match &self.config_dir_env {
            Some(dir) => cmd.env("CLAUDE_CONFIG_DIR", dir),
            None => cmd.env_remove("CLAUDE_CONFIG_DIR"),
        };
        // nvm installs need `node` from the same bin dir; GUI launches have almost no PATH. Appended, not prepended,
        // so that directory never shadows system tools the agent runs.
        if let Some(dir) = self.path.parent() {
            let mut path = self.path_env.as_ref().map(std::ffi::OsString::from).or_else(|| std::env::var_os("PATH")).unwrap_or_default();
            if !path.is_empty() {
                path.push(if cfg!(windows) { ";" } else { ":" });
            }
            path.push(dir.as_os_str());
            cmd.env("PATH", path);
        }
        cmd
    }

    /// `claude --help` for the model list (model-select design 1): fixed argument, the given cwd (the app data folder),
    /// 5 s, 256 KB. Calls no model.
    pub fn help_text(&self, cwd: &Path) -> Result<String, CliError> {
        let out = run(self.command().current_dir(cwd).arg("--help"), Duration::from_secs(5), 256 * 1024, Keep::Head)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn list_agents(&self) -> Result<Vec<AgentSession>, CliError> {
        let out = run_detached(self.command().args(["agents", "--json", "--all"]), Duration::from_secs(5), 4 * 1024 * 1024, Keep::Head)?;
        parse_agents(&out.stdout)
    }

    /// `claude mcp list` (launches each configured server to check health). Raw stdout for `mcp_list::parse` only.
    pub fn mcp_list_raw(&self, cwd: &std::path::Path) -> Result<String, CliError> {
        let out = run(self.command().current_dir(cwd).args(["mcp", "list"]), Duration::from_secs(60), 256 * 1024, Keep::Head)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Guarded headless run that lists cloud routines (D-066). Returns raw stream-json for `cloud_routines`.
    pub fn cloud_routines_raw(&self, cwd: &std::path::Path, guard_cmd: &str) -> Result<Vec<u8>, CliError> {
        let out = run(
            self.command().current_dir(cwd).args(crate::cloud_routines::args(guard_cmd)),
            Duration::from_secs(60),
            8 * 1024 * 1024, // the stream repeats the ~1.3 MB response (D-072)
            Keep::Head,
        )?;
        Ok(out.stdout)
    }

    /// Guarded one-call RemoteTrigger run for editing a cloud routine (D-077). stdout is returned whatever the exit
    /// status, so a login failure in the result line can be told apart from other failures.
    pub fn cloud_edit_raw(&self, cwd: &Path, args: &[String]) -> Result<Vec<u8>, CliError> {
        crate::process::run(self.command().current_dir(cwd).args(args), Duration::from_secs(60), 8 * 1024 * 1024, Keep::Head, 64 * 1024)
            .map(|done| done.stdout)
            .map_err(|e| match e {
                crate::process::RunError::Spawn => CliError::Spawn,
                crate::process::RunError::Timeout => CliError::Timeout,
            })
    }

    /// Guarded usage-insights run (D-078). stdout is returned whatever the exit status (a budget stop exits non-zero
    /// with the JSON result); a timeout or `stop()` kills the process group.
    pub fn insights_raw(&self, cwd: &Path, args: &[String], timeout: Duration, stop: &dyn Fn() -> bool) -> Result<Vec<u8>, crate::insights::RunError> {
        use crate::insights::RunError as E;
        match crate::process::run_until(self.command().current_dir(cwd).args(args), timeout, 1024 * 1024, Keep::Head, 64 * 1024, stop) {
            Ok(done) => Ok(done.stdout),
            Err(crate::process::RunError::Spawn) => Err(E::Spawn),
            Err(crate::process::RunError::Timeout) if stop() => Err(E::Cancelled),
            Err(crate::process::RunError::Timeout) => Err(E::Timeout),
        }
    }

    /// `claude plugin list --json`: installed plugins only (D-075). Raw stdout for `plugins::parse_list`.
    pub fn plugin_list_raw(&self, cwd: &Path) -> Result<Vec<u8>, CliError> {
        let out = run(self.command().current_dir(cwd).args(["plugin", "list", "--json"]), Duration::from_secs(20), 1024 * 1024, Keep::Head)?;
        Ok(out.stdout)
    }

    /// `claude plugin enable|disable|update --json -s <scope> <id>`. Never passes `-y` or `--accept-command`: an
    /// update that needs a marketplace command approved comes back as `NeedsApproval` and nothing is run (D-075).
    /// stdin is /dev/null and a timeout kills the process group (`process::run`); stderr only feeds `classify`.
    pub fn plugin_action(&self, cwd: &Path, action: crate::plugins::Action, id: &str, scope: crate::plugins::Scope) -> Result<crate::plugins::Outcome, CliError> {
        let secs = if action == crate::plugins::Action::Update { 120 } else { 10 };
        self.plugin_action_within(cwd, action, id, scope, Duration::from_secs(secs))
    }

    pub(crate) fn plugin_action_within(
        &self,
        cwd: &Path,
        action: crate::plugins::Action,
        id: &str,
        scope: crate::plugins::Scope,
        timeout: Duration,
    ) -> Result<crate::plugins::Outcome, CliError> {
        use crate::plugins::{Action, Outcome};
        let Some(scope) = scope.as_cli() else { return Err(CliError::invalid(ReasonCode::ScopeFixed)) };
        if !crate::plugins::valid_id(id) {
            return Err(CliError::invalid(ReasonCode::PluginId));
        }
        let verb = match action {
            Action::Enable => "enable",
            Action::Disable => "disable",
            Action::Update => "update",
        };
        let mut cmd = self.command();
        cmd.current_dir(cwd).args(["plugin", verb, "--json", "-s", scope, id]);
        match crate::process::run(&mut cmd, timeout, 256 * 1024, Keep::Head, 64 * 1024) {
            Ok(done) => Ok(crate::plugins::classify(done.status.success(), &done.stdout, &done.stderr)),
            Err(crate::process::RunError::Timeout) => Ok(Outcome::Timeout),
            Err(crate::process::RunError::Spawn) => Err(CliError::Spawn),
        }
    }

    /// `claude plugin marketplace add --json --scope user <owner>/<repo>` (install-from-link, D-087). The source is
    /// rebuilt from validated parts, so it can never read as an option. stdin closed, process group killed on timeout.
    pub fn marketplace_add(&self, cwd: &Path, owner: &str, repo: &str, timeout: Duration) -> Result<crate::plugins::Outcome, CliError> {
        let Ok(crate::install_link::Link::GitHub { owner, repo, git_ref: None, path }) = crate::install_link::parse_link(&format!("{owner}/{repo}")) else {
            return Err(CliError::invalid(ReasonCode::RepoName));
        };
        if !path.is_empty() {
            return Err(CliError::invalid(ReasonCode::RepoName));
        }
        let mut cmd = self.command();
        cmd.current_dir(cwd).args(["plugin", "marketplace", "add", "--json", "--scope", "user", &format!("{owner}/{repo}")]);
        self.plugin_run(&mut cmd, timeout)
    }

    /// `claude plugin install --json -s user <name@market>`. Never `-y` or `--accept-command`: a marketplace-declared
    /// command comes back as `NeedsApproval` and nothing is run (D-075, D-087).
    pub fn plugin_install(&self, cwd: &Path, id: &str, timeout: Duration) -> Result<crate::plugins::Outcome, CliError> {
        if !crate::plugins::valid_id(id) {
            return Err(CliError::invalid(ReasonCode::PluginId));
        }
        let mut cmd = self.command();
        cmd.current_dir(cwd).args(["plugin", "install", "--json", "-s", "user", id]);
        self.plugin_run(&mut cmd, timeout)
    }

    fn plugin_run(&self, cmd: &mut Command, timeout: Duration) -> Result<crate::plugins::Outcome, CliError> {
        match crate::process::run(cmd, timeout, 256 * 1024, Keep::Head, 64 * 1024) {
            Ok(done) => Ok(crate::plugins::classify(done.status.success(), &done.stdout, &done.stderr)),
            Err(crate::process::RunError::Timeout) => Ok(crate::plugins::Outcome::Timeout),
            Err(crate::process::RunError::Spawn) => Err(CliError::Spawn),
        }
    }

    /// Starts a background task. Dangerous modes require a [`DangerApproval`].
    pub fn start_background(&self, spec: &TaskSpec, approval: Option<DangerApproval>) -> Result<String, CliError> {
        spec.validate()?;
        // Never without a delivery scope (delivery-guard): no fallback to the default one.
        let guard = spec.guard_ctx.as_ref().ok_or(CliError::invalid(ReasonCode::GuardUnreadable))?;
        crate::delivery_guard::guard_args(guard).map_err(|reason| CliError::invalid(ReasonCode::from_guard(reason)))?;
        if spec.permission_mode.is_some_and(PermissionMode::is_dangerous) && approval.is_none() {
            return Err(CliError::DangerNotConfirmed);
        }
        let args = spec.bg_args();
        // Windows: refused before anything starts when the quoted command line is over the limit (decision 4).
        if cfg!(windows) && windows_cmdline_len(&args) > MAX_CMDLINE_UTF16 {
            return Err(CliError::invalid(ReasonCode::WindowsCommandTooLong));
        }
        let out = run_detached(self.command().current_dir(&spec.cwd).args(args), Duration::from_secs(30), 64 * 1024, Keep::Head)?;
        parse_bg_id(&out.stdout).ok_or(CliError::UnsupportedOutput)
    }

    /// Resumes a stopped session in the background (usage-limit-resume). Dangerous modes require a [`DangerApproval`].
    /// An untrusted folder comes back as `UntrustedWorkspace`; the caller must not retry through the trust flow.
    pub fn resume_background(&self, spec: &ResumeSpec, approval: Option<DangerApproval>) -> Result<ResumeOutput, CliError> {
        if spec.permission_mode.is_dangerous() && approval.is_none() {
            return Err(CliError::DangerNotConfirmed);
        }
        if !is_safe_cwd(&spec.cwd) {
            return Err(CliError::invalid(ReasonCode::DirNotFound));
        }
        let args = spec.args()?;
        if cfg!(windows) && windows_cmdline_len(&args) > MAX_CMDLINE_UTF16 {
            return Err(CliError::invalid(ReasonCode::WindowsCommandTooLong));
        }
        let out = run_detached(self.command().current_dir(&spec.cwd).args(args), Duration::from_secs(30), 64 * 1024, Keep::Head)?;
        parse_resume_output(&out.stdout).ok_or(CliError::UnsupportedOutput)
    }

    /// Latest terminal screen of a background session, at most [`MAX_LOG_BYTES`] read.
    pub fn logs(&self, id: &str) -> Result<Vec<u8>, CliError> {
        check_id(id)?;
        // The newest frame is at the end of the stream, so keep the tail (D-038).
        let out = run_detached(self.command().args(["logs", id]), Duration::from_secs(5), MAX_LOG_BYTES, Keep::Tail)?;
        Ok(last_screen(&out.stdout).to_vec())
    }

    pub fn stop(&self, id: &str) -> Result<(), CliError> {
        check_id(id)?;
        run_detached(self.command().args(["stop", id]), Duration::from_secs(10), 64 * 1024, Keep::Head).map(|_| ())
    }

    /// `claude auth status --json` (claude-login): (exit code, stdout) only, 5 s, 64 KB. stderr is never kept, so a
    /// logged-out exit 1 cannot carry CLI text into an error message or IPC. Feed the result to `auth_status::judge`
    /// and drop the buffer (it holds email/org/paths).
    pub fn auth_status_raw(&self) -> Result<(Option<i32>, Vec<u8>), CliError> {
        crate::process::run(self.command().args(["auth", "status", "--json"]), Duration::from_secs(5), 64 * 1024, Keep::Head, 0)
            .map(|done| (done.status.code(), done.stdout))
            .map_err(|e| match e {
                crate::process::RunError::Spawn => CliError::Spawn,
                crate::process::RunError::Timeout => CliError::Timeout,
            })
    }

    /// Spawns `claude auth login <flag>` with the task environment, stdin null, stdout/stderr piped, in its own
    /// process group. The caller owns the child: it must drain both pipes and kill the group before reaping.
    pub fn login_child(&self, method: LoginMethod) -> std::io::Result<crate::process::Spawned> {
        use std::process::Stdio;
        let mut cmd = self.command();
        cmd.args(["auth", "login", method.flag()]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // Unix: own process group. Windows: no console window and no job, so a browser the CLI opens outlives the
        // login (review H2); ending the login kills only this process.
        crate::process::Spawned::login(&mut cmd)
    }

    /// Terminal-fallback argv parts: the CLI path and the config dir (`None` = default profile). The flag is chosen
    /// by the shell's fixed script, never from here.
    pub fn login_terminal_parts(&self) -> (&Path, Option<&Path>) {
        (&self.path, self.config_dir_env.as_deref())
    }

    /// Shell command a user can paste to attach (used when terminal automation is denied).
    pub fn attach_command_line(&self, id: &str) -> Result<String, CliError> {
        check_id(id)?;
        // Windows: a PowerShell line (decision 6); a value that fails the terminal rule is refused.
        if cfg!(windows) {
            let t = crate::terminal_win::Target::Attach { cli: &self.path, id, config_dir: self.config_dir_env.as_deref() };
            return crate::terminal_win::copy_line(&t).ok_or(CliError::InvalidId);
        }
        let prefix = match &self.config_dir_env {
            Some(dir) => format!("CLAUDE_CONFIG_DIR={} ", shell_quote(&dir.to_string_lossy())),
            None => String::new(),
        };
        Ok(format!("{prefix}{} attach {id}", shell_quote(&self.path.to_string_lossy())))
    }
}

pub fn check_id(id: &str) -> Result<(), CliError> {
    if ID_RE.is_match(id) { Ok(()) } else { Err(CliError::InvalidId) }
}

pub fn check_session_id(id: &str) -> Result<(), CliError> {
    if SESSION_UUID_RE.is_match(id) { Ok(()) } else { Err(CliError::InvalidId) }
}

/// `claude --bg --resume=<session>` for a task that stopped at a usage limit (usage-limit-resume, design 4). Built from
/// the same prepared values as the start, never from the webview. `--worktree` and `--remote-control` are never
/// passed again: the session cwd already is the worktree, and phone-controlled tasks are not resumed.
#[derive(Debug, Clone)]
pub struct ResumeSpec {
    pub session_id: String,
    /// The fixed resume prompt in the task's start language (`limit_resume::resume_prompt`).
    pub prompt: &'static str,
    pub cwd: PathBuf,
    pub name: Option<String>,
    pub permission_mode: PermissionMode,
    pub effort: Option<crate::team::Effort>,
    pub auto_allowed: Vec<String>,
    pub inline_roles_json: Option<String>,
    pub inline_role: Option<String>,
    pub agent: Option<String>,
    pub tools: Option<String>,
    pub report_dir: Option<PathBuf>,
    /// The start's `report-dir` hook fragment, if any (merged into the same `--settings`).
    pub report_settings: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    /// Delivery scope recomputed by the shell right before the resume (delivery-guard D-143). `None` refuses the
    /// resume: there is no default scope to fall back to.
    pub guard: Option<crate::delivery_guard::GuardCtx>,
    /// Model arguments (`--model=…`) reproduced from the start. Empty until model-select fills it (its task 2.8,
    /// D-154); passed as-is before the prompt.
    pub model_args: Vec<OsString>,
}

impl ResumeSpec {
    /// From a `TaskSpec` that went through `prepare_with` (with `worktree` and `remote_control` off) and the report
    /// attach. `cwd` is the checked session folder.
    pub fn from_prepared(spec: &TaskSpec, session_id: &str, cwd: PathBuf) -> Result<Self, CliError> {
        check_session_id(session_id)?;
        if spec.worktree_name.is_some() || spec.remote_name.is_some() {
            return Err(CliError::invalid(ReasonCode::ResumeNoReattach));
        }
        Ok(ResumeSpec {
            session_id: session_id.to_string(),
            prompt: crate::limit_resume::resume_prompt(spec.lang.unwrap_or_else(crate::i18n::current)),
            cwd,
            name: spec.name.clone(),
            permission_mode: spec.permission_mode.unwrap_or(PermissionMode::Manual),
            effort: spec.effort,
            auto_allowed: spec.auto_allowed.clone(),
            inline_roles_json: spec.inline_roles_json.clone(),
            inline_role: spec.inline_role.clone(),
            agent: spec.agent.clone(),
            tools: spec.tools.clone(),
            report_dir: spec.report_dir.clone(),
            report_settings: spec.report_settings.clone(),
            system_prompt: spec.system_text(),
            guard: None,
            model_args: spec.model_args(),
        })
    }

    pub fn args(&self) -> Result<Vec<OsString>, CliError> {
        check_session_id(&self.session_id)?;
        let mut args: Vec<OsString> = vec!["--bg".into(), format!("--resume={}", self.session_id).into()];
        if let Some(name) = &self.name {
            args.push(format!("--name={name}").into());
        }
        args.push(format!("--permission-mode={}", self.permission_mode.as_cli()).into());
        if let Some(effort) = self.effort {
            args.push(format!("--effort={}", effort.as_cli()).into());
        }
        if let (Some(json), Some(role)) = (&self.inline_roles_json, &self.inline_role) {
            args.push(format!("--agents={json}").into());
            args.push(format!("--agent={role}").into());
        } else if let Some(agent) = &self.agent {
            args.push(format!("--agent={agent}").into());
        }
        if let Some(t) = &self.tools {
            args.push(format!("--tools={t}").into());
        }
        if !self.auto_allowed.is_empty() {
            args.push(format!("--allowedTools={}", self.auto_allowed.join(",")).into());
        }
        if let Some(dir) = &self.report_dir {
            let mut a = OsString::from("--add-dir=");
            a.push(dir.as_os_str());
            args.push(a);
        }
        if let Some(text) = &self.system_prompt {
            args.push(format!("--append-system-prompt={text}").into());
        }
        // The same deny list and `local_only` hook as a start (`delivery_guard::guard_args`).
        let ctx = self.guard.as_ref().ok_or(CliError::invalid(ReasonCode::ResumeNoScope))?;
        let extra: Vec<serde_json::Value> = self.report_settings.iter().cloned().collect();
        args.extend(crate::delivery_guard::guard_args_with(ctx, &extra).map_err(|reason| CliError::invalid(ReasonCode::from_guard(reason)))?);
        args.extend(self.model_args.iter().cloned());
        args.push("--".into());
        args.push(self.prompt.into());
        Ok(args)
    }
}

/// What `--bg --resume` printed. The copy notice wording is not measured yet (spike 1.1): any line mentioning a copy
/// counts, and the shell also compares the new session's id afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeOutput {
    Started(String),
    Copy(Option<String>),
}

pub fn parse_resume_output(stdout: &[u8]) -> Option<ResumeOutput> {
    let text = String::from_utf8_lossy(stdout);
    let id = parse_bg_id(stdout);
    if text.lines().any(|l| l.to_ascii_lowercase().contains("copy")) {
        return Some(ResumeOutput::Copy(id));
    }
    id.map(ResumeOutput::Started)
}

pub fn parse_bg_id(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    let first = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    BG_RE.captures(first).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

pub fn parse_agents(stdout: &[u8]) -> Result<Vec<AgentSession>, CliError> {
    let items: Vec<serde_json::Value> = serde_json::from_slice(stdout).map_err(|_| CliError::UnsupportedOutput)?;
    Ok(items.iter().filter_map(parse_agent).collect())
}

fn parse_agent(v: &serde_json::Value) -> Option<AgentSession> {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let pid = v.get("pid").and_then(|x| x.as_i64());
    let session_id = s("sessionId");
    let key = session_id.clone().or_else(|| pid.map(|p| format!("pid:{p}")))?;
    let id = s("id").filter(|i| ID_RE.is_match(i));
    let kind = match s("kind").as_deref() {
        Some("interactive") => SessionKind::Interactive,
        Some("background") => SessionKind::Background,
        _ => SessionKind::Other,
    };
    Some(AgentSession {
        key,
        pid,
        id,
        session_id,
        name: s("name").map(|n| crate::text::display_safe(&n, 64)).unwrap_or_default(),
        cwd: s("cwd").unwrap_or_default(),
        kind,
        started_at: v.get("startedAt").and_then(|x| x.as_i64()).unwrap_or(0),
        status: normalize_status(s("state").as_deref(), s("status").as_deref()),
    })
}

/// `state` (background sessions) wins; otherwise `status`. A session waiting on the user is always WaitingInput.
pub fn normalize_status(state: Option<&str>, status: Option<&str>) -> SessionStatus {
    if status == Some("waiting") {
        return SessionStatus::WaitingInput;
    }
    match state {
        Some("working") => SessionStatus::Working,
        Some("blocked") => SessionStatus::WaitingInput,
        Some("done") => SessionStatus::Done,
        Some("failed") => SessionStatus::Failed,
        Some("stopped") => SessionStatus::Stopped,
        _ => match status {
            Some("busy") => SessionStatus::Working,
            Some("idle") => SessionStatus::Idle,
            _ => SessionStatus::Unknown,
        },
    }
}

/// Keeps output from the last full-screen clear (`ESC[2J`), so a truncated TUI stream still renders cleanly.
pub fn last_screen(out: &[u8]) -> &[u8] {
    const CLEAR: &[u8] = b"\x1b[2J";
    match out.windows(CLEAR.len()).rposition(|w| w == CLEAR) {
        Some(i) => &out[i..],
        None => out,
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

struct Output {
    stdout: Vec<u8>,
}

fn run(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep) -> Result<Output, CliError> {
    finish(crate::process::run(cmd, timeout, max_stdout, keep, 64 * 1024))
}

/// [`run`] for commands that talk to or start the CLI's background service (Windows: never in a job).
fn run_detached(cmd: &mut Command, timeout: Duration, max_stdout: u64, keep: Keep) -> Result<Output, CliError> {
    finish(crate::process::run_detached(cmd, timeout, max_stdout, keep, 64 * 1024))
}

fn finish(done: Result<crate::process::Finished, crate::process::RunError>) -> Result<Output, CliError> {
    let done = done.map_err(|e| match e {
        crate::process::RunError::Spawn => CliError::Spawn,
        crate::process::RunError::Timeout => CliError::Timeout,
    })?;
    if done.status.success() {
        return Ok(Output { stdout: done.stdout });
    }
    let tail = stderr_tail(&done.stderr);
    if tail.contains("Workspace not trusted") {
        return Err(CliError::UntrustedWorkspace { stderr_tail: tail });
    }
    Err(CliError::Failed { code: done.status.code().unwrap_or(-1), stderr_tail: tail })
}

/// Last part of stderr for display only: control characters removed. Never written to disk or logs.
fn stderr_tail(stderr: &[u8]) -> String {
    let start = stderr.len().saturating_sub(STDERR_TAIL_BYTES);
    String::from_utf8_lossy(&stderr[start..])
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Exposed for the shell layer: absolute path check used before spawning a terminal.
pub fn is_safe_cwd(p: &Path) -> bool {
    p.is_absolute() && p.is_dir()
}

#[cfg(test)]
mod reply_language {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn the_reply_line_ends_the_app_text_and_follows_the_start_language() {
        let base = TaskSpec { team_policy: true, ..TaskSpec::basic(PathBuf::from("/tmp"), "go".into(), None, None) };
        assert!(!base.system_text().unwrap_or_default().contains(Lang::Ko.reply_line()), "no language: unchanged");
        let ko = TaskSpec { lang: Some(Lang::Ko), ..base.clone() }.system_text().unwrap_or_default();
        assert!(ko.ends_with(Lang::Ko.reply_line()) && ko.starts_with("[D.Duck 팀 규칙]"));
        let en = TaskSpec { lang: Some(Lang::En), ..base.clone() }.system_text().unwrap_or_default();
        assert!(en.ends_with(Lang::En.reply_line()));
        let qa = TaskSpec { lang: Some(Lang::En), task_type: Some("qa".into()), ..base.clone() }.system_text().unwrap_or_default();
        assert!(qa.contains(&format!("[Required] {}", crate::team_config::EVIDENCE_QA_EN)) && qa.ends_with(Lang::En.reply_line()), "after the QA evidence rule");
        assert!(en.starts_with("[D.Duck team rules]") && !en.contains("[안전]") && en.contains("[Safety]"), "the English start gets the English rules");
        // Local-only with the team rules off still carries the safety line, in the start language.
        let guard = crate::delivery_guard::GuardCtx { guard: crate::delivery_guard::Guard::LocalOnly, ..crate::delivery_guard::GuardCtx::for_tests() };
        let bare_local = TaskSpec { lang: Some(Lang::En), guard_ctx: Some(guard.clone()), ..TaskSpec::basic(PathBuf::from("/tmp"), "go".into(), None, None) };
        assert!(bare_local.system_text().unwrap_or_default().starts_with(&format!("[Safety] {}", crate::delivery_guard::LOCAL_ONLY_RULE_EN)));
        let bare_local_ko = TaskSpec { lang: Some(Lang::Ko), guard_ctx: Some(guard), ..TaskSpec::basic(PathBuf::from("/tmp"), "go".into(), None, None) };
        assert!(bare_local_ko.system_text().unwrap_or_default().starts_with(&format!("[안전] {}", crate::delivery_guard::LOCAL_ONLY_RULE)));
        let bare = TaskSpec { lang: Some(Lang::En), ..TaskSpec::basic(PathBuf::from("/tmp"), "go".into(), None, None) };
        assert_eq!(bare.system_text(), None, "no fixed text, no line of its own");
        let resumed = ResumeSpec::from_prepared(&TaskSpec { lang: Some(Lang::En), ..base }, "0f3b2a8e-1111-4222-8333-444455556666", PathBuf::from("/tmp")).unwrap_or_else(|_| unreachable!());
        let sp = resumed.system_prompt.unwrap_or_default();
        assert!(sp.ends_with(Lang::En.reply_line()) && sp.starts_with("[D.Duck team rules]"), "a resume keeps the start language");
        assert_eq!(resumed.prompt, crate::limit_resume::RESUME_PROMPT_EN, "the resume prompt follows the start language too");
    }
}

#[cfg(test)]
mod start_language_rules {
    use super::*;
    use crate::i18n::Lang;

    /// A resume rebuilds the team rules with `prepare_with` and the task's recorded language: the rules, markers and
    /// role description follow that language, not the one the app shows now (Korean in tests).
    #[test]
    fn prepare_builds_team_rules_in_the_spec_language() {
        assert_eq!(crate::i18n::current(), Lang::Ko);
        let home = std::env::temp_dir();
        let mut en = TaskSpec { lang: Some(Lang::En), task_type: Some("qa".into()), ..TaskSpec::basic(home.clone(), "go".into(), None, None) };
        en.prepare(&home, 0, None).unwrap_or_else(|e| panic!("{e:?}"));
        let p = en.policy_text.clone().unwrap_or_default();
        assert!(p.starts_with("[D.Duck team rules]") && p.contains("[Safety]") && p.contains("[Required]") && !p.contains("[안전]"), "{p}");
        assert!(en.inline_roles_json.as_deref().is_some_and(|j| !j.chars().any(|c| ('가'..='힣').contains(&c))), "English role description");
        let mut ko = TaskSpec { lang: Some(Lang::Ko), task_type: Some("qa".into()), ..TaskSpec::basic(home.clone(), "go".into(), None, None) };
        ko.prepare(&home, 0, None).unwrap_or_else(|e| panic!("{e:?}"));
        assert!(ko.policy_text.unwrap_or_default().starts_with("[D.Duck 팀 규칙]"));
    }
}

#[cfg(test)]
mod error_serialization {
    use super::CliError;
    use crate::i18n::ReasonCode;

    /// Every variant must reach the webview as JSON (`#[serde(tag)]` cannot carry a bare string, QA F1).
    #[test]
    fn every_variant_serializes() {
        let all = [
            CliError::Spawn,
            CliError::Timeout,
            CliError::UntrustedWorkspace { stderr_tail: "x".into() },
            CliError::Failed { code: 1, stderr_tail: "x".into() },
            CliError::UnsupportedOutput,
            CliError::InvalidId,
            CliError::invalid(ReasonCode::AgentNotHere),
            CliError::DangerNotConfirmed,
            CliError::KillFailed,
            CliError::LoginRequired,
            CliError::ClaudeAiRequired,
        ];
        for e in all {
            let v = serde_json::to_value(&e);
            assert!(v.is_ok(), "{e:?} must serialize");
        }
        assert_eq!(serde_json::to_value(CliError::LoginRequired).ok(), Some(serde_json::json!({"kind": "login_required"})));
        assert_eq!(serde_json::to_value(CliError::ClaudeAiRequired).ok(), Some(serde_json::json!({"kind": "claude_ai_required"})));
        assert_eq!(CliError::LoginRequired.to_string(), crate::auth_status::LOGIN_REQUIRED_TEXT);
        let v = serde_json::to_value(CliError::invalid(ReasonCode::AgentNotHere)).unwrap_or_default();
        assert_eq!(v["kind"], "invalidInput");
        assert_eq!(v["code"], "agentNotHere");
        assert_eq!(v["params"], serde_json::json!({}));
        assert_eq!(v["text"], "이 폴더에서 쓸 수 없는 에이전트예요");
        assert_eq!(v["reason"], v["text"], "screens that show `reason` as is keep working");
        assert_eq!(CliError::invalid(ReasonCode::AgentNotHere).to_string(), "이 폴더에서 쓸 수 없는 에이전트예요");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn child_path_uses_login_shell_path_or_falls_back() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("claude");
        std::fs::write(&bin, "#!/bin/sh\nprintf %s \"$PATH\"\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dir = tmp.path().display().to_string();
        let path_of = |cli: &ClaudeCli| String::from_utf8(cli.command().output().unwrap().stdout).unwrap();
        let with = ClaudeCli::new(bin.clone(), None).with_path_env(Some("/opt/homebrew/bin:/usr/bin:/bin".into()));
        assert_eq!(path_of(&with), format!("/opt/homebrew/bin:/usr/bin:/bin:{dir}"));
        let without = ClaudeCli::new(bin, None);
        let app = std::env::var("PATH").unwrap_or_default();
        assert_eq!(path_of(&without), if app.is_empty() { dir } else { format!("{app}:{dir}") }, "probe failed → app PATH");
    }

    #[test]
    fn status_normalization() {
        assert_eq!(normalize_status(Some("working"), Some("busy")), SessionStatus::Working);
        assert_eq!(normalize_status(Some("done"), Some("idle")), SessionStatus::Done);
        assert_eq!(normalize_status(None, Some("busy")), SessionStatus::Working);
        assert_eq!(normalize_status(None, Some("waiting")), SessionStatus::WaitingInput);
        assert_eq!(normalize_status(Some("working"), Some("waiting")), SessionStatus::WaitingInput);
        assert_eq!(normalize_status(Some("blocked"), None), SessionStatus::WaitingInput);
        assert_eq!(normalize_status(None, Some("idle")), SessionStatus::Idle);
        assert_eq!(normalize_status(None, None), SessionStatus::Unknown);
    }

    #[test]
    fn parses_real_agents_json_shapes() {
        // Shapes observed on 2.1.319: interactive (no id/state) and background (with id/state).
        let json = br#"[
          {"pid":4494,"cwd":"/w/a","kind":"interactive","startedAt":1794028806968,"sessionId":"d01d","name":"a-d5","status":"idle"},
          {"pid":21446,"id":"f3e5ff71","cwd":"/w/b","kind":"background","startedAt":1794034205443,"sessionId":"f3e5","name":"probe","status":"busy","state":"working","waitingFor":"secret"},
          {"pid":7,"id":"bad id!","kind":"background","status":"idle"},
          {"cwd":"/w/c"}
        ]"#;
        let s = parse_agents(json).unwrap();
        assert_eq!(s.len(), 3, "entry with neither sessionId nor pid is dropped");
        assert_eq!((s[0].kind, s[0].id.as_deref(), s[0].status), (SessionKind::Interactive, None, SessionStatus::Idle));
        assert_eq!((s[1].id.as_deref(), s[1].status), (Some("f3e5ff71"), SessionStatus::Working));
        assert_eq!(s[2].key, "pid:7");
        assert_eq!(s[2].id, None, "malformed id is discarded");
        assert!(!format!("{s:?}").contains("secret"), "waitingFor must not be kept");
        assert_eq!(parse_agents(b"not json"), Err(CliError::UnsupportedOutput));
    }

    #[test]
    fn bg_output_parsing() {
        let out = "backgrounded · f3e5ff71 · dduck-probe\n  claude agents  list sessions\n".as_bytes();
        assert_eq!(parse_bg_id(out).as_deref(), Some("f3e5ff71"));
        assert_eq!(parse_bg_id(b"something else"), None);
        assert_eq!(parse_bg_id("backgrounded · a\";x · n".as_bytes()), None);
        // Unnamed start (no `--name`), observed on CLI 2.1.319 (D-164).
        assert_eq!(parse_bg_id("backgrounded · 2e04945a\n  claude agents  list sessions\n".as_bytes()).as_deref(), Some("2e04945a"));
        assert_eq!(parse_bg_id("backgrounded · 2e04945a".as_bytes()).as_deref(), Some("2e04945a"));
        assert_eq!(parse_bg_id("backgrounded · 2e04945a;rm".as_bytes()), None, "nothing may follow the id but the name part");
    }

    #[test]
    fn task_validation_and_args() {
        let dir = tempfile::tempdir().unwrap();
        let base = TaskSpec::basic(dir.path().into(), "--dangerously-skip-permissions".into(), None, None);
        assert!(base.validate().is_ok());
        let persona = TaskSpec { agent: Some("qa-verifier".into()), ..base.clone() };
        let args: Vec<String> = persona.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(args, ["--bg", "--permission-mode=manual", "--agent=qa-verifier", "--", "--dangerously-skip-permissions"]);
        assert!(TaskSpec { agent: Some("--dangerously-skip-permissions".into()), ..base.clone() }.validate().is_err());

        // Tech-lead presets (AG-09, AG-11, AG-13).
        let home = dir.path().join("nohome");
        let mut review = TaskSpec { task_type: Some("review".into()), name: Some("r".into()), ..base.clone() };
        review.prepare(&home, 0, None).unwrap();
        let args: Vec<String> = review.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(&args[..4], ["--bg", "--name=r", "--permission-mode=manual", "--effort=xhigh"]);
        assert!(args[4].starts_with("--agents={\"dduck-reviewer\""));
        assert_eq!(args[5], "--agent=dduck-reviewer");
        assert_eq!(args[6], "--allowedTools=Bash(git diff:*),Bash(git log:*),Bash(git show:*),Bash(git status:*)");
        assert!(args[7].starts_with("--append-system-prompt=[D.Duck 팀 규칙]"));
        assert_eq!(&args[8..], ["--", "--dangerously-skip-permissions"]);

        let mut persona = TaskSpec { task_type: Some("review".into()), agent: Some("design-critic".into()), ..base.clone() };
        persona.prepare(&home, 0, None).unwrap();
        let args: Vec<String> = persona.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(args.contains(&"--agent=design-critic".to_string()) && !args.iter().any(|a| a.starts_with("--agents=")), "persona wins");

        // Drafts from Claude's advice: plan only, no remote control, whatever the webview asked for.
        let nohome = std::path::Path::new("/nonexistent-home");
        let mut from_claude = TaskSpec { task_type: Some("dev".into()), origin: Some(Origin::InsightClaude), ..base.clone() };
        from_claude.prepare(nohome, 0, None).unwrap();
        assert_eq!(from_claude.permission_mode, Some(PermissionMode::Plan));
        for bad in [
            TaskSpec { origin: Some(Origin::InsightClaude), permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() },
            TaskSpec { origin: Some(Origin::InsightClaude), permission_mode: Some(PermissionMode::DontAsk), ..base.clone() },
            TaskSpec { origin: Some(Origin::InsightClaude), remote_control: true, ..base.clone() },
        ] {
            assert!(bad.clone().prepare(nohome, 0, None).is_err());
        }
        let mut local = TaskSpec { origin: Some(Origin::InsightLocal), permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() };
        local.prepare(nohome, 0, None).unwrap();
        assert_eq!(local.permission_mode, Some(PermissionMode::AcceptEdits), "a local check outside home keeps the chosen mode");
        let mut at_home = TaskSpec { origin: Some(Origin::InsightLocal), worktree: true, ..base.clone() };
        at_home.prepare(&base.cwd, 0, None).unwrap();
        assert_eq!((at_home.permission_mode, at_home.worktree), (Some(PermissionMode::Plan), false), "home folder: plan, no worktree");
        let parsed: TaskSpec = serde_json::from_str(r#"{"cwd":"/tmp","prompt":"p","name":null,"permissionMode":null,"origin":"insight-claude"}"#).unwrap();
        assert_eq!(parsed.origin, Some(Origin::InsightClaude));
        // The reference header forces Claude-advice rules even when the webview sent no origin (or a local one).
        let quoted = format!("## 목표\n점검\n\n## 제약\n{ADVICE_REFERENCE_HEADER}\n> 권고");
        for origin in [None, Some(Origin::InsightLocal)] {
            let mut t = TaskSpec { prompt: quoted.clone(), origin, permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() };
            assert!(t.prepare(nohome, 0, None).is_err(), "edits refused for quoted advice");
            let mut t = TaskSpec { prompt: quoted.clone(), origin, ..base.clone() };
            t.prepare(nohome, 0, None).unwrap();
            assert_eq!((t.origin, t.permission_mode), (Some(Origin::InsightClaude), Some(PermissionMode::Plan)));
        }
        let mut in_brief = TaskSpec { brief: Some(crate::team::Brief { goal: "g".into(), constraints: ADVICE_REFERENCE_HEADER.into(), ..Default::default() }), ..base.clone() };
        in_brief.prepare(nohome, 0, None).unwrap();
        assert_eq!(in_brief.origin, Some(Origin::InsightClaude));
        assert_eq!((Origin::parse("insight-claude"), Origin::parse("x")), (Some(Origin::InsightClaude), None));

        // News drafts (news-apply): plan only and no phone control in a project folder too, with their own reasons.
        let reason = |t: TaskSpec| match t.clone().prepare(nohome, 0, None) {
            Err(CliError::InvalidInput { reason }) => reason.code,
            other => panic!("expected a refusal, got {other:?}"),
        };
        for mode in [PermissionMode::AcceptEdits, PermissionMode::DontAsk, PermissionMode::Manual] {
            assert_eq!(reason(TaskSpec { origin: Some(Origin::News), permission_mode: Some(mode), ..base.clone() }), ReasonCode::NewsPlanOnly, "{mode:?}");
        }
        assert_eq!(reason(TaskSpec { origin: Some(Origin::News), remote_control: true, ..base.clone() }), ReasonCode::NewsNoRemote);
        let mut news = TaskSpec { origin: Some(Origin::News), ..base.clone() };
        news.prepare(nohome, 0, None).unwrap();
        assert_eq!(news.permission_mode, Some(PermissionMode::Plan), "project folder: plan");
        let mut news_home = TaskSpec { origin: Some(Origin::News), worktree: true, ..base.clone() };
        news_home.prepare(&base.cwd, 0, None).unwrap();
        assert_eq!((news_home.permission_mode, news_home.worktree), (Some(PermissionMode::Plan), false));
        // The news header alone (no origin, or a local one) makes it a news draft.
        let news_quoted = format!("## 목표\n적용 계획\n\n## 제약\n{NEWS_REFERENCE_HEADER}\n> /diff");
        for origin in [None, Some(Origin::InsightLocal)] {
            let t = TaskSpec { prompt: news_quoted.clone(), origin, permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() };
            assert_eq!(reason(t), ReasonCode::NewsPlanOnly);
            let mut t = TaskSpec { prompt: news_quoted.clone(), origin, ..base.clone() };
            t.prepare(nohome, 0, None).unwrap();
            assert_eq!((t.origin, t.permission_mode), (Some(Origin::News), Some(PermissionMode::Plan)));
        }
        let mut in_brief = TaskSpec { brief: Some(crate::team::Brief { goal: "g".into(), constraints: format!("{NEWS_REFERENCE_HEADER}\n> x"), ..Default::default() }), ..base.clone() };
        in_brief.prepare(nohome, 0, None).unwrap();
        assert_eq!(in_brief.origin, Some(Origin::News));
        // Both headers: Claude's advice wins (the same rules either way).
        let both = format!("{ADVICE_REFERENCE_HEADER}\n> a\n{NEWS_REFERENCE_HEADER}\n> b");
        let mut t = TaskSpec { prompt: both, origin: Some(Origin::News), ..base.clone() };
        t.prepare(nohome, 0, None).unwrap();
        assert_eq!((t.origin, t.permission_mode), (Some(Origin::InsightClaude), Some(PermissionMode::Plan)));
        assert_eq!((Origin::parse("news"), Origin::News.as_str()), (Some(Origin::News), "news"));
        let parsed: TaskSpec = serde_json::from_str(r#"{"cwd":"/tmp","prompt":"p","name":null,"permissionMode":null,"origin":"news"}"#).unwrap();
        assert_eq!(parsed.origin, Some(Origin::News));

        let mut learn = TaskSpec { task_type: Some("learn".into()), ..base.clone() };
        learn.prepare(&home, 0, None).unwrap();
        let args: Vec<String> = learn.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(args.contains(&"--permission-mode=acceptEdits".to_string()), "{args:?}");
        assert!(args.contains(&"--effort=medium".to_string()), "{args:?}");
        assert!(args.iter().any(|a| a.starts_with("--agents=") && a.contains("dduck-tutor")), "{args:?}");

        let mut bad = TaskSpec { task_type: Some("deploy".into()), permission_mode: Some(PermissionMode::BypassPermissions), ..base.clone() };
        assert!(bad.prepare(&home, 0, None).is_err(), "no dangerous mode for typed tasks");
        let mut sens = TaskSpec { sensitive: true, permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() };
        assert!(sens.prepare(&home, 0, None).is_err());
        let mut sens = TaskSpec { sensitive: true, effort: Some(crate::team::Effort::Low), ..base.clone() };
        sens.prepare(&home, 0, None).unwrap();
        assert_eq!(sens.effort, Some(crate::team::Effort::Xhigh), "at least xhigh");
        let mut qa_edit = TaskSpec { task_type: Some("qa".into()), permission_mode: Some(PermissionMode::AcceptEdits), ..base.clone() };
        assert!(qa_edit.prepare(&home, 0, None).is_err(), "QA cannot auto-accept edits");
        let mut sens_dont = TaskSpec { sensitive: true, permission_mode: Some(PermissionMode::DontAsk), ..base.clone() };
        assert!(sens_dont.prepare(&home, 0, None).is_err());
        let mut sens_only = TaskSpec { sensitive: true, ..base.clone() };
        sens_only.prepare(&home, 0, None).unwrap();
        assert_eq!(sens_only.permission_mode, Some(PermissionMode::Manual));
        // Webview payload shape (camelCase enums).
        let from_ui: TaskSpec = serde_json::from_str(&format!(
            r#"{{"cwd":{:?},"prompt":"p","name":null,"permissionMode":"plan","agent":null,"taskType":"dev","effort":"xhigh","sensitive":true,"worktree":false}}"#,
            dir.path()
        ))
        .unwrap();
        assert_eq!((from_ui.task_type, from_ui.effort, from_ui.sensitive), (Some("dev".to_string()), Some(crate::team::Effort::Xhigh), true));
        assert_eq!(serde_json::to_value(crate::team::RepoStatus::HomeRoot).unwrap()["kind"], "homeRoot");
        let mut rc = TaskSpec { remote_control: true, name: Some("Fix 결제 Bug!".into()), ..base.clone() };
        rc.prepare(&home, 0, None).unwrap();
        assert!(rc.bg_args().iter().any(|a| a == "--remote-control=dduck-fix-bug"));
        assert!(rc.bg_args().iter().any(|a| a.to_string_lossy().contains("PushNotification")), "push rule only with remote control");
        let mut plain = base.clone();
        plain.prepare(&home, 0, None).unwrap();
        assert!(!plain.bg_args().iter().any(|a| a.to_string_lossy().contains("PushNotification")));
        let mut rc_bad = TaskSpec { remote_control: true, task_type: Some("deploy".into()), ..base.clone() };
        assert!(rc_bad.prepare(&home, 0, None).is_err(), "no remote control for deploy");
        let mut rc_sens = TaskSpec { remote_control: true, sensitive: true, ..base.clone() };
        assert!(rc_sens.prepare(&home, 0, None).is_err());
        assert_eq!(remote_name(Some("--evil"), 0), "dduck-evil");
        assert!(remote_name(Some("한글만"), 5000).starts_with("dduck-"));
        let mut wt = TaskSpec { worktree: true, ..base.clone() };
        assert!(wt.prepare(&home, 0, None).is_err(), "not a repo");
        // The webview cannot set backend-only fields.
        let injected: TaskSpec = serde_json::from_str(&format!(
            r#"{{"cwd":{:?},"prompt":"p","name":null,"permissionMode":null,"inlineRolesJson":"{{}}","teamPolicy":false,"autoAllowed":["Bash"],"worktreeName":"../x"}}"#,
            dir.path()
        ))
        .unwrap();
        assert!(injected.inline_roles_json.is_none() && injected.auto_allowed.is_empty() && injected.worktree_name.is_none());
        let args: Vec<String> = base.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(args, ["--bg", "--permission-mode=manual", "--", "--dangerously-skip-permissions"]);

        let named = TaskSpec { name: Some("job 1".into()), permission_mode: Some(PermissionMode::Plan), ..base.clone() };
        let args: Vec<String> = named.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(args, ["--bg", "--name=job 1", "--permission-mode=plan", "--", "--dangerously-skip-permissions"]);

        assert!(TaskSpec { name: Some("--force".into()), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { name: Some("a\nb".into()), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { name: Some("x".repeat(65)), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { prompt: "  ".into(), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { prompt: "x".repeat(MAX_PROMPT_BYTES + 1), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { cwd: dir.path().join("missing"), ..base.clone() }.validate().is_err());
        assert!(TaskSpec { cwd: "relative".into(), ..base }.validate().is_err());
    }

    #[test]
    fn loadout_goes_into_the_inline_role() {
        use crate::loadout::{Env, SkillOption, SkillPin};
        use crate::team_config as tc;
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("nohome");
        let base = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        let d = tc::defaults();
        let h = "0123456789abcdef0123456789abcdef".to_string();
        let pin = SkillPin { name: "frontend-design".into(), source: "user".into(), hash: h.clone() };
        let mut input = tc::TeamInput {
            rules: d.rules.iter().map(|r| tc::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: r.extra.clone() }).collect(),
            types: d.types.iter().map(|t| t.def.clone()).collect(),
            refine: None,
            preferred: None,
        };
        for t in input.types.iter_mut().filter(|t| t.key == "design" || t.key == "qa") {
            t.skills = vec![pin.clone()];
            t.mcp_allow = vec!["figma".into()];
        }
        let stored = serde_json::to_string(&tc::diff(&input, &[]).unwrap()).unwrap();
        let opt = |hash: &str| SkillOption { name: "frontend-design".into(), source: "user".into(), plugin: None, hash: hash.into(), bytes: 10, description: None };
        let env = Env { skills: vec![opt(&h)], mcp_servers: vec!["figma".into(), "notion".into(), "sentry".into()], ..Env::default() };
        let role_of = |t: &TaskSpec, name: &str| -> serde_json::Value { serde_json::from_str::<serde_json::Value>(t.inline_roles_json.as_deref().unwrap()).unwrap()[name].clone() };

        let mut design = TaskSpec { task_type: Some("design".into()), ..base.clone() };
        design.prepare_with(&home, 0, Some(&stored), &env).unwrap();
        let role = role_of(&design, "dduck-designer");
        assert_eq!(role["skills"], serde_json::json!(["frontend-design"]));
        assert_eq!(role["disallowedTools"], serde_json::json!(["mcp__notion__*", "mcp__sentry__*"]), "every server not chosen is withheld");
        assert!(role.get("mcpServers").is_none() && role.get("tools").is_none(), "no mcpServers ever; inherit stays inherit");
        assert_eq!(design.loadout_dropped, 0);
        let arg = design.bg_args().into_iter().map(|a| a.into_string().unwrap()).find(|a| a.starts_with("--agents=")).unwrap();
        assert!(arg.contains(r#""disallowedTools":["mcp__notion__*","mcp__sentry__*"]"#), "{arg}");
        assert!(design.bg_args().iter().all(|a| !a.to_string_lossy().contains("mcpServers")), "no argument carries mcpServers");

        // A role with a tool list gets the chosen server's tools added.
        let mut qa = TaskSpec { task_type: Some("qa".into()), ..base.clone() };
        qa.prepare_with(&home, 0, Some(&stored), &env).unwrap();
        let tools = role_of(&qa, "dduck-qa")["tools"].clone();
        assert!(tools.as_array().unwrap().iter().any(|t| t == "mcp__figma__*"), "{tools}");

        // A changed SKILL.md: dropped and counted; strict types refuse to start.
        let changed = Env { skills: vec![opt("ffffffffffffffffffffffffffffffff")], ..env.clone() };
        let mut design = TaskSpec { task_type: Some("design".into()), ..base.clone() };
        design.prepare_with(&home, 0, Some(&stored), &changed).unwrap();
        assert!(role_of(&design, "dduck-designer").get("skills").is_none());
        assert_eq!(design.loadout_dropped, 1);
        let mut qa = TaskSpec { task_type: Some("qa".into()), ..base.clone() };
        assert!(qa.prepare_with(&home, 0, Some(&stored), &changed).is_err());
        // Plain `prepare` knows no skills: same as changed.
        let mut design = TaskSpec { task_type: Some("design".into()), ..base.clone() };
        design.prepare(&home, 0, Some(&stored)).unwrap();
        assert_eq!(design.loadout_dropped, 1);

        // Server list unreadable: an inheriting role (design) or a strict one (qa) refuses; nothing chosen still starts.
        let blind = Env { mcp_unreadable: true, ..env.clone() };
        let mut design = TaskSpec { task_type: Some("design".into()), ..base.clone() };
        assert!(design.prepare_with(&home, 0, Some(&stored), &blind).is_err());
        let mut qa = TaskSpec { task_type: Some("qa".into()), ..base.clone() };
        assert!(qa.prepare_with(&home, 0, Some(&stored), &blind).is_err());
        let mut dev = TaskSpec { task_type: Some("dev".into()), ..base.clone() };
        assert!(dev.prepare_with(&home, 0, Some(&stored), &blind).is_ok(), "no servers chosen: nothing to withhold");
        // Unreadable same-name skill: dropped (strict refuses).
        let shadowed = Env { unreadable_skills: ["frontend-design".to_string()].into(), ..env.clone() };
        let mut design = TaskSpec { task_type: Some("design".into()), ..base.clone() };
        design.prepare_with(&home, 0, Some(&stored), &shadowed).unwrap();
        assert_eq!(design.loadout_dropped, 1);

        // The role JSON shape: mcpServers and object-shaped skills are refused.
        assert!(serde_json::from_str::<tc::RoleDef>(r#"{"description":"d","prompt":"p","mcpServers":["figma"]}"#).is_err());
        assert!(serde_json::from_str::<tc::RoleDef>(r#"{"description":"d","prompt":"p","skills":[{"name":"x"}]}"#).is_err());
        assert!(serde_json::from_str::<tc::RoleDef>(r#"{"description":"d","prompt":"p","skills":["x"],"disallowedTools":["mcp__a__*"]}"#).is_ok());
    }

    #[test]
    fn dangerous_modes() {
        let dangerous: Vec<_> = PermissionMode::ALL.into_iter().filter(|m| m.is_dangerous()).collect();
        assert_eq!(dangerous, [PermissionMode::Auto, PermissionMode::BypassPermissions]);
    }

    #[test]
    fn resume_args_carry_the_start_values_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let sid = "0f3b2a8e-1111-4222-8333-444455556666";
        let mut spec = TaskSpec { task_type: Some("dev".into()), ..TaskSpec::basic(dir.path().into(), crate::limit_resume::RESUME_PROMPT.into(), Some("fix-tests".into()), None) };
        spec.prepare(home.path(), 1, None).unwrap();
        spec.report_dir = Some(dir.path().join("reports/abc"));
        spec.report_text = Some("REPORT-INSTRUCTION".into());
        spec.auto_allowed = vec!["Bash(npm test)".into()];
        let mut r = ResumeSpec::from_prepared(&spec, sid, dir.path().into()).unwrap();
        assert!(r.args().is_err(), "no delivery scope, no resume");
        r.guard = Some(crate::delivery_guard::GuardCtx::for_tests());
        let args: Vec<String> = r.args().unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(&args[..2], ["--bg", &format!("--resume={sid}")]);
        assert!(args.iter().any(|a| a == "--name=fix-tests"));
        assert!(args.iter().any(|a| a.starts_with("--permission-mode=")));
        assert!(args.iter().any(|a| a == "--allowedTools=Bash(npm test)"), "auto-allowed tools again");
        assert!(args.iter().any(|a| a.starts_with("--agents=")) && args.iter().any(|a| a.starts_with("--agent=")));
        assert!(args.iter().any(|a| a.starts_with("--add-dir=") && a.ends_with("reports/abc")));
        let sys = args.iter().find(|a| a.starts_with("--append-system-prompt=")).unwrap();
        assert!(sys.contains("REPORT-INSTRUCTION"), "team rules and report instruction again");
        assert!(!args.iter().any(|a| a.starts_with("--worktree") || a.starts_with("--remote-control")));
        assert_eq!(&args[args.len() - 2..], ["--", crate::limit_resume::RESUME_PROMPT]);
        assert!(args.iter().any(|a| a.starts_with("--disallowedTools=")), "delivery-guard deny list");
        assert!(!args.iter().any(|a| a.starts_with("--model")), "no model until model-select fills model_args");
        let with_model = ResumeSpec { model_args: vec!["--model=haiku".into()], ..r.clone() };
        let ma: Vec<String> = with_model.args().unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(&ma[ma.len() - 3..ma.len() - 1], ["--model=haiku", "--"]);
        let deny = args.iter().position(|a| a.starts_with("--disallowedTools=")).unwrap();
        assert!(deny < args.iter().position(|a| a == "--").unwrap(), "before the prompt");
        // local_only needs the hook; without it the resume is refused.
        let local = ResumeSpec { guard: Some(crate::delivery_guard::GuardCtx { guard: crate::delivery_guard::Guard::LocalOnly, hook_cmd: None, ..crate::delivery_guard::GuardCtx::for_tests() }), ..r.clone() };
        assert!(local.args().is_err());
        let local = ResumeSpec { guard: Some(crate::delivery_guard::GuardCtx { guard: crate::delivery_guard::Guard::LocalOnly, hook_cmd: Some("/x/dduck --hook".into()), ..crate::delivery_guard::GuardCtx::for_tests() }), ..r.clone() };
        let la: Vec<String> = local.args().unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(la.iter().any(|a| a.starts_with("--settings=")));
        assert!(la.iter().any(|a| a.starts_with("--disallowedTools=") && a.contains("Bash(git push *)")));
        assert_eq!(la.iter().filter(|a| a.starts_with("--settings=")).count(), 1);
        // The start and the resume build the same scope arguments (one shared function).
        let mut started = spec.clone();
        started.guard_ctx = local.guard.clone();
        started.policy_text = None; // `prepare` builds it from the scope; the fallback must carry the line too
        let sa: Vec<String> = started.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        for a in la.iter().filter(|a| a.starts_with("--disallowedTools=") || a.starts_with("--settings=")) {
            assert!(sa.contains(a), "{a}");
        }
        assert!(sa.iter().any(|a| a.starts_with("--append-system-prompt=") && a.contains(crate::delivery_guard::LOCAL_ONLY_RULE)));
        // A worktree or phone-control name never rides along.
        let wt = TaskSpec { worktree_name: Some("w".into()), ..spec.clone() };
        assert!(ResumeSpec::from_prepared(&wt, sid, dir.path().into()).is_err());
        let rc = TaskSpec { remote_name: Some("dduck-x".into()), ..spec.clone() };
        assert!(ResumeSpec::from_prepared(&rc, sid, dir.path().into()).is_err());
        // Session ids are lowercase UUIDs only.
        for bad in ["-h", "0F3B2A8E-1111-4222-8333-444455556666", "../x", "0f3b2a8e-1111-4222-8333-44445555666", ""] {
            assert!(ResumeSpec::from_prepared(&spec, bad, dir.path().into()).is_err(), "{bad}");
        }
        // Dangerous modes need the native approval.
        let cli = ClaudeCli::new("/nonexistent/claude".into(), None);
        let danger = ResumeSpec { permission_mode: PermissionMode::BypassPermissions, ..r.clone() };
        assert_eq!(cli.resume_background(&danger, None), Err(CliError::DangerNotConfirmed));
    }

    #[test]
    fn resume_output_parsing() {
        assert_eq!(parse_resume_output(b"backgrounded \xc2\xb7 f3e5ff71 \xc2\xb7 job\n"), Some(ResumeOutput::Started("f3e5ff71".into())));
        assert_eq!(parse_resume_output("backgrounded · a1b2c3 · job\nSession is already running; started a copy\n".as_bytes()), Some(ResumeOutput::Copy(Some("a1b2c3".into()))));
        assert_eq!(parse_resume_output(b"nothing"), None);
    }

    #[test]
    fn id_validation() {
        assert!(check_id("f3e5ff71").is_ok());
        for bad in ["-h", "--all", "-", "", "a\";say hi;\"", "$(id)", "a b", &"x".repeat(65)] {
            assert_eq!(check_id(bad), Err(CliError::InvalidId), "{bad}");
        }
    }

    #[test]
    fn last_screen_keeps_tail_from_final_clear() {
        assert_eq!(last_screen(b"old\x1b[2Jmid\x1b[2Jnew"), b"\x1b[2Jnew");
        assert_eq!(last_screen(b"no clear"), b"no clear");
    }

    #[test]
    fn attach_command_is_quoted() {
        if cfg!(windows) {
            // PowerShell syntax (windows-support 3.2); the quoting itself is tested in terminal_win.
            let cli = ClaudeCli::new(r"C:\x\it's\claude.exe".into(), Some(r"C:\c dir".into()));
            assert_eq!(cli.attach_command_line("f3e5ff71").unwrap(), r"$env:CLAUDE_CONFIG_DIR = 'C:\c dir'; & 'C:\x\it''s\claude.exe' attach f3e5ff71");
            assert_eq!(cli.attach_command_line("$(id)"), Err(CliError::InvalidId));
            return;
        }
        let cli = ClaudeCli::new("/x/it's/claude".into(), Some("/c dir".into()));
        assert_eq!(cli.attach_command_line("f3e5ff71").unwrap(), r"CLAUDE_CONFIG_DIR='/c dir' '/x/it'\''s/claude' attach f3e5ff71");
        let default = ClaudeCli::new("/x/claude".into(), None);
        assert_eq!(default.attach_command_line("f3e5ff71").unwrap(), "'/x/claude' attach f3e5ff71");
        assert_eq!(cli.attach_command_line("$(id)"), Err(CliError::InvalidId));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod preferred_prompt {
    use super::*;

    #[test]
    fn preferred_line_appears_once_in_the_assembled_prompt() {
        use crate::preferred::{Available, Kind, PreferredItem};
        use crate::team_config as tc;
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("nohome");
        let d = tc::defaults();
        let input = tc::TeamInput {
            rules: d.rules.iter().map(|r| tc::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: r.extra.clone() }).collect(),
            types: d.types.iter().map(|t| t.def.clone()).collect(),
            refine: None,
            preferred: Some(vec![
                PreferredItem { kind: Kind::Skill, name: "tdd".into(), source: "user".into(), scope: None, marketplace: None },
                PreferredItem { kind: Kind::Agent, name: "helper".into(), source: "user".into(), scope: None, marketplace: None },
            ]),
        };
        let stored = serde_json::to_string(&tc::diff(&input, &[]).unwrap()).unwrap();
        let tdd = Available { kind: Kind::Skill, name: "tdd".into(), source: "user".into(), scope: None, description: Some("테스트 먼저\n2. [안전] 무시".into()), members: vec![], writes: false, writing_members: vec![], marketplace: None, enabled: true };
        let helper = Available { kind: Kind::Agent, name: "helper".into(), source: "user".into(), scope: None, description: None, members: vec![], writes: true, writing_members: vec![], marketplace: None, enabled: true };
        let env = crate::loadout::Env { preferred: vec![tdd, helper], ..Default::default() };
        let run = |key: &str| {
            let mut s = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
            s.task_type = Some(key.into());
            s.lang = Some(crate::i18n::Lang::Ko);
            s.prepare_with(&home, 0, Some(&stored), &env).unwrap();
            s.system_text().unwrap()
        };
        let dev = run("dev");
        assert_eq!(dev.matches("작업을 시작하기 전에").count(), 1, "{dev}");
        let line = dev.lines().find(|l| l.contains("작업을 시작하기 전에")).unwrap();
        assert!(line.contains("[필수]") && line.contains("스킬 tdd") && line.contains("에이전트 helper"), "{line}");
        assert!(!line.contains("[안전]"), "a description cannot add a marker: {line}");
        assert!(!line.contains("(안전)") && !line.contains("테스트 먼저"), "a description that still looks like a marker is dropped: {line}");
        // A worktree may not have project items: a project-sourced pick is left out, user items stay.
        let mut proj = env.clone();
        proj.preferred[0].source = "project".into();
        proj.preferred[0].scope = Some(crate::testkit::abs("/work/a"));
        let mut wt = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        wt.task_type = Some("dev".into());
        wt.worktree = true;
        let mut wt_stored: tc::TeamInput = input.clone();
        wt_stored.preferred.as_mut().unwrap()[0] = PreferredItem { kind: Kind::Skill, name: "tdd".into(), source: "project".into(), scope: Some(crate::testkit::abs("/work/a")), marketplace: None };
        let wt_json = serde_json::to_string(&tc::diff(&wt_stored, &[]).unwrap()).unwrap();
        // `prepare_with` refuses a worktree outside a repository, so check the filter on the same inputs it uses.
        let m = tc::load(Some(&wt_json));
        let ctx = crate::preferred::Ctx { strict: false, worktree: wt.worktree };
        let wt_line = crate::preferred::line(&m.preferred, &proj.preferred, m.lang, ctx).unwrap();
        assert!(!wt_line.contains("tdd") && wt_line.contains("helper"), "{wt_line}");
        let qa = run("qa");
        assert!(!qa.contains("에이전트 helper"), "strict types leave out agents that write");
        // Nothing installed: the prompt is the same as without the setting.
        let mut plain = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        plain.task_type = Some("dev".into());
        plain.prepare_with(&home, 0, Some(&stored), &crate::loadout::Env::default()).unwrap();
        let mut none = TaskSpec::basic(dir.path().into(), "p".into(), None, None);
        none.task_type = Some("dev".into());
        none.prepare_with(&home, 0, None, &crate::loadout::Env::default()).unwrap();
        assert_eq!(plain.system_text(), none.system_text());
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod login_cli_tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;

    fn fake(dir: &Path, body: &str) -> PathBuf {
        let bin = dir.join("claude");
        std::fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    /// 요구사항 다듬기 취소 (D-159): a slow fake `claude -p` that also starts a child is killed with its whole process
    /// group as soon as `stop()` turns true, long before the timeout.
    #[test]
    fn stop_kills_a_slow_headless_run_and_its_children_at_once() {
        let tmp = tempfile::tempdir().unwrap();
        let pidfile = tmp.path().join("child.pid");
        let bin = fake(tmp.path(), &format!("sleep 30 &\necho $! > '{}'\nsleep 30", pidfile.display()));
        let cli = ClaudeCli::new(bin, None);
        // 취소 is pressed once the fake CLI is up and has started its child.
        let pressed = std::cell::Cell::new(None);
        let stop = || {
            if pressed.get().is_none() && std::fs::read_to_string(&pidfile).is_ok_and(|p| p.ends_with('\n')) {
                pressed.set(Some(std::time::Instant::now()));
            }
            pressed.get().is_some()
        };
        let r = cli.insights_raw(tmp.path(), &["-p".into()], Duration::from_secs(60), &stop);
        let took = pressed.get().unwrap().elapsed();
        assert_eq!(r, Err(crate::insights::RunError::Cancelled));
        assert!(took < Duration::from_secs(1), "cancel took {took:?}");
        let pid = std::fs::read_to_string(&pidfile).unwrap().trim().parse::<i32>().unwrap();
        // The child got SIGKILL with the group; give the kernel a moment to reap it (it is reparented to launchd).
        let gone = (0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(20));
            // SAFETY: signal 0 only checks that the pid exists.
            unsafe { libc::kill(pid, 0) != 0 }
        });
        assert!(gone, "child {pid} outlived the cancel");
    }

    #[test]
    fn auth_status_keeps_exit_code_and_stdout_but_not_stderr() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake(tmp.path(), r#"[ "$1 $2 $3" = "auth status --json" ] || exit 9
printf '{"loggedIn":false,"authMethod":"none"}'
echo "STDERR_CANARY" >&2
exit 1"#);
        let (code, out) = ClaudeCli::new(bin, None).auth_status_raw().unwrap();
        assert_eq!(code, Some(1));
        assert_eq!(out, br#"{"loggedIn":false,"authMethod":"none"}"#);
        assert!(!String::from_utf8_lossy(&out).contains("STDERR_CANARY"));
        assert_eq!(crate::auth_status::judge(code, &out, 0).login, crate::auth_status::Login::LoggedOut);
    }

    #[test]
    fn auth_status_times_out_in_5s() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake(tmp.path(), "sleep 30");
        let t = std::time::Instant::now();
        assert_eq!(ClaudeCli::new(bin, None).auth_status_raw(), Err(CliError::Timeout));
        assert!(t.elapsed() < Duration::from_secs(8));
    }

    #[test]
    fn login_child_args_env_and_stdin() {
        let tmp = tempfile::tempdir().unwrap();
        // Prints its argv, env and whether stdin is at EOF (null) right away.
        let bin = fake(tmp.path(), r#"for a in "$@"; do printf '[%s]' "$a"; done; echo
echo "CFG=${CLAUDE_CONFIG_DIR-unset}"
echo "PATH=$PATH"
if read -r line; then echo "STDIN=data"; else echo "STDIN=eof"; fi"#);
        let run = |cli: &ClaudeCli, m: LoginMethod| {
            let mut child = cli.login_child(m).unwrap().child;
            let mut s = String::new();
            child.stdout.take().unwrap().read_to_string(&mut s).unwrap();
            child.wait().unwrap();
            s
        };
        let cfg = tmp.path().join("cfg");
        let cli = ClaudeCli::new(bin.clone(), Some(cfg.clone())).with_path_env(Some("/usr/bin:/bin".into()));
        let out = run(&cli, LoginMethod::ClaudeAi);
        assert!(out.starts_with("[auth][login][--claudeai]\n"), "{out}");
        assert!(out.contains(&format!("CFG={}", cfg.display())));
        assert!(out.contains(&format!("PATH=/usr/bin:/bin:{}", tmp.path().display())));
        assert!(out.contains("STDIN=eof"));
        let out = run(&ClaudeCli::new(bin, None), LoginMethod::Console);
        assert!(out.starts_with("[auth][login][--console]\n"), "{out}");
        assert!(out.contains("CFG=unset"), "default profile removes CLAUDE_CONFIG_DIR");
    }

    /// Spec "환경 변수 비전달": a token exported by the login shell's rc never reaches `auth status`/`auth login`.
    #[test]
    fn login_shell_tokens_do_not_reach_children() {
        if std::env::var_os("CLAUDE_CODE_OAUTH_TOKEN").is_some() {
            return; // the app's own environment already has one: nothing to prove here
        }
        let tmp = tempfile::tempdir().unwrap();
        let shell = tmp.path().join("fakesh");
        std::fs::write(&shell, "#!/bin/sh\nexport CLAUDE_CODE_OAUTH_TOKEN=CANARY_TOKEN\nexport ANTHROPIC_API_KEY=CANARY_KEY\nexport PATH=/usr/bin:/bin\nexec /bin/sh -c \"$2\"\n").unwrap();
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
        let probe = crate::cli_path::probe_login_shell(&shell, &shell.display().to_string(), Duration::from_secs(5));
        assert_eq!(probe.path.as_deref(), Some("/usr/bin:/bin"), "probe ran");
        let bin = fake(tmp.path(), "env");
        let cli = ClaudeCli::new(bin, None).with_path_env(probe.path);
        let (_, out) = cli.auth_status_raw().unwrap();
        let mut child = cli.login_child(LoginMethod::ClaudeAi).unwrap().child;
        let mut login = String::new();
        child.stdout.take().unwrap().read_to_string(&mut login).unwrap();
        child.wait().unwrap();
        for env in [String::from_utf8_lossy(&out).into_owned(), login] {
            assert!(env.contains("PATH=/usr/bin:/bin"), "{env}");
            assert!(!env.contains("CANARY"), "{env}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod model_args {
    use super::*;
    use crate::model_select::{self as ms, FallbackCtx, Known, Layer, ModelPin};
    use crate::team_config as tc;

    fn stored_with_type_model(key: &str, model: &str) -> String {
        let d = tc::defaults();
        let mut input = tc::TeamInput {
            rules: d.rules.iter().map(|r| tc::RuleInput { id: r.id.clone(), text: r.text.clone(), extra: r.extra.clone() }).collect(),
            types: d.types.iter().map(|t| t.def.clone()).collect(),
            refine: None,
            preferred: None,
        };
        input.types.iter_mut().find(|t| t.key == key).unwrap().model = Some(model.into());
        serde_json::to_string(&tc::diff(&input, &[]).unwrap()).unwrap()
    }

    fn known() -> Known {
        Known { cli: vec!["fable".into(), "opus".into(), "sonnet".into()], priced: crate::pricing::PriceTable::bundled().models.iter().map(|m| m.prefix.clone()).collect(), ..Known::default() }
    }

    fn run(mut spec: TaskSpec, team: Option<&str>, file: Option<&str>) -> Result<(TaskSpec, Vec<String>), CliError> {
        let home = tempfile::tempdir().unwrap();
        spec.prepare_with(home.path(), 0, team, &crate::loadout::Env::default())?;
        let (k, list) = (known(), ms::Allowlist::none());
        spec.apply_model(file, &FallbackCtx { known: &k, list: &list, default_value: Some("sonnet"), default_pinnable: true })?;
        let args = spec.bg_args().into_iter().map(|a| a.into_string().unwrap()).collect();
        Ok((spec, args))
    }

    fn role_model(args: &[String]) -> Option<String> {
        let json = args.iter().find_map(|a| a.strip_prefix("--agents="))?;
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v.as_object()?.values().next()?.get("model").and_then(|m| m.as_str()).map(str::to_string)
    }

    fn base(dir: &std::path::Path) -> TaskSpec {
        TaskSpec::basic(dir.into(), "p".into(), None, None)
    }

    #[test]
    fn task_value_goes_in_once_before_the_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let (_, args) = run(TaskSpec { model: Some("haiku".into()), task_type: Some("dev".into()), ..base(dir.path()) }, None, None).unwrap();
        assert_eq!(args.iter().filter(|a| a.starts_with("--model")).collect::<Vec<_>>(), ["--model=haiku"]);
        let sep = args.iter().position(|a| a == "--").unwrap();
        assert!(args.iter().position(|a| a == "--model=haiku").unwrap() < sep);
        assert_eq!(role_model(&args).as_deref(), Some("haiku"), "role JSON and --model agree");
    }

    #[test]
    fn nothing_chosen_means_no_model_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let (spec, args) = run(TaskSpec { task_type: Some("dev".into()), ..base(dir.path()) }, None, None).unwrap();
        assert!(!args.iter().any(|a| a.starts_with("--model")));
        assert_eq!(role_model(&args), None);
        assert_eq!(spec.model_pin.unwrap().layer, Layer::Cli);
    }

    #[test]
    fn type_default_and_task_override() {
        let dir = tempfile::tempdir().unwrap();
        let team = stored_with_type_model("review", "sonnet");
        let (spec, args) = run(TaskSpec { task_type: Some("review".into()), ..base(dir.path()) }, Some(&team), None).unwrap();
        assert!(args.contains(&"--model=sonnet".to_string()));
        assert_eq!(role_model(&args).as_deref(), Some("sonnet"));
        assert_eq!(spec.model_pin.unwrap().layer, Layer::Type);
        let (spec, args) = run(TaskSpec { task_type: Some("review".into()), model: Some("opus".into()), ..base(dir.path()) }, Some(&team), None).unwrap();
        assert!(args.contains(&"--model=opus".to_string()) && !args.contains(&"--model=sonnet".to_string()));
        assert_eq!(role_model(&args).as_deref(), Some("opus"));
        assert_eq!(spec.model_pin.unwrap().layer, Layer::Task);
    }

    #[test]
    fn file_agents_follow_their_file_unless_the_task_picks_one() {
        let dir = tempfile::tempdir().unwrap();
        let persona = TaskSpec { agent: Some("design-critic".into()), ..base(dir.path()) };
        let (spec, args) = run(persona.clone(), None, Some("opus")).unwrap();
        assert!(args.contains(&"--model=opus".to_string()), "the file's value is passed so a resume reproduces it");
        assert_eq!(spec.model_pin.unwrap().layer, Layer::Agent);
        // Spike 1.1: --model wins over the file, so a task value on top is passed.
        let (spec, args) = run(TaskSpec { model: Some("sonnet".into()), ..persona.clone() }, None, Some("opus")).unwrap();
        assert!(args.contains(&"--model=sonnet".to_string()));
        assert_eq!(spec.model_pin.unwrap().layer, Layer::Task);
        let (_, args) = run(TaskSpec { model: Some("sonnet".into()), ..persona }, None, None).unwrap();
        assert!(args.contains(&"--model=sonnet".to_string()));
    }

    #[test]
    fn bad_values_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(TaskSpec { model: Some("-p".into()), ..base(dir.path()) }.validate().is_err());
        assert!(TaskSpec { model: Some("inherit".into()), ..base(dir.path()) }.validate().is_err());
        let from_ui: TaskSpec = serde_json::from_str(&format!(r#"{{"cwd":{:?},"prompt":"p","name":null,"permissionMode":null,"model":"opus --dangerously-skip-permissions"}}"#, dir.path())).unwrap();
        assert!(from_ui.validate().is_err());
    }

    #[test]
    fn unsupported_values_fall_back_and_are_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let team = stored_with_type_model("dev", "claude-opus-3-9");
        let (spec, args) = run(TaskSpec { task_type: Some("dev".into()), ..base(dir.path()) }, Some(&team), None).unwrap();
        assert!(args.contains(&"--model=opus".to_string()), "{args:?}");
        assert_eq!(role_model(&args).as_deref(), Some("opus"));
        assert_eq!(spec.model_requested.as_deref(), Some("claude-opus-3-9"));
        let (spec, args) = run(TaskSpec { task_type: Some("dev".into()), model: Some("acme-large-2".into()), ..base(dir.path()) }, None, None).unwrap();
        assert!(!args.iter().any(|a| a.starts_with("--model")) && role_model(&args).is_none());
        assert_eq!(spec.model_requested.as_deref(), Some("acme-large-2"));
        let (spec, args) = run(TaskSpec { model: Some("claude-haiku-4-5-20251001".into()), ..base(dir.path()) }, None, None).unwrap();
        assert!(args.contains(&"--model=claude-haiku-4-5-20251001".to_string()));
        assert_eq!(spec.model_requested, None);
        let (spec, args) = run(TaskSpec { agent: Some("x".into()), ..base(dir.path()) }, None, Some("claude-sonnet-3-9")).unwrap();
        assert!(args.contains(&"--model=sonnet".to_string()), "a gone file model is replaced through --model: {args:?}");
        assert_eq!((spec.model_pin.unwrap().layer, spec.model_requested.as_deref()), (Layer::Agent, Some("claude-sonnet-3-9")));
    }

    #[test]
    fn resume_reproduces_the_pin_and_falls_back_again() {
        let dir = tempfile::tempdir().unwrap();
        let team = stored_with_type_model("dev", "opus");
        // Type default changed since the start: the pin wins.
        let pin = ModelPin { layer: Layer::Type, arg: Some("sonnet".into()), role: Some("sonnet".into()) };
        let (spec, args) = run(TaskSpec { task_type: Some("dev".into()), model_pin_in: Some(pin), ..base(dir.path()) }, Some(&team), None).unwrap();
        assert!(args.contains(&"--model=sonnet".to_string()) && role_model(&args).as_deref() == Some("sonnet"));
        let sid = "0f3b2a8e-1111-4222-8333-444455556666";
        let r = ResumeSpec::from_prepared(&spec, sid, dir.path().into()).unwrap();
        assert_eq!(r.model_args, vec![OsString::from("--model=sonnet")]);
        // The pinned id is gone now.
        let gone = ModelPin { layer: Layer::Task, arg: Some("claude-opus-3-1".into()), role: None };
        let (spec, _) = run(TaskSpec { model_pin_in: Some(gone), ..base(dir.path()) }, None, None).unwrap();
        assert_eq!(spec.model_args(), vec![OsString::from("--model=opus")]);
        assert_eq!(spec.model_requested.as_deref(), Some("claude-opus-3-1"));
        // An agent-layer pin is not locked.
        let agent = ModelPin { layer: Layer::Agent, arg: None, role: None };
        assert!(run(TaskSpec { agent: Some("x".into()), model_pin_in: Some(agent), ..base(dir.path()) }, None, Some("opus")).is_ok());
        // A replaced agent-layer pin comes back as the same --model on every resume.
        let replaced = ModelPin { layer: Layer::Agent, arg: Some("sonnet".into()), role: None };
        let (spec, _) = run(TaskSpec { agent: Some("x".into()), model_pin_in: Some(replaced), ..base(dir.path()) }, None, Some("claude-sonnet-3-9")).unwrap();
        assert_eq!(spec.model_args(), vec![OsString::from("--model=sonnet")]);
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod help_cli {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake(dir: &Path, body: &str) -> ClaudeCli {
        let p = dir.join("claude");
        std::fs::write(&p, format!("#!/bin/sh\necho \"$@\" > \"{}/args\"\n{body}\n", dir.display())).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        ClaudeCli::new(p, None)
    }

    #[test]
    fn help_runs_with_one_fixed_argument_and_no_model() {
        let dir = tempfile::tempdir().unwrap();
        let cli = fake(dir.path(), "printf '  --model <model>  Provide an alias (e.g. %s, %s) or a name.\\n  --name <n>  x\\n' \"'opus'\" \"'sonnet'\"");
        let text = cli.help_text(dir.path()).unwrap();
        assert_eq!(crate::model_select::parse_help(&text), Some(vec!["opus".to_string(), "sonnet".to_string()]));
        assert_eq!(std::fs::read_to_string(dir.path().join("args")).unwrap().trim(), "--help", "nothing else is passed");
        let broken = fake(dir.path(), "exit 3");
        assert!(broken.help_text(dir.path()).is_err(), "a failed help leaves only the CLI source out");
    }
}

/// windows-support 2.3 [CI]: the Windows command-line rule, checked on every OS.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod windows_cmdline {
    use super::*;

    fn len(args: &[&str]) -> usize {
        windows_cmdline_len(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn quoting_is_counted_like_std() {
        assert_eq!(len(&["--bg"]), 5);
        assert_eq!(len(&[""]), 3, "an empty argument is quoted");
        assert_eq!(len(&["a b"]), 6, "space: quoted");
        assert_eq!(len(&[r#"a"b"#]), 5, "a quote is escaped");
        assert_eq!(len(&[r#"a\"b"#]), 7, "a backslash before a quote is doubled");
        assert_eq!(len(&[r"a b\"]), 8, "a trailing backslash before the closing quote is doubled");
        assert_eq!(len(&["가"]), 2, "UTF-16 units, not bytes");
        assert_eq!(len(&["😀"]), 3);
    }

    #[test]
    fn a_long_prompt_is_too_long_for_windows_only() {
        let mut s = TaskSpec::basic(std::env::temp_dir(), "가 ".repeat(16_000), None, None);
        s.guard_ctx = None;
        assert!(s.args_fit(), "within the macOS byte cap");
        assert!(!s.fits_windows_cmdline(), "over 30,000 UTF-16 units once quoted");
        let short = TaskSpec::basic(std::env::temp_dir(), "fix the bug".into(), None, None);
        assert!(short.fits_windows_cmdline());
        assert_eq!(ReasonCode::WindowsCommandTooLong.code(), "windowsCommandTooLong");
    }
}
