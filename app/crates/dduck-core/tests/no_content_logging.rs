//! D-021: nothing in the app may print or log transcript-derived data. The core and shell contain no logging calls
//! at all; this test keeps it that way (examples are excluded: they print numbers only and are dev tools).
#![allow(clippy::unwrap_used)]

use std::path::Path;

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_logging_macros_in_app_sources() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&core.join("src"), &mut files);
    rust_files(&core.join("../../src-tauri/src"), &mut files);
    assert!(files.len() > 5);
    // Word-boundary patterns so `dialog::` or `eprintln!` do not match `log::` / `println!`.
    let banned = regex::Regex::new(r"(^|[^A-Za-z0-9_])(println!|print!|eprint!|dbg!|log::|tracing::)").unwrap();
    let eprintln = regex::Regex::new(r"(^|[^A-Za-z0-9_])eprintln!").unwrap();
    let mut hits = Vec::new();
    for f in &files {
        // The step-timing module (D-159) is the one exception; `timing_module_records_numbers_only` checks it.
        if f.ends_with("src-tauri/src/timing.rs") {
            continue;
        }
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            // The only allowed stderr line is the fixed startup failure message.
            let allowed_eprintln = eprintln.is_match(line) && line.contains("\"D.Duck failed to start\"");
            if banned.is_match(line) || (eprintln.is_match(line) && !allowed_eprintln) {
                hits.push(format!("{}:{}", f.display(), n + 1));
            }
        }
    }
    assert!(hits.is_empty(), "logging calls found: {hits:?}");
}

/// D-159: timing.rs records fixed step names and milliseconds only, and its writer drops debug-formatted values.
#[test]
fn timing_module_records_numbers_only() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(core.join("../../src-tauri/src/timing.rs")).unwrap();
    let body = src.split("#[cfg(test)]").next().unwrap();
    let events: Vec<&str> = body.lines().filter(|l| l.contains("tracing::info!")).collect();
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(events[0].contains("flow = self.flow, steps = steps.as_str(), total_ms = ms("), "{}", events[0]);
    assert!(body.contains("laps: Vec<(&'static str, u64)>") && body.contains("flow: &'static str"));
    assert!(body.contains("fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}"));
    assert!(!body.contains("println!") && !body.contains("dbg!"));
}
