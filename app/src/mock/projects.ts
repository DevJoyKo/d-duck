// Mock-mode data for the per-tab project picker (project-switcher, D-127). Design review only: the real list, opaque ids,
// link and home checks and the settings setter live in Rust (projects.rs, projects_cmds.rs).
const hour = 3_600_000;
const now = Date.now();

export const MOCK_IDS = { dev: "1f3a5c7e9b2d4f60", side: "2b4d6f8a0c1e3a5b", home: "3c5e7a9b1d2f4a6c", lead: "key:4d6f8a0b2c3e5f7a", note: "5e7a9b1c3d4f6a8b", wt: "6f8a0b2c4d6e8f0a" };

const row = (id: string, name: string, parentDir: string | null, extra: Record<string, unknown> = {}) => ({
  id, name, parentDir, path: parentDir ? `${parentDir}/${name}` : null, folder: "ok", isHome: false, offReason: null, memory: false, memories: 0, rules: 0, lastActive: null, worktreeOf: null, empty: false, ...extra,
});

export const mockProjects = [
  row(MOCK_IDS.dev, "dev_team", "/Users/me/Agents", { memory: true, memories: 4, rules: 7, lastActive: now - hour }),
  row(MOCK_IDS.wt, "lint-sweep", "/Users/me/Agents/dev_team/.claude/worktrees", { worktreeOf: MOCK_IDS.dev, lastActive: now - 2 * hour }),
  row(MOCK_IDS.side, "side-project", "/Users/me/Agents", { lastActive: now - 3 * hour }),
  row(MOCK_IDS.home, "me", "/Users", { path: "/Users/me", isHome: true, offReason: "홈 폴더는 전역과 같아요", offCode: "home", memory: true, memories: 4, lastActive: now - 30 * hour }),
  row(MOCK_IDS.lead, "-Users-me--claude-worktrees-lead", null, { folder: "unknown", offReason: "폴더를 몰라요", offCode: "unknown", memory: true, empty: true }),
  row(MOCK_IDS.note, "notebook-projects", "/Users/me", { empty: true }),
];

const PAGES = ["permissions", "memory", "extensions", "mcp", "review"];
const pick = typeof location === "undefined" ? null : new URLSearchParams(location.search).get("project");
const first = pick && pick in MOCK_IDS ? MOCK_IDS[pick as keyof typeof MOCK_IDS] : MOCK_IDS.dev;
// `?project=none`: every tab starts with "프로젝트 없음"; otherwise no tab has its own pick and all follow recent[0].
let pages: Record<string, string | null> = pick === "none" ? Object.fromEntries(PAGES.map((p) => [p, null])) : {};
let recent: string[] = [first, ...[MOCK_IDS.dev, MOCK_IDS.side].filter((r) => r !== first)];

/** "원격 push 막기" ticks (delivery-guard); worktrees follow their parent. */
export const mockTicked = new Set<string>([MOCK_IDS.side]);
export function mockLocalOnly(id: string): boolean {
  const parent = mockProjects.find((p) => p.id === id)?.worktreeOf ?? null;
  return mockTicked.has(id) || (parent !== null && mockTicked.has(parent));
}

export function mockProjectPath(id: unknown): string | null {
  return mockProjects.find((p) => p.id === id)?.path ?? null;
}

export function mockProjectName(id: unknown): string | null {
  return mockProjects.find((p) => p.id === id)?.name ?? null;
}

export const projectHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  projects_known: () => ({ projects: mockProjects.map((p) => ({ ...p, localOnly: mockLocalOnly(p.id) })), pages: { ...pages }, recent }),
  set_page_project: (a) => {
    const id = (a.id as string | null) ?? null;
    const page = String(a.page ?? "");
    if (!PAGES.includes(page) || (id && !mockProjects.some((p) => p.id === id))) throw "목록에 없는 프로젝트예요";
    pages = { ...pages, [page]: id };
    if (id) recent = [id, ...recent.filter((r) => r !== id)].slice(0, 8);
  },
};
