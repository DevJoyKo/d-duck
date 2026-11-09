// review-inbox (D-140): card lines from summaries, role grouping.
import { describe, expect, it } from "vitest";
import type { InboxItem, InboxSummary } from "./api";
import { asReport, cardView, groupByRole } from "./review";
import { verdict } from "./report";

const summary = (over: Partial<InboxSummary> = {}): InboxSummary => ({
  status: "done",
  workCount: 2,
  blocked: [],
  decisions: [{ title: "시계 주입", reversible: "easy", assumed: true, deviatesFromBrief: false }],
  criteria: [
    { criterion: "20회 통과", met: "yes", evidence: "" },
    { criterion: "원인 설명", met: "yes", evidence: "" },
    { criterion: "문서", met: "partial", evidence: "" },
  ],
  legacy: true,
  testReport: false,
  attachmentCount: 0,
  firstAttachment: null,
  ...over,
});

const item = (over: Partial<InboxItem> = {}): InboxItem => ({
  bgId: "aaaaaaa1",
  taskType: "dev",
  folder: "dev_team",
  createdAt: 1,
  reviewed: false,
  running: null,
  goal: "auth 테스트 고치기",
  stored: true,
  done: "20회 통과\n원인 설명\n문서",
  report: { kind: "ready", summary: summary() },
  ...over,
});

describe("cardView", () => {
  it("uses the report screen's verdict, counts and first caution", () => {
    const v = cardView(item());
    expect(v.line).toBe("일부 충족 1 · 묻지 않고 정함 1 → 확인이 필요해요");
    expect(v.tone).toBe("warn");
    expect(v.counts).toBe("충족 2 · 일부 충족 1");
    expect(v.first?.labels).toEqual(["일부 충족"]);
    expect(v.first?.text).toBe("문서");
    expect(v.markable).toBe(true);
  });

  it("matches the full report screen for the same report", () => {
    const s = summary();
    const d = { name: null, brief: { goal: "", outOfScope: "", constraints: "", done: "20회 통과\n원인 설명\n문서" }, prompt: null, taskType: "dev", permissionMode: null, folder: null, stored: true, routine: false };
    expect(cardView(item()).line).toBe(verdict(asReport(s), d, null).line);
  });

  it("counts leave out zero parts and show 못 했어요; red only when a criterion was not met", () => {
    const v = cardView(item({ report: { kind: "ready", summary: summary({ decisions: [], criteria: [{ criterion: "20회 통과", met: "yes", evidence: "" }, { criterion: "원인 설명", met: "no", evidence: "" }, { criterion: "문서", met: "yes", evidence: "" }] }) } }));
    expect(v.counts).toBe("충족 2 · 못 했어요 1");
    expect(v.tone).toBe("danger");
    // One caution the verdict line already names is not repeated as a chip.
    expect(v.line).toBe("못 했어요 1 → 확인이 필요해요");
    expect(v.first).toBeNull();
    const clean = cardView(item({ report: { kind: "ready", summary: summary({ decisions: [], criteria: [{ criterion: "20회 통과", met: "yes", evidence: "" }, { criterion: "원인 설명", met: "yes", evidence: "" }, { criterion: "문서", met: "yes", evidence: "" }] }) } }));
    expect([clean.line, clean.tone, clean.counts]).toEqual(["합의대로 마쳤어요", "plain", "충족 3"]);
  });

  it("an ended task without report.json still shows and can be marked", () => {
    const v = cardView(item({ report: { kind: "missing" } }));
    expect([v.line, v.markable, v.counts]).toEqual(["보고서가 아직 없어요", true, null]);
  });

  it("running and waiting tasks show their state and cannot be marked", () => {
    expect(cardView(item({ running: "working" }))).toMatchObject({ line: "일하는 중이에요", markable: false });
    expect(cardView(item({ running: "waiting", report: { kind: "missing" } }))).toMatchObject({ line: "답을 기다려요", markable: false });
  });

  it("a refused report reads as unsafe, without a path", () => {
    expect(cardView(item({ report: { kind: "invalid", code: "refused" } }))).toMatchObject({ line: "보고서 파일이 안전하지 않아 읽지 않았어요", tone: "warn" });
  });

  it("no brief: the agent's criteria count as agreed, like the report screen", () => {
    const v = cardView(item({ done: null, report: { kind: "ready", summary: summary({ decisions: [], criteria: [{ criterion: "x", met: "yes", evidence: "" }] }) } }));
    expect(v.line).toBe("합의대로 마쳤어요");
    expect(cardView(item({ goal: null, stored: false })).goal).toBe("목표를 남기지 않은 작업이에요");
  });
});

describe("card preview", () => {
  it("shows up to three criteria rows with one line of evidence, then how many more", () => {
    const crit = ["a", "b", "c", "d", "e"].map((c, i) => ({ criterion: c, met: "yes" as const, evidence: `근거 ${i}` }));
    const v = cardView(item({ done: null, report: { kind: "ready", summary: summary({ decisions: [], criteria: crit }) } }));
    expect(v.preview.map((r) => [r.criterion, r.evidence])).toEqual([["a", "근거 0"], ["b", "근거 1"], ["c", "근거 2"]]);
    expect(v.more).toBe(2);
    expect(cardView(item({ report: { kind: "missing" } })).preview).toEqual([]);
  });
});

describe("groupByRole", () => {
  it("follows the team type order, then deleted types, then no type", () => {
    const types = [{ key: "dev" }, { key: "design" }, { key: "qa" }] as never;
    const items = [item({ bgId: "1", taskType: null }), item({ bgId: "2", taskType: "qa" }), item({ bgId: "3", taskType: "old" }), item({ bgId: "4", taskType: "dev" }), item({ bgId: "5", taskType: "dev" })];
    const g = groupByRole(items, types, (k) => k ?? "기본");
    expect(g.map((x) => x.key)).toEqual(["dev", "qa", "old", null]);
    expect(g[0].items.map((i) => i.bgId)).toEqual(["4", "5"]);
  });
});
