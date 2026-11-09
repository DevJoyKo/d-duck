//! Checks trust lookup and first-prompt lookup against this machine's real Claude data (prints facts, no content).
//! Usage: cargo run -p dduck-core --example trust_prompt_check -- <trusted-dir> <untrusted-dir>
use dduck_core::{discover, prompt, trust};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let cfg = trust::config_file(&home, None);
    for dir in &args {
        println!("trusted({dir}) = {:?}", trust::is_trusted(&cfg, dir.as_ref()));
    }
    let Some(root) = discover::projects_root(&home.join(".claude")) else { return };
    let files = discover::discover(&root);
    let mains: Vec<_> = files.iter().filter(|f| f.kind == discover::TranscriptKind::Main).collect();
    let found = mains.iter().filter(|f| prompt::first_prompt(&root, &f.context.session_id).is_some()).count();
    println!("sessions with a first prompt: {found}/{}", mains.len());
}
