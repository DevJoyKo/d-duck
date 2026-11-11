import { describe, expect, it } from "vitest";
import type { NewsItem } from "./api";
import { categoryColor, dayMs, hostOf, splitEarlier } from "./news";
import { ROLE_COLOR_INDEX } from "./roleColors";

const item = (id: string, date: string): NewsItem => ({
  id, title: id, version: null, date, category: "other", summary: "", what: "", whenToUse: "", bestPractice: "", sourceUrl: "", firstSeenMs: 0, seen: false,
});

describe("news helpers", () => {
  it("puts items dated more than 30 days ago under Earlier and keeps the order", () => {
    const now = new Date(2026, 10, 11, 12).getTime();
    const s = splitEarlier([item("a", "2026-11-10"), item("b", "2026-10-13"), item("c", "2026-10-11"), item("d", "2025-01-01")], now);
    expect(s.recent.map((i) => i.id)).toEqual(["a", "b"]);
    expect(s.earlier.map((i) => i.id)).toEqual(["c", "d"]);
  });

  it("reads only YYYY-MM-DD dates", () => {
    expect(Number.isNaN(dayMs("11/10/2026"))).toBe(true);
    expect(new Date(dayMs("2026-11-10")).getDate()).toBe(10);
  });

  it("gives every category its own palette colour", () => {
    const cats = ["model", "command", "setting", "agent", "integration", "ide", "security", "other"] as const;
    expect(new Set(cats.map((c) => categoryColor(c).body)).size).toBe(cats.length);
    expect(Object.keys(ROLE_COLOR_INDEX).length).toBeGreaterThan(0);
  });

  it("shows the host of a link and nothing for junk", () => {
    expect(hostOf("https://code.claude.com/docs/en/changelog")).toBe("code.claude.com");
    expect(hostOf("not a url")).toBe("");
  });
});
