//! Real check for cloud routines (D-066): guarded headless run → parse → clean. Prints names and dates only.
use dduck_core::{claude_cli::ClaudeCli, cli_path, cloud_routines as cr};

fn main() -> Result<(), String> {
    if std::env::args().nth(1).as_deref() == Some(cr::GUARD_FLAG) {
        use std::io::Read;
        let mut input = String::new();
        let _ = std::io::stdin().read_to_string(&mut input);
        std::process::exit(if cr::guard_allows(&input) { 0 } else { 2 });
    }
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let probe = cli_path::ShellProbe::default();
    let info = cli_path::resolve(None, &home, &probe).ok_or("claude not found")?;
    let cfg = cli_path::effective_config_dir(None, &probe, &home);
    let cli = ClaudeCli::new(info.path, cfg.env_value().map(Into::into));
    // Same layout as the app (path with a space, …/cloud-routines/cwd) and the same parse → clean order (D-072).
    let base = std::env::temp_dir().join(format!("dduck probe {}", std::process::id())).join("cloud-routines");
    let cwd = base.join("cwd");
    std::fs::create_dir_all(&cwd).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let t = std::time::Instant::now();
    let raw = cli.cloud_routines_raw(&cwd, &cr::guard_command(&exe)).map_err(|e| format!("{e:?}"))?;
    let projects = cfg.path.join("projects");
    let parsed = cr::parse_stream(&raw, &projects).map_err(|e| format!("{e:?}"))?;
    if let Some(l) = &parsed.leftovers {
        cr::clean(&projects, l);
        println!("cleaned leftovers: {}", !l.exists());
    }
    cr::clean_cwd_project(&projects, &cwd);
    println!("project folder removed: {}", !projects.join(cr::project_key(&cwd.canonicalize().map_err(|e| e.to_string())?)).exists());
    let list = parsed.list.ok_or("empty")?;
    println!("{} routines, cost {:?}, {:?}", list.routines.len(), list.cost_usd, t.elapsed());
    for r in &list.routines {
        println!("- {} | enabled={} | next={:?} | ended={:?}", r.name, r.enabled, r.next_run_at.map(|m| chrono::DateTime::from_timestamp_millis(m).map(|d| d.to_rfc3339())), r.ended_reason);
    }
    let _ = std::fs::remove_dir_all(base.parent().unwrap_or(&base));
    Ok(())
}
