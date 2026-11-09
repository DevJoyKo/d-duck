//! Claude CLI login state from `claude auth status --json` (claude-login, D-150, D-155).
//!
//! Only six allowed fields are deserialized. `email`, `orgId`, `orgName` and the directory paths never reach a
//! struct: they live in the caller's stdout buffer until it is dropped right after [`judge`]. The app never reads
//! credentials; logging in is the CLI's job and the app only watches the state (D-004).

use serde::{Deserialize, Serialize};

/// Fixed code for a refused model call or a routine run recorded while logged out.
pub const LOGIN_REQUIRED: &str = "login_required";
/// Fixed code for a claude.ai-only feature (cloud routines) under another sign-in method.
pub const CLAUDE_AI_REQUIRED: &str = "claude_ai_required";
/// The gate's one-line copy (design 8). Also used for late login failures.
pub const LOGIN_REQUIRED_TEXT: &str = "Claude에 로그인해야 이 기능을 쓸 수 있어요";
pub const CLAUDE_AI_REQUIRED_TEXT: &str = "클라우드 루틴은 Claude 계정으로 로그인해야 써요";

const DAY_MS: i64 = 24 * 3600 * 1000;
/// `loginExpiresAt` outside now ± 10 years is dropped.
const EXPIRY_RANGE_MS: i64 = 3653 * DAY_MS;
/// A claude.ai login this close to expiry gets the soft notice.
pub const EXPIRES_SOON_MS: i64 = 3 * DAY_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Login {
    LoggedIn,
    LoggedOut,
    /// Spawn failure, timeout, unparsable output, another exit code, or a code that disagrees with `loggedIn`.
    /// Never blocks anything.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    None,
    ClaudeAi,
    OauthToken,
    ApiKey,
    ApiKeyHelper,
    ThirdParty,
    Other,
}

impl AuthMethod {
    fn parse(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "claude.ai" => Self::ClaudeAi,
            "oauth_token" => Self::OauthToken,
            "api_key" => Self::ApiKey,
            "api_key_helper" => Self::ApiKeyHelper,
            "third_party" => Self::ThirdParty,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    FirstParty,
    Other,
}

/// The judged state. Shared by every feature that needs to know about the sign-in (model-select reads the plan
/// only after the login check).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub login: Login,
    pub method: Option<AuthMethod>,
    pub provider: Option<Provider>,
    /// `subscriptionType` / `planTier` as given, only when they match `[a-z0-9_]{1,32}` (D-155).
    pub subscription: Option<String>,
    pub plan_tier: Option<String>,
    /// `loginExpiresAt` in epoch ms, within now ± 10 years.
    pub expires_at: Option<i64>,
}

/// The six allowed fields. Everything else in the JSON is skipped by serde and never copied.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    #[serde(default)]
    logged_in: Option<serde_json::Value>,
    #[serde(default)]
    auth_method: Option<serde_json::Value>,
    #[serde(default)]
    api_provider: Option<serde_json::Value>,
    #[serde(default)]
    subscription_type: Option<serde_json::Value>,
    #[serde(default)]
    plan_tier: Option<serde_json::Value>,
    #[serde(default)]
    login_expires_at: Option<serde_json::Value>,
}

/// Which kind of sign-in a feature needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Any working sign-in (model calls: tasks, refine, insights, routines, resume).
    Model,
    /// A claude.ai login (cloud routines).
    ClaudeAi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    LoginRequired,
    ClaudeAiRequired,
}

impl Block {
    pub fn code(self) -> &'static str {
        match self {
            Self::LoginRequired => LOGIN_REQUIRED,
            Self::ClaudeAiRequired => CLAUDE_AI_REQUIRED,
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Self::LoginRequired => LOGIN_REQUIRED_TEXT,
            Self::ClaudeAiRequired => CLAUDE_AI_REQUIRED_TEXT,
        }
    }
}

fn plan_value(v: Option<&serde_json::Value>) -> Option<String> {
    let s = v?.as_str()?;
    let ok = (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ok.then(|| s.to_string())
}

fn expiry(v: Option<&serde_json::Value>, now_ms: i64) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(v?.as_str()?).ok()?.timestamp_millis();
    ((t - now_ms).abs() <= EXPIRY_RANGE_MS).then_some(t)
}

