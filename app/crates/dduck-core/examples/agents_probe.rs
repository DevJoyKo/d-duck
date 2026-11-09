//! Prints the local agent catalog and live activity (names and states only; no descriptions).
use dduck_core::claude_cli::AgentSession;
use dduck_core::{agents_catalog, live_agents};

fn main() {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let cfg = home.join(".claude");
    let roots = agents_catalog::Roots { config_dir: cfg.clone(), project_dirs: std::env::args().skip(1).map(Into::into).collect() };
    for d in agents_catalog::discover(&roots).iter().filter(|d| d.primary) {
        println!("{:<28} {:?} editable={} malformed={} risk={}", d.name, d.source, d.editable, d.malformed, d.risk.any());
    }
    let out = std::process::Command::new("claude").args(["agents", "--json", "--all"]).output().map(|o| o.stdout).unwrap_or_default();
    let sessions: Vec<AgentSession> = dduck_core::claude_cli::parse_agents(&out).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    for a in live_agents::collect(&cfg, &sessions, &|_| None, now) {
        println!("activity: {:<20} {:?} count={} total={} subagent={}", if a.agent.is_empty() { "(기본 Claude)" } else { &a.agent }, a.state, a.count, a.total, a.subagent);
    }
}
