// Display rules for routines: schedule sentences, run status names and reasons. Kept pure for Vitest.
import type { RoutineMode, Schedule } from "./api";
import { t } from "../i18n";

const WEEKDAY_KEYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;

/** Monday-first day names in the app language (bit i of `weekdays` = day i). Call at render time. */
export function weekdayNames(): string[] {
  return WEEKDAY_KEYS.map((k) => t(`routines.weekday.${k}`));
}

/** "월·금" / "Mon, Fri" for a weekday bit set. */
export function weekdaysText(bits: number): string {
  return weekdayNames()
    .filter((_, i) => bits & (1 << i))
    .join(t("routines.schedule.daySep"));
}

export const hm = (h: number, m: number) => `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;

/** Same sentence the Rust side shows in the approval dialog (routines::Schedule::describe), in the app language. */
export function scheduleText(s: Schedule): string {
  switch (s.kind) {
    case "daily":
      return t("routines.schedule.daily", { time: hm(s.hour, s.minute) });
    case "weekly":
      return t("routines.schedule.weekly", { days: weekdaysText(s.weekdays), time: hm(s.hour, s.minute) });
    case "interval":
      return t("routines.schedule.interval", { count: s.hours });
  }
}

const STATUS_KEY = {
  starting: "starting",
  start_failed: "startFailed",
  running: "running",
  waiting: "waiting",
  done: "done",
  failed: "failed",
  stopped: "stopped",
  missed: "missed",
  skipped: "skipped",
  lost: "lost",
} as const;

const REASON_KEY = {
  busy: "busy",
  concurrency: "concurrency",
  notApproved: "notApproved",
  folderChanged: "folderChanged",
  invalidRoutine: "invalidRoutine",
  guardUnreadable: "guardUnreadable",
  guardRefused: "guardRefused",
  untrustedWorkspace: "untrustedWorkspace",
  interrupted: "interrupted",
  spawn: "spawn",
  timeout: "timeout",
  failed: "failed",
  unsupportedOutput: "unsupportedOutput",
  invalidInput: "invalidInput",
  login_required: "loginRequired",
} as const;

const own = <T extends object>(o: T, k: string): k is Extract<keyof T, string> => Object.prototype.hasOwnProperty.call(o, k);

/** "시작 실패(신뢰되지 않은 폴더)" etc. Unknown statuses fall back to the raw value rather than hiding it. */
export function runStatusText(status: string, error: string | null): string {
  const label = own(STATUS_KEY, status) ? t(`routines.status.${STATUS_KEY[status]}`) : status;
  const reason = error ? (own(REASON_KEY, error) ? t(`routines.reason.${REASON_KEY[error]}`) : error) : null;
  return reason ? t("routines.status.withReason", { label, reason }) : label;
}

/** Tone for the status chip: review-worthy outcomes stand out, finished ones stay quiet. */
export function runTone(status: string): "attention" | "active" | "quiet" | "danger" {
  if (status === "waiting") return "attention";
  if (status === "running" || status === "starting") return "active";
  if (status === "start_failed" || status === "failed" || status === "lost") return "danger";
  return "quiet";
}

/** Labels and hints come from `modeLabel` / `modeHint` at render time. */
export const ROUTINE_MODES: { id: RoutineMode; warn?: boolean }[] = [{ id: "plan" }, { id: "manual" }, { id: "acceptEdits", warn: true }, { id: "dontAsk", warn: true }];

const isMode = (m: string): m is RoutineMode => ROUTINE_MODES.some((x) => x.id === m);

export function modeLabel(m: string): string {
  return isMode(m) ? t(`routines.mode.${m}.label`) : m;
}

export function modeHint(m: string): string {
  return isMode(m) ? t(`routines.mode.${m}.hint`) : "";
}

/** Form-side checks mirroring RoutineInput::validate, so mistakes show before the native dialog. */
export function validateRoutineForm(f: { name: string; cwd: string; prompt: string; schedule: Schedule }): string | null {
  if (!f.name.trim()) return t("routines.validate.name");
  if (f.name.trim().length > 64) return t("routines.validate.nameLong");
  if (!f.cwd.trim()) return t("routines.validate.cwd");
  if (!f.prompt.trim()) return t("routines.validate.prompt");
  const s = f.schedule;
  if (s.kind === "interval" && (!Number.isInteger(s.hours) || s.hours < 1 || s.hours > 24)) return t("routines.validate.interval");
  // Integer checks too: comparisons with undefined/NaN are always false and would let a broken time through.
  if (s.kind !== "interval" && (!Number.isInteger(s.hour) || !Number.isInteger(s.minute) || s.hour < 0 || s.hour > 23 || s.minute < 0 || s.minute > 59))
    return t("routines.validate.time");
  if (s.kind === "weekly" && s.weekdays === 0) return t("routines.validate.weekdays");
  return null;
}

export function relativeNext(ms: number | null, now = Date.now()): string {
  if (ms == null) return "-";
  const d = ms - now;
  if (d <= 60_000) return t("routines.next.soon");
  const min = Math.round(d / 60_000);
  if (min < 60) return t("routines.next.minutes", { count: min });
  const h = Math.round(min / 60);
  if (h < 48) return t("routines.next.hours", { count: h });
  return t("routines.next.days", { count: Math.round(h / 24) });
}
