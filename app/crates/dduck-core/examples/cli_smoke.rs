//! Real-CLI smoke test: start a tiny plan-mode background task through the adapter, list it, read logs, stop it.
//! Usage: cargo run -p dduck-core --example cli_smoke -- <workdir>
use dduck_core::{claude_cli::*, cli_path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workdir = std::env::args().nth(1).ok_or("usage: cli_smoke <workdir>")?;
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let probe = cli_path::ShellProbe::default();
    let info = cli_path::resolve(None, &home, &probe).ok_or("claude not found")?;
    let cfg = cli_path::effective_config_dir(None, &probe, &home);
    let cli = ClaudeCli::new(info.path, cfg.env_value().map(Into::into));
    let mut spec = TaskSpec::basic(workdir.into(), "-p reply with just OK and do not use tools".into(), Some("dduck-smoke".into()), Some(PermissionMode::Plan));
    spec.guard_ctx = Some(dduck_core::delivery_guard::GuardCtx::for_tests());
    let id = cli.start_background(&spec, None).map_err(|e| format!("{e:?}"))?;
    println!("started id={id}");
    let mut seen = None;
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let list = cli.list_agents().map_err(|e| format!("{e:?}"))?;
        if let Some(s) = list.iter().find(|s| s.id.as_deref() == Some(id.as_str())) {
            println!("name={} kind={:?} status={:?}", s.name, s.kind, s.status);
            seen = Some(s.status);
            if s.status.is_ended() { break; }
        }
    }
    let screen = cli.logs(&id).map_err(|e| format!("{e:?}"))?;
    println!("logs bytes={} starts_with_clear={}", screen.len(), screen.starts_with(b"\x1b[2J"));
    let _ = cli.stop(&id);
    println!("final={seen:?} id={id}");
    Ok(())
}
