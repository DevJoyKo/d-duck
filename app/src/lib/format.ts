import { messageOf, type AgentSession, type BoardCard, type CliError, type Lane, type SessionStatus, type TaskTag } from "./api";
import { currentLang, currentLocale } from "../i18n/language";
import { t, translate, type MessageKey } from "../i18n";

export function compact(n: number): string {
  if (Math.abs(n) >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (Math.abs(n) >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (Math.abs(n) >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(n);
}

export function usd(n: number): string {
  if (n === 0) return "$0";
  if (n >= 100) return `$${n.toFixed(0)}`;
  if (n >= 0.01) return `$${n.toFixed(2)}`;
  return "<$0.01";
}

/** Axis ticks: whole dollars when the scale allows it. */
export function usdTick(n: number): string {
  return n >= 1 || n === 0 ? `$${Math.round(n)}` : `$${n.toFixed(2)}`;
}

export function pct(n: number): string {
  return `${n.toFixed(1)}%`;
}

/** List time in the app language ("11. 09. 오후 08:19" / "11/09, 08:19 PM"). `locale` defaults to the current one. */
export function dateTime(ms: number, locale: string = currentLocale()): string {
  if (!ms) return "-";
  return new Date(ms).toLocaleString(locale, { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

/** Time of day in the app language ("오후 08:19" / "08:19 PM"). */
export function timeOfDay(ms: number, locale: string = currentLocale()): string {
  return ms ? new Date(ms).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }) : "";
}

/** Fixed 24-hour "HH:MM" whatever the language: routine schedules and time pickers stay 24-hour (app-language). */
export function clock24(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Grouped number in the app language ("12,345"). */
export function num(n: number, locale: string = currentLocale()): string {
  return new Intl.NumberFormat(locale).format(n);
}

const RELATIVE_STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["second", 60],
  ["minute", 60],
  ["hour", 24],
  ["day", 7],
  ["week", 4.35],
  ["month", 12],
  ["year", Infinity],
];

/** "3분 전" / "3 minutes ago" (and "3분 후" / "in 3 minutes" for the future), in the largest whole unit. */
export function relativeTime(ms: number, now: number = Date.now(), locale: string = currentLocale()): string {
  let v = (ms - now) / 1000;
  for (const [unit, size] of RELATIVE_STEPS) {
    if (Math.abs(v) < size) return new Intl.RelativeTimeFormat(locale, { numeric: "always" }).format(Math.trunc(v), unit);
    v /= size;
  }
  return "";
}

export function sessionLabel(title: string | null, sessionId: string): string {
  return title?.trim() ? title : sessionId.slice(0, 8);
}

/** One name per state, used everywhere (cards, detail, tray tooltip copy). */
/** A record whose values are read in the current language on every access (never frozen at module load). */
export function localized<K extends string>(keys: Record<K, MessageKey>): Record<K, string> {
  const out = {} as Record<K, string>;
  for (const k of Object.keys(keys) as K[]) Object.defineProperty(out, k, { get: () => translate(currentLang(), keys[k]), enumerable: true });
  return out;
}

// task-lifecycle: "done" is the duck finishing; the task is not 완료 until it is reviewed and wrapped up.
export const STATUS_LABEL: Record<SessionStatus, string> = localized({
  working: "tasks.status.working",
  waitingInput: "tasks.status.waitingInput",
  idle: "tasks.status.idle",
  done: "tasks.status.done",
  failed: "tasks.status.failed",
  stopped: "tasks.status.stopped",
  unknown: "tasks.status.unknown",
});

/** `/Users/me/Agents/dev_team` → `…/Agents/dev_team`: keeps the project folder visible in narrow cards. */
export function shortPath(p: string): string {
  const parts = p.split("/").filter(Boolean);
  return parts.length <= 2 ? p : `…/${parts.slice(-2).join("/")}`;
}

/** Board columns in workflow order (D-042, task-lifecycle). "review" (확인 필요) is what needs a person: waiting for
 * input, or failed without a report to review. A finished task waits in "reviewWait" (the review badge's rows), then
 * "wrapup" (정리) until "정리 끝" takes it off the board. There is no "완료" column. */
export type Group = "queued" | "running" | "review" | "reviewWait" | "wrapup";

export const GROUP_ORDER: Group[] = ["queued", "running", "review", "reviewWait", "wrapup"];

export const GROUP_LABEL: Record<Group, string> = localized({
  queued: "tasks.group.queued",
  running: "tasks.group.running",
  review: "tasks.group.review",
  reviewWait: "tasks.group.reviewWait",
  wrapup: "tasks.group.wrapup",
});

/** Over, by the review tab's rule: done, failed, stopped and unknown (report.ts `isEnded`). */
export function endedStatus(s: SessionStatus): boolean {
  return s === "done" || s === "failed" || s === "stopped" || s === "unknown";
}

/** The column of a listed session; `null` = wrapped up (off the board). `lane` comes from the backend's rule. */
export function groupOf(s: SessionStatus, lane?: Lane): Group | null {
  if (s === "working") return "running";
  if (s === "waitingInput") return "review";
  if (s === "idle") return "queued";
  if (lane === "reviewWait") return "reviewWait";
  if (lane === "done") return null;
  if (lane === "wrapup") return "wrapup";
  return s === "failed" ? "review" : "wrapup";
}

/** The state line of a card: never "완료" before 정리 끝. */
export function statusLabel(s: SessionStatus, tag?: Pick<TaskTag, "lane" | "reviewed">): string {
  if (!endedStatus(s)) return STATUS_LABEL[s];
  switch (tag?.lane) {
    case "reviewWait":
      return t(s === "failed" ? "tasks.statusLine.failedReviewWait" : s === "stopped" ? "tasks.statusLine.stoppedReviewWait" : "tasks.statusLine.doneReviewWait");
    case "wrapup":
      return t(!tag.reviewed ? "tasks.statusLine.unreviewedWrapup" : s === "failed" ? "tasks.statusLine.failedReviewedWrapup" : "tasks.statusLine.reviewedWrapup");
    case "done":
      return t("tasks.statusLine.wrapped");
    default:
      return STATUS_LABEL[s];
  }
}

/** The state on a board card: short, and without the lane its column already names ("· 리뷰 대기", 3.2 review; the
 * 진행 중 column's card says 일하는 중, not 진행 중 again). */
export function cardStatusLabel(s: SessionStatus, tag?: Pick<TaskTag, "lane" | "reviewed">): string {
  if (s === "working" || s === "idle") return t(`tasks.statusShort.${s}`);
  if (!endedStatus(s)) return STATUS_LABEL[s];
  switch (tag?.lane) {
    case "reviewWait":
      return t(s === "failed" ? "tasks.statusShort.failed" : s === "stopped" ? "tasks.statusShort.stopped" : "tasks.statusShort.done");
    case "wrapup":
      // A reviewed failure keeps saying it failed (5.2).
      return t(!tag.reviewed ? "tasks.statusShort.unreviewed" : s === "failed" ? "tasks.statusShort.failedReviewed" : "tasks.statusShort.reviewed");
    case "done":
      return t("tasks.statusLine.wrapped");
    default:
      return STATUS_LABEL[s];
  }
}

/** The state on a record card (a task the CLI no longer lists): how it last ended, kept by the backend
 * (`ended_status`); unknown → "목록에서 사라진 작업", never a guessed "끝냈어요". */
export function recordStatusLabel(c: Pick<BoardCard, "ended" | "reviewed">, lane: "reviewWait" | "wrapup"): string {
  if (lane === "wrapup") return t(!c.reviewed ? "tasks.statusShort.unreviewed" : c.ended === "failed" ? "tasks.statusShort.failedReviewed" : "tasks.statusShort.reviewed");
  switch (c.ended) {
    case "done":
      return t("tasks.statusShort.done");
    case "failed":
      return t("tasks.statusShort.failed");
    case "stopped":
      return t("tasks.statusShort.stopped");
    default:
      return t("tasks.wrapup.recordGone");
  }
}

/** Live columns in workflow order, newest first inside each (local-agent-tasks: 상태 그룹과 정렬). */
export function groupSessions(list: AgentSession[], tags?: Record<string, TaskTag>): { group: Group; sessions: AgentSession[] }[] {
  return GROUP_ORDER.map((group) => ({
    group,
    sessions: list.filter((s) => groupOf(s.status, tags?.[s.key]?.lane) === group).sort((a, b) => b.startedAt - a.startedAt),
  }));
}

/** Logs, stop and attach need a background session with a short id. */
export function canControl(s: AgentSession): boolean {
  return s.kind === "background" && !!s.id;
}

export function cliErrorMessage(e: unknown): string {
  return messageOf(e);
}

/** stderr tail for display only (kept in memory, never stored). */
export function cliErrorDetail(e: unknown): string | null {
  const err = e as CliError | undefined;
  if (err && (err.kind === "failed" || err.kind === "untrustedWorkspace")) return err.stderr_tail || null;
  return null;
}

/** Mirrors the backend's name rules so the form can explain problems before submitting. */
export function validateTaskName(name: string): string | null {
  if (!name) return null;
  if ([...name].length > 64) return t("tasks.taskName.tooLong");
  if (name.startsWith("-")) return t("tasks.taskName.dash");
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(name)) return t("tasks.taskName.control");
  return null;
}

export const DANGEROUS_MODES = new Set(["bypassPermissions", "auto"]);
