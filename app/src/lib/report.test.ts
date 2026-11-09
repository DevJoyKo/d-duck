import { describe, expect, it } from "vitest";
import type { Delegated, Report } from "./api";
import { cautions, criteriaRows, doneLines, flow, FLOW_DECISIONS, reportChip, verdict } from "./report";

const dec = (title: string, extra: Partial<Report["decisions"][number]> = {}) => ({ title, options: ["a", "b"], chosen: "a", reason: "", reversible: "easy" as const, assumed: false, deviatesFromBrief: false, afterStep: null, ...extra });
const base: Report = { version: 1, status: "done", updatedAt: null, work: [], blocked: [], decisions: [], criteria: [], references: [], attachments: [], legacy: true };
const delegated: Delegated = { name: "x", brief: { goal: "g", outOfScope: "", constraints: "", done: "- 테스트 통과\n2. 문서 갱신\n\n원인 설명" }, prompt: null, taskType: "dev", permissionMode: "manual", folder: "p", stored: true, routine: false };

describe("작업 보고서 helpers", () => {
  it("orders caution chips like the verdict line: needs-user, ✕, deviates, △, hard, assumed (one chip per decision)", () => {
    const r: Report = {
      ...base,
      decisions: [dec("가정", { assumed: true }), dec("평범"), dec("어려움", { reversible: "hard" }), dec("둘 다", { reversible: "hard", deviatesFromBrief: true, assumed: true })],
      blocked: [
        { what: "열림", cause: "", resolution: "", open: true, needsUser: true, afterStep: null },
        { what: "풀림", cause: "", resolution: "", open: false, needsUser: true, afterStep: null },
      ],
      criteria: [
        { criterion: "문서 갱신", met: "partial", summary: "", evidence: [], legacy: true },
        { criterion: "테스트 통과", met: "no", summary: "", evidence: [], legacy: true },
      ],
    };
    const c = cautions(r, criteriaRows(r, delegated));
    expect(c.map((a) => `${a.labels.join("+")}:${a.text}:${a.danger}`)).toEqual([
      "답이 필요해요:열림:true",
      "못 했어요:테스트 통과:false",
      "합의와 다름+되돌리기 어려워요+묻지 않고 정함:둘 다:true",
      "일부 충족:문서 갱신:false",
      "되돌리기 어려워요:어려움:true",
      "묻지 않고 정함:가정:false",
    ]);
    expect(c[0].target).toBe("rpt-blk-0");
    expect(c[2].target).toBe("rpt-dec-3");
  });

  it("matches agreed criteria line by line and keeps the agent's extras", () => {
    expect(doneLines(delegated.brief!.done)).toEqual(["테스트 통과", "문서 갱신", "원인 설명"]);
    const r: Report = { ...base, criteria: [{ criterion: "테스트 통과", met: "yes", summary: "20/20", evidence: [], legacy: true }, { criterion: "린트", met: "no", summary: "", evidence: [], legacy: true }] };
    const rows = criteriaRows(r, delegated);
    expect(rows.map((x) => [x.criterion, x.met, x.agreed])).toEqual([
      ["테스트 통과", "yes", true],
      ["문서 갱신", null, true],
      ["원인 설명", null, true],
      ["린트", "no", false],
    ]);
  });

  it("prefers an exact criterion match, never matches an empty one, and uses substrings only when unambiguous", () => {
    const d: Delegated = { ...delegated, brief: { ...delegated.brief!, done: "테스트\n테스트 통과\n문서" } };
    const r: Report = {
      ...base,
      criteria: [
        { criterion: "", met: "yes", summary: "빈 기준", evidence: [], legacy: true },
        { criterion: "테스트", met: "partial", summary: "정확히 일치", evidence: [], legacy: true },
        { criterion: "테스트 통과", met: "yes", summary: "정확히 일치 2", evidence: [], legacy: true },
        { criterion: "문서 갱신", met: "no", summary: "부분 일치", evidence: [], legacy: true },
      ],
    };
    expect(criteriaRows(r, d).map((x) => [x.criterion, x.evidence])).toEqual([
      ["테스트", "정확히 일치"],
      ["테스트 통과", "정확히 일치 2"],
      ["문서", "부분 일치"],
    ]);
    // Two agreed lines that both fit one criterion: neither takes it.
    const amb: Delegated = { ...delegated, brief: { ...delegated.brief!, done: "API 문서\nAPI 문서 예제" } };
    const r2: Report = { ...base, criteria: [{ criterion: "API 문서 예제 갱신", met: "yes", summary: "", evidence: [], legacy: true }] };
    expect(criteriaRows(r2, amb).map((x) => [x.criterion, x.met, x.agreed])).toEqual([
      ["API 문서", null, true],
      ["API 문서 예제", null, true],
      ["API 문서 예제 갱신", "yes", false],
    ]);
  });

  it("orders the flow by step and written order, keeps an open block last, draws flagged decisions first", () => {
    const r: Report = {
      ...base,
      status: "in_progress",
      work: [{ step: "s1", result: "", files: [] }, { step: "s2", result: "", files: [] }],
      blocked: [
        { what: "open", cause: "", resolution: "", open: true, needsUser: true, afterStep: 1 },
        { what: "b", cause: "", resolution: "", open: false, needsUser: false, afterStep: 1 },
      ],
      decisions: Array.from({ length: 9 }, (_, i) => dec(`d${i}`, { afterStep: i === 0 ? 1 : i === 1 ? 99 : null, assumed: i === 8 })),
    };
    const { items, hiddenDecisions } = flow(r);
    expect(hiddenDecisions).toBe(9 - FLOW_DECISIONS);
    const names = items.map((it) => (it.kind === "step" ? it.work.step : it.kind === "decision" ? it.decision.title : it.blocked.what));
    expect(names).toEqual(["s1", "d0", "b", "open", "s2", "d1", "d2", "d3", "d4", "d5", "d8"]);
    // The duck stands on the open puddle, not the last step.
    expect(items.filter((it) => (it.kind === "step" || it.kind === "blocked") && it.here).map((it) => (it.kind === "blocked" ? it.blocked.what : ""))).toEqual(["open"]);
    const noBlock = flow({ ...r, blocked: [] }).items;
    expect(noBlock.find((it) => it.kind === "step" && it.here)).toMatchObject({ n: 2 });
    expect(flow({ ...r, status: "done", blocked: [] }).items.some((it) => it.kind !== "decision" && it.here)).toBe(false);
  });

  it("puts a one-line verdict first, with the pose", () => {
    const clean: Report = { ...base, criteria: [{ criterion: "테스트 통과", met: "yes", summary: "", evidence: [], legacy: true }, { criterion: "문서 갱신", met: "yes", summary: "", evidence: [], legacy: true }, { criterion: "원인 설명", met: "yes", summary: "", evidence: [], legacy: true }] };
    expect(verdict(clean, delegated, "done")).toEqual({ line: "합의대로 마쳤어요", pose: "done" });
    const off: Report = { ...clean, decisions: [dec("x", { deviatesFromBrief: true })], criteria: [...clean.criteria.slice(0, 2), { criterion: "원인 설명", met: "partial", summary: "", evidence: [], legacy: true }] };
    expect(verdict(off, delegated, "done")).toEqual({ line: "합의와 다름 1 · 일부 충족 1 → 확인이 필요해요", pose: "waiting" });
    const running: Report = { ...base, status: "in_progress", work: [{ step: "s", result: "", files: [] }], decisions: [dec("x", { assumed: true })], blocked: [{ what: "w", cause: "", resolution: "", open: true, needsUser: true, afterStep: null }] };
    expect(verdict(running, null, "waitingInput")).toEqual({ line: "답을 기다려요 · 확인할 것 2개", pose: "waiting" });
    expect(verdict({ ...running, blocked: [], decisions: [] }, null, "working")).toEqual({ line: "일하는 중이에요 · 단계 1개째", pose: "working" });
    expect(verdict({ ...running, blocked: [], decisions: [] }, null, "stopped")).toEqual({ line: "진행 중에 멈췄어요 · 단계 1개까지 기록됐어요", pose: "waiting" });
    expect(verdict({ ...base, status: "failed" }, null, "failed")).toEqual({ line: "실패로 끝났어요", pose: "failed" });
    expect(verdict({ ...base, criteria: [{ criterion: "c", met: "no", summary: "", evidence: [], legacy: true }] }, null, "done").line).toBe("못 했어요 1 → 확인이 필요해요");
  });

  it("shows list chips only when they mean something", () => {
    expect(reportChip(undefined, "done")).toBeNull();
    expect(reportChip("missing", "working")).toBeNull();
    expect(reportChip("missing", "done")?.text).toBe("보고서 없음");
    expect(reportChip("invalid", "working")?.tone).toBe("danger");
    expect(reportChip("ready", "working")?.text).toBe("보고서");
  });
});
