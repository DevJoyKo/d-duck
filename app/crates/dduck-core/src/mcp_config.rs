//! MCP server list for the MCP tab (nav-ia, D-114), read statically so opening the tab never starts a server.
//!
//! Sources: the top-level `mcpServers` of Claude Code's user file (`~/.claude.json`), the current project's entry under
//! `projects[<folder>].mcpServers` there, and the project's `.mcp.json`. Each server is parsed into a struct that only
//! has `type` and `url`: `env`, `headers`, `args`, `command` and every other key (`oauthAccount`, …) are not fields,
//! so serde skips them and they never reach memory as values. Only name, transport, scope and the host of an http(s)
//! URL (no userinfo, port, path or query) come out. Files over 1 MB and links are refused.

use crate::platform::CanonExt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::text::display_safe;

pub const MAX_BYTES: u64 = 1024 * 1024;
const MAX_SERVERS: usize = 200;
const MAX_PROJECT_KEYS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Stdio,
    Http,
    Sse,
    Other,
}

/// `user` = every project, `local` = this project on this computer only, `project` = `.mcp.json` (shared by git).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum McpScope {
    User,
    Local,
    Project,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub transport: Transport,
    pub scope: McpScope,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpList {
    pub servers: Vec<McpServer>,
    /// Files that exist but were not read (too large, a link, not JSON), by label: "사용자 설정", ".mcp.json".
    pub unreadable: Vec<&'static str>,
}

/// Only the two keys we need. Every other key of a server entry is skipped with `IgnoredAny` while parsing; nothing
/// is buffered (no `untagged`/`flatten`, which would hold the whole entry in memory first).
#[derive(Default)]
struct Server {
    kind: Option<String>,
    url: Option<String>,
}

/// A string, or `None` for any other JSON value (skipped without keeping it).
struct Text(Option<String>);

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Text;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any value")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Text, E> {
                Ok(Text(Some(v.chars().take(2048).collect())))
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Text, E> {
                Ok(Text(None))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Text, A::Error> {
                while a.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Text(None))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Text, A::Error> {
                while a.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Text(None))
            }
        }
        d.deserialize_any(V)
    }
}

/// A server entry: an object yields `Some` with `type`/`url` only; any other value is skipped (`None`).
struct Entry(Option<Server>);

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Entry;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a server entry")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Entry, A::Error> {
                let mut s = Server::default();
                while let Some(k) = a.next_key::<String>()? {
                    match k.as_str() {
                        "type" => s.kind = a.next_value::<Text>()?.0,
                        "url" => s.url = a.next_value::<Text>()?.0,
                        _ => {
                            a.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(Entry(Some(s)))
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Entry, E> {
                Ok(Entry(None))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Entry, A::Error> {
                while a.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Entry(None))
            }
        }
        d.deserialize_any(V)
    }
}

/// `{ "mcpServers": {...}, <anything else skipped> }`; a non-object value counts as no servers.
#[derive(Default)]
struct Servers {
    mcp_servers: Option<BTreeMap<String, Entry>>,
}

impl<'de> Deserialize<'de> for Servers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Servers;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Servers, A::Error> {
                let mut out = Servers::default();
                while let Some(k) = a.next_key::<String>()? {
                    if k == "mcpServers" {
                        out.mcp_servers = a.next_value::<Option<BTreeMap<String, Entry>>>().ok().flatten();
                    } else {
                        a.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(out)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Servers, E> {
                Ok(Servers::default())
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Servers, A::Error> {
                while a.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Servers::default())
            }
        }
        d.deserialize_any(V)
    }
}

/// The user file: top-level `mcpServers` and `projects`; every other key (`oauthAccount`, …) is skipped by serde.
#[derive(Deserialize)]
struct UserFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: Option<BTreeMap<String, Entry>>,
    #[serde(default)]
    projects: Option<BTreeMap<String, Servers>>,
}

/// Host of an http(s) URL, without userinfo, port, path or query.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = if host.starts_with('[') { host.split(']').next().map(|h| &h[1..])? } else { host.split(':').next()? };
    (!host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')).then(|| host.to_ascii_lowercase())
}

