// News tab helpers (news-tab, D-204): the "Earlier" split and each category's thumbnail. Rust already sorts the
// list (date, then numeric version); this file never reorders it.
import type { NewsCategory, NewsItem } from "./api";
import type { ProjectsPayload } from "./projects";
import { DUCK_COLORS, type DuckColor } from "../components/DuckAvatar";
import { splitPath } from "./platform";

/** Items dated more than this many days ago go under "이미 나온 기능". */
export const EARLIER_DAYS = 30;
const DAY_MS = 86_400_000;

/** Local midnight of a `YYYY-MM-DD` date (NaN for anything else). */
export function dayMs(date: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  return m ? new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])).getTime() : NaN;
}

/** Splits the sorted list into recent and earlier, keeping the order inside each. */
export function splitEarlier(items: NewsItem[], now: number = Date.now()): { recent: NewsItem[]; earlier: NewsItem[] } {
  const cut = now - EARLIER_DAYS * DAY_MS;
  const recent: NewsItem[] = [];
  const earlier: NewsItem[] = [];
  for (const it of items) (dayMs(it.date) < cut ? earlier : recent).push(it);
  return { recent, earlier };
}

/** Category → palette slot. Kept apart from the task-type colours (roleColors.ts) on purpose (design review LOW). */
const CATEGORY_COLOR_INDEX: Record<NewsCategory, number> = {
  model: 3, // lavender
  command: 1, // sky
  setting: 9, // slate
  agent: 7, // mint
  integration: 5, // teal
  ide: 8, // lilac
  security: 0, // coral
  other: 6, // pink (lime read too close to the duck's yellow)
};

export function categoryColor(c: NewsCategory): DuckColor {
  return DUCK_COLORS[CATEGORY_COLOR_INDEX[c] ?? 2];
}

/** The host shown next to "원문 열기" (the link itself is opened by Rust from the stored value). */
export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

/** A project "내 프로젝트에 적용" can target: a real folder that isn't home (news-apply design 5). */
export interface NewsTarget {
  path: string;
  name: string;
}

/** Targets for the sheet's picker, recent picks first, and the default: the first usable recent pick, else none. */
export function newsTargets(data: Pick<ProjectsPayload, "projects" | "recent"> | undefined): { targets: NewsTarget[]; fallback: string } {
  if (!data) return { targets: [], fallback: "" };
  const usable = data.projects.filter((p) => p.path && p.folder === "ok" && !p.isHome && !p.offReason);
  const rank = (id: string) => {
    const i = data.recent.indexOf(id);
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  const sorted = [...usable].sort((a, b) => rank(a.id) - rank(b.id) || (b.lastActive ?? 0) - (a.lastActive ?? 0));
  const firstRecent = sorted.find((p) => data.recent.includes(p.id));
  // Two projects with the same name (a fork, a worktree) get their parent folder's name after them (test review L3).
  const count = (name: string) => sorted.filter((p) => p.name === name).length;
  const parent = (p: (typeof sorted)[number]) => splitPath(p.parentDir ?? "").pop() ?? "";
  const label = (p: (typeof sorted)[number]) => (count(p.name) > 1 && parent(p) ? `${p.name} · ${parent(p)}` : p.name);
  return { targets: sorted.map((p) => ({ path: p.path as string, name: label(p) })), fallback: firstRecent?.path ?? "" };
}
