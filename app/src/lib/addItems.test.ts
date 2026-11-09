import { describe, expect, it } from "vitest";
import rawTemplates from "../../crates/dduck-core/data/authoring_templates.json";
import { pickLang } from "./bundled";
import { fillTemplate, isNew, nameError, pendingReviews, targetPath, type Job } from "./addItems";

// Bundled text fields are {ko, en} pairs (app-language); this file uses the Korean side.
const templates = pickLang(rawTemplates);

describe("add items helpers", () => {
  it("validates names like Rust", () => {
    for (const ok of ["standup", "pr-writer", "a_1", "A".repeat(64)]) expect(nameError(ok)).toBeNull();
    for (const bad of ["-x", "../x", "a b", "가나", "a".repeat(65), ".hidden"]) expect(nameError(bad)).not.toBeNull();
    expect(nameError("")).toBeNull();
  });

  it("fills templates with one safe line", () => {
    const wf = fillTemplate(templates.workflow, "triage", "it's a \"test\"\nnext\\");
    expect(wf).toContain("name: 'triage'");
    expect(wf).toContain("description: 'it s a  test  next'");
    const skill = fillTemplate(templates.skill, "pr-writer", "PR 설명");
    expect(skill.startsWith("---\nname: pr-writer\ndescription: PR 설명\n---")).toBe(true);
    expect(skill).not.toMatch(/allowed-tools|!`/);
    expect(fillTemplate(templates.command, "", "")).toContain("description: 언제 쓰는지");
  });

  it("shows where an item lands", () => {
    expect(targetPath("command", null, "standup")).toBe("~/.claude/commands/standup.md");
    expect(targetPath("skill", "/p", "x")).toBe("/p/.claude/skills/x/SKILL.md");
    expect(targetPath("workflow", null, "")).toBe("~/.claude/workflows/<이름>.js");
  });

  it("marks items made in the last 7 days by kind, name and folder", () => {
    const made = [{ kind: "skill" as const, name: "a", project: null, atMs: 1_000 }];
    expect(isNew(made, "skill", "a", null, 2_000)).toBe(true);
    expect(isNew(made, "command", "a", null, 2_000)).toBe(false);
    expect(isNew(made, "skill", "a", "/p", 2_000)).toBe(false);
    expect(isNew(made, "skill", "a", null, 1_000 + 8 * 86_400_000)).toBe(false);
  });

  it("counts results still waiting for review", () => {
    const risk = { allowedTools: null, hooks: false, shellLines: 0, danger: false };
    const item = (name: string, problem: string | null = null) => ({ kind: "skill" as const, name, content: "", files: [], problem, warnings: [], risk });
    const jobs: Job[] = [{ token: "t", kind: "skill", createdMs: 100, bgId: null, title: "", stray: [], produced: [item("a"), item("b"), item("bad", "x")] }];
    expect(pendingReviews(jobs, [])).toBe(2);
    expect(pendingReviews(jobs, [{ kind: "skill", name: "a", project: null, atMs: 150 }])).toBe(1);
    expect(pendingReviews(jobs, [{ kind: "skill", name: "a", project: null, atMs: 50 }])).toBe(2);
  });
});
