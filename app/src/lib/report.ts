// 작업 보고서 (decision-report, D-108): pure helpers for the report screen. The report JSON was already checked
// in Rust (shape, counts, lengths, control characters); this file only arranges it for display.
import type { Delegated, EvidenceKind, EvidenceStatus, EvidenceView, Report, ReportBlocked, ReportCriterion, ReportDecision, ReportErrorCode, ReportPresence, ReportWork, SessionStatus } from "./api";
import type { Pose } from "./agents";
import { t, type MessageKey } from "../i18n";

/** Labels read at access time (never frozen at module load), so a language change shows on the next render. */
function lazy<K extends string>(keys: Record<K, MessageKey>): Readonly<Record<K, string>> {
  const o = {} as Record<K, string>;
  for (const k of Object.keys(keys) as K[]) Object.defineProperty(o, k, { get: () => t(keys[k] as never), enumerable: true });
  return o;
}

/** Decisions drawn on the flow; the rest stay in the table below. Flagged ones are drawn first. */
export const FLOW_DECISIONS = 7;
/** Caution chips shown before "외 N개". */
export const CAUTION_CHIPS = 5;

export const MET_MARK: Record<ReportCriterion["met"], string> = { yes: "✓", partial: "△", no: "✕" };
export const MET_LABEL = lazy<ReportCriterion["met"]>({ yes: "report.met.yes", partial: "report.met.partial", no: "report.met.no" });

export type FlagKind = "deviates" | "hard" | "assumed";
export const FLAG_LABEL = lazy<FlagKind>({ deviates: "report.flag.deviates", hard: "report.flag.hard", assumed: "report.flag.assumed" });
/** Fixed labels shared by the verdict line, caution chips and evidence badges. */
export const LABEL = lazy({
  needsUser: "report.label.needsUser",
  noEvidence: "report.label.noEvidence",
  noTestReport: "report.label.noTestReport",
  testsFailed: "report.label.testsFailed",
  legacy: "report.label.legacy",
});

/** Review and QA (team_config.rs `is_evidence_strict`): evidence is a locked rule and a missing one is danger. Not
 *  team.ts `isReviewType` (리뷰 용량). */
export const isEvidenceStrictType = (key: string | null | undefined) => key === "review" || key === "qa";

const ALL_KINDS: EvidenceKind[] = ["screenshot", "test-report", "command", "file"];
/** Evidence kinds that count for a role: review reads code (file only), QA runs things (no file). */
export function validKinds(taskType: string | null | undefined): EvidenceKind[] {
  if (taskType === "review") return ["file"];
  if (taskType === "qa") return ["screenshot", "test-report", "command"];
  return ALL_KINDS;
}

/** Types that may hand in attachments without criteria (team_config.rs `report_shape`). */
export const isFreeShape = (key: string | null | undefined) => !!key && ["research", "architecture", "comms", "learn"].includes(key);

export const KIND_LABEL = lazy<EvidenceKind>({ screenshot: "report.evidence.kind.screenshot", "test-report": "report.evidence.kind.testReport", command: "report.evidence.kind.command", file: "report.evidence.kind.file" });
const STATUS_TEXT = lazy<Exclude<EvidenceStatus, "valid">>({
  missing: "report.evidence.status.missing",
  refused: "report.evidence.status.refused",
  tooLarge: "report.evidence.status.tooLarge",
  badType: "report.evidence.status.badType",
  tooMany: "report.evidence.status.tooMany",
});
export const EVIDENCE_STATUS_TEXT: Readonly<Record<EvidenceStatus, string>> = new Proxy({} as Record<EvidenceStatus, string>, {
  get: (_, k) => (k === "valid" ? "" : STATUS_TEXT[k as Exclude<EvidenceStatus, "valid">]),
});

/** A decision's flags, most important first. */
export function flagsOf(d: ReportDecision): FlagKind[] {
  const f: FlagKind[] = [];
  if (d.deviatesFromBrief) f.push("deviates");
  if (d.reversible === "hard") f.push("hard");
  if (d.assumed) f.push("assumed");
  return f;
}

/** DOM ids of the rows a caution chip jumps to. */
export const rowId = { criterion: (i: number) => `rpt-crit-${i}`, decision: (i: number) => `rpt-dec-${i}`, blocked: (i: number) => `rpt-blk-${i}` };

export interface Caution {
  /** Row to scroll to and highlight. */
  target: string;
  labels: string[];
  text: string;
  /** Only "확인이 필요해요" and "되돌리기 어려움" are drawn in danger. */
  danger: boolean;
  met?: ReportCriterion["met"];
}

const ENDED: SessionStatus[] = ["done", "failed", "stopped", "unknown"];
export const isEnded = (s: SessionStatus | null | undefined) => !s || ENDED.includes(s);

