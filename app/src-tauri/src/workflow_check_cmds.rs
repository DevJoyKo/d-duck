//! 워크플로 meta 검사 (workflow-meta-check, D-183): the editor's live check, the same Rust rules the save path uses.
//! Pure: it reads no file and runs nothing. The answer is a reason code and a line number, never file text.

use dduck_core::authoring::{self, MetaCheck};

#[tauri::command]
pub fn check_workflow_meta(text: String, name: String) -> MetaCheck {
    authoring::check_workflow_meta(&text, &name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dduck_core::authoring::MetaReason;

    #[test]
    fn answers_codes_only() {
        assert!(check_workflow_meta("export const meta = { name: 'a', description: 'd' }".into(), "a".into()).ok);
        let bad = check_workflow_meta("export const meta = { name: 'a', description: `${x}` }".into(), "a".into());
        assert_eq!((bad.ok, bad.reason, bad.line), (false, Some(MetaReason::TemplateInterpolation), Some(1)));
        let renamed = check_workflow_meta("export const meta = { name: 'a', description: 'd' }".into(), "b".into());
        assert_eq!((renamed.reason, renamed.registered_name.as_deref()), (Some(MetaReason::NameMismatch), Some("a")));
    }
}
