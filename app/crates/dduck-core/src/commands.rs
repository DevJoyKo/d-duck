//! Classifies slash command names into built-in commands and skills (D-019).

use std::collections::HashSet;
use std::sync::LazyLock;

static BUILTINS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let raw = include_str!("../data/builtin_commands.json");
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("commands").cloned())
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Builtin,
    Skill,
}

impl CommandKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CommandKind::Builtin => "builtin",
            CommandKind::Skill => "skill",
        }
    }
}

/// `name` has no leading `/`. Unknown names count as skills (custom commands, plugin skills).
pub fn classify(name: &str) -> CommandKind {
    if BUILTINS.contains(name) { CommandKind::Builtin } else { CommandKind::Skill }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn builtin_list_loads() {
        assert!(BUILTINS.len() > 20);
    }

    #[test]
    fn classifies_builtins_and_skills() {
        assert_eq!(classify("upgrade"), CommandKind::Builtin);
        assert_eq!(classify("reload-plugins"), CommandKind::Builtin);
        assert_eq!(classify("opsx:propose"), CommandKind::Skill);
        assert_eq!(classify("code-review"), CommandKind::Skill);
        assert_eq!(classify("my-custom"), CommandKind::Skill);
    }
}
