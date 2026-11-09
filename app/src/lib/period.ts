// Period presets → query ranges aligned to the viewer's local midnight (usage-analytics: 기간 선택).
import type { Query } from "./api";
import { t } from "../i18n";

export type Preset = "today" | "7d" | "30d" | "all" | "custom";

const PRESET_KEY = { today: "tasks.period.today", "7d": "tasks.period.d7", "30d": "tasks.period.d30", all: "tasks.period.all", custom: "tasks.period.custom" } as const;

// `label` is read in the current language on each access.
export const PRESETS: { id: Preset; label: string }[] = (["today", "7d", "30d", "all", "custom"] as const).map((id) => ({
  id,
  get label() {
    return t(PRESET_KEY[id]);
  },
}));

const DAY = 86_400_000;

function startOfLocalDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function addLocalDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

/** `custom` uses inclusive local dates `YYYY-MM-DD`. */
export function rangeFor(preset: Preset, now: Date, custom?: { from: string; to: string }): { fromMs: number; toMs: number } {
  const today = startOfLocalDay(now);
  const tomorrow = addLocalDays(today, 1).getTime();
  switch (preset) {
    case "today":
      return { fromMs: today.getTime(), toMs: tomorrow };
    case "7d":
      return { fromMs: addLocalDays(today, -6).getTime(), toMs: tomorrow };
    case "30d":
      return { fromMs: addLocalDays(today, -29).getTime(), toMs: tomorrow };
    case "all":
      return { fromMs: 0, toMs: tomorrow };
    case "custom": {
      const from = parseLocalDate(custom?.from) ?? today;
      const to = parseLocalDate(custom?.to) ?? today;
      const [a, b] = from <= to ? [from, to] : [to, from];
      return { fromMs: a.getTime(), toMs: addLocalDays(b, 1).getTime() };
    }
  }
}

export function parseLocalDate(s?: string): Date | null {
  const m = s?.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isNaN(d.getTime()) ? null : d;
}

export function toLocalDateInput(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function buildQuery(
  preset: Preset,
  now: Date,
  filters: { projects: string[]; models: string[] },
  custom?: { from: string; to: string },
): Query {
  return {
    ...rangeFor(preset, now, custom),
    // getTimezoneOffset is minutes *behind* UTC; the backend wants the UTC offset (KST = +540).
    tzOffsetMinutes: -now.getTimezoneOffset(),
    projects: filters.projects,
    models: filters.models,
  };
}

export const DAY_MS = DAY;

export function msUntilNextLocalMidnight(now: Date): number {
  return addLocalDays(startOfLocalDay(now), 1).getTime() - now.getTime();
}
