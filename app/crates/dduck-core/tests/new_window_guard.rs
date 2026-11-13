//! windows-support 3.1: WebView2 new-window requests stay denied. wry 0.57 marks every NewWindowRequested as
//! handled (denied) when no handler is registered, so the shell must never register one or allow navigation
//! elsewhere. This test fails if a handler appears in the shell source.
#![allow(clippy::unwrap_used)]

#[test]
fn shell_registers_no_new_window_handler() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/src");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(&p).unwrap();
            assert!(!text.contains("on_new_window") && !text.contains("NewWindowResponse"), "{} opens new webview windows", p.display());
        }
    }
}
