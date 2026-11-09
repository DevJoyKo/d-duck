//! Connector (MCP server) status from `claude mcp list`, run only when the user asks (it launches every server).
//! Each line is `<name>: <target> - <status>`. Only the name, a kind, the host of a remote URL and a status keyword
//! survive parsing; commands, arguments, paths, URL paths/queries and raw status text are dropped (S5).

use serde::Serialize;

use crate::text::display_safe;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectorKind {
    ClaudeAi,
    Plugin,
    Remote,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectorState {
    Connected,
    NeedsAuth,
    Failed,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connector {
    pub name: String,
    pub kind: ConnectorKind,
    pub host: Option<String>,
    pub state: ConnectorState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorList {
    pub connectors: Vec<Connector>,
    /// Lines that did not look like a server entry (headers, notices) — counted, never shown.
    pub skipped: usize,
}

/// Host of an http(s) URL, without userinfo, port, path or query.
fn host_of(target: &str) -> Option<String> {
    let rest = target.strip_prefix("https://").or_else(|| target.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#', ' ']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).then(|| host.to_ascii_lowercase())
}

fn state_of(status: &str) -> ConnectorState {
    let s = status.to_ascii_lowercase();
    if s.contains("connected") && !s.contains("not connected") && !s.contains("disconnected") {
        ConnectorState::Connected
    } else if s.contains("auth") {
        ConnectorState::NeedsAuth
    } else if s.contains("fail") || s.contains("error") || s.contains("✗") {
        ConnectorState::Failed
    } else {
        ConnectorState::Other
    }
}

pub fn parse(stdout: &str) -> ConnectorList {
    let mut connectors = Vec::new();
    let mut skipped = 0;
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()).take(500) {
        let (Some(colon), Some(dash)) = (line.find(": "), line.rfind(" - ")) else {
            skipped += 1;
            continue;
        };
        if dash <= colon {
            skipped += 1;
            continue;
        }
        let name = &line[..colon];
        let target = line[colon + 2..dash].trim();
        let status = &line[dash + 3..];
        let target = target.trim_end_matches(" (HTTP)").trim_end_matches(" (SSE)").trim();
        let host = host_of(target);
        let kind = if name.starts_with("claude.ai ") {
            ConnectorKind::ClaudeAi
        } else if name.starts_with("plugin:") {
            ConnectorKind::Plugin
        } else if host.is_some() {
            ConnectorKind::Remote
        } else {
            ConnectorKind::Local
        };
        connectors.push(Connector {
            name: display_safe(name, 100),
            // Local commands never leave this function (they can carry tokens in arguments).
            host: if kind == ConnectorKind::Local { None } else { host },
            kind,
            state: state_of(status),
        });
    }
    ConnectorList { connectors, skipped }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn keeps_name_kind_host_state_only() {
        let out = "Checking MCP server health…\n\n\
            claude.ai Gmail: https://gmailmcp.googleapis.com/mcp?x=CANARY_Q - ✔ Connected\n\
            claude.ai Atlassian (2): https://mcp.atlassian.com/v1/sse - ! Needs authentication\n\
            plugin:cloudflare:cloudflare: npx -y mcp-remote https://u:CANARY_PW@bindings.mcp.cloudflare.com/sse - ! Needs authentication\n\
            notebooklm-mcp: /usr/local/bin/nlm serve --token CANARY_TOKEN - ✔ Connected\n\
            remote-x: https://user:CANARY@api.example.com:8443/mcp (HTTP) - ✗ Failed to connect\n\
            No MCP servers configured. Use `claude mcp add`";
        let l = parse(out);
        let json = serde_json::to_string(&l).unwrap();
        assert!(!json.contains("CANARY"), "{json}");
        assert!(!json.contains("nlm serve") && !json.contains("npx"), "commands dropped");
        assert_eq!(l.connectors.len(), 5);
        assert_eq!(l.skipped, 2);
        let c = |n: &str| l.connectors.iter().find(|c| c.name == n).unwrap().clone();
        assert_eq!((c("claude.ai Gmail").kind, c("claude.ai Gmail").state, c("claude.ai Gmail").host.as_deref()), (ConnectorKind::ClaudeAi, ConnectorState::Connected, Some("gmailmcp.googleapis.com")));
        assert_eq!(c("claude.ai Atlassian (2)").state, ConnectorState::NeedsAuth);
        assert_eq!(c("plugin:cloudflare:cloudflare").kind, ConnectorKind::Plugin);
        assert_eq!((c("notebooklm-mcp").kind, c("notebooklm-mcp").host.clone()), (ConnectorKind::Local, None));
        assert_eq!((c("remote-x").kind, c("remote-x").state, c("remote-x").host.as_deref()), (ConnectorKind::Remote, ConnectorState::Failed, Some("api.example.com")));
    }
}
