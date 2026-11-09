//! Unattended runs must never show dialogs, open terminals or carry a danger approval (D-049, D-051).

fn body_of<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src.find(signature).unwrap_or_else(|| panic!("{signature} not found"));
    let rest = &src[start..];
    // Up to the next top-level item.
    let end = ["\nfn ", "\n#[", "\n///", "\nconst "]
        .iter()
        .filter_map(|m| rest[1..].find(m))
        .min()
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    &rest[..end]
}

const FORBIDDEN: [&str; 6] = ["dialog", "DangerApproval", "trust_then_retry", "open_terminal", "Some(&", "blocking_show"];

#[test]
fn scheduler_paths_have_no_interactive_calls() {
    let shell = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/src/lib.rs")).unwrap();
    for sig in ["fn start_claim(", "fn spawn_scheduler(", "async fn run_routine_now(", "fn attach_report("] {
        let body = body_of(&shell, sig);
        for bad in FORBIDDEN {
            assert!(!body.contains(bad), "{sig} must not use {bad}");
        }
    }
    // The claimed spec may only gain its 작업 보고서 folder (decision-report) before it starts, with no approval.
    let claim = body_of(&shell, "fn start_claim(");
    assert!(claim.contains("let mut spec = claim.spec;"));
    assert!(claim.contains("start_background(&spec, None)"));
    assert_eq!(claim.matches("start_background(").count(), 1);
    assert_eq!(claim.matches("&mut spec").count(), 1, "attach_report is the only change to the claimed spec");
    assert!(claim.contains("attach_report(&app, &mut spec, None)"));
    for bad in ["remote", "push", "permission_mode", "spec.agent", "spec.tools"] {
        assert!(!claim.contains(bad), "start_claim must not touch {bad}");
    }
    // The report helper adds only the folder and its text: no remote control, no push, no permission changes.
    let attach = body_of(&shell, "fn attach_report(");
    for bad in ["remote_name =", "remote_control =", "push", "permission_mode", "approval"] {
        assert!(!attach.contains(bad), "attach_report must not touch {bad}");
    }
    let core: String = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/routines.rs"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for bad in ["DangerApproval", "dialog", "Command::new"] {
        assert!(!core.contains(bad), "routines.rs must not use {bad}");
    }
}
