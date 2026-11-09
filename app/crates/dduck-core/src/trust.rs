//! Reads Claude's workspace trust for a folder (D-047). Read-only: D.Duck never writes Claude's config, because
//! running Claude processes rewrite that file and a concurrent write could lose the user's settings.
//!
//! Trust lives in `<home>/.claude.json` (or `$CLAUDE_CONFIG_DIR/.claude.json` when a config dir is set) under
//! `projects["<abs path>"].hasTrustDialogAccepted`. Trusting a folder also covers its subfolders.

use std::path::{Path, PathBuf};

const MAX_CONFIG_BYTES: u64 = 32 * 1024 * 1024;

/// Location of Claude's main config file for the given setup.
pub fn config_file(home: &Path, config_dir_env: Option<&Path>) -> PathBuf {
    match config_dir_env {
        Some(dir) => dir.join(".claude.json"),
        None => home.join(".claude.json"),
    }
}

/// `Some(true)` when `dir` or one of its ancestors is trusted, `Some(false)` when not, `None` when unknown
/// (config missing or unreadable). Only the trust flags are looked at; nothing else in the file is kept.
pub fn is_trusted(config_file: &Path, dir: &Path) -> Option<bool> {
    let meta = std::fs::metadata(config_file).ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(config_file).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let projects = v.get("projects")?.as_object()?;
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let trusted = dir.ancestors().any(|p| {
        p.to_str()
            .and_then(|k| projects.get(k))
            .and_then(|e| e.get("hasTrustDialogAccepted"))
            .and_then(|t| t.as_bool())
            .unwrap_or(false)
    });
    Some(trusted)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn reads_trust_for_folder_and_ancestors() {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let trusted = root.join("work");
        let child = trusted.join("app");
        let other = root.join("other");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let cfg = root.join(".claude.json");
        let json = serde_json::json!({
            "oauthAccount": {"emailAddress": "not-read@example.com"},
            "projects": {
                trusted.to_str().unwrap(): {"hasTrustDialogAccepted": true},
                other.to_str().unwrap(): {"hasTrustDialogAccepted": false}
            }
        });
        std::fs::write(&cfg, json.to_string()).unwrap();
        assert_eq!(is_trusted(&cfg, &trusted), Some(true));
        assert_eq!(is_trusted(&cfg, &child), Some(true), "subfolder of a trusted folder");
        assert_eq!(is_trusted(&cfg, &other), Some(false));
        assert_eq!(is_trusted(&cfg, &root), Some(false));
        assert_eq!(is_trusted(&root.join("missing.json"), &trusted), None);
    }

    #[test]
    fn config_location() {
        assert_eq!(config_file(Path::new("/h"), None), PathBuf::from("/h/.claude.json"));
        assert_eq!(config_file(Path::new("/h"), Some(Path::new("/c"))), PathBuf::from("/c/.claude.json"));
    }
}
