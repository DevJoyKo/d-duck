// News tab helpers (news-tab, D-204): the "Earlier" split and each category's thumbnail. Rust already sorts the
// list (date, then numeric version); this file never reorders it.
import type { NewsCategory, NewsItem } from "./api";
import { DUCK_COLORS, type DuckColor } from "../components/DuckAvatar";

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
  other: 2, // lime
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
