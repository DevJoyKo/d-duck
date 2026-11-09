//! Does first_prompt find the instruction of a given session? Prints only length + first 30 chars.
fn main() {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let Some(root) = dduck_core::discover::projects_root(&home.join(".claude")) else { return };
    for sid in std::env::args().skip(1) {
        match dduck_core::prompt::first_prompt(&root, &sid) {
            Some(p) => println!("{sid}: {} chars, starts {:?}", p.chars().count(), p.chars().take(30).collect::<String>()),
            None => println!("{sid}: NONE"),
        }
    }
}
