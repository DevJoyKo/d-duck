// Page ids and the one place old ids are mapped (nav-ia). Skills and plugins became one "확장" page; remembered
// pages, `?page=`, usage-insight links, tour cards and the checklist may still say `skills` or `plugins`.

export type Page = "agents" | "tasks" | "review" | "routines" | "artifacts" | "extensions" | "mcp" | "permissions" | "memory" | "usage" | "settings";
export type LegacyPage = "skills" | "plugins";
/** Anything a link may ask for; `normalizePage` turns it into a real page. */
export type NavTarget = Page | LegacyPage;
export type ExtChip = "all" | "skill" | "command" | "workflow" | "plugin";

export const PAGES: readonly Page[] = ["agents", "tasks", "review", "routines", "artifacts", "extensions", "mcp", "permissions", "memory", "usage", "settings"];
const LEGACY: Record<LegacyPage, { page: Page; chip: ExtChip }> = {
  skills: { page: "extensions", chip: "skill" },
  plugins: { page: "extensions", chip: "plugin" },
};

/** A known page (with the 확장 filter chip an old id implied), or null for anything else. Static allow-list only. */
export function normalizePage(raw: unknown): { page: Page; chip: ExtChip | null } | null {
  if (typeof raw !== "string") return null;
  if ((PAGES as readonly string[]).includes(raw)) return { page: raw as Page, chip: null };
  if (Object.prototype.hasOwnProperty.call(LEGACY, raw)) return LEGACY[raw as LegacyPage];
  return null;
}
