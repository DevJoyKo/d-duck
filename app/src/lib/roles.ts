// Role-coded duck colours and card ordering for the agents page.
// Same role → same colour, so a glance tells reviewers from developers (user request, D-064).
import type { DuckColor } from "../components/DuckAvatar";
import { t } from "../i18n";

export interface Role {
  id: string;
  label: string;
  color: DuckColor;
}

type RoleId = "security" | "qa" | "review" | "release" | "tutor" | "research" | "plan" | "writer" | "dev" | "general" | "default";

/** The label is read at access time so it follows the current language. */
function role(id: RoleId, color: DuckColor): Role {
  return {
    id,
    get label() {
      return t(`agents.roles.${id}`);
    },
    color,
  };
}

/** Checked in order; the first match wins. Matches the agent name, then its description. */
const ROLES: { role: Role; words: RegExp }[] = [
  { role: role("security", { body: "#ffb3a1", scarf: "#d9533b" }), words: /security|\bsecur|privacy|보안|취약/i },
  { role: role("qa", { body: "#9ee6c0", scarf: "#1b8f58" }), words: /\bqa\b|\btests?\b|tester|test-|verif|검증|테스트/i },
  { role: role("review", { body: "#c9b6ff", scarf: "#6b4fd1" }), words: /review|critic|리뷰|검토|비판/i },
  { role: role("release", { body: "#ffbf8a", scarf: "#d06a1f" }), words: /release|deploy|\bship\b|배포|릴리스/i },
  { role: role("tutor", { body: "#ffe08a", scarf: "#b07d00" }), words: /tutor|learn|study|학습|공부|튜터/i },
  { role: role("research", { body: "#7fd6d6", scarf: "#147f80" }), words: /research|explore|search|fetch|guide|조사|탐색|리서치/i },
  { role: role("plan", { body: "#cbe88a", scarf: "#5f8f17" }), words: /\bplan\b|architect|design|설계|계획/i },
  { role: role("writer", { body: "#ffb0d0", scarf: "#cc3f7b" }), words: /writer|\bdocs?\b|comms|communicat|문서|공지|커뮤니케이션/i },
  { role: role("dev", { body: "#9fd0ff", scarf: "#2f7dd1" }), words: /develop|\bdev\b|engineer|coder|implement|\bbuild|개발|구현|perf/i },
];

const GENERAL: Role = role("general", { body: "#b8c2d6", scarf: "#4b6a96" });
export const DEFAULT_ROLE: Role = role("default", { body: "#ffffff", scarf: "" });

/** Built-ins whose descriptions would mislead the keyword match. */
const PINNED: Record<string, string> = { "general-purpose": "general", "statusline-setup": "general" };

export function roleOf(name: string, description?: string | null): Role {
  const base = name.includes(":") ? name.slice(name.lastIndexOf(":") + 1) : name;
  const pinned = PINNED[base];
  if (pinned) return ALL_ROLES.find((r) => r.id === pinned) ?? GENERAL;
  for (const { role, words } of ROLES) if (words.test(base)) return role;
  if (description) for (const { role, words } of ROLES) if (words.test(description)) return role;
  return GENERAL;
}

export const ALL_ROLES: Role[] = [...ROLES.map((r) => r.role), GENERAL];

// ---------- ordering (per viewer, browser storage; a convenience, not data) ----------

export type OrderMode = "abc" | "custom";
const MODE_KEY = "dduck.agentOrder.mode";
const CUSTOM_KEY = "dduck.agentOrder.custom";

export function loadOrder(): { mode: OrderMode; custom: string[] } {
  try {
    const mode = localStorage.getItem(MODE_KEY) === "custom" ? "custom" : "abc";
    const raw = JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? "[]");
    const custom = Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string").slice(0, 500) : [];
    return { mode, custom };
  } catch {
    return { mode: "abc", custom: [] };
  }
}

export function saveOrder(mode: OrderMode, custom: string[]) {
  try {
    localStorage.setItem(MODE_KEY, mode);
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(custom));
  } catch {
    // Storage unavailable: the order simply is not remembered.
  }
}

/**
 * Active agents first (user request), then the chosen order: alphabetical, or the user's custom list with
 * unknown names appended alphabetically. "기본 Claude" (key "") leads its group.
 */
export function orderCards<T extends { name: string; key: string; active: boolean }>(cards: T[], mode: OrderMode, custom: string[]): T[] {
  const pos = new Map(custom.map((n, i) => [n, i]));
  const rank = (c: T) => (c.key === "__default" ? -1 : mode === "custom" ? (pos.get(c.name) ?? 10_000) : 0);
  return [...cards].sort((a, b) => Number(b.active) - Number(a.active) || rank(a) - rank(b) || a.name.localeCompare(b.name));
}

/** Moves `name` one step within the current visible order and returns the new custom list. */
export function moveInOrder(current: string[], name: string, delta: -1 | 1): string[] {
  const list = [...current];
  const i = list.indexOf(name);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= list.length) return list;
  [list[i], list[j]] = [list[j], list[i]];
  return list;
}
