## Why

The Agents tab showed a "형식 오류" (malformed) agent card for the `mattpocock-skills` plugin. The plugin has no agents; it ships a documentation file at `docs/agents/triage-labels.md`. D.Duck's agent catalog treated every folder named `agents` anywhere in the plugin cache (up to 6 levels deep) as agent definitions, while Claude Code reads only the plugin's root `agents/` folder and the `agents` paths in `plugin.json`. The plugins tab already used the correct rule; the agent catalog didn't.

## What Changes

- Plugin agents come only from `agents/` at the plugin's install root (newest version) plus `plugin.json` `agents` paths inside the install folder — the same rule as the plugins tab (`plugins::plugin_agent_dirs`).
- A folder named `agents` elsewhere in a plugin (e.g. `docs/agents/`) is ignored, so documentation never shows as a broken agent.

## Impact

- `app/crates/dduck-core/src/agents_catalog.rs` (plugin discovery), `app/crates/dduck-core/src/plugins.rs` (shared helper), a regression test.
- No UI, DB, or permission change.
