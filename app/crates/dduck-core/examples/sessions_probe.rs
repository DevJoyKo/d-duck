//! Real-data check for sessions-skills-connectors 4.1 (counts and kinds only; no transcript text).
use dduck_core::{db, discover, mcp_list, skills_catalog, timeline};

fn main() {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let cfg = home.join(".claude");
    let db_path = home.join("Library/Application Support/dev.dduck.desktop/dduck.db");
    let conn = db::open(&db_path).ok();
    let projects = vec![home.join("Agents/dev_team"), home.join("Agents/side-project")];
    let defs = skills_catalog::discover(&cfg, &projects);
    let used = conn.as_ref().and_then(|c| skills_catalog::usage(c).ok()).unwrap_or_default();
    let cat = skills_catalog::catalog(defs, used);
    let by = |s: skills_catalog::SkillSource| cat.iter().filter(|x| x.source == s).count();
    println!("skills: {} total | project {} user {} plugin {} builtin/undefined {}", cat.len(), by(skills_catalog::SkillSource::Project), by(skills_catalog::SkillSource::User), by(skills_catalog::SkillSource::Plugin), by(skills_catalog::SkillSource::Builtin));
    for s in cat.iter().take(5) {
        println!("  top: {} ({:?}) uses={}", s.name, s.source, s.uses);
    }
    if let (Some(root), Some(sid)) = (discover::projects_root(&cfg), std::env::args().nth(1)) {
        let t = std::time::Instant::now();
        match timeline::read(&root, &sid) {
            Some(tl) => {
                let n = |k: &str| tl.items.iter().filter(|i| serde_json::to_value(i).unwrap_or_default()["kind"] == k).count();
                println!("timeline: {} items (prompt {} reply {} tool {} compact {}) truncated={} skipped={} in {:?}", tl.items.len(), n("prompt"), n("reply"), n("tool"), n("compact"), tl.truncated_front, tl.skipped_lines, t.elapsed());
            }
            None => println!("timeline: not found"),
        }
    }
    if let Ok(out) = std::process::Command::new("claude").args(["mcp", "list"]).current_dir(&home).output() {
        let l = mcp_list::parse(&String::from_utf8_lossy(&out.stdout));
        let states: Vec<String> = l.connectors.iter().map(|c| format!("{}={:?}/{:?}", c.name, c.kind, c.state)).collect();
        println!("connectors: {} (skipped {}) {:?}", l.connectors.len(), l.skipped, states);
    }
}
