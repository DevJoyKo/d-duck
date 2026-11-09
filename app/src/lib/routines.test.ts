import { describe, expect, it } from "vitest";
import { relativeNext, runStatusText, scheduleText, validateRoutineForm } from "./routines";

describe("routine display", () => {
  it("writes schedules like the Rust approval dialog", () => {
    expect(scheduleText({ kind: "daily", hour: 9, minute: 5 })).toBe("매일 09:05");
    expect(scheduleText({ kind: "weekly", weekdays: 0b1000001, hour: 14, minute: 30 })).toBe("매주 월·일 14:30");
    expect(scheduleText({ kind: "interval", hours: 6 })).toBe("6시간마다");
  });
  it("names run statuses with reasons", () => {
    expect(runStatusText("start_failed", "untrustedWorkspace")).toBe("시작 실패(신뢰되지 않은 폴더)");
    expect(runStatusText("skipped", "busy")).toBe("건너뜀(이전 실행 중)");
    expect(runStatusText("start_failed", "notApproved")).toBe("시작 실패(승인되지 않음)");
    expect(runStatusText("missed", null)).toBe("놓침");
    expect(runStatusText("weird", null)).toBe("weird");
  });
  it("validates the form before the native dialog", () => {
    const ok = { name: "아침 점검", cwd: "/x", prompt: "p", schedule: { kind: "daily", hour: 9, minute: 0 } as const };
    expect(validateRoutineForm(ok)).toBeNull();
    expect(validateRoutineForm({ ...ok, name: " " })).toMatch("이름");
    expect(validateRoutineForm({ ...ok, schedule: { kind: "interval", hours: 25 } })).toMatch("1–24");
    expect(validateRoutineForm({ ...ok, schedule: { kind: "weekly", weekdays: 0, hour: 9, minute: 0 } })).toMatch("요일");
  });
  it("describes the next run relative to now", () => {
    expect(relativeNext(null)).toBe("-");
    expect(relativeNext(1_000 + 30 * 60_000, 1_000)).toBe("30분 후");
    expect(relativeNext(5 * 3_600_000, 0)).toBe("5시간 후");
  });
});
