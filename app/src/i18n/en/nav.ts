import type ko from "../ko/nav";
import type { Shape } from "../types";

export default {
  groups: {
    workspace: "Workspace",
    claude: "Claude setup",
  },
  pages: {
    agents: "Agents",
    tasks: "Tasks",
    review: "Review",
    routines: "Routines",
    artifacts: "Artifacts",
    extensions: "Extensions",
    mcp: "MCP",
    permissions: "Permissions",
    memory: "Memory",
    news: "News",
    usage: "Usage",
    settings: "Settings",
  },
  theme: {
    label: "Theme: {current}. Click for {next}",
    title: "{current} (click for {next})",
  },
  language: {
    label: "Change language",
    menu: "Language",
  },
  cli: {
    found: "Claude Code {version}",
    missing: "Claude CLI not found",
  },
  tourAgain: "Replay tutorial",
  ingest: {
    backfill: "Reading history {done}/{total}",
    error: "Couldn't read history",
    fresh: "History is up to date",
  },
} satisfies Shape<typeof ko>;
