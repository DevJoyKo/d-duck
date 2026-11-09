// Per-tab project (project-switcher, D-127, replacing D-111's app-wide pick): typed wrappers, picker ordering and the shared "unsaved edits"
// registry. The webview only ever holds opaque ids from `projects_known`; Rust resolves every folder again.
import { invoke } from "@tauri-apps/api/core";
import { DISCARD_TEXT } from "./rules";
import { translate, type MessageKey } from "../i18n";
import { currentLang } from "../i18n/language";

export interface KnownProject {
  id: string;
  name: string;
  parentDir: string | null;
  path: string | null;
  folder: "ok" | "missing" | "unknown";
  isHome: boolean;
  /** Why folder-based screens cannot use it (home, folder unknown or gone). */
  offReason: string | null;
  /** Fixed code of `offReason` (`home` | `noHome` | `unknown` | `missing`), worded by `offReasonText`. */
  offCode?: string | null;
  memory: boolean;
  memories: number;
  rules: number;
  lastActive: number | null;
  worktreeOf: string | null;
  empty: boolean;
  /** In a "원격 push 막기" project (delivery-guard): the picker shows "로컬만". */
  localOnly?: boolean;
}

export interface ProjectsPayload {
  projects: KnownProject[];
  /** Each project tab's own pick (D-127). Missing = follow `recent[0]`; null = "프로젝트 없음" on purpose. */
  pages: Partial<Record<ProjectPage, string | null>>;
  recent: string[];
}

/** Tabs that pick their own project (D-127); every other page has no project bar. */
/** `review` (D-140) starts on 모든 프로젝트 with a screen-local scope, like 확장 and MCP. */
export const PROJECT_PAGES = ["permissions", "memory", "extensions", "mcp", "review"] as const;
export type ProjectPage = (typeof PROJECT_PAGES)[number];

export function projectPageOf(page: string): ProjectPage | null {
  return (PROJECT_PAGES as readonly string[]).includes(page) ? (page as ProjectPage) : null;
}

/** The id a tab uses: its own pick, else the most recently used project. */
export function pageProjectId(data: Pick<ProjectsPayload, "pages" | "recent"> | undefined, page: ProjectPage | null): string | null {
  if (!data || !page) return null;
  return page in data.pages ? (data.pages[page] ?? null) : (data.recent[0] ?? null);
}

export const projectsApi = {
  known: () => invoke<ProjectsPayload>("projects_known"),
  setPage: (page: ProjectPage, id: string | null) => invoke<void>("set_page_project", { page, id }),
};

export type ScopeNeed = "folder" | "memory";

/** Rust's fixed off-reason codes (dduck-core `projects::off_code`, delivery `homeNotAllowed`) → app wording. */
const OFF_CODE_KEY: Record<string, MessageKey> = {
  homeNotAllowed: "tasks.scope.homeOff",
  home: "tasks.scope.homeReason",
  noHome: "tasks.scope.noHomeReason",
  unknown: "tasks.scope.unknownReason",
  missing: "tasks.delivery.missing",
};

/** An off reason in the app language: by its `code`, else the backend sentence as sent. */
export function offReasonText(code: string | null | undefined, raw: string | null = null): string | null {
  const key = code ? OFF_CODE_KEY[code] : undefined;
  return key ? translate(currentLang(), key) : raw;
}

/** Why this project cannot be used where `need` applies; null = fine. Memory works without a folder and for home. */
export function offFor(p: KnownProject | null, need: ScopeNeed): string | null {
  if (!p) return null;
  if (need === "memory") return null;
  return offReasonText(p.offCode, p.offReason);
}

export interface PickerRow {
  p: KnownProject;
  /** Worktree shown under its parent. */
  child: boolean;
  /** Worktrees folded under this row. */
  children: number;
  section: "recent" | "all";
}

/**
 * Picker rows: recent picks first, then the rest (most recently active first, folder-less last). Worktrees fold under
 * their parent unless `expanded` has the parent or a search is typed. Empty projects only when `showEmpty`; the
 * current and recent ones always show.
 */
export function pickerRows(list: KnownProject[], recent: string[], current: string | null, query: string, showEmpty: boolean, expanded: Set<string> = new Set()): PickerRow[] {
  const q = query.trim().toLowerCase();
  const keep = (p: KnownProject) => p.id === current || recent.includes(p.id) || showEmpty || !p.empty;
  const match = (p: KnownProject) => !q || p.name.toLowerCase().includes(q) || (p.parentDir ?? "").toLowerCase().includes(q);
  const visible = list.filter((p) => keep(p) && match(p));
  const ids = new Set(visible.map((p) => p.id));
  const kids = (id: string) => visible.filter((c) => c.worktreeOf === id);
  const rank = (p: KnownProject) => (p.folder === "ok" ? 0 : 1);
  const recentRows = recent.map((id) => visible.find((p) => p.id === id)).filter((p): p is KnownProject => !!p && !(p.worktreeOf && ids.has(p.worktreeOf) && !q));
  const seen = new Set(recentRows.map((p) => p.id));
  const rest = visible.filter((p) => !seen.has(p.id)).sort((a, b) => rank(a) - rank(b));
  const out: PickerRow[] = [];
  const push = (p: KnownProject, section: PickerRow["section"]) => {
    const c = q ? [] : kids(p.id);
    out.push({ p, child: false, children: c.length, section });
    if (expanded.has(p.id)) out.push(...c.map((k) => ({ p: k, child: true, children: 0, section })));
  };
  recentRows.forEach((p) => push(p, "recent"));
  for (const p of rest) {
    // A worktree sits under its parent (folded) unless a search is typed.
    if (!q && p.worktreeOf && ids.has(p.worktreeOf)) continue;
    push(p, "all");
  }
  return out;
}

// ---------- unsaved edits (one registry for project switching, sidebar navigation and reloads) ----------

// Same wording as the rules editor's own guard, so there is one message.
export { DISCARD_TEXT };
const dirty = new Map<string, boolean>();

export function setDirty(key: string, d: boolean) {
  if (d) dirty.set(key, true);
  else dirty.delete(key);
}

export function anyDirty(): boolean {
  return dirty.size > 0;
}

/** True when it is fine to leave: nothing unsaved, or the user agreed to drop it (then the registry is cleared). */
export function confirmLeave(ask: (q: string) => boolean = (q) => window.confirm(q)): boolean {
  if (!anyDirty()) return true;
  if (!ask(DISCARD_TEXT)) return false;
  dirty.clear();
  return true;
}
