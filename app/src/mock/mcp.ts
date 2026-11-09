// Mock-mode data for the MCP tab (nav-ia). Design review only: the real list is read by Rust from the settings
// files (mcp_config.rs) and only names, transport, scope and host come back.
import { MOCK_IDS } from "./projects";

const user = [
  { name: "notebooklm-mcp", transport: "stdio", scope: "user", host: null },
  { name: "notion", transport: "http", scope: "user", host: "mcp.notion.com" },
  { name: "sentry", transport: "http", scope: "user", host: "mcp.sentry.dev" },
];
const dev = [
  { name: "figma", transport: "http", scope: "local", host: "mcp.figma.com" },
  { name: "postgres", transport: "stdio", scope: "project", host: null },
];

export const mcpHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  mcp_servers: (a) => ({ servers: [...user, ...(a.project === MOCK_IDS.dev ? dev : [])], unreadable: [] }),
};
