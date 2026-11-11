// 리뷰 탭 (review-inbox, D-140): pure helpers. The backend sends card summaries only; this file widens a summary
// back into the report shape so the card's verdict line, counts and first caution come from the same code as the
// report screen (report.ts), never a second copy of its rules.
import type { Delegated, EvidenceView, InboxCriterion, InboxGroupTotal, InboxItem, InboxSummary, Report, ReportErrorCode, SessionStatus } from "./api";
import type { Pose } from "./agents";
import { cautions, criteriaRows, isEvidenceStrictType, MET_LABEL, REPORT_ERROR_TEXT, validKinds, verdict, type Caution, type CriterionRow } from "./report";
import type { TeamType } from "./team";
import { t } from "../i18n";

/** Criteria rows a card previews before "+N개 더". */
export const PREVIEW_ROWS = 3;

/** "checking": no good `claude agents` poll yet, so the app cannot tell whether the task still runs. */
export const STATUS_LINE = {
  get working() {
    return t("report.review.status.working");
  },
  get waiting() {
    return t("report.review.status.waiting");
  },
  get checking() {
    return t("report.review.status.checking");
  },
};
/** A card whose task left no goal, name or instruction (or "작업 지시 저장" was off). */
export const noGoal = () => t("report.review.noGoal");

/** A card criterion's counts as evidence items (no names): valid ones per kind, then the invalid rest. */
function itemsOf(c: InboxCriterion): EvidenceView[] {
  const v = c.verified ?? { screenshot: 0, testReport: 0, command: 0, file: 0 };
  const make = (kind: EvidenceView["kind"], n: number): EvidenceView[] => Array.from({ length: n }, () => ({ kind, label: "", caption: "", status: "valid" as const }));
  const valid = [...make("screenshot", v.screenshot), ...make("test-report", v.testReport), ...make("command", v.command), ...make("file", v.file)];
  const firstTest = valid.find((i) => i.kind === "test-report");
  if (firstTest && c.testsFailed) firstTest.failed = c.testsFailed;
  const rest = Math.max(0, (c.evidenceCount ?? 0) - valid.length);
  return [...valid, ...Array.from({ length: rest }, () => ({ kind: "file" as const, label: "", caption: "", status: "missing" as const }))];
}

/** A summary in the report's shape: empty work steps (only the count matters), no evidence names, reasons or references. */
export function asReport(s: InboxSummary): Report {
  return {
    version: s.legacy ? 1 : 2,
    legacy: !!s.legacy,
    // Counts only (no titles but the first): valid, then broken, then empty after cleaning.
    attachments: [
      ...Array.from({ length: s.attachmentValid ?? s.attachmentCount ?? 0 }, () => ({ title: "", kind: "html" as const, status: "valid" as const, empty: false })),
      ...Array.from({ length: s.attachmentBroken ?? 0 }, () => ({ title: "", kind: "html" as const, status: "missing" as const, empty: false })),
      ...Array.from({ length: s.attachmentEmpty ?? 0 }, () => ({ title: "", kind: "html" as const, status: "valid" as const, empty: true })),
    ],
    status: s.status,
    updatedAt: null,
    work: Array.from({ length: s.workCount }, () => ({ step: "", result: "", files: [] })),
    blocked: s.blocked.map((b) => ({ ...b, cause: "", resolution: "", afterStep: null })),
    decisions: s.decisions.map((d) => ({ ...d, options: [], chosen: "", reason: "", afterStep: null })),
    criteria: s.criteria.map((c) => ({ criterion: c.criterion, met: c.met, summary: c.evidence, evidence: itemsOf(c), legacy: !!s.legacy })),
    references: [],
  };
}

/** What the verdict needs of "위임 내용": the agreed done lines, or no brief. */
function delegatedOf(item: InboxItem): Delegated {
  return {
    name: null,
    brief: item.done === null ? null : { goal: item.goal ?? "", outOfScope: "", constraints: "", done: item.done },
    prompt: null,
    taskType: item.taskType,
    permissionMode: null,
    folder: item.folder,
    stored: item.stored,
    routine: false,
  };
}

export interface CardView {
  goal: string;
  /** Verdict line, status line ("일하는 중이에요"/"답을 기다려요") or why there is no report. */
  line: string;
  /** danger only when a criterion was not met (못 했어요); warn for anything to check and for report errors. */
  tone: "plain" | "warn" | "danger";
  pose: Pose;
  /** "충족 N · 일부 충족 M · 못 했어요 K", parts that are 0 left out; null without criteria. A met criterion without
   *  evidence or with failing tests is not 충족 (the "근거 없음" badge carries the first). */
  counts: string | null;
  /** Met criteria whose tests fail: "테스트 실패 N" after the counts, in danger tone (0 = not shown). */
  failedRows: number;
  /** "근거 N" (valid evidence of the role's kinds) and "근거 없음 N" with its tone; null without criteria or legacy. */
  evidence: { count: number; missing: number; strict: boolean } | null;
  /** A card without criteria: the first attachment's title and line. */
  attachment: { count: number; title: string; line: string | null } | null;
  /** First caution, or null when the verdict line already says all of it. */
  first: Caution | null;
  /** "리뷰했어요" can be pressed. */
  markable: boolean;
  /** Up to three criteria rows (mark, criterion, one line of evidence) and how many more there are. */
  preview: CriterionRow[];
  more: number;
}

