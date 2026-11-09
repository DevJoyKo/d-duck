import { describe, expect, it } from "vitest";
import type { LimitEntry, LimitView } from "./api";
import { bannerLines, bannerTime, entryFor, LIMIT_REASON_TEXT, limitChip, resumeLabel, resumeLabels } from "./limits";

const entry = (over: Partial<LimitEntry>): LimitEntry => ({ id: 1, sessionId: "s1", bgId: "ab12", resetsAt: 1, dueAt: 2, state: "auto", reason: null, attempts: 0, name: "release-notes", startedAt: 1, startedLabel: "오후 3:12", label: "오후 1:50", dueLabel: "오후 1:52", ...over });
const view = (entries: LimitEntry[], over: Partial<LimitView> = {}): LimitView => ({ entries, banner: null, late: 0, unknown: 0, autoShipped: true, autoOn: true, ...over });

describe("limit chips", () => {
  it("waiting, manual and resumed", () => {
    expect(limitChip(entry({}))).toMatchObject({ text: "한도 대기 · 오후 1:52에 이어서", note: "Mac이 깨어 있어야 이어요", canSkip: true, canResume: false });
    expect(limitChip(entry({ state: "manual", reason: "danger" }))).toMatchObject({ text: "한도 · 오후 1:50 리셋", note: "위험 권한 작업은 직접 이어야 해요", canResume: true });
    expect(limitChip(entry({ state: "manual", reason: "cap", attempts: 3 }))?.note).toBe("자동으로 3번 이어서 멈췄어요");
    expect(limitChip(entry({ state: "manual", reason: "no_reset", label: null }))).toMatchObject({ text: "한도에 닿았어요", note: "리셋 시각을 몰라요" });
    expect(limitChip(entry({ state: "resumed" }))?.text).toBe("리셋 후 이어서 시작했어요");
    expect(limitChip(entry({ state: "closed", reason: "cancelled" }))).toBeNull();
  });

  it("carries the time inside the line so the screen keeps it whole", () => {
    const chip = limitChip(entry({}))!;
    expect(chip.when).toBe("오후 1:52");
    expect(chip.text).toContain(chip.when);
    expect(limitChip(entry({ state: "manual", reason: "no_reset", label: null }))?.when).toBeUndefined();
    const v = view([], { banner: { resetsAt: 1, label: "오후 1:50", auto: 1, manual: 0, text: "" } });
    expect(bannerLines(v)[0]).toContain(bannerTime(v));
    expect(bannerTime(view([]))).toBe("");
  });

  it("never offers to run terminal sessions or deploy tasks", () => {
    expect(limitChip(entry({ state: "manual", reason: "not_dduck", bgId: null }))?.canResume).toBe(false);
    expect(limitChip(entry({ state: "manual", reason: "deploy" }))?.canResume).toBe(false);
    expect(limitChip(entry({ state: "needs_choice", reason: "late" }))).toMatchObject({ text: "앱이 꺼져 있어 리셋 때 못 이었어요", canResume: true, canSkip: true });
  });

  it("finds the open stop of a session first", () => {
    const v = view([entry({ id: 1, state: "resumed" }), entry({ id: 2, state: "manual", reason: "auto_off" }), entry({ id: 3, sessionId: "other" })]);
    expect(entryFor(v, { sessionId: "s1" })?.id).toBe(2);
    expect(entryFor(v, { sessionId: null })).toBeUndefined();
    expect(entryFor(undefined, { sessionId: "s1" })).toBeUndefined();
  });

  it("every reason line is one 해요체 line", () => {
    for (const t of Object.values(LIMIT_REASON_TEXT)) {
      expect(t).not.toMatch(/\n|니다/);
      expect(t!.length).toBeLessThanOrEqual(40);
    }
  });
});

describe("resume labels", () => {
  it("names the task, never the id", () => {
    expect(resumeLabel(entry({}), true)).toBe("지금 이어서 · release-notes");
    expect(resumeLabel(entry({ name: null }), true)).toBe("지금 이어서");
    expect(resumeLabel(entry({}), false)).toBe("지금 이어서");
    expect(resumeLabel(entry({ name: null, bgId: "9988aabb" }), true)).not.toContain("9988aabb");
  });

  it("same names get the start time; unnamed ones never show an id", () => {
    const l = resumeLabels([
      entry({ id: 1, name: "dev_team", startedLabel: "오후 3:12" }),
      entry({ id: 2, name: "dev_team", startedLabel: "오후 4:05" }),
      entry({ id: 3, name: "release-notes" }),
    ]);
    expect([...l.values()]).toEqual(["지금 이어서 · dev_team · 오후 3:12", "지금 이어서 · dev_team · 오후 4:05", "지금 이어서 · release-notes"]);
    const bare = resumeLabels([entry({ id: 1, name: null, bgId: "9988aabb" }), entry({ id: 2, name: null, bgId: "66bb44dd", startedLabel: "오후 4:05" })]);
    expect([...bare.values()]).toEqual(["지금 이어서 · 오후 3:12", "지금 이어서 · 오후 4:05"]);
    for (const v of bare.values()) expect(v).not.toMatch(/9988aabb|66bb44dd/);
    expect([...resumeLabels([entry({ id: 1 })]).values()]).toEqual(["지금 이어서"]);
  });

  it("needs_choice and manual cards can be skipped too", () => {
    expect(limitChip(entry({ state: "needs_choice", reason: "late" }))?.canSkip).toBe(true);
    expect(limitChip(entry({ state: "manual", reason: "auto_off" }))?.canSkip).toBe(true);
    expect(limitChip(entry({ state: "starting" }))?.canSkip).toBe(false);
  });

  it("delivery-scope failures have their own lines", () => {
    expect(LIMIT_REASON_TEXT.guard_hook_missing).toBe("로컬만 작업 훅을 준비하지 못해 못 이었어요");
    expect(LIMIT_REASON_TEXT.guard_unreadable).toBe("원격 push 막기 설정을 못 읽어 못 이었어요");
    expect(LIMIT_REASON_TEXT.local_only_refused).toBe("로컬만 프로젝트라 직접 이어야 해요");
  });
});

describe("banner lines", () => {
  it("mixed, auto off and late", () => {
    const mixed = "사용량 한도 · 오후 1:50 리셋 · 2개는 이어서 할게요 · 1개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요";
    expect(bannerLines(view([], { banner: { resetsAt: 1, label: "오후 1:50", auto: 2, manual: 1, text: mixed } }))).toEqual([mixed]);
    const off = "사용량 한도 · 오후 1:50 리셋 · 2개는 직접 이어야 해요";
    expect(bannerLines(view([], { banner: { resetsAt: 1, label: "오후 1:50", auto: 0, manual: 2, text: off }, late: 1, unknown: 1 }))).toEqual([
      off,
      "앱이 꺼져 있어 리셋 때 못 이었어요 · 1개를 지금 이을지 골라 주세요",
      "리셋 시각을 몰라요 · 1개는 직접 이어야 해요",
    ]);
    expect(bannerLines(undefined)).toEqual([]);
  });
});
