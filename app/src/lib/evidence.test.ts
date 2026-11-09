// report-evidence: 근거 없음, strict review/QA, test reports, legacy reports and attachment-only reports.
import { describe, expect, it } from "vitest";
import type { Delegated, EvidenceView, InboxItem, Report, ReportCriterion } from "./api";
import { cautions, criteriaRows, isEvidenceStrictType, isFreeShape, markFlag, validKinds, verdict } from "./report";
import { cardView } from "./review";

const ok = (kind: EvidenceView["kind"], label = "x", failed?: number): EvidenceView => ({ kind, label, caption: "", status: "valid", ...(failed != null ? { failed } : {}) });
const crit = (criterion: string, met: ReportCriterion["met"], evidence: EvidenceView[] = []): ReportCriterion => ({ criterion, met, summary: "s", evidence, legacy: false });
const report = (criteria: ReportCriterion[], extra: Partial<Report> = {}): Report => ({ version: 2, status: "done", updatedAt: null, work: [], blocked: [], decisions: [], criteria, references: [], attachments: [], legacy: false, ...extra });
const del = (taskType: string, done: string): Delegated => ({ name: "x", brief: { goal: "g", outOfScope: "", constraints: "", done }, prompt: null, taskType, permissionMode: "manual", folder: "p", stored: true, routine: false });

