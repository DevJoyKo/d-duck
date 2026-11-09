import { afterEach, describe, expect, it } from "vitest";
import { applyLanguage } from "../i18n";
import { boardSummary } from "./boardSummary";

// palette-refresh 2.3: the one-line count next to the Tasks title (spec "머리 요약").
describe("board summary", () => {
  afterEach(() => applyLanguage("ko"));

  it("counts what needs you, Korean", () => {
    expect(boardSummary({ waiting: 1, review: 5 })).toBe("답을 기다리는 작업 1개, 리뷰할 작업 5개");
  });

  it("leaves out zero counts and is empty when nothing needs you", () => {
    expect(boardSummary({ waiting: 0, review: 2 })).toBe("리뷰할 작업 2개");
    expect(boardSummary({ waiting: 3, review: 0 })).toBe("답을 기다리는 작업 3개");
    expect(boardSummary({ waiting: 0, review: 0 })).toBe("");
  });

  it("uses English plurals", () => {
    applyLanguage("en");
    expect(boardSummary({ waiting: 1, review: 5 })).toBe("1 task waiting for you, 5 tasks to review");
    expect(boardSummary({ waiting: 2, review: 1 })).toBe("2 tasks waiting for you, 1 task to review");
  });

  it("holds only numbers, never a task name or path", () => {
    const line = boardSummary({ waiting: 2, review: 3 });
    expect(line.replace(/\d+/g, "#")).toBe("답을 기다리는 작업 #개, 리뷰할 작업 #개");
  });
});