const sessionOf = (item: InboxItem): SessionStatus | null => (item.running === "waiting" ? "waitingInput" : item.running === "working" ? "working" : null);

export function cardView(item: InboxItem): CardView {
  const goal = item.goal ?? noGoal();
  const markable = item.running === null;
  const base = { goal, markable, counts: null, failedRows: 0, first: null, preview: [], more: 0, evidence: null, attachment: null };
  const notReady = (code: ReportErrorCode): CardView => ({ ...base, line: REPORT_ERROR_TEXT[code], tone: code === "missing" ? "plain" : "warn", pose: "waiting" });
  if (item.report.kind === "none") {
    // D-196: nothing to check against, so no verdict and no mark; only how it ended.
    return { ...base, markable: false, line: t(`report.review.ended.${item.ended ?? "unknown"}`), tone: "plain", pose: item.ended === "failed" ? "failed" : "done" };
  }
  if (item.report.kind !== "ready") {
    if (item.running) return { ...base, line: STATUS_LINE[item.running], tone: "plain", pose: item.running === "working" ? "working" : "waiting" };
    return notReady(item.report.kind === "missing" ? "missing" : item.report.code);
  }
  const r = asReport(item.report.summary);
  const d = delegatedOf(item);
  const rows = criteriaRows(r, d);
  // A met criterion without evidence or with failing tests is not counted as met; "근거 없음" is the badge only.
  const noEv = rows.filter((c) => c.noEvidence).length;
  const parts: [string, number][] = [
    [MET_LABEL.yes, rows.filter((c) => c.met === "yes" && !c.noEvidence && !c.testsFailed).length],
    [MET_LABEL.partial, rows.filter((c) => c.met === "partial").length],
    [MET_LABEL.no, rows.filter((c) => c.met === "no").length],
  ];
  const shown = parts.filter(([, n]) => n > 0);
  const counts = shown.length ? shown.map(([m, n]) => `${m} ${n}`).join(" · ") : null;
  const failedRows = rows.filter((c) => c.met === "yes" && c.testsFailed).length;
  const kinds = validKinds(item.taskType);
  const legacy = !!item.report.summary.legacy;
  const evidence = rows.length && !legacy ? { count: r.criteria.reduce((n, c) => n + c.evidence.filter((i) => i.status === "valid" && kinds.includes(i.kind)).length, 0), missing: noEv, strict: isEvidenceStrictType(item.taskType) } : null;
  const s = item.report.summary;
  const attachment = r.criteria.length === 0 && (s.attachmentCount ?? 0) > 0 ? { count: s.attachmentCount ?? 0, title: s.firstAttachment?.title ?? "", line: s.firstAttachment?.line ?? null } : null;
  const all = cautions(r, rows, item.taskType);
  const preview = rows.slice(0, PREVIEW_ROWS);
  const more = Math.max(0, rows.length - PREVIEW_ROWS);
  if (item.running) return { goal, markable, counts, failedRows, first: all[0] ?? null, preview, more, evidence, attachment, line: STATUS_LINE[item.running], tone: "plain", pose: item.running === "working" ? "working" : "waiting" };
  const v = verdict(r, d, sessionOf(item));
  // One caution whose label the verdict line already names would only repeat it.
  const first = all.length === 1 && all[0].labels.every((l) => v.line.includes(l)) ? null : (all[0] ?? null);
  const tone = rows.some((c) => c.met === "no") || v.danger ? "danger" : v.pose === "done" ? "plain" : "warn";
  return { goal, markable, counts, failedRows, first, preview, more, evidence, attachment, line: v.line, tone, pose: v.pose };
}

export interface RoleGroup {
  key: string | null;
  label: string;
  items: InboxItem[];
  /** Matching cards in this role on every page (the server's count), at least the loaded ones. */
  total: number;
}

/** Role groups in the team config's type order, then deleted types (by key), then no type ("기본"). Items keep the
 *  backend's newest-first order. */
export function groupByRole(items: InboxItem[], types: TeamType[] | undefined, label: (key: string | null) => string, totals: InboxGroupTotal[] = []): RoleGroup[] {
  const order = (types ?? []).map((t) => t.key);
  const keys = [...new Set(items.map((i) => i.taskType))];
  const rank = (k: string | null) => (k === null ? Number.MAX_SAFE_INTEGER : order.includes(k) ? order.indexOf(k) : order.length);
  keys.sort((a, b) => rank(a) - rank(b) || String(a).localeCompare(String(b)));
  return keys.map((key) => {
    const mine = items.filter((i) => i.taskType === key);
    return { key, label: label(key), items: mine, total: Math.max(mine.length, totals.find((t) => t.taskType === key)?.total ?? 0) };
  });
}

/** Badge text for screen readers. */
export const badgeLabel = { tasks: (n: number) => t("report.review.badge.tasks", { count: n }), review: (n: number) => t("report.review.badge.review", { count: n }) };