/** Lines of the brief's 완료 기준, without list markers. */
export function doneLines(done: string | undefined | null): string[] {
  return (done ?? "")
    .split("\n")
    .map((l) => l.replace(/^\s*(?:[-*•]|\d+[.)])\s*/, "").trim())
    .filter(Boolean);
}

const norm = (s: string) => s.replace(/\s+/g, " ").trim().toLowerCase();

export interface CriterionRow {
  criterion: string;
  met: ReportCriterion["met"] | null;
  /** The summary line. */
  evidence: string;
  /** In the agreed brief (as opposed to one the agent added). */
  agreed: boolean;
  /** Index into `report.criteria` (for previews); null when the agent did not write this one. */
  index?: number | null;
  items?: EvidenceView[];
  legacy?: boolean;
  /** Met or partly met, but no valid evidence of the role's kinds (not for legacy reports). */
  noEvidence?: boolean;
  /** Failed tests in this criterion's valid test reports, when it is met. */
  testsFailed?: number;
  /** Review/QA: a missing evidence is danger. */
  strict?: boolean;
  /** Evidence kinds this role counts (review: file; QA: no file). */
  accepts?: EvidenceKind[];
}

function rowOf(r: Report, k: number, agreed: boolean, criterion: string, kinds: EvidenceKind[]): CriterionRow {
  const c = r.criteria[k];
  const items = Array.isArray(c.evidence) ? c.evidence : [];
  const legacy = !!(c.legacy || r.legacy);
  const valid = items.filter((i) => i.status === "valid" && kinds.includes(i.kind));
  const noEvidence = !legacy && (c.met === "yes" || c.met === "partial") && valid.length === 0;
  const testsFailed = c.met === "yes" ? valid.filter((i) => i.kind === "test-report").reduce((n, i) => n + (i.failed ?? 0), 0) : 0;
  return { criterion, met: c.met, evidence: c.summary ?? "", agreed, index: k, items, legacy, noEvidence, testsFailed, strict: kinds !== ALL_KINDS, accepts: kinds };
}

/** "합의 vs 결과": each agreed criterion with the agent's verdict (or none yet), then criteria the agent added.
 *  Exact (whitespace- and case-insensitive) matches first; a substring match only when it is the one candidate
 *  both ways. An empty criterion never matches. */
export function criteriaRows(r: Report, d: Delegated | null): CriterionRow[] {
  const lines = doneLines(d?.brief?.done).map(norm);
  const crit = r.criteria.map((c) => norm(c.criterion));
  const match = lines.map(() => -1);
  const used = new Set<number>();
  lines.forEach((l, li) => {
    const k = crit.findIndex((c, k) => c !== "" && !used.has(k) && c === l);
    if (k >= 0) {
      match[li] = k;
      used.add(k);
    }
  });
  const overlaps = (a: string, b: string) => a !== "" && b !== "" && (a.includes(b) || b.includes(a));
  lines.forEach((l, li) => {
    if (match[li] >= 0) return;
    const cands = crit.map((_, k) => k).filter((k) => !used.has(k) && overlaps(crit[k], l));
    if (cands.length !== 1) return;
    const k = cands[0];
    if (lines.some((l2, lj) => lj !== li && match[lj] < 0 && overlaps(crit[k], l2))) return;
    match[li] = k;
    used.add(k);
  });
  const agreedLines = doneLines(d?.brief?.done);
  const kinds = validKinds(d?.taskType);
  const rows: CriterionRow[] = agreedLines.map((line, li) => {
    const k = match[li];
    return k < 0 ? { criterion: line, met: null, evidence: "", agreed: true, index: null, items: [] } : rowOf(r, k, true, line, kinds);
  });
  r.criteria.forEach((c, k) => {
    if (!used.has(k) && crit[k] !== "") rows.push(rowOf(r, k, !d?.brief, c.criterion, kinds));
  });
  return rows;
}

export type MarkFlag = "noEvidence" | "testsFailed";
export const MARK_FLAG_LABEL = lazy<MarkFlag>({ noEvidence: "report.label.noEvidence", testsFailed: "report.label.testsFailed" });
/** How a criterion's mark is downgraded: failing tests first, then missing evidence (design HIGH). */
export function markFlag(c: CriterionRow): MarkFlag | null {
  if (c.testsFailed) return "testsFailed";
  if (c.noEvidence) return "noEvidence";
  return null;
}

/** QA finished without a valid test report anywhere ("테스트 리포트 없음"). */
export function missingTestReport(r: Report, d: Delegated | null): boolean {
  if (d?.taskType !== "qa" || r.status !== "done" || r.legacy) return false;
  return !r.criteria.some((c) => (Array.isArray(c.evidence) ? c.evidence : []).some((i) => i.kind === "test-report" && i.status === "valid"));
}

