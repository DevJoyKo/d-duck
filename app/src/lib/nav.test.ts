import { describe, expect, it } from "vitest";
import { normalizePage } from "./nav";

describe("normalizePage", () => {
  it("keeps current pages and maps old ids to 확장 with a chip", () => {
    expect(normalizePage("mcp")).toEqual({ page: "mcp", chip: null });
    expect(normalizePage("skills")).toEqual({ page: "extensions", chip: "skill" });
    expect(normalizePage("plugins")).toEqual({ page: "extensions", chip: "plugin" });
  });

  it("refuses anything off the static list", () => {
    for (const bad of ["", "toString", "__proto__", "constructor", "Skills", "extensions ", null, 3, undefined]) expect(normalizePage(bad)).toBeNull();
  });
});
