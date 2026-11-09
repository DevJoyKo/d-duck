import { describe, expect, it } from "vitest";
import { applyChanges, checkRule, customRules, inheritedNote, changeCount, parseJson, recommendedDiff, shown, ruleWarnings, setToggle, stateOf, toJson, type Catalog, type PermEdit, type ScopeView } from "./permissions";

const catalog: Catalog = {
  groups: [{ id: "git", label: "git" }],
  items: [
    { group: "git", rule: "Bash(git push:*)", risk: "high", recommended: "ask", description: "", reason: "" },
    { group: "git", rule: "Edit", risk: "medium", recommended: "allow", recommendedGlobal: "ask", description: "", reason: "" },
    { group: "git", rule: "Read(./.env)", risk: "high", recommended: "deny", description: "", reason: "" },
  ],
};
const edit: PermEdit = { allow: ["Bash(git:*)", "Bash(git push:*)"], ask: [], deny: ["Read(./.env)"], defaultMode: null };
const view: ScopeView = { path: "/p/.claude/settings.json", exists: true, token: "t", ...edit, other: [["additionalDirectories", ["../x"]]], keys: ["allow", "additionalDirectories", "deny"], readonlyReason: null, managedOnly: false, error: null };

describe("toggles", () => {
  it("reads exact matches and moves only that rule", () => {
    expect(stateOf(edit, "Bash(git push:*)")).toBe("allow");
    expect(stateOf(edit, "Bash(git push)")).toBe("none");
    const next = setToggle(edit, "Bash(git push:*)", "deny");
    expect(next.allow).toEqual(["Bash(git:*)"]);
    expect(next.deny).toEqual(["Read(./.env)", "Bash(git push:*)"]);
    expect(setToggle(next, "Bash(git push:*)", "none").deny).toEqual(["Read(./.env)"]);
  });
  it("diffs against scope-specific recommendations", () => {
    const user = recommendedDiff(edit, catalog, "user");
    expect(user).toEqual([
      { rule: "Bash(git push:*)", risk: "high", from: "allow", to: "ask" },
      { rule: "Edit", risk: "medium", from: "none", to: "ask" },
    ]);
    expect(recommendedDiff(edit, catalog, "shared").find((c) => c.rule === "Edit")?.to).toBe("allow");
    const applied = applyChanges(edit, user);
    expect(applied.allow).toEqual(["Bash(git:*)"]);
    expect(applied.ask).toEqual(["Bash(git push:*)", "Edit"]);
    expect(recommendedDiff(applied, catalog, "user")).toEqual([]);
  });
  it("lists custom rules and inherited states", () => {
    expect(customRules(edit, catalog)).toEqual([{ list: "allow", rule: "Bash(git:*)" }]);
    expect(inheritedNote("Edit", "allow", [{ label: "전역", edit: { ...edit, deny: ["Edit"] } }])).toBe("전역: 거부");
    expect(inheritedNote("Edit", "deny", [{ label: "전역", edit: { ...edit, deny: ["Edit"] } }])).toBeNull();
  });
});

describe("rules", () => {
  it("accepts documented forms and rejects broken ones", () => {
    for (const ok of ["Read", "Bash(git push:*)", "mcp__x__get_*", "mcp__*", "*", "Read(!./a/**)", "Agent(Explore)", "Skill(opsx:propose)"]) expect(checkRule(ok)).toBeNull();
    for (const bad of ["", " Read", "Read(", "Read()", "Bash*", "Read\u202e"]) expect(checkRule(bad)).not.toBeNull();
    expect(ruleWarnings("mcp__*", "allow")).toHaveLength(1);
    expect(ruleWarnings("mcp__*", "deny")).toHaveLength(0);
    expect(ruleWarnings("mcp__x(y)", "deny")).toHaveLength(1);
  });
});

describe("JSON editor", () => {
  it("round-trips the preview", () => {
    const text = toJson(edit, view);
    expect(Object.keys(JSON.parse(text))).toEqual(["allow", "additionalDirectories", "deny"]);
    expect(JSON.parse(text)).toEqual({ allow: edit.allow, deny: edit.deny, additionalDirectories: ["../x"] });
    expect(JSON.parse(toJson({ ...edit, ask: ["WebFetch"] }, view)).ask).toEqual(["WebFetch"]);
    expect(parseJson(text, view)).toEqual({ edit });
  });
  it("reports syntax errors with a line and keeps other keys read-only", () => {
    const broken = '{\n  "allow": ["Read"]\n  "deny": []\n}';
    const r = parseJson(broken, view);
    expect("error" in r && r.error).toMatch(/줄/);
    expect(parseJson('{"allow": [], "additionalDirectories": ["/etc"]}', view)).toHaveProperty("error");
    expect(parseJson('{"allow": []}', view)).toHaveProperty("error");
    expect(parseJson('{"allow": ["bad("], "additionalDirectories": ["../x"]}', view)).toHaveProperty("error");
    expect(parseJson('{"allow": [], "defaultMode": "bypassPermissions", "additionalDirectories": ["../x"]}', view)).toHaveProperty("error");
    const keepsUnknown = parseJson('{"allow": ["weird(("], "additionalDirectories": ["../x"]}', { ...view, allow: ["weird(("] });
    expect(keepsUnknown).toHaveProperty("edit");
  });
});

describe("raw rules", () => {
  it("shows hidden characters without changing the saved text", () => {
    const raw = "Re\u200bad";
    expect(shown(raw)).toBe("Re\\u200bad");
    expect(shown("x".repeat(400)).length).toBe(301);
    const base: ScopeView = { ...view, allow: [raw, "Bash(cat <<EOF\nhi\nEOF)"], keys: ["allow", "additionalDirectories"] };
    const text = toJson({ ...edit, allow: base.allow, deny: [] }, base);
    expect(text).toContain("\\u200b");
    const back = parseJson(text, base);
    expect("edit" in back && back.edit.allow).toEqual(base.allow);
  });
  it("counts changes and reports JSON errors in Korean", () => {
    expect(changeCount(edit, { ...edit, deny: [], defaultMode: "plan" })).toBe(2);
    const r = parseJson('{"allow": ["Read",]}', view);
    expect("error" in r && r.error).toMatch(/[가-힣]/);
    expect("error" in r && r.error).not.toMatch(/Unexpected|Expected/);
  });
});
