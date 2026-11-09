import { describe, expect, it } from "vitest";

// palette-refresh 2.5 (D-168): `break-words` (overflow-wrap: break-word) overrides the app-wide `anywhere` and keeps
// a long token's min-content width, so one URL in a log line widened an auto table to 2,300px. Screens use
// `wrap-anywhere` (or break-all / truncate) instead.
// @ts-expect-error no @types/node in this project (same as paletteSource.test.ts)
const { readFileSync, readdirSync, statSync } = await import("node:fs");

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name: string) => {
    const p = `${dir}/${name}`;
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

const SOURCES = walk("src").filter((p) => /\.tsx?$/.test(p) && !/\.test\.tsx?$/.test(p));

describe("long text wraps anywhere (2.5)", () => {
  it("has no break-words class in screens", () => {
    const hits = SOURCES.filter((p) => /\bbreak-words\b/.test(readFileSync(p, "utf8") as string));
    expect(hits).toEqual([]);
  });

  it("keeps the app-wide rule in the token sheet", () => {
    expect(readFileSync("src/index.css", "utf8")).toMatch(/body\s*{\s*overflow-wrap:\s*anywhere;/);
  });
});