fn transport(s: &Server) -> Transport {
    match s.kind.as_deref() {
        Some("stdio") => Transport::Stdio,
        Some("http") | Some("streamable-http") => Transport::Http,
        Some("sse") => Transport::Sse,
        None if s.url.is_some() => Transport::Http,
        None => Transport::Stdio,
        Some(_) => Transport::Other,
    }
}

/// Reads a regular file (not a link) of at most 1 MB: opened without following a final link, checked on the open
/// descriptor (no swap between check and read) and read with a hard cap.
fn read_small(path: &Path) -> Result<Option<Vec<u8>>, ()> {
    use std::io::Read;
    let f = match crate::artifact_preview::open_flags(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let m = f.metadata().map_err(|_| ())?;
    if !m.is_file() || m.len() > MAX_BYTES {
        return Err(());
    }
    let mut bytes = Vec::new();
    f.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| ())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(());
    }
    Ok(Some(bytes))
}

fn push(out: &mut Vec<McpServer>, map: Option<BTreeMap<String, Entry>>, scope: McpScope) {
    for (name, e) in map.unwrap_or_default() {
        if out.len() >= MAX_SERVERS {
            return;
        }
        let Entry(Some(s)) = e else { continue };
        let name = display_safe(&name, 80);
        if name.is_empty() {
            continue;
        }
        let host = match transport(&s) {
            Transport::Http | Transport::Sse | Transport::Other => s.url.as_deref().and_then(host_of),
            Transport::Stdio => None,
        };
        out.push(McpServer { name, transport: transport(&s), scope, host });
    }
}

fn same_folder(key: &str, project: &Path) -> bool {
    let k = Path::new(key);
    crate::platform::same_path(k, project) || (k.is_absolute() && k.canon().is_ok_and(|c| crate::platform::same_path(&c, project)))
}

/// Servers from the user file and, when a project folder is given, its local entry and `.mcp.json`.
/// `project` must already be canonical (a folder the app resolved from a project id).
pub fn list(user_file: &Path, project: Option<&Path>) -> McpList {
    let mut out = McpList::default();
    match read_small(user_file) {
        Ok(Some(bytes)) => match serde_json::from_slice::<UserFile>(&bytes) {
            Ok(f) => {
                push(&mut out.servers, f.mcp_servers, McpScope::User);
                if let (Some(p), Some(projects)) = (project, f.projects) {
                    let local = projects.into_iter().take(MAX_PROJECT_KEYS).find(|(k, _)| same_folder(k, p)).and_then(|(_, v)| v.mcp_servers);
                    push(&mut out.servers, local, McpScope::Local);
                }
            }
            Err(_) => out.unreadable.push(crate::i18n::Fail::UserSettings.str()),
        },
        Ok(None) => {}
        Err(()) => out.unreadable.push(crate::i18n::Fail::UserSettings.str()),
    }
    if let Some(p) = project {
        match read_small(&p.join(".mcp.json")) {
            Ok(Some(bytes)) => match serde_json::from_slice::<Servers>(&bytes) {
                Ok(s) => push(&mut out.servers, s.mcp_servers, McpScope::Project),
                Err(_) => out.unreadable.push(".mcp.json"),
            },
            Ok(None) => {}
            Err(()) => out.unreadable.push(".mcp.json"),
        }
    }
    out
}

