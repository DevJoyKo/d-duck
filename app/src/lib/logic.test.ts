import { describe, expect, it } from "vitest";
import type { AgentSession } from "./api";
import { canControl, cardStatusLabel, cliErrorMessage, GROUP_LABEL, groupSessions, pct, recordStatusLabel, sessionLabel, STATUS_LABEL, statusLabel, validateTaskName } from "./format";
import { buildQuery, msUntilNextLocalMidnight, parseLocalDate, rangeFor } from "./period";
import { lastScreen } from "./terminal";

const now = new Date(2026, 10, 7, 15, 30); // local time

describe("rangeFor", () => {
  it("today covers local midnight to midnight", () => {
    const r = rangeFor("today", now);
    expect(new Date(r.fromMs)).toEqual(new Date(2026, 10, 7));
    expect(new Date(r.toMs)).toEqual(new Date(2026, 10, 8));
  });
  it("7d includes today and the six previous days", () => {
    const r = rangeFor("7d", now);
    expect(new Date(r.fromMs)).toEqual(new Date(2026, 10, 1));
    expect(new Date(r.toMs)).toEqual(new Date(2026, 10, 8));
  });
  it("custom is inclusive and order-insensitive", () => {
    const r = rangeFor("custom", now, { from: "2026-11-05", to: "2026-11-03" });
    expect(new Date(r.fromMs)).toEqual(new Date(2026, 10, 3));
    expect(new Date(r.toMs)).toEqual(new Date(2026, 10, 6));
  });
  it("computes the delay to the next local midnight", () => {
    expect(msUntilNextLocalMidnight(new Date(2026, 10, 7, 23, 59, 0))).toBe(60_000);
  });
  it("rolls 'today' over after midnight", () => {
    const before = rangeFor("today", new Date(2026, 10, 7, 23, 59));
    const after = rangeFor("today", new Date(2026, 10, 8, 0, 1));
    expect(after.fromMs).toBe(before.toMs);
  });
  it("rejects malformed dates", () => {
    expect(parseLocalDate("2026/11/05")).toBeNull();
  });
  it("sends the UTC offset with the sign the backend expects", () => {
    const q = buildQuery("today", now, { projects: ["a"], models: [] });
    expect(q.tzOffsetMinutes).toBe(-now.getTimezoneOffset());
    expect(q.projects).toEqual(["a"]);
  });
});

const s = (key: string, status: AgentSession["status"], startedAt: number, kind: AgentSession["kind"] = "background"): AgentSession => ({
  key,
  pid: 1,
  id: kind === "background" ? "ab12cd34" : null,
  sessionId: key,
  name: key,
  cwd: "/w",
  kind,
  startedAt,
  status,
});

