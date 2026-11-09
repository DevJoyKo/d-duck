// Mock usage-limit stops for design review (usage-limit-resume). `?limits=off` = auto resume off, `?limits=late` =
// the app was closed at the reset, `?limits=unknown` = no reset time, `?limits=weekly` = weekly limit,
// `?limits=none` = nothing. Default: two tasks resume by themselves, one needs the user.
import type { LimitEntry, LimitView } from "../lib/api";
import { backendLang, bt } from "./lang";

const param = (() => {
  try {
    return new URLSearchParams(window.location.search).get("limits");
  } catch {
    return null;
  }
})();

const H = 3_600_000;
const now = Date.now();
// Same shapes as i18n.rs clock_label, in the backend's language.
const clock = (ms: number) => {
  const d = new Date(ms);
  const h = d.getHours();
  const h12 = h % 12 === 0 ? 12 : h % 12;
  const mm = String(d.getMinutes()).padStart(2, "0");
  return backendLang() === "en" ? `${h12}:${mm} ${h < 12 ? "AM" : "PM"}` : `${h < 12 ? "오전" : "오후"} ${h12}:${mm}`;
};
const DAYS = ["일", "월", "화", "수", "목", "금", "토"];
const DAYS_EN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS_EN = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const label = (ms: number) => {
  if (ms - now <= 24 * H) return clock(ms);
  const d = new Date(ms);
  return backendLang() === "en"
    ? `${DAYS_EN[d.getDay()]}, ${MONTHS_EN[d.getMonth()]} ${d.getDate()}, ${clock(ms)}`
    : `${d.getMonth() + 1}월 ${d.getDate()}일(${DAYS[d.getDay()]}) ${clock(ms)}`;
};

let skipped = new Set<number>();

function build(): LimitView {
  const reset = param === "weekly" ? now + 3 * 24 * H : param === "late" ? now - 6 * H : now + 2 * H + 20 * 60_000;
  const auto = param !== "off" && param !== "late";
  const e = (id: number, sessionId: string, bgId: string | null, over: Partial<LimitEntry> = {}): LimitEntry => ({
    id,
    sessionId,
    bgId,
    resetsAt: reset,
    dueAt: auto ? reset + 90_000 : null,
    state: auto ? "auto" : "manual",
    reason: auto ? null : "auto_off",
    attempts: 0,
    name: ({ k4: "lint-sweep", k5: "migrate-db", k6: "release-notes" } as Record<string, string>)[sessionId] ?? null,
    startedAt: now - 5 * H,
    startedLabel: clock(now - 5 * H),
    label: label(reset),
    dueLabel: auto ? label(reset + 90_000) : null,
    ...over,
  });
  let entries: LimitEntry[] = [];
  // `?board=nolive` has no live session, so nothing waits for a reset either (5.2).
  const noLive = typeof location !== "undefined" && new URLSearchParams(location.search).get("board") === "nolive";
  if (param === "none" || noLive) entries = [];
  else if (param === "late") entries = [e(1, "k4", "9988aabb", { state: "needs_choice", reason: "late" }), e(2, "k6", "66bb44dd", { state: "needs_choice", reason: "late" })];
  else if (param === "unknown") entries = [e(1, "k4", "9988aabb", { state: "manual", reason: "no_reset", resetsAt: null, label: null, dueAt: null, dueLabel: null })];
  else entries = [e(1, "k4", "9988aabb"), e(2, "k5", "77aa55cc"), e(3, "k6", "66bb44dd", { state: "manual", reason: "danger", dueAt: null, dueLabel: null })];
  entries = entries.filter((x) => !skipped.has(x.id));
  const upcoming = entries.filter((x) => (x.state === "auto" || x.state === "manual") && x.resetsAt && x.resetsAt > now);
  const a = upcoming.filter((x) => x.state === "auto").length;
  const m = upcoming.length - a;
  const parts = [a > 0 && bt(`${a}개는 이어서 할게요`, `${a} continue on their own`), m > 0 && bt(`${m}개는 직접 이어야 해요`, `${m} need you to continue`), a > 0 && bt("Mac이 깨어 있어야 이어요", "Your Mac needs to be awake")].filter(Boolean);
  return {
    entries,
    banner: upcoming.length ? { resetsAt: reset, label: label(reset), auto: a, manual: m, text: bt(`사용량 한도 · ${label(reset)} 리셋 · ${parts.join(" · ")}`, `Usage limit · resets ${label(reset)} · ${parts.join(" · ")}`) } : null,
    late: entries.filter((x) => x.state === "needs_choice").length,
    unknown: entries.filter((x) => x.state === "manual" && x.resetsAt === null).length,
    autoShipped: true,
    autoOn: param !== "off",
  };
}

export const limitHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  limit_status: () => build(),
  limit_cancel: (a) => {
    skipped = new Set([...skipped, Number(a.id)]);
    return true;
  },
  limit_resume_now: (a) => {
    skipped = new Set([...skipped, Number(a.id)]);
    return null;
  },
  set_limit_opt_out: () => true,
};
