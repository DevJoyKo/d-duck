import { describe, expect, it } from "vitest";
import type { AgentSession, AgentsPayload } from "./api";
import { blockedCount, canFollowUp, canMarkReviewed, cleanName, composeBrief, followUpDraft, overCapacity, parseBrief, reviewLoad } from "./brief";
import type { TeamType } from "./team";

const session = (key: string, status: AgentSession["status"], name = key): AgentSession => ({
  key, pid: 1, id: `${key}000000`.slice(0, 8), sessionId: key, name, cwd: "/w/app/.claude/worktrees/x", kind: "background", startedAt: 0, status,
});

describe("brief", () => {
  it("assembles and parses back with the backend's headings", () => {
    const b = { goal: "버그 고치기", outOfScope: "", constraints: "의존성 추가 금지", done: " 테스트 통과 " };
    const text = composeBrief(b);
    expect(text).toBe("## 목표\n버그 고치기\n\n## 제약\n의존성 추가 금지\n\n## 완료 기준\n테스트 통과");
    expect(parseBrief(text)).toEqual({ goal: "버그 고치기", outOfScope: "", constraints: "의존성 추가 금지", done: "테스트 통과" });
    expect(parseBrief("그냥 한 줄 지시").goal).toBe("그냥 한 줄 지시");
    expect(parseBrief("앞말 ## 목표 라는 문자열").goal).toBe("앞말 ## 목표 라는 문자열");
  });

  it("follow-ups open a fresh task in the dev session's own folder with a cleaned name", () => {
    const d = followUpDraft(session("k", "done", "fix\u202e-flaky"), "review");
    expect(d.cwd).toBe("/w/app/.claude/worktrees/x");
    expect(d.taskType).toBe("review");
    expect(d.brief.goal).toContain("'fix-flaky'");
    expect(cleanName("a".repeat(100))).toHaveLength(64);
    expect(canFollowUp(session("k", "working"), "dev")).toBe(false);
    // task-lifecycle: only once the dev task is over (확인 필요 failed, 리뷰 대기, 정리), not while it waits for an answer.
    expect(canFollowUp(session("k", "waitingInput"), "dev")).toBe(false);
    for (const st of ["failed", "done", "stopped", "unknown"] as const) expect(canFollowUp(session("k", st), "dev")).toBe(true);
    expect(canFollowUp(session("k", "done"), "qa")).toBe(false);
  });

  it("counts unreviewed code tasks, including finished ones", () => {
    const agents: AgentsPayload = {
      sessions: [session("a", "done"), session("b", "working"), session("c", "done"), session("d", "waitingInput"), session("e", "stopped")],
      error: null,
      tags: {
        a: { agent: null, taskType: "dev", reviewed: false },
        b: { agent: null, taskType: "deploy" },
        c: { agent: null, taskType: "dev", reviewed: true },
        d: { agent: null, taskType: "review" },
        e: { agent: null, taskType: "dev" },
      },
    };
    expect(reviewLoad(agents)).toBe(2);
    expect(canMarkReviewed("deploy")).toBe(true);
    expect(canMarkReviewed("review")).toBe(false);
    // The config decides which types count (a user type with review on; dev turned off).
    const types = [{ key: "dev", review: false }, { key: "deploy", review: false }, { key: "ops", review: true }] as TeamType[];
    expect(canMarkReviewed("ops", types)).toBe(true);
    expect(reviewLoad(agents, types)).toBe(0);
    expect(blockedCount(agents)).toBe(1);
    // A failed task with a report is 리뷰 대기, not 확인 필요; one without a report still counts.
    const failed: AgentsPayload = {
      sessions: [session("f", "failed"), session("g", "failed"), session("w", "waitingInput")],
      error: null,
      tags: { f: { agent: null, taskType: "dev", lane: "reviewWait" }, g: { agent: null, taskType: "dev" } },
    };
    expect(blockedCount(failed)).toBe(2);
    // Without the config, the bundled design type counts too.
    expect(canMarkReviewed("design")).toBe(true);
  });

  it("review capacity: a number warns at the limit, 제한 없음 never warns", () => {
    expect(overCapacity(2, 3)).toBe(false);
    expect(overCapacity(3, 3)).toBe(true);
    expect(overCapacity(3, undefined)).toBe(true); // status not loaded: default 3
    expect(overCapacity(999, "unlimited")).toBe(false);
    expect(overCapacity(0, "unlimited")).toBe(false);
  });
});