impl AuthState {
    pub fn unknown() -> Self {
        Self { login: Login::Unknown, method: None, provider: None, subscription: None, plan_tier: None, expires_at: None }
    }

    /// The gate (design 4). `Unknown` always passes: a wrong lock on every feature is worse than a late failure.
    pub fn check(&self, need: Need) -> Result<(), Block> {
        match (self.login, need) {
            (Login::LoggedOut, _) => Err(Block::LoginRequired),
            (Login::LoggedIn, Need::ClaudeAi) if self.method != Some(AuthMethod::ClaudeAi) => Err(Block::ClaudeAiRequired),
            _ => Ok(()),
        }
    }

    /// Whether the connection is a claude.ai login (cloud routines, claude.ai connectors).
    pub fn is_claude_ai(&self) -> bool {
        self.login == Login::LoggedIn && self.method == Some(AuthMethod::ClaudeAi)
    }

    /// The soft "곧 만료돼요" notice: claude.ai login expiring within 3 days (not already past).
    pub fn expires_soon(&self, now_ms: i64) -> bool {
        self.is_claude_ai() && self.expires_at.is_some_and(|at| at > now_ms && at - now_ms <= EXPIRES_SOON_MS)
    }

    /// How a logged-in state is connected, as a fixed code (`claudeAi`, `token`, `other`; design 8). The webview
    /// words it; [`connection_text`](Self::connection_text) is the fallback sentence.
    pub fn connection_code(&self) -> Option<&'static str> {
        if self.login != Login::LoggedIn {
            return None;
        }
        if self.provider == Some(Provider::Other) {
            return Some("other");
        }
        Some(match self.method {
            Some(AuthMethod::ClaudeAi) => "claudeAi",
            Some(AuthMethod::OauthToken) => "token",
            _ => "other",
        })
    }

    /// One-line connection text for a logged-in state in the current app language.
    pub fn connection_text(&self) -> Option<&'static str> {
        self.connection_text_in(crate::i18n::current())
    }

    pub fn connection_text_in(&self, lang: crate::i18n::Lang) -> Option<&'static str> {
        use crate::i18n::Code;
        self.connection_code().map(|c| {
            crate::i18n::text(lang, match c {
                "claudeAi" => Code::AuthClaudeAi,
                "token" => Code::AuthToken,
                _ => Code::AuthOther,
            })
        })
    }

    /// What the webview gets: computed fields only, no raw CLI text.
    pub fn view(&self, now_ms: i64) -> AuthView {
        AuthView {
            login: self.login,
            method: self.method,
            connection_code: self.connection_code(),
            connection: self.connection_text(),
            plan: if self.is_claude_ai() { self.plan_tier.clone().or_else(|| self.subscription.clone()) } else { None },
            claude_ai: self.is_claude_ai(),
            expires_soon: self.expires_soon(now_ms),
        }
    }
}

/// IPC shape of the login state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthView {
    pub login: Login,
    pub method: Option<AuthMethod>,
    /// `claudeAi` | `token` | `other`; the webview's wording key.
    pub connection_code: Option<&'static str>,
    /// Fallback sentence in the current language.
    pub connection: Option<&'static str>,
    pub plan: Option<String>,
    pub claude_ai: bool,
    pub expires_soon: bool,
}

/// Judges one `auth status --json` run. `exit_code` is `None` when the process did not exit normally.
/// Logged in = exit 0 + `loggedIn: true`; logged out = exit 1 + `loggedIn: false`; anything else is unknown.
pub fn judge(exit_code: Option<i32>, stdout: &[u8], now_ms: i64) -> AuthState {
    let Ok(raw) = serde_json::from_slice::<Raw>(stdout) else { return AuthState::unknown() };
    let login = match (exit_code, raw.logged_in.as_ref().and_then(serde_json::Value::as_bool)) {
        (Some(0), Some(true)) => Login::LoggedIn,
        (Some(1), Some(false)) => Login::LoggedOut,
        _ => return AuthState::unknown(),
    };
    let method = raw.auth_method.as_ref().and_then(serde_json::Value::as_str).map(AuthMethod::parse);
    let provider = raw.api_provider.as_ref().and_then(serde_json::Value::as_str).map(|p| if p == "firstParty" { Provider::FirstParty } else { Provider::Other });
    if login == Login::LoggedOut {
        return AuthState { login, method: Some(AuthMethod::None), provider: None, subscription: None, plan_tier: None, expires_at: None };
    }
    AuthState {
        login,
        method,
        provider,
        subscription: plan_value(raw.subscription_type.as_ref()),
        plan_tier: plan_value(raw.plan_tier.as_ref()),
        expires_at: expiry(raw.login_expires_at.as_ref(), now_ms),
    }
}

