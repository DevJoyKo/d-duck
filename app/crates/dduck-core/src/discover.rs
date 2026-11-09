//! Finds the transcript files D.Duck is allowed to read (D-022).
//!
//! Allowed, relative to `<config>/projects`:
//! - `<project>/<session-uuid>.jsonl`
//! - `<project>/<session-uuid>/subagents/agent-<id>.jsonl`
//! - `<project>/<session-uuid>/subagents/agent-<id>.meta.json`
//!
//! Symlinks are never followed, anything resolving outside the root is dropped, and names that look like
//! credentials or prompt history are rejected even if they match a pattern.

use std::path::{Path, PathBuf};

use crate::parse::FileContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptKind {
    Main,
    Subagent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptFile {
    pub path: PathBuf,
    /// Path relative to the projects root, `/`-separated. Used as the DB key so absolute paths (which embed the
    /// account name and project path) are not stored, and keys survive a config-dir move.
    pub rel_path: String,
    pub kind: TranscriptKind,
    pub project_dir: String,
    pub context: FileContext,
    /// Present for subagent transcripts whose `.meta.json` exists.
    pub meta_path: Option<PathBuf>,
}

/// `<config>/projects`, canonicalized. Returns `None` when it does not exist.
pub fn projects_root(config_dir: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(config_dir.join("projects")).ok().filter(|p| p.is_dir())
}

pub fn discover(root: &Path) -> Vec<TranscriptFile> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(root).follow_links(false).min_depth(2).max_depth(4);
    for entry in walker.into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue; // skips symlinks and directories
        }
        if let Some(file) = classify(root, entry.path()) {
            out.push(file);
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Decides whether one path is an allowed transcript. Public for the file watcher.
///
/// `path` must lie lexically under the canonical `root`, and canonicalizing it must not change it: that rejects a
/// symlink anywhere in the chain (file or directory), including links that point back inside `projects/`.
pub fn classify(root: &Path, path: &Path) -> Option<TranscriptFile> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || hard_linked(&meta) {
        return None;
    }
    let rel = path.strip_prefix(root).ok()?;
    let canonical = std::fs::canonicalize(path).ok()?;
    if canonical != root.join(rel) {
        return None;
    }
    // Any non-UTF-8 component rejects the path instead of being silently dropped.
    let parts: Vec<&str> = rel.iter().map(|c| c.to_str()).collect::<Option<Vec<_>>>()?;
    let rel_path = parts.join("/");
    let file_name = *parts.last()?;
    if is_forbidden_name(file_name) {
        return None;
    }
    match parts.as_slice() {
        [project, name] => {
            let session = name.strip_suffix(".jsonl").filter(|s| is_uuid(s))?;
            Some(TranscriptFile {
                path: canonical.clone(),
                rel_path,
                kind: TranscriptKind::Main,
                project_dir: project.to_string(),
                context: FileContext { session_id: session.to_string(), agent_id: None },
                meta_path: None,
            })
        }
        [project, session, "subagents", name] if is_uuid(session) => {
            let agent = name.strip_prefix("agent-")?.strip_suffix(".jsonl").filter(|a| is_agent_id(a))?;
            let meta = canonical.with_file_name(format!("agent-{agent}.meta.json"));
            let meta_ok = std::fs::symlink_metadata(&meta).map(|m| m.file_type().is_file()).unwrap_or(false);
            Some(TranscriptFile {
                path: canonical.clone(),
                rel_path,
                kind: TranscriptKind::Subagent,
                project_dir: project.to_string(),
                context: FileContext { session_id: session.to_string(), agent_id: Some(agent.to_string()) },
                meta_path: meta_ok.then_some(meta),
            })
        }
        _ => None,
    }
}

/// A transcript is written by one process into one place; extra hard links mean it was linked in from elsewhere.
#[cfg(unix)]
fn hard_linked(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.nlink() > 1
}

#[cfg(not(unix))]
fn hard_linked(_meta: &std::fs::Metadata) -> bool {
    false
}

fn is_forbidden_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("credential") || lower.contains("history") || lower.ends_with(".key")
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() })
}

pub fn is_agent_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;

    const S: &str = "11111111-1111-4111-8111-111111111111";

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "{}\n").unwrap();
    }

    #[test]
    fn only_allowed_files_are_found() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("claude");
        let projects = config.join("projects");
        let proj = projects.join("-tmp-proj");
        touch(&proj.join(format!("{S}.jsonl")));
        touch(&proj.join(S).join("subagents").join("agent-a1b2.jsonl"));
        touch(&proj.join(S).join("subagents").join("agent-a1b2.meta.json"));
        // Not allowed:
        touch(&proj.join("notes.jsonl"));
        touch(&proj.join(format!("{S}.json")));
        touch(&proj.join(S).join("other").join("agent-x.jsonl"));
        touch(&config.join(".credentials.json"));
        touch(&config.join("history.jsonl"));
        touch(&config.join("sessions").join("123.abc.key"));
        touch(&tmp.path().join("outside.jsonl"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            tmp.path().join("outside.jsonl"),
            proj.join("22222222-2222-4222-8222-222222222222.jsonl"),
        )
        .unwrap();

        // Links that stay inside projects/: a symlinked file and a symlinked project directory.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                proj.join(format!("{S}.jsonl")),
                proj.join("33333333-3333-4333-8333-333333333333.jsonl"),
            )
            .unwrap();
            std::os::unix::fs::symlink(&proj, projects.join("linked-proj")).unwrap();
            // A hard link pulled in from outside.
            fs::hard_link(tmp.path().join("outside.jsonl"), proj.join("44444444-4444-4444-8444-444444444444.jsonl")).unwrap();
        }

        let root = projects_root(&config).unwrap();
        let found = discover(&root);
        let rel: Vec<String> =
            found.iter().map(|f| f.path.strip_prefix(&root).unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(rel, vec![format!("-tmp-proj/{S}/subagents/agent-a1b2.jsonl"), format!("-tmp-proj/{S}.jsonl")]);

        assert_eq!(found[1].rel_path, format!("-tmp-proj/{S}.jsonl"));
        // classify() on a path through the symlinked directory is rejected too (watcher events).
        #[cfg(unix)]
        assert!(classify(&root, &root.join("linked-proj").join(format!("{S}.jsonl"))).is_none());

        let sub = found.iter().find(|f| f.kind == TranscriptKind::Subagent).unwrap();
        assert_eq!(sub.context.session_id, S);
        assert_eq!(sub.context.agent_id.as_deref(), Some("a1b2"));
        assert!(sub.meta_path.is_some());
    }

    #[test]
    fn missing_projects_dir_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(projects_root(tmp.path()).is_none());
    }

    #[test]
    fn forbidden_names_rejected_even_if_pattern_matches() {
        assert!(is_forbidden_name("agent-history.jsonl"));
        assert!(is_forbidden_name(".credentials.json"));
        assert!(!is_forbidden_name(&format!("{S}.jsonl")));
    }
}