/** "먼저 확인할 것", in the same order as the verdict line (D-130): 확인이 필요해요 → 못 함 → 합의와 다름 → 일부 충족 →
 *  되돌리기 어려움 → 묻지 않고 정함. One chip per flagged decision with its flags merged, placed by its strongest flag;
 *  equal ranks keep the written order. */
export function cautions(r: Report, rows: CriterionRow[], taskType?: string | null): Caution[] {
  const strict = isEvidenceStrictType(taskType);
  const RANK = { needsUser: 0, no: 1, deviates: 2, partial: 3, hard: 4, assumed: 5 } as const;
  const NO_EV = strict ? 1.5 : 3.5;
  const out: { rank: number; c: Caution }[] = [];
  r.blocked.forEach((b, i) => {
    if (b.open && b.needsUser) out.push({ rank: RANK.needsUser, c: { target: rowId.blocked(i), labels: [LABEL.needsUser], text: b.what, danger: true } });
  });
  rows.forEach((c, i) => {
    if (c.met === "no" || c.met === "partial") out.push({ rank: RANK[c.met], c: { target: rowId.criterion(i), labels: [MET_LABEL[c.met]], text: c.criterion, danger: false, met: c.met } });
    if (c.noEvidence) out.push({ rank: NO_EV, c: { target: rowId.criterion(i), labels: [LABEL.noEvidence], text: c.criterion, danger: strict } });
    if (c.testsFailed) out.push({ rank: 6, c: { target: rowId.criterion(i), labels: [`${LABEL.testsFailed} ${c.testsFailed}`], text: c.criterion, danger: false } });
  });
  r.decisions.forEach((d, i) => {
    const f = flagsOf(d);
    if (f.length) out.push({ rank: Math.min(...f.map((k) => RANK[k])), c: { target: rowId.decision(i), labels: f.map((k) => FLAG_LABEL[k]), text: d.title, danger: f.includes("hard") } });
  });
  // Array.prototype.sort is stable.
  return out.sort((a, b) => a.rank - b.rank).map((x) => x.c);
}

/** The one line at the top of the report, the duck's pose and whether the line is danger (red). Order (design 4):
 *  확인이 필요해요 → 못 했어요 → [엄격] 근거 없음 → [QA] 테스트 리포트 없음 → 합의와 다름 → 일부 충족 → [일반] 근거 없음 →
 *  되돌리기 어려워요 → 묻지 않고 정함 → 테스트 실패 → 대조 안 함, at most three. */
export function verdict(r: Report, d: Delegated | null, session: SessionStatus | null | undefined): { line: string; pose: Pose; danger?: boolean } {
  const rows = criteriaRows(r, d);
  const n = cautions(r, rows, d?.taskType).length;
  const strict = isEvidenceStrictType(d?.taskType);
  const count = (k: string, v: number) => (v ? [`${k} ${v}`] : []);
  const waitingUser = r.blocked.filter((b) => b.open && b.needsUser).length;
  const noEv = rows.filter((c) => c.noEvidence).length;
  const noTests = missingTestReport(r, d);
  const failedTests = rows.reduce((a, c) => a + (c.testsFailed ?? 0), 0);
  const parts = [
    ...count(LABEL.needsUser, waitingUser),
    ...count(MET_LABEL.no, rows.filter((c) => c.met === "no").length),
    ...(strict ? count(LABEL.noEvidence, noEv) : []),
    ...(noTests ? [LABEL.noTestReport] : []),
    ...count(FLAG_LABEL.deviates, r.decisions.filter((x) => x.deviatesFromBrief).length),
    ...count(MET_LABEL.partial, rows.filter((c) => c.met === "partial").length),
    ...(strict ? [] : count(LABEL.noEvidence, noEv)),
    ...count(FLAG_LABEL.hard, r.decisions.filter((x) => x.reversible === "hard").length),
    ...count(FLAG_LABEL.assumed, r.decisions.filter((x) => x.assumed).length),
    ...count(LABEL.testsFailed, failedTests),
    ...count(t("report.verdict.notCompared"), r.status === "done" ? rows.filter((c) => c.agreed && c.met === null).length : 0),
  ].slice(0, 3);
  const isDanger = (strict && noEv > 0) || noTests;
  const danger = isDanger ? { danger: true } : {};
  const needsCheck = (issues: string) => t("report.verdict.needsCheck", { issues });
  const issues = parts.length ? needsCheck(parts.join(" · ")) : "";
  const attached = r.attachments?.length ?? 0;
  if (r.status === "in_progress" && !isEnded(session)) {
    const toCheck = t("report.verdict.toCheck", { count: n });
    if (waitingUser) return { line: `${t("report.verdict.waitingUser")} · ${toCheck}`, pose: "waiting" };
    return { line: `${t("report.verdict.working", { count: r.work.length })}${n ? ` · ${toCheck}` : ""}`, pose: "working" };
  }
  if (r.status === "in_progress") return { line: t("report.verdict.stopped", { detail: issues || t("report.verdict.stoppedSteps", { count: r.work.length }) }), pose: "waiting", ...danger };
  if (r.status === "failed") return { line: issues ? t("report.verdict.failedWith", { issues }) : t("report.verdict.failed"), pose: "failed", ...danger };
  // No criteria anywhere (report and brief) but attachments (D-156).
  if (rows.length === 0 && attached > 0) {
    // Broken or empty attachments are said, never hidden (design MAJOR): "첨부 4개 · 열 수 없음 1 · 빈 문서 1".
    const broken = r.attachments.filter((a) => a.status !== "valid").length;
    const empty = r.attachments.filter((a) => a.status === "valid" && a.empty).length;
    const att =
      broken || empty
        ? [t("report.verdict.attached", { count: attached }), ...count(t("report.verdict.unopenable"), broken), ...count(t("report.verdict.emptyDoc"), empty)].join(" · ")
        : t("report.verdict.attachedReports", { count: attached });
    if (!isFreeShape(d?.taskType)) return { line: `${t("report.verdict.noCriteria")} · ${needsCheck(att)}`, pose: "waiting" };
    if (broken || empty) return { line: issues ? `${att} · ${issues}` : needsCheck(att), pose: "waiting", ...danger };
    return issues ? { line: `${att} · ${issues}`, pose: "waiting", ...danger } : { line: att, pose: "done" };
  }
  if (issues) return { line: issues, pose: "waiting", ...danger };
  return { line: rows.some((c) => c.agreed) ? t("report.verdict.asAgreed") : t("report.verdict.clean"), pose: "done" };
}