/// The single core contract for the limit-resume loop (design 4): while logged out the shell does not call
/// `limit_resume::tick`, so `auto` rows stay `auto` (no `starting`, no `manual`, no cap count) until login.
pub fn should_tick(auth: &AuthState) -> bool {
    auth.login != Login::LoggedOut
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const NOW: i64 = 1_794_100_000_000;

    fn out(logged_in: bool, method: &str, extra: &str) -> Vec<u8> {
        format!(r#"{{"loggedIn":{logged_in},"authMethod":"{method}","apiProvider":"firstParty"{extra}}}"#).into_bytes()
    }

    #[test]
    fn exit_code_and_json_must_agree() {
        assert_eq!(judge(Some(0), &out(true, "claude.ai", ""), NOW).login, Login::LoggedIn);
        assert_eq!(judge(Some(1), &out(false, "none", ""), NOW).login, Login::LoggedOut);
        assert_eq!(judge(Some(0), &out(false, "none", ""), NOW).login, Login::Unknown, "mismatch");
        assert_eq!(judge(Some(1), &out(true, "claude.ai", ""), NOW).login, Login::Unknown, "mismatch");
        assert_eq!(judge(Some(2), &out(false, "none", ""), NOW).login, Login::Unknown);
        assert_eq!(judge(None, &out(true, "claude.ai", ""), NOW).login, Login::Unknown, "killed");
        assert_eq!(judge(Some(0), b"Logged in as someone", NOW), AuthState::unknown());
        assert_eq!(judge(Some(0), b"", NOW), AuthState::unknown());
        assert_eq!(judge(Some(0), br#"{"loggedIn":"yes"}"#, NOW).login, Login::Unknown);
    }

    #[test]
    fn real_logged_out_shape() {
        // Shape of `claude auth status` with an empty CLAUDE_CONFIG_DIR (2.1.319), personal values replaced.
        let s = br#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty","analyticsDisabled":false,"projectsDirectory":"/x","configDirectory":"/y"}"#;
        let st = judge(Some(1), s, NOW);
        assert_eq!(st.login, Login::LoggedOut);
        assert_eq!(st.check(Need::Model), Err(Block::LoginRequired));
        assert!(!should_tick(&st));
    }

    #[test]
    fn methods_map_to_an_allowlist() {
        let st = |m: &str| judge(Some(0), &out(true, m, ""), NOW);
        assert_eq!(st("claude.ai").method, Some(AuthMethod::ClaudeAi));
        assert_eq!(st("oauth_token").method, Some(AuthMethod::OauthToken));
        assert_eq!(st("api_key_helper").method, Some(AuthMethod::ApiKeyHelper));
        assert_eq!(st("third_party").method, Some(AuthMethod::ThirdParty));
        let odd = st("weird-new-thing");
        assert_eq!(odd.method, Some(AuthMethod::Other));
        assert!(!format!("{odd:?}").contains("weird"), "the unknown string is not kept");
        assert_eq!(odd.connection_text(), Some("다른 방식으로 연결돼 있어요"));
        assert_eq!(odd.connection_code(), Some("other"));
        assert_eq!(st("claude.ai").connection_code(), Some("claudeAi"));
        assert_eq!(st("claude.ai").connection_text_in(crate::i18n::Lang::En), Some("Connected with your Claude account"));
        assert_eq!(st("claude.ai").connection_text(), Some("Claude 계정으로 연결돼 있어요"));
        assert_eq!(st("oauth_token").connection_text(), Some("토큰으로 연결돼 있어요"));
        assert_eq!(st("third_party").connection_text(), Some("다른 방식으로 연결돼 있어요"));
        assert_eq!(st("api_key").connection_text(), Some("다른 방식으로 연결돼 있어요"));
        let bedrock = judge(Some(0), br#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"bedrock"}"#, NOW);
        assert_eq!(bedrock.provider, Some(Provider::Other));
        assert_eq!(bedrock.connection_text(), Some("다른 방식으로 연결돼 있어요"));
    }

    #[test]
    fn plan_values_are_kept_only_when_safe() {
        let st = judge(Some(0), &out(true, "claude.ai", r#","subscriptionType":"max","planTier":"max_5x""#), NOW);
        assert_eq!((st.subscription.as_deref(), st.plan_tier.as_deref()), (Some("max"), Some("max_5x")));
        assert_eq!(st.view(NOW).plan.as_deref(), Some("max_5x"));
        let bad = judge(Some(0), &out(true, "claude.ai", r#","subscriptionType":"","planTier":"Max 5x!""#), NOW);
        assert_eq!((bad.subscription.clone(), bad.plan_tier.clone()), (None, None));
        assert!(!format!("{bad:?}").contains("Max 5x"));
        let long = judge(Some(0), &out(true, "claude.ai", &format!(r#","planTier":"{}""#, "a".repeat(33))), NOW);
        assert_eq!(long.plan_tier, None);
    }

    #[test]
    fn gate_by_need() {
        let token = judge(Some(0), &out(true, "oauth_token", ""), NOW);
        assert_eq!(token.check(Need::Model), Ok(()));
        assert_eq!(token.check(Need::ClaudeAi), Err(Block::ClaudeAiRequired));
        let ai = judge(Some(0), &out(true, "claude.ai", ""), NOW);
        assert_eq!(ai.check(Need::ClaudeAi), Ok(()));
        let unknown = AuthState::unknown();
        assert_eq!(unknown.check(Need::Model), Ok(()), "unknown never blocks");
        assert_eq!(unknown.check(Need::ClaudeAi), Ok(()));
        assert!(should_tick(&unknown) && should_tick(&ai));
        assert_eq!(Block::LoginRequired.code(), "login_required");
        assert_eq!(Block::ClaudeAiRequired.code(), "claude_ai_required");
    }

    #[test]
    fn expiry_window() {
        let at = |ms: i64| {
            let t = chrono::DateTime::from_timestamp_millis(NOW + ms).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            judge(Some(0), &out(true, "claude.ai", &format!(r#","loginExpiresAt":"{t}""#)), NOW)
        };
        assert!(at(2 * DAY_MS).expires_soon(NOW), "2 days");
        assert!(!at(10 * DAY_MS).expires_soon(NOW), "10 days");
        assert!(!at(-DAY_MS).expires_soon(NOW), "past");
        assert_eq!(at(11 * 365 * DAY_MS).expires_at, None, "beyond 10 years");
        let bad = judge(Some(0), &out(true, "claude.ai", r#","loginExpiresAt":"tomorrow""#), NOW);
        assert_eq!(bad.expires_at, None);
        // Only a claude.ai login gets the notice.
        let t = chrono::DateTime::from_timestamp_millis(NOW + DAY_MS).unwrap().to_rfc3339();
        assert!(!judge(Some(0), &out(true, "oauth_token", &format!(r#","loginExpiresAt":"{t}""#)), NOW).expires_soon(NOW));
    }

    #[test]
    fn personal_fields_never_survive() {
        let s = br#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"CANARY_EMAIL_1","orgId":"CANARY_ORGID_2","orgName":"CANARY_ORGNAME_3","projectsDirectory":"/CANARY_PROJ_4","configDirectory":"/CANARY_CONF_5","subscriptionType":"max","planTier":"max_5x","storedClaudeAiLogin":true}"#;
        let st = judge(Some(0), s, NOW);
        assert_eq!(st.login, Login::LoggedIn);
        let texts = [serde_json::to_string(&st).unwrap(), format!("{st:?}"), serde_json::to_string(&st.view(NOW)).unwrap(), format!("{:?}", st.view(NOW))];
        for t in texts {
            assert!(!t.contains("CANARY"), "{t}");
        }
    }
}
