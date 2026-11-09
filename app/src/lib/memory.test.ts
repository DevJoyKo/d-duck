import { describe, expect, it } from "vitest";
import { orderProjects, previewIndexLine, type MemoryProject } from "./memory";

const p = (id: string, extra: Partial<MemoryProject> = {}): MemoryProject => ({ id, label: id, path: null, parent: null, on: 1, off: 0, empty: false, overLimit: false, ...extra });

describe("memory helpers", () => {
  it("puts worktree keys under their parent and hides empty projects by default", () => {
    const list = [p("-a--claude-worktrees-x", { parent: "-a" }), p("-b", { empty: true }), p("-a")];
    expect(orderProjects(list, false).map((x) => [x.id, x.child])).toEqual([
      ["-a", false],
      ["-a--claude-worktrees-x", true],
    ]);
    expect(orderProjects(list, true).map((x) => x.id)).toContain("-b");
    // A project with rules but no memory is listed by default (rules-editor).
    expect(orderProjects([p("-r", { empty: true, rules: 2 })], false).map((x) => x.id)).toEqual(["-r"]);
  });

  it("previews the index line a save would leave", () => {
    expect(previewIndexLine("- [Style](style.md) — old", "new\ntext")).toBe("- [Style](style.md) — new text");
    expect(previewIndexLine("- [Style](style.md): other form", "new")).toBe("- [Style](style.md): other form");
    expect(previewIndexLine(null, "x")).toBeNull();
  });
});
