import { describe, expect, it } from "vitest";
import type { AgentSession } from "../lib/api";
import { STARTING_FOR_MS, startingCards } from "./StartingCards";

const session = (id: string): AgentSession => ({ key: id, pid: null, id, sessionId: null, name: "x", cwd: "/p", kind: "background", startedAt: 0, status: "working" } as AgentSession);

describe("startingCards (D-159)", () => {
  it("shows a pending start right away", () => {
    expect(startingCards([{ status: "pending", label: "로그인 고치기", id: null, at: 1_000 }], [], 1_500)).toEqual([{ key: 1_000, label: "로그인 고치기", phase: "starting", since: 1_000, secs: 0 }]);
  });

  it("keeps a started task until it shows up in the agents list", () => {
    const snap = { status: "success", label: "a", id: "bg1", at: 1_000 };
    expect(startingCards([snap], [], 2_000)[0]?.phase).toBe("waiting");
    expect(startingCards([snap], [session("bg1")], 2_000)).toEqual([]);
  });

  it("gives up after 30 s and never shows failed starts", () => {
    expect(startingCards([{ status: "success", label: "a", id: "bg1", at: 0 }], [], STARTING_FOR_MS)).toEqual([]);
    expect(startingCards([{ status: "error", label: "a", id: null, at: 0 }], [], 10)).toEqual([]);
  });
});
