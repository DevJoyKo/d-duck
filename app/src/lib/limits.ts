// Usage-limit stops on the tasks board (usage-limit-resume). Rust sends ids, times, labels and fixed codes; the
// one-line copy for each code lives here.
import type { AgentSession, LimitEntry, LimitReason, LimitView } from "./api";
import { t, type MessageKey } from "../i18n";
import { localized } from "./format";
import { currentLang, currentLocale } from "../i18n/language";

/** A reset or start time. Korean keeps Rust's label (its exact wording, with the date and weekday past 24 hours);
 * other languages word it here from the ms field, so a language change never waits for the backend. */
export function clockLabel(ms: number | null | undefined, label: string | null | undefined, now: number = Date.now()): string {
  if (currentLang() === "ko" || ms == null) return label ?? "";
  const far = ms - now > 24 * 3600 * 1000;
  return new Date(ms).toLocaleString(currentLocale(), far ? { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" } : { hour: "numeric", minute: "2-digit" });
}

/** Why a stop needs the user, one line each (해요체). Closed reasons are never shown. */
export const LIMIT_REASON_TEXT: Partial<Record<LimitReason, string>> = localized({
  not_dduck: "tasks.limits.reason.not_dduck",
  no_reset: "tasks.limits.reason.no_reset",
  opt_out: "tasks.limits.reason.opt_out",
  cap: "tasks.limits.reason.cap",
  deploy: "tasks.limits.reason.deploy",
  remote: "tasks.limits.reason.remote",
  danger: "tasks.limits.reason.danger",
  auto_off: "tasks.limits.reason.auto_off",
  not_shipped: "tasks.limits.reason.not_shipped",
  cwd_mismatch: "tasks.limits.reason.cwd_mismatch",
  untrusted: "tasks.limits.reason.untrusted",
  link: "tasks.limits.reason.link",
  risk_changed: "tasks.limits.reason.risk_changed",
  copy_started: "tasks.limits.reason.copy_started",
  cli_failed: "tasks.limits.reason.cli_failed",
  attached: "tasks.limits.reason.attached",
  interrupted: "tasks.limits.reason.interrupted",
  session_mismatch: "tasks.limits.reason.session_mismatch",
  late: "tasks.limits.reason.late",
  guard_hook_missing: "tasks.limits.reason.guard_hook_missing",
  guard_unreadable: "tasks.limits.reason.guard_unreadable",
  local_only_refused: "tasks.limits.reason.local_only_refused",
} satisfies Partial<Record<LimitReason, MessageKey>>);

/** "지금 이어서", with the task's name when several are listed. Never an id: no name → no suffix. */
export function resumeLabel(e: Pick<LimitEntry, "name">, many: boolean): string {
  return many && e.name ? t("tasks.limits.resumeNamed", { name: e.name }) : t("tasks.limits.resume");
}

/** Labels for several buttons side by side: name (Rust falls back goal → folder), then the start time when two
 * would still read the same. Never an id. */
export function resumeLabels(list: (Pick<LimitEntry, "id" | "name" | "startedLabel"> & Partial<Pick<LimitEntry, "startedAt">>)[]): Map<number, string> {
  const base = new Map(list.map((e) => [e.id, resumeLabel(e, list.length > 1)]));
  const count = (l: string) => [...base.values()].filter((x) => x === l).length;
  return new Map(
    list.map((e) => {
      const l = base.get(e.id) as string;
      const started = clockLabel(e.startedAt, e.startedLabel, e.startedAt ?? undefined);
      return [e.id, list.length > 1 && count(l) > 1 && started ? `${l} · ${started}` : l];
    }),
  );
}

const OPEN: LimitEntry["state"][] = ["auto", "manual", "needs_choice", "starting"];

/** The stop shown on a session's card: an open one first, else one that just resumed. */
export function entryFor(view: LimitView | undefined, s: Pick<AgentSession, "sessionId">): LimitEntry | undefined {
  if (!view || !s.sessionId) return undefined;
  const mine = view.entries.filter((e) => e.sessionId === s.sessionId);
  return mine.find((e) => OPEN.includes(e.state)) ?? mine.find((e) => e.state === "resumed");
}

export interface LimitChip {
  text: string;
  /** The time inside `text`, so the line can keep it whole and shorten the words around it. */
  when?: string;
  /** Extra line under the chip (why it needs the user). */
  note?: string;
  tone: "wait" | "manual" | "done";
  canResume: boolean;
  canSkip: boolean;
}

/** Card chip for one stop. `null` when there is nothing to show. */
export function limitChip(e: LimitEntry | undefined): LimitChip | null {
  if (!e) return null;
  // D.Duck runs only tasks it started; deploy tasks are continued in the terminal.
  const resumable = !!e.bgId && e.reason !== "not_dduck" && e.reason !== "deploy";
  switch (e.state) {
    case "auto": {
      const when = clockLabel(e.dueAt, e.dueLabel) || clockLabel(e.resetsAt, e.label);
      return { text: t("tasks.limits.auto", { when }), when: when || undefined, note: t("tasks.limits.autoNote"), tone: "wait", canResume: false, canSkip: true };
    }
    case "starting":
      return { text: t("tasks.limits.starting"), tone: "wait", canResume: false, canSkip: false };
    case "resumed":
      return { text: t("tasks.limits.resumed"), tone: "done", canResume: false, canSkip: false };
    case "needs_choice":
      return { text: t("tasks.limits.reason.late"), tone: "manual", canResume: resumable, canSkip: true };
    case "manual": {
      const when = clockLabel(e.resetsAt, e.label);
      const head = when ? t("tasks.limits.manual", { when }) : t("tasks.limits.reached");
      return { text: head, when: when || undefined, note: (e.reason && LIMIT_REASON_TEXT[e.reason]) || t("tasks.limits.manualNote"), tone: "manual", canResume: resumable, canSkip: true };
    }
    default:
      return null;
  }
}

/** The reset time inside the first banner line (kept whole on screen while the words around it shorten). */
export function bannerTime(view: LimitView | undefined): string {
  return view?.banner ? clockLabel(view.banner.resetsAt, view.banner.label) : "";
}

/** Banner lines above the board: the coming reset, late wake-ups, and stops with an unknown reset. */
export function bannerLines(view: LimitView | undefined): string[] {
  if (!view) return [];
  const lines: string[] = [];
  if (view.banner) {
    // Worded here from the counts (the backend's `text` is its fallback in its own language).
    const b = view.banner;
    const counts = [b.auto > 0 && t("tasks.limits.bannerAuto", { count: b.auto }), b.manual > 0 && t("tasks.limits.bannerManual", { count: b.manual }), b.auto > 0 && t("tasks.limits.autoNote")].filter(Boolean);
    lines.push(counts.length ? t("tasks.limits.banner", { time: clockLabel(b.resetsAt, b.label), counts: counts.join(" · ") }) : b.text);
  }
  if (view.late > 0) lines.push(t("tasks.limits.bannerLate", { count: view.late }));
  if (view.unknown > 0) lines.push(t("tasks.limits.bannerUnknown", { count: view.unknown }));
  return lines;
}
