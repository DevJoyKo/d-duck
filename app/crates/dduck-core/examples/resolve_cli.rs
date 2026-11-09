//! Prints how D.Duck resolves the Claude CLI and config dir on this machine.
use dduck_core::cli_path;

fn main() {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let probe = if std::env::args().any(|a| a == "--probe") { cli_path::probe_default_shell() } else { Default::default() };
    println!("probe: {probe:?}");
    println!("cli: {:?}", cli_path::resolve(None, &home, &probe));
    println!("config_dir: {:?}", cli_path::effective_config_dir(None, &probe, &home));
}
