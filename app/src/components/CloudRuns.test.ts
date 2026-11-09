import { describe, expect, it } from "vitest";
import { lastRunText } from "./RoutinesPage";

describe("lastRunText (D-166)", () => {
  const base = { failureReason: null, firedAt: null, finishedAt: null, sessionId: null };
  it("names each outcome and its tone", () => {
    expect(lastRunText({ ...base, status: "succeeded" })).toEqual({ text: "성공", tone: "ok" });
    expect(lastRunText({ ...base, status: "running" }).tone).toBe("wait");
    expect(lastRunText({ ...base, status: "unknown" }).text).toBe("결과 모름");
  });
  it("shows the failure reason readably", () => {
    expect(lastRunText({ ...base, status: "failed", failureReason: "ENVIRONMENT_NOT_FOUND" })).toEqual({ text: "실패 (environment not found)", tone: "danger" });
  });
  it("adds when it fired", () => {
    expect(lastRunText({ ...base, status: "succeeded", firedAt: Date.UTC(2026, 10, 9, 0, 0) }).text.startsWith("성공 · ")).toBe(true);
  });
});
