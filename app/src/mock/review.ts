// Mock 리뷰 탭 (review-inbox) for design review: the task board's report records plus a finished QA and design task,
// one record from before the tab existed (only under 전체), a running and a waiting task (전체 only), a reviewed one,
// a missing and a malformed report. `?review=empty` shows no records, `?report=off` the report setting turned off.
// Summaries are cut the same way Rust does (verdict fields only). Generic names only.
import { mockProjectPath } from "./projects";
import { mockTaskReport } from "./report";
import { EVIDENCE_RECORDS, mockEvidenceReport, mockFirstAttachment, normalize } from "./evidence";

const day = 86_400_000;
const now = Date.now();
// Read on every call: tests change the URL without reloading this module.
const param = (k: string) => (typeof location === "undefined" ? null : new URLSearchParams(location.search).get(k));
const empty = () => param("review") === "empty";
const reportOff = () => param("report") === "off";
/** `?poll=none`: no good `claude agents` poll yet. */
export const pollUnknown = () => param("poll") === "none";
export const since = now - 3 * day;

const brief = (goal: string, done: string) => ({ goal, outOfScope: "", constraints: "", done });

const qaReport = {
  version: 1,
  status: "done",
  updatedAt: new Date(now - 70_000_000).toISOString(),
  work: [
    { step: "린트 정리 브랜치 받기", result: "변경 파일 14개", files: [] },
    { step: "테스트·린트 실행", result: "vitest 212 통과, eslint 0 오류", files: [] },
    { step: "공개 API 비교", result: "유틸 이름 하나가 바뀜", files: ["src/lib/format.ts"] },
  ],
  blocked: [],
  decisions: [],
  criteria: [
    { criterion: "완료 기준별 PASS/FAIL", met: "yes", evidence: "4개 중 3 PASS, 1 FAIL" },
    { criterion: "재현 절차", met: "yes", evidence: "README에 3단계" },
    { criterion: "공개 API 그대로", met: "no", evidence: "formatDate → formatDay" },
  ],
  references: [],
};

const designReport = {
  version: 1,
  status: "done",
  updatedAt: new Date(now - 30_000_000).toISOString(),
  work: [
    { step: "설정 화면 정보 구조 정리", result: "묶음 5개 → 4개", files: [] },
    { step: "다크 모드 대비 확인", result: "본문 대비 7.1:1", files: [] },
  ],
  blocked: [],
  decisions: [{ title: "고급 설정 접기", options: ["접기", "그대로"], chosen: "접기", reason: "처음 보는 사람에게 짧게", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 1 }],
  criteria: [
    { criterion: "한 화면에 들어오기", met: "yes", evidence: "900px에서 스크롤 없음" },
    { criterion: "대비 4.5:1 이상", met: "yes", evidence: "최저 5.2:1" },
  ],
  references: [],
};

/** Reports for the two records only this mock has; the rest come from the task board's mock. */
export function mockReviewReport(id: string) {
  if (id === "5a5a5a5a")
    return { kind: "ready", delegated: { name: "qa-lint-sweep", brief: brief("'lint-sweep' 작업 결과를 실제로 실행해 검증", "완료 기준별 PASS/FAIL\n재현 절차"), prompt: null, taskType: "qa", permissionMode: "plan", folder: "dev_team", stored: true, routine: false }, report: qaReport };
  if (id === "4b4b4b4b")
    return { kind: "ready", delegated: { name: "settings-layout", brief: brief("설정 화면을 한 화면에 들어오게 정리", "한 화면에 들어오기\n대비 4.5:1 이상"), prompt: null, taskType: "design", permissionMode: "plan", folder: "side-project", stored: true, routine: false }, report: designReport };
  return null;
}

export interface Rec {
  bgId: string;
  cwd: string;
  taskType: string | null;
  createdAt: number;
  running: "working" | "waiting" | null;
}

