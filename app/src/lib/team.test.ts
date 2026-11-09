import { describe, expect, it } from "vitest";
import type { AgentDef } from "./api";
import { isOffAnywhere, offState, typeProblems, type AgentControls, type TeamType } from "./team";

const def = (name: string, source: AgentDef["source"], scope: string | null = null) => ({ name, source, scope }) as AgentDef;
const type = (o: Partial<TeamType>): TeamType =>
  ({ key: "ops", label: "운영", agent: null, rolePrompt: "p", roleTools: ["Read"], mode: "manual", modes: ["plan", "manual"], strict: true, ...o }) as TeamType;

describe("team settings", () => {
  it("knows which file turned an agent off", () => {
    const c: AgentControls = { userOff: ["code-reviewer"], projectOff: [["/p", ["old-helper"]]], elsewhereOff: ["Plan"], hidden: [] };
    expect(offState(def("code-reviewer", "user"), c)).toBe("off");
    expect(offState(def("old-helper", "project", "/p"), c)).toBe("off");
    expect(offState(def("old-helper", "project", "/q"), c)).toBeNull();
    expect(offState(def("code-reviewer", "project", "/q"), c)).toBe("elsewhere");
    expect(offState(def("Plan", "builtin"), c)).toBe("elsewhere");
    expect(offState(def("old-helper", "user"), c)).toBe("elsewhere");
    expect(isOffAnywhere("old-helper", c)).toBe(true);
    expect(isOffAnywhere("Explore", c)).toBe(false);
  });

  it("flags what the backend will refuse", () => {
    expect(typeProblems(type({}), undefined)).toEqual([]);
    expect(typeProblems(type({ roleTools: ["Read", "Bash"] }), undefined)).toHaveLength(1);
    expect(typeProblems(type({ roleTools: ["Read", "Bash"] }), type({ roleTools: ["Bash"] }))).toEqual([]);
    expect(typeProblems(type({ modes: ["manual", "acceptEdits"] }), undefined)).toHaveLength(1);
    expect(typeProblems(type({ roleTools: null }), undefined)).toHaveLength(1);
    expect(typeProblems(type({ strict: false, roleTools: null, modes: ["manual", "dontAsk"] }), undefined)).toEqual([]);
    expect(typeProblems(type({ key: "Bad" }), undefined)).toHaveLength(1);
  });

  it("keeps strict types away from write-capable agents and honours a bare Agent deny", () => {
    const cat = [
      { name: "reader", primary: true, tools: ["Read", "Grep"] },
      { name: "writer", primary: true, tools: ["Read", "Edit"] },
      { name: "all", primary: true, tools: [] },
    ] as unknown as AgentDef[];
    expect(typeProblems(type({ agent: "reader" }), undefined, cat)).toEqual([]);
    expect(typeProblems(type({ agent: "writer" }), undefined, cat)).toHaveLength(1);
    expect(typeProblems(type({ agent: "all" }), undefined, cat)).toHaveLength(1);
    expect(typeProblems(type({ agent: "reader", fixedStrict: true }), undefined, cat)).toHaveLength(1);
    const c: AgentControls = { userOff: [], projectOff: [], elsewhereOff: ["*"], hidden: [] };
    expect(isOffAnywhere("anything", c)).toBe(true);
    expect(offState(def("x", "user"), c)).toBe("elsewhere");
  });
});
