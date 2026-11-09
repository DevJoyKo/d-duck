//! Starts one real "review"-type task through `TaskSpec::prepare` (no webview) for integration check 4.1.
use dduck_core::claude_cli::{ClaudeCli, TaskSpec};
use dduck_core::cli_path;

fn main() -> Result<(), String> {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let cwd = std::env::args().nth(1).ok_or("usage: techlead_probe <cwd>")?;
    let probe = cli_path::ShellProbe::default();
    let info = cli_path::resolve(None, &home, &probe).ok_or("claude not found")?;
    let cfg = cli_path::effective_config_dir(None, &probe, &home);
    let cli = ClaudeCli::new(info.path, cfg.env_value().map(Into::into));
    let mut spec = TaskSpec::basic(
        cwd.into(),
        "1) Run `git status --short | head -3` and report only how many lines it printed. 2) Try to create the file .dduck-probe-x.txt with the Write or Edit tool; if you have no such tool, say NO-EDIT-TOOL. 3) Your team rules tell you to start a blocker line with a specific word; write that word. Keep the whole answer under 5 lines.".into(),
        Some("dduck-techlead-probe".into()),
        None,
    );
    spec.task_type = Some("review".into());
    spec.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    spec.prepare(&home, 0, None).map_err(|e| format!("{e:?}"))?;
    let id = cli.start_background(&spec, None).map_err(|e| format!("{e:?}"))?;
    println!("{id}");
    Ok(())
}