describe("task list", () => {
  it("puts sessions in queued, running, review, reviewWait, wrapup columns by lane, newest first", () => {
    const g = groupSessions(
      [s("c", "done", 3), s("b", "working", 2), s("a", "waitingInput", 1), s("d", "waitingInput", 5), s("e", "idle", 4), s("f", "failed", 6), s("g", "stopped", 7), s("h", "failed", 8), s("w", "done", 9), s("x", "done", 10)],
      {
        c: { agent: null, taskType: "dev", lane: "reviewWait" },
        g: { agent: null, taskType: "dev", lane: "reviewWait" },
        h: { agent: null, taskType: "dev", lane: "reviewWait" },
        w: { agent: null, taskType: "dev", lane: "wrapup", reviewed: true },
        x: { agent: null, taskType: "dev", lane: "done" },
      },
    );
    expect(g.map((x) => x.group)).toEqual(["queued", "running", "review", "reviewWait", "wrapup"]);
    expect(g[0].sessions.map((x) => x.key)).toEqual(["e"]);
    expect(g[1].sessions.map((x) => x.key)).toEqual(["b"]);
    // A failed task with a report waits for review; one without stays in 확인 필요.
    expect(g[2].sessions.map((x) => x.key)).toEqual(["f", "d", "a"]);
    expect(g[3].sessions.map((x) => x.key)).toEqual(["h", "g", "c"]);
    expect(g[4].sessions.map((x) => x.key)).toEqual(["w"]);
    // 정리 끝 is off the board, and no session sits in two columns.
    expect(g.flatMap((x) => x.sessions).map((x) => x.key).sort()).toEqual(["a", "b", "c", "d", "e", "f", "g", "h", "w"]);
  });
  it("never calls an unreviewed task 완료", () => {
    expect(STATUS_LABEL.done).toBe("오리가 끝냈어요");
    expect(statusLabel("done", { lane: "reviewWait" })).toBe("오리가 끝냈어요 · 리뷰 대기");
    expect(statusLabel("failed", { lane: "reviewWait" })).toBe("실패했어요 · 리뷰 대기");
    expect(statusLabel("stopped", { lane: "reviewWait" })).toBe("멈췄어요 · 리뷰 대기");
    expect(statusLabel("done", { lane: "wrapup", reviewed: true })).toBe("리뷰했어요 · 정리 남음");
    expect(statusLabel("done", { lane: "wrapup", reviewed: false })).toBe("리뷰 없이 끝났어요");
    expect(statusLabel("done", { lane: "done", reviewed: true })).toBe("완료");
    for (const st of ["done", "failed", "stopped", "unknown", "working", "idle", "waitingInput"] as const) {
      for (const lane of [undefined, "reviewWait", "wrapup"] as const) expect(statusLabel(st, { lane })).not.toContain("완료");
    }
    expect(Object.values(GROUP_LABEL)).not.toContain("완료");
    expect(GROUP_LABEL.reviewWait).toBe("리뷰 대기");
    expect(GROUP_LABEL.wrapup).toBe("정리");
  });
  it("a reviewed failure keeps saying it failed, and a record card never guesses 끝냈어요 (5.2)", () => {
    expect(cardStatusLabel("failed", { lane: "wrapup", reviewed: true })).toBe("실패 · 리뷰했어요");
    expect(statusLabel("failed", { lane: "wrapup", reviewed: true })).toBe("실패했어요 · 리뷰했어요 · 정리 남음");
    expect(recordStatusLabel({ ended: "failed", reviewed: false }, "reviewWait")).toBe("실패");
    expect(recordStatusLabel({ ended: "stopped", reviewed: false }, "reviewWait")).toBe("멈췄어요");
    expect(recordStatusLabel({ ended: "done", reviewed: false }, "reviewWait")).toBe("끝냈어요");
    expect(recordStatusLabel({ ended: null, reviewed: false }, "reviewWait")).toBe("목록에서 사라진 작업");
    expect(recordStatusLabel({ ended: "unknown", reviewed: false }, "reviewWait")).toBe("목록에서 사라진 작업");
    expect(recordStatusLabel({ ended: "failed", reviewed: true }, "wrapup")).toBe("실패 · 리뷰했어요");
    expect(recordStatusLabel({ ended: null, reviewed: true }, "wrapup")).toBe("리뷰했어요");
  });
  it("card status drops the lane its column already names (3.2 review)", () => {
    expect(cardStatusLabel("done", { lane: "reviewWait" })).toBe("끝냈어요");
    expect(cardStatusLabel("failed", { lane: "reviewWait" })).toBe("실패");
    expect(cardStatusLabel("stopped", { lane: "reviewWait" })).toBe("멈췄어요");
    expect(cardStatusLabel("done", { lane: "wrapup", reviewed: true })).toBe("리뷰했어요");
    expect(cardStatusLabel("done", { lane: "wrapup", reviewed: false })).toBe("리뷰 없음");
    expect(cardStatusLabel("waitingInput")).toBe(STATUS_LABEL.waitingInput);
    for (const st of ["done", "failed", "stopped"] as const) {
      for (const lane of ["reviewWait", "wrapup"] as const) expect(cardStatusLabel(st, { lane }).length).toBeLessThanOrEqual(5);
    }
  });
  it("interactive sessions cannot be controlled", () => {
    expect(canControl(s("i", "idle", 0, "interactive"))).toBe(false);
    expect(canControl(s("b", "working", 0))).toBe(true);
  });
});

describe("labels and validation", () => {
  it("falls back to the session id prefix", () => {
    expect(sessionLabel(null, "11111111-2222")).toBe("11111111");
    expect(sessionLabel("Fix tests", "x")).toBe("Fix tests");
  });
  it("validates task names like the backend", () => {
    expect(validateTaskName("--force")).not.toBeNull();
    expect(validateTaskName("a\nb")).not.toBeNull();
    expect(validateTaskName("x".repeat(65))).not.toBeNull();
    expect(validateTaskName("fix-ci")).toBeNull();
  });
  it("maps CLI errors", () => {
    expect(cliErrorMessage({ kind: "untrustedWorkspace", stderr_tail: "" })).toContain("신뢰");
    expect(cliErrorMessage({ kind: "failed", code: 2, stderr_tail: "x" })).toContain("2");
  });
  it("formats percentages", () => {
    expect(pct(33.333)).toBe("33.3%");
  });
});

describe("lastScreen", () => {
  it("keeps the tail from the final clear", () => {
    expect(lastScreen("old\x1b[2Jmid\x1b[2Jnew")).toBe("\x1b[2Jnew");
    expect(lastScreen("plain")).toBe("plain");
  });
});
