// Mock-mode data for "기본 장비" (agent-loadout). Design review only: Rust lists preloadable skills with their SKILL.md
// hash and size (loadout.rs) and the MCP servers from the settings files (mcp_config.rs).
const skill = (name: string, source: string, bytes: number, description: string) => ({
  name,
  source,
  plugin: source.startsWith("plugin:") ? source.slice(7) : null,
  hash: `${name.replace(/[^a-z]/g, "").padEnd(8, "a").slice(0, 8)}`.padEnd(32, "0"),
  bytes,
  description,
});

export const mockSkills = [
  skill("frontend-design", "user", 9_800, "의도가 보이는 화면 디자인"),
  skill("openspec-apply-change", "project", 6_200, "OpenSpec 변경의 태스크 구현"),
  skill("anthropic-skills:docx", "user", 14_500, "Word 문서 만들기"),
  skill("cloudflare:wrangler", "plugin:cloudflare", 7_400, "Wrangler CLI 실행과 문제 해결"),
  skill("cloudflare:workers-best-practices", "plugin:cloudflare", 11_900, "Workers 모범 사례"),
  skill("expo:expo-ui", "plugin:expo", 8_300, "SwiftUI·Compose 네이티브 UI"),
  skill("expo:expo-router", "plugin:expo", 6_900, "Expo Router 내비게이션"),
];

const servers = [
  { name: "notebooklm-mcp", transport: "stdio", scope: "user", host: null },
  { name: "notion", transport: "http", scope: "user", host: "mcp.notion.com" },
  { name: "sentry", transport: "http", scope: "user", host: "mcp.sentry.dev" },
  { name: "figma", transport: "http", scope: "local", host: "mcp.figma.com" },
  { name: "reports__v2", transport: "http", scope: "user", host: "reports.example" },
];

export const loadoutHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  loadout_options: () => ({ skills: mockSkills, unreadableSkills: [], mcp: servers, unreadable: [] }),
  save_agent_loadout: () => undefined,
};