export type FlowItem =
  | { kind: "step"; n: number; work: ReportWork; here: boolean }
  | { kind: "decision"; decision: ReportDecision; index: number; flagged: boolean }
  | { kind: "blocked"; blocked: ReportBlocked; index: number; here: boolean };

/** Work steps in order; decisions and blocks after the step they name (`afterStep`), in the order they were
 *  written, except that a block still open sorts last in its step (it is where the duck stands). Items without a
 *  usable `afterStep` go at the end. Up to [`FLOW_DECISIONS`] decisions are drawn, flagged ones first.
 *  The current position is the first open block (one that needs the user first), otherwise the last step while
 *  the report is not done. */
export function flow(r: Report): { items: FlowItem[]; hiddenDecisions: number } {
  const n = r.work.length;
  const at = (s: number | null) => (s != null && s >= 1 && s <= n ? s : n + 1);
  const order = r.decisions.map((d, i) => ({ i, flagged: flagsOf(d).length > 0 }));
  const pick = new Set([...order.filter((x) => x.flagged), ...order.filter((x) => !x.flagged)].slice(0, FLOW_DECISIONS).map((x) => x.i));
  const openAt = r.blocked.findIndex((b) => b.open && b.needsUser);
  const here = openAt >= 0 ? openAt : r.blocked.findIndex((b) => b.open);
  const events: { step: number; late: boolean; item: FlowItem }[] = [];
  r.decisions.forEach((d, i) => {
    if (pick.has(i)) events.push({ step: at(d.afterStep), late: false, item: { kind: "decision", decision: d, index: i, flagged: order[i].flagged } });
  });
  r.blocked.forEach((b, i) => events.push({ step: at(b.afterStep), late: b.open, item: { kind: "blocked", blocked: b, index: i, here: i === here } }));
  // Array.prototype.sort is stable: equal keys keep the written order.
  events.sort((a, b) => a.step - b.step || Number(a.late) - Number(b.late));
  const items: FlowItem[] = [];
  let e = 0;
  for (let s = 1; s <= n + 1; s++) {
    if (s <= n) items.push({ kind: "step", n: s, work: r.work[s - 1], here: here < 0 && s === n && r.status !== "done" });
    while (e < events.length && events[e].step === s) items.push(events[e++].item);
  }
  return { items, hiddenDecisions: r.decisions.length - pick.size };
}

export const REPORT_ERROR_TEXT = lazy<ReportErrorCode>({
  missing: "report.error.missing",
  tooLarge: "report.error.tooLarge",
  invalid: "report.error.invalid",
  refused: "report.error.refused",
  unknown: "report.error.unknown",
});

/** List chip text and tone, or nothing (no report for this task, or still being written). */
export function reportChip(p: ReportPresence | undefined, session: SessionStatus): { text: string; tone: "plain" | "danger" | "muted" } | null {
  if (p === "ready") return { text: t("report.chip.ready"), tone: "plain" };
  if (p === "invalid") return { text: t("report.chip.invalid"), tone: "danger" };
  if (p === "missing" && isEnded(session)) return { text: t("report.chip.missing"), tone: "muted" };
  return null;
}