const all: Rec[] = [
      { bgId: "f3e5ff71", cwd: "/Users/me/Agents/dev_team", taskType: "dev", createdAt: now - 900_000, running: "waiting" },
      { bgId: "ab12cd34", cwd: "/Users/me/Agents/side-project", taskType: "dev", createdAt: now - 300_000, running: "working" },
      { bgId: "4b4b4b4b", cwd: "/Users/me/Agents/side-project", taskType: "design", createdAt: now - 3_600_000, running: null },
      { bgId: "77aa55cc", cwd: "/Users/me/Agents/side-project", taskType: "qa", createdAt: now - 1_800_000, running: null },
      { bgId: "5a5a5a5a", cwd: "/Users/me/Agents/dev_team", taskType: "qa", createdAt: now - 70_000_000, running: null },
      { bgId: "66bb44dd", cwd: "/Users/me/Agents/dev_team", taskType: "comms", createdAt: now - 5_400_000, running: null },
      { bgId: "9988aabb", cwd: "/Users/me/Agents/dev_team/.claude/worktrees/lint-sweep", taskType: "dev", createdAt: now - 86_000_000, running: null },
      { bgId: "aa11bb22", cwd: "/Users/me/Agents/side-project", taskType: "dev", createdAt: now - 2 * day, running: null },
      { bgId: "cc33dd44", cwd: "/Users/me/Agents/side-project", taskType: "deploy", createdAt: now - 5 * day, running: null },
].sort((a, b) => b.createdAt - a.createdAt) as Rec[];
// `?review=many`: 15 more finished dev tasks (no report yet) so the list runs to a second page.
const many: Rec[] = Array.from({ length: 15 }, (_, i) => ({ bgId: `e${String(i).padStart(7, "0")}`, cwd: "/Users/me/Agents/dev_team", taskType: "dev", createdAt: now - 40 * 3_600_000 - i * 60_000, running: null }));
// `?evidence=1`: the report-evidence samples (근거·첨부) join the list.
const evidenceRecs = (): Rec[] => (param("evidence") ? EVIDENCE_RECORDS.map((r) => ({ bgId: r.bgId, cwd: "/Users/me/Agents/dev_team", taskType: r.taskType, createdAt: now - r.ago, running: null })) : []);
export const recs = () => (empty() ? [] : param("review") === "many" ? [...all, ...many].sort((a, b) => b.createdAt - a.createdAt) : [...all, ...evidenceRecs()].sort((a, b) => b.createdAt - a.createdAt));
const reviewed = new Set(["aa11bb22"]);
// task-lifecycle `?board=demo|nolive`: lint-sweep (live, worktree) is reviewed too and waits in 정리.
const boardDemo = () => param("board") !== null;
const demoReviewed = new Set(["9988aabb"]);
/** 정리 끝 (task-lifecycle). */
export const wrapped = new Set<string>();
const allDone = () => param("review") === "done";

export const isReviewed = (id: string) => reviewed.has(id) || allDone() || (boardDemo() && demoReviewed.has(id));
export const pending = (r: Rec) => !isReviewed(r.bgId) && r.running === null && r.createdAt >= since;

