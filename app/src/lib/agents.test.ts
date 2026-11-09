import { describe, expect, it } from "vitest";
import type { AgentDef } from "./api";
import { personasFor, statusLine, taskTypeLabel } from "./agents";
import type { TeamType } from "./team";
import { applyLanguage } from "../i18n";

const def = (name: string, source: AgentDef["source"], scope: string | null = null): AgentDef => ({
  id: `${source}-${name}-${scope}`, name, source, scope, path: null, description: null, model: null, tools: [], disallowedTools: [], malformed: false, editable: true, readonlyReason: null, primary: true, usable: source !== "project" || !!scope?.startsWith("/Users/me/dev_team"),
  risk: { hooks: false, mcpServers: false, dangerMode: false, unreadable: false },
});

describe("agent cards", () => {
  it("writes the line under the duck", () => {
    expect(statusLine(undefined)).toEqual({ pose: "idle", head: "쉬는 중", detail: null });
    expect(statusLine({ agent: "a", state: "working", count: 1, total: 1, detail: "설계 비판 검토", sessionKey: "k", subagent: true })).toEqual({ pose: "working", head: "일하는 중", detail: "설계 비판 검토" });
    expect(statusLine({ agent: "a", state: "waiting", count: 2, total: 3, detail: "flaky", sessionKey: null, subagent: false })).toEqual({ pose: "waiting", head: "답을 기다려요 2건", detail: "flaky · 다른 작업 1건" });
  });
  it("offers only personas usable in the folder", () => {
    const defs = [def("design-critic", "project", "/Users/me/dev_team"), def("design-critic", "user"), def("helper", "user"), def("Explore", "builtin"), def("x", "project", "/Users/me/dev")];
    const here = personasFor(defs);
    expect(here.map((d) => `${d.name}:${d.source}`)).toEqual(["design-critic:project", "helper:user", "Explore:builtin"]);
    const elsewhere = personasFor(defs.map((d) => ({ ...d, usable: d.source !== "project" })));
    expect(elsewhere.find((d) => d.name === "design-critic")?.source).toBe("user");
    expect(elsewhere.some((d) => d.name === "x")).toBe(false);
  });
  it("labels task types from the team config, keeping removed keys visible", () => {
    const types = [{ key: "comms", label: "커뮤니케이션" }, { key: "ops", label: "운영" }] as TeamType[];
    expect(taskTypeLabel("comms", types)).toBe("커뮤니케이션");
    expect(taskTypeLabel("ops", types)).toBe("운영");
    expect(taskTypeLabel("gone", types)).toBe("gone (삭제된 유형)");
    expect(taskTypeLabel("dev")).toBe("dev");
    expect(taskTypeLabel(null, types)).toBeNull();
  });
  it("shows a bundled type's name in the app language unless the user renamed it (2.3)", () => {
    const types = [{ key: "comms", label: "커뮤니케이션" }, { key: "dev", label: "Development" }, { key: "qa", label: "품질 담당" }] as TeamType[];
    try {
      applyLanguage("en");
      expect(taskTypeLabel("comms", types)).toBe("Communication");
      expect(taskTypeLabel("dev", types)).toBe("Development");
      // Renamed by the user: shown as written, never translated (D-171).
      expect(taskTypeLabel("qa", types)).toBe("품질 담당");
      applyLanguage("ko");
      expect(taskTypeLabel("dev", types)).toBe("개발");
      expect(taskTypeLabel("comms", types)).toBe("커뮤니케이션");
    } finally {
      applyLanguage("ko");
    }
  });
});
