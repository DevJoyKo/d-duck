## Why

The Agents tab showed a "형식 오류" (malformed) agent card for the `mattpocock-skills` plugin. The plugin has no agents; it ships a documentation file at `docs/agents/triage-labels.md`. D.Duck's agent catalog treated every folder named `agents` anywhere in the plugin cache (up to 6 levels deep) as agent definitions, while Claude Code reads only the plugin's root `agents/` folder and the `.md` files listed in `plugin.json` `agents` (a folder there fails `claude plugin validate`: "must end with .md"). The catalog also picked the newest-modified version folder, while Claude Code loads the `installPath` recorded in `installed_plugins.json` (on a real machine mattpocock-skills has `1.3.1` recorded and a newer `49dd158d1076` folder).

## What Changes

- One shared helper (`plugins::plugin_agent_files`) lists a plugin's agent files: `agents/*.md` plus `plugin.json` `agents` `.md` files (relative, inside the install folder, regular files, no links). Folder entries, `..` and absolute paths are ignored. Both the plugins tab and the agents tab use it.
- One shared helper (`plugins::plugin_roots`) picks the install folder(s): the recorded `installPath`s (validated like the plugins tab's install path, in that plugin's own cache folder; all records are read), else the newest-modified version folder not marked `.orphaned_at`. The agents tab and the skills tab use it.
- A folder named `agents` elsewhere in a plugin (e.g. `docs/agents/`) is ignored, so documentation never shows as a broken agent.
- Plugin agent paths and ids are now based on the canonical install path.

## Impact

- `app/crates/dduck-core/src/agents_catalog.rs`, `plugins.rs`, `skills_catalog.rs`, regression tests.
- Spec: `agent-profiles` "Agent 목록" (modified plugin path and version rule).
- No UI, DB, or permission change.