describe("근거 판정", () => {
  it("names review and QA as the strict types and gives each role its kinds", () => {
    expect([isEvidenceStrictType("review"), isEvidenceStrictType("qa"), isEvidenceStrictType("dev"), isEvidenceStrictType(null)]).toEqual([true, true, false, false]);
    expect(validKinds("review")).toEqual(["file"]);
    expect(validKinds("qa")).not.toContain("file");
    expect(validKinds("dev")).toHaveLength(4);
  });

  it("a dev task with one criterion without evidence: warn, not '합의대로 마쳤어요'; the card counts it apart", () => {
    const r = report([crit("a", "yes", [ok("screenshot")]), crit("b", "yes", [ok("test-report")]), crit("c", "yes")]);
    const d = del("dev", "a\nb\nc");
    const v = verdict(r, d, "done");
    expect(v.line).toBe("근거 없음 1 → 확인이 필요해요");
    expect(v.danger).toBeUndefined();
    const item = {
      bgId: "a1",
      taskType: "dev",
      folder: "p",
      createdAt: 1,
      reviewed: false,
      running: null,
      goal: "g",
      stored: true,
      done: "a\nb\nc",
      report: {
        kind: "ready",
        summary: {
          status: "done",
          workCount: 0,
          blocked: [],
          decisions: [],
          legacy: false,
          testReport: true,
          attachmentCount: 0,
          firstAttachment: null,
          criteria: [
            { criterion: "a", met: "yes", evidence: "", evidenceCount: 1, verified: { screenshot: 1, testReport: 0, command: 0, file: 0 } },
            { criterion: "b", met: "yes", evidence: "", evidenceCount: 1, verified: { screenshot: 0, testReport: 1, command: 0, file: 0 } },
            { criterion: "c", met: "yes", evidence: "", evidenceCount: 0, verified: { screenshot: 0, testReport: 0, command: 0, file: 0 } },
          ],
        },
      },
    } as InboxItem;
    const c = cardView(item);
    // "근거 없음" is the badge only; the unbacked criterion is not counted as 충족.
    expect(c.counts).toBe("충족 2");
    expect(c.evidence).toEqual({ count: 2, missing: 1, strict: false });
  });

  it("QA: a file-only criterion has no evidence (danger, right after 못 했어요) and no test report is danger", () => {
    const r = report([crit("a", "yes", [ok("screenshot")]), crit("b", "yes", [ok("file")])]);
    const v = verdict(r, del("qa", "a\nb"), "done");
    expect(v.line).toBe("근거 없음 1 · 테스트 리포트 없음 → 확인이 필요해요");
    expect(v.danger).toBe(true);
    const all = verdict(report([crit("a", "yes", [ok("screenshot")])]), del("qa", "a"), "done");
    expect(all.line).toBe("테스트 리포트 없음 → 확인이 필요해요");
    expect(all.line).not.toContain("합의대로");
    const ordered = verdict(report([crit("a", "no"), crit("b", "partial")]), del("qa", "a\nb"), "done");
    expect(ordered.line.startsWith("못 했어요 1 · 근거 없음 1")).toBe(true);
  });

  it("review: file:line evidence counts, nothing missing", () => {
    const r = report([crit("a", "yes", [ok("file", "src/a.ts:10–42")]), crit("b", "yes", [ok("file", "src/b.ts:3")])]);
    const rows = criteriaRows(r, del("review", "a\nb"));
    expect(rows.every((x) => !x.noEvidence)).toBe(true);
    expect(verdict(r, del("review", "a\nb"), "done").line).toBe("합의대로 마쳤어요");
    // A review screenshot does not count (the reviewer cannot capture).
    expect(criteriaRows(report([crit("a", "yes", [ok("screenshot")])]), del("review", "a"))[0].noEvidence).toBe(true);
  });

  it("an unbacked 충족 never shows a green ✓: △ without evidence, ! with failing tests", () => {
    const rows = criteriaRows(report([crit("a", "yes"), crit("b", "yes", [ok("test-report", "t.xml", 1)]), crit("c", "yes", [ok("file")])]), del("dev", "a\nb\nc"));
    expect(rows.map(markFlag)).toEqual(["noEvidence", "testsFailed", null]);
  });

  it("a card does not count a criterion with failing tests as 충족", () => {
    const crit2 = (criterion: string, failed: number) => ({ criterion, met: "yes" as const, evidence: "", evidenceCount: 1, verified: { screenshot: 0, testReport: 1, command: 0, file: 0 }, testsFailed: failed });
    const item = {
      bgId: "a2",
      taskType: "dev",
      folder: "p",
      createdAt: 1,
      reviewed: false,
      running: null,
      goal: "g",
      stored: true,
      done: "a\nb",
      report: { kind: "ready", summary: { status: "done", workCount: 0, blocked: [], decisions: [], legacy: false, testReport: true, attachmentCount: 0, firstAttachment: null, criteria: [crit2("a", 0), crit2("b", 2)] } },
    } as InboxItem;
    expect(cardView(item).counts).toBe("충족 1");
    // ...but it does not vanish: "테스트 실패 1" follows the counts (danger tone in ReviewPage).
    expect(cardView(item).failedRows).toBe(1);
  });

  it("isFreeShape matches team_config.rs report_shape", async () => {
    // @ts-expect-error no @types/node in this project (same as TourDeck.test.tsx)
    const { readFileSync } = await import("node:fs");
    const rust: string = readFileSync("crates/dduck-core/src/team_config.rs", "utf8");
    const arm = /Some\(((?:"[a-z-]+"\s*\|\s*)*"[a-z-]+")\) => ReportShape::Free/.exec(rust);
    expect(arm).not.toBeNull();
    const free = [...arm![1].matchAll(/"([a-z-]+)"/g)].map((m) => m[1]).sort();
    expect(free).toEqual(["architecture", "comms", "learn", "research"]);
    for (const k of ["research", "architecture", "comms", "learn", "dev", "design", "deploy", "security", "review", "qa", "my-type"]) expect(isFreeShape(k), k).toBe(free.includes(k));
  });

  it("못 했어요 needs no evidence, and an invalid item does not count", () => {
    const rows = criteriaRows(report([crit("a", "no"), crit("b", "yes", [{ ...ok("screenshot"), status: "badType" }])]), del("dev", "a\nb"));
    expect(rows.map((r) => !!r.noEvidence)).toEqual([false, true]);
  });

  it("test failures under a met criterion become a chip pointing at it", () => {
    const r = report([crit("a", "yes", [ok("test-report", "vitest.xml", 2)])]);
    const c = cautions(r, criteriaRows(r, del("dev", "a")), "dev");
    expect(c.map((x) => `${x.labels.join()}:${x.target}`)).toEqual(["테스트 실패 2:rpt-crit-0"]);
    expect(verdict(r, del("dev", "a"), "done").line).toBe("테스트 실패 2 → 확인이 필요해요");
  });

  it("a version 1 report keeps the old verdict and counts no missing evidence", () => {
    const legacy = report([{ criterion: "a", met: "yes", summary: "20/20", evidence: [], legacy: true }], { version: 1, legacy: true });
    expect(verdict(legacy, del("qa", "a"), "done").line).toBe("합의대로 마쳤어요");
  });

  it("attachments without criteria: free types say how many, criteria types ask for a check", () => {
    const att = [{ title: "DB 후보 비교", kind: "html" as const, status: "valid" as const }, { title: "표", kind: "markdown" as const, status: "valid" as const }];
    const r = report([], { attachments: att });
    expect(verdict(r, del("architecture", ""), "done")).toEqual({ line: "첨부 보고서 2개", pose: "done" });
    expect(verdict(report([], { attachments: att.slice(0, 1) }), del("design", ""), "done").line).toBe("기준 대조가 없어요 · 첨부 보고서 1개 → 확인이 필요해요");
    const assumed = report([], { attachments: att, decisions: [{ title: "t", options: [], chosen: "", reason: "", reversible: "easy", assumed: true, deviatesFromBrief: false, afterStep: null }] });
    expect(verdict(assumed, del("research", ""), "done").line).toBe("첨부 보고서 2개 · 묻지 않고 정함 1 → 확인이 필요해요");
    // Broken and empty attachments are said, never hidden.
    const four = [...att, { title: "x", kind: "markdown" as const, status: "badType" as const }, { title: "빈", kind: "html" as const, status: "valid" as const, empty: true }];
    expect(verdict(report([], { attachments: four }), del("research", ""), "done").line).toBe("첨부 4개 · 열 수 없음 1 · 빈 문서 1 → 확인이 필요해요");
    // A brief with done lines compares as usual, also for a free type.
    expect(verdict(r, del("research", "a"), "done").line).toBe("대조 안 함 1 → 확인이 필요해요");
  });
});