type View = ReturnType<typeof mockTaskReport>;
function card(r: Rec) {
  const v = (mockEvidenceReport(r.bgId) ?? mockReviewReport(r.bgId) ?? mockTaskReport(r.bgId)) as View & { report?: typeof qaReport; code?: string };
  const d = v.delegated;
  type Item = { kind: string; status: string; failed?: number };
  type Crit = { criterion: string; met: string; summary: string; evidence: Item[] };
  const rep = v.report ? (normalize(v.report as unknown as Record<string, unknown>) as unknown as { status: string; work: unknown[]; blocked: never[]; decisions: never[]; criteria: Crit[]; legacy: boolean; attachments: unknown[] }) : undefined;
  const ok = (c: Crit, k: string) => c.evidence.filter((i) => i.status === "valid" && i.kind === k).length;
  const report =
    v.kind === "ready" && rep
      ? {
          kind: "ready",
          summary: {
            status: rep.status,
            workCount: rep.work.length,
            blocked: rep.blocked.map((b: { what: string; open: boolean; needsUser: boolean }) => ({ what: b.what, open: b.open, needsUser: b.needsUser })),
            decisions: rep.decisions.map((x: { title: string; reversible: string | null; assumed: boolean; deviatesFromBrief: boolean }) => ({ title: x.title, reversible: x.reversible, assumed: x.assumed, deviatesFromBrief: x.deviatesFromBrief })),
            criteria: rep.criteria.map((c) => ({
              criterion: c.criterion,
              met: c.met,
              evidence: c.summary.slice(0, 120),
              evidenceCount: c.evidence.length,
              verified: { screenshot: ok(c, "screenshot"), testReport: ok(c, "test-report"), command: ok(c, "command"), file: ok(c, "file") },
              testsFailed: c.evidence.filter((i) => i.status === "valid").reduce((n, i) => n + (i.failed ?? 0), 0),
            })),
            legacy: rep.legacy,
            testReport: rep.criteria.some((c) => ok(c, "test-report") > 0),
            attachmentCount: rep.attachments.length,
            attachmentValid: (rep.attachments as { status: string; empty?: boolean }[]).filter((a) => a.status === "valid" && !a.empty).length,
            attachmentBroken: (rep.attachments as { status: string }[]).filter((a) => a.status !== "valid").length,
            attachmentEmpty: (rep.attachments as { status: string; empty?: boolean }[]).filter((a) => a.status === "valid" && a.empty).length,
            firstAttachment: rep.criteria.length === 0 ? mockFirstAttachment(r.bgId) : null,
          },
        }
      : v.kind === "invalid"
        ? { kind: "invalid", code: v.code ?? "invalid" }
        : { kind: "missing" };
  return {
    bgId: r.bgId,
    taskType: r.taskType,
    folder: r.cwd.split("/").pop() ?? null,
    createdAt: r.createdAt,
    reviewed: isReviewed(r.bgId),
    running: pollUnknown() && !isReviewed(r.bgId) ? "checking" : r.running,
    goal: (d?.brief?.goal || d?.name || null)?.slice(0, 120) ?? null,
    stored: d?.stored ?? false,
    done: d?.brief ? d.brief.done : null,
    report,
    deliveryAlert: (d as { deliveryAlert?: number | null } | null)?.deliveryAlert ?? null,
    wrapped: wrapped.has(r.bgId),
  };
}

const inScope = (cwd: string, root: string | null) => !root || cwd === root || cwd.startsWith(`${root}/`);

export function mockReviewPending(): number {
  return recs().filter(pending).length;
}

export const reviewHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  review_inbox: (a) => {
    const root = a.project ? mockProjectPath(a.project) : null;
    if (a.project && !root) throw "이 프로젝트 폴더를 찾지 못했어요";
    const records = recs();
    const scoped = records.filter((r) => inScope(r.cwd, root));
    const matching = scoped.filter((r) => a.filter === "all" || pending(r));
    const offset = Number(a.offset ?? 0);
    const limit = Math.min(Math.max(Number(a.limit ?? 10), 1), 40);
    return {
      reportSetting: !reportOff(),
      total: matching.length,
      pendingOther: records.filter(pending).length - scoped.filter(pending).length,
      hasAny: scoped.length > 0,
      running: scoped.filter((r) => r.running).length,
      groups: [...new Set(matching.map((r) => r.taskType))].map((taskType) => ({ taskType, total: matching.filter((r) => r.taskType === taskType).length })),
      items: matching.slice(offset, offset + limit).map(card),
    };
  },
  inbox_mark_reviewed: (a) => {
    const r = recs().find((x) => x.bgId === a.id);
    if (!r || r.running || pollUnknown()) throw "아직 리뷰할 수 없는 작업이에요";
    reviewed.add(r.bgId);
    return null;
  },
  // Undoing the review undoes 정리 끝 too (task-lifecycle).
  unmark_reviewed: (a) => (wrapped.delete(String(a.id)), demoReviewed.delete(String(a.id)), reviewed.delete(String(a.id))),
};