/// `~/.claude.json` normally; inside the config folder when `CLAUDE_CONFIG_DIR` (or the app's choice) points elsewhere.
pub fn user_file(home: &Path, explicit_config_dir: Option<&Path>) -> PathBuf {
    match explicit_config_dir {
        Some(c) => c.join(".claude.json"),
        None => home.join(".claude.json"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const CANARY: &str = "CANARY-7f3e-secret";

    fn user_json(project: &Path) -> String {
        format!(
            r#"{{
  "oauthAccount": {{ "emailAddress": "{CANARY}@example.com", "accountUuid": "{CANARY}" }},
  "primaryApiKey": "{CANARY}",
  "mcpServers": {{
    "notion": {{ "type": "http", "url": "https://user:{CANARY}@mcp.notion.com:8443/v1/sse?token={CANARY}", "headers": {{ "Authorization": "Bearer {CANARY}" }} }},
    "local-db": {{ "command": "/Users/me/{CANARY}/bin/db", "args": ["--password", "{CANARY}"], "env": {{ "DB_PASSWORD": "{CANARY}" }} }},
    "odd": "{CANARY}",
    "events": {{ "type": "sse", "url": "http://localhost:3000/events" }}
  }},
  "projects": {{
    {}: {{ "mcpServers": {{ "figma": {{ "type": "http", "url": "https://mcp.figma.com/mcp", "headers": {{ "X-Key": "{CANARY}" }} }} }}, "history": ["{CANARY}"] }},
    "/elsewhere": {{ "mcpServers": {{ "other": {{ "command": "x" }} }} }}
  }}
}}"#,
            serde_json::to_string(&project.display().to_string()).unwrap()
        )
    }

    #[test]
    fn reads_only_names_transport_scope_and_host() {
        let t = tempfile::tempdir().unwrap();
        let proj = t.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        let proj = proj.canon().unwrap();
        let user = t.path().join(".claude.json");
        std::fs::write(&user, user_json(&proj)).unwrap();
        std::fs::write(proj.join(".mcp.json"), format!(r#"{{ "mcpServers": {{ "sentry": {{ "type": "http", "url": "https://mcp.sentry.dev/mcp", "env": {{ "K": "{CANARY}" }} }} }} }}"#)).unwrap();
        let l = list(&user, Some(&proj));
        let names: Vec<(&str, McpScope, Transport, Option<&str>)> = l.servers.iter().map(|s| (s.name.as_str(), s.scope, s.transport, s.host.as_deref())).collect();
        assert_eq!(
            names,
            vec![
                ("events", McpScope::User, Transport::Sse, Some("localhost")),
                ("local-db", McpScope::User, Transport::Stdio, None),
                ("notion", McpScope::User, Transport::Http, Some("mcp.notion.com")),
                ("figma", McpScope::Local, Transport::Http, Some("mcp.figma.com")),
                ("sentry", McpScope::Project, Transport::Http, Some("mcp.sentry.dev")),
            ]
        );
        let json = serde_json::to_string(&l).unwrap();
        assert!(!json.contains(CANARY), "no env, header, arg, command, userinfo, query or account value leaks");
        assert!(!json.contains("/Users/me"));
        assert!(!json.contains("other"), "another project's local servers are not listed");
    }

    #[test]
    fn without_a_project_only_user_servers() {
        let t = tempfile::tempdir().unwrap();
        let user = t.path().join(".claude.json");
        std::fs::write(&user, user_json(Path::new("/nowhere"))).unwrap();
        assert!(list(&user, None).servers.iter().all(|s| s.scope == McpScope::User));
        assert_eq!(list(&t.path().join("missing.json"), None), McpList::default());
    }

    #[test]
    fn refuses_big_files_links_and_junk() {
        let t = tempfile::tempdir().unwrap();
        let big = t.path().join("big.json");
        std::fs::write(&big, vec![b' '; (MAX_BYTES + 1) as usize]).unwrap();
        assert_eq!(list(&big, None).unreadable, vec!["사용자 설정"]);
        let junk = t.path().join("junk.json");
        std::fs::write(&junk, "not json").unwrap();
        assert_eq!(list(&junk, None).unreadable, vec!["사용자 설정"]);
        #[cfg(unix)]
        {
            let real = t.path().join("real.json");
            std::fs::write(&real, r#"{"mcpServers":{"a":{"command":"x"}}}"#).unwrap();
            let link = t.path().join("link.json");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let l = list(&link, None);
            assert!(l.servers.is_empty());
            assert_eq!(l.unreadable, vec!["사용자 설정"]);
        }
    }

    #[test]
    fn host_drops_userinfo_port_path_query() {
        assert_eq!(host_of("https://u:p@Api.Example.com:8443/a?b=c").as_deref(), Some("api.example.com"));
        assert_eq!(host_of("ftp://x"), None);
        assert_eq!(host_of("https://"), None);
        assert_eq!(host_of("https://bad host/"), None);
    }
}
