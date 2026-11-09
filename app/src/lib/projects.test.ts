import { describe, expect, it, vi } from "vitest";
import { anyDirty, confirmLeave, offFor, pickerRows, setDirty, type KnownProject } from "./projects";

const p = (id: string, extra: Partial<KnownProject> = {}): KnownProject => ({
  id,
  name: id,
  parentDir: "/Users/me",
  path: `/Users/me/${id}`,
  folder: "ok",
  isHome: false,
  offReason: null,
  memory: false,
  memories: 0,
  rules: 0,
  lastActive: null,
  worktreeOf: null,
  empty: false,
  ...extra,
});

describe("pickerRows", () => {
  const list = [p("a"), p("b"), p("wt", { worktreeOf: "a" }), p("gone", { folder: "unknown" }), p("c"), p("idle", { empty: true })];

  it("puts recent picks first, folder-less last, and hides empty projects", () => {
    const rows = pickerRows(list, ["c"], null, "", false);
    expect(rows.map((r) => r.p.id)).toEqual(["c", "a", "b", "gone"]);
    expect(rows[0].section).toBe("recent");
    expect(rows.find((r) => r.p.id === "a")?.children).toBe(1);
  });

  it("unfolds worktrees under their parent, and shows them flat while searching", () => {
    expect(pickerRows(list, [], null, "", false, new Set(["a"])).map((r) => [r.p.id, r.child])).toContainEqual(["wt", true]);
    expect(pickerRows(list, [], null, "w", false).map((r) => r.p.id)).toEqual(["wt"]);
  });

  it("shows empty projects on request and always the current one", () => {
    expect(pickerRows(list, [], null, "", true).some((r) => r.p.id === "idle")).toBe(true);
    expect(pickerRows(list, [], "idle", "", false).some((r) => r.p.id === "idle")).toBe(true);
  });
});

describe("offFor", () => {
  it("allows memory for home and folder-less projects but not folder screens", () => {
    const home = p("me", { isHome: true, offReason: "홈 폴더는 전역과 같아요", offCode: "home" });
    expect(offFor(home, "memory")).toBeNull();
    expect(offFor(home, "folder")).toBe("홈 폴더는 전역과 같아요");
    expect(offFor(null, "folder")).toBeNull();
  });
});

describe("unsaved edits registry", () => {
  it("asks once, and clears when the user agrees", () => {
    expect(confirmLeave(() => false)).toBe(true);
    setDirty("rules", true);
    const no = vi.fn(() => false);
    expect(confirmLeave(no)).toBe(false);
    expect(no).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
    expect(anyDirty()).toBe(true);
    expect(confirmLeave(() => true)).toBe(true);
    expect(anyDirty()).toBe(false);
  });
});
