import { describe, expect, it } from "vitest";
import { addedTools, addPins, agentListsFor, agentLoadoutLock, chosenFromAgent, chosenFromDisallowed, disallowedFor, frontmatterList, heavyTokens, MAX_SKILLS, pinnedBytes, pinState, pluginBundles, roleLoadoutChips, separable, sourceLabel, tokenText, withheldServers, type SkillOption } from "./loadout";
import { wouldEmptyTools } from "./loadout";
import { isWriteTool } from "./team";

const opt = (name: string, source = "user", bytes = 4000, hash = "a".repeat(32)): SkillOption => ({ name, source, plugin: source.startsWith("plugin:") ? source.slice(7) : null, hash, bytes, description: null });

describe("loadout", () => {
  it("states pins by name, source and hash", () => {
    const opts = [opt("frontend-design"), opt("expo:expo-ui", "plugin:expo")];
    expect(pinState({ name: "frontend-design", source: "user", hash: "a".repeat(32) }, opts)).toBe("ok");
    expect(pinState({ name: "frontend-design", source: "user", hash: "b".repeat(32) }, opts)).toBe("changed");
    expect(pinState({ name: "frontend-design", source: "project", hash: "a".repeat(32) }, opts)).toBe("missing");
    expect(pinState({ name: "gone", source: "user", hash: "a".repeat(32) }, opts)).toBe("missing");
    expect(pinState({ name: "frontend-design", source: "user", hash: "a".repeat(32) }, opts, ["frontend-design"])).toBe("changed");
    expect(pinnedBytes([{ name: "frontend-design", source: "user", hash: "a".repeat(32) }, { name: "gone", source: "user", hash: "" }], opts)).toBe(4000);
  });

  it("bundles plugin skills and caps the list", () => {
    const opts = [opt("a"), opt("p:x", "plugin:p"), opt("p:y", "plugin:p"), opt("q:z", "plugin:q")];
    expect(pluginBundles(opts).map((b) => [b.plugin, b.skills.length])).toEqual([["p", 2], ["q", 1]]);
    const many = Array.from({ length: 20 }, (_, i) => opt(`s${i}`));
    expect(addPins([], many)).toHaveLength(MAX_SKILLS);
    expect(addPins([{ name: "a", source: "user", hash: "" }], [opt("a")])).toHaveLength(1);
  });

  it("maps file agents' disallowedTools to chosen servers and back", () => {
    const servers = ["figma", "notion", "sentry", "reports__v2"];
    expect(withheldServers(["Edit", "mcp__notion", "mcp__sentry__*", "mcp__a__b__*"])).toEqual(["notion", "sentry"]);
    expect(chosenFromDisallowed(["Edit"], servers)).toEqual([]);
    expect(chosenFromDisallowed(["mcp__notion__*", "mcp__sentry__*"], servers)).toEqual(["figma"]);
    expect(disallowedFor(["Edit", "mcp__notion__*", "mcp__other__*"], ["figma"], servers)).toEqual(["Edit", "mcp__other__*", "mcp__notion__*", "mcp__sentry__*", "mcp__reports__v2__*"]);
    expect(disallowedFor(["Edit", "mcp__notion__*"], [], servers)).toEqual(["Edit"]);
    expect(separable("reports__v2")).toBe(false);
  });

  it("labels and chips", () => {
    expect([sourceLabel("user"), sourceLabel("project"), sourceLabel("plugin:expo")]).toEqual(["전역", "프로젝트", "expo"]);
    expect(tokenText(0)).toBe("");
    expect(tokenText(100)).toBe("1천 토큰 미만");
    expect(tokenText(3999)).toBe("1천 토큰 미만");
    expect(tokenText(9800)).toBe("약 2천 토큰");
    expect([tokenText(40_000), tokenText(90_000)]).toEqual(["약 1만 토큰", "약 2만 3천 토큰"]);
    expect([heavyTokens(80_000), heavyTokens(80_004)]).toEqual([false, true]);
    expect(roleLoadoutChips({ agent: null, skills: [{ name: "frontend-design", source: "user", hash: "" }], mcpAllow: ["figma"] })).toEqual([
      { kind: "skill", label: "frontend-design 스킬" },
      { kind: "mcp", label: "figma MCP" },
    ]);
    expect(roleLoadoutChips({ agent: "x", skills: [{ name: "a", source: "user", hash: "" }], mcpAllow: [] })).toEqual([]);
    expect(agentLoadoutLock({ source: "plugin", editable: false, readonlyReason: null })).toContain("MCP 지정이 무시돼요");
    expect(agentLoadoutLock({ source: "user", editable: true, readonlyReason: null })).toBeNull();
    expect(isWriteTool("mcp__figma__*")).toBe(true);
  });

  it("adds chosen servers to a file agent's tools line and keeps everything else", () => {
    const servers = ["figma", "notion", "sentry", "reports__v2"];
    expect(chosenFromAgent(["Read"], [], servers)).toEqual([]);
    expect(chosenFromAgent(["Read", "mcp__figma__*", "mcp__notion"], ["mcp__notion__*"], servers)).toEqual(["figma"]);
    expect(chosenFromAgent([], ["mcp__notion__*"], servers)).toEqual(["figma", "sentry"]);
    const out = agentListsFor(["Read", "mcp__notion__*", "mcp__x__tool"], ["Edit", "mcp__figma__*"], ["figma"], servers);
    expect(out).toEqual({ tools: ["Read", "mcp__x__tool", "mcp__figma__*"], disallowed: ["Edit"] });
    expect(addedTools(["Read"], out.tools)).toEqual(["mcp__x__tool", "mcp__figma__*"]);
    // Without a tools line the old rule (withhold the rest) applies and tools stay empty.
    expect(agentListsFor([], ["Edit"], ["figma"], servers)).toEqual({ tools: [], disallowed: ["Edit", "mcp__notion__*", "mcp__sentry__*", "mcp__reports__v2__*"] });
  });

  it("reads one-line frontmatter lists as written", () => {
    const text = "\uFEFF---\r\nname: a\r\ntools: [Read, \"mcp__figma__*\"]\r\ndisallowedTools: Edit\r\n---\r\nbody tools: x\r\n";
    expect(frontmatterList(text, "tools")).toEqual(["Read", "mcp__figma__*"]);
    expect(frontmatterList(text, "disallowedTools")).toEqual(["Edit"]);
    expect(frontmatterList(text, "skills")).toEqual([]);
    expect(frontmatterList("no frontmatter", "tools")).toEqual([]);
  });
});

describe("wouldEmptyTools (re-review HIGH)", () => {
  it("flags a tools line that would be emptied by removing its last MCP server", () => {
    expect(wouldEmptyTools(["mcp__figma__*"], [], ["figma"])).toBe(true);
    expect(wouldEmptyTools(["mcp__figma__*"], ["figma"], ["figma"])).toBe(false);
    expect(wouldEmptyTools([], [], ["figma"])).toBe(false);
    expect(wouldEmptyTools(["Read", "mcp__figma__*"], [], ["figma"])).toBe(false);
  });
});

