import { describe, expect, it } from "vitest";

// palette-refresh 1.1 (D-176): token values, the two dark blocks staying in sync, and 4.5:1 text contrast.
// @ts-expect-error no @types/node in this project (same as TourDeck.test.tsx)
const { readFileSync } = await import("node:fs");
const css: string = readFileSync("src/index.css", "utf8");

/** Body of the first `{ … }` block that follows `marker`, matching nested braces. */
function block(marker: string): string {
  const at = css.indexOf(marker);
  if (at < 0) throw new Error(`missing ${marker}`);
  const open = css.indexOf("{", at + marker.length);
  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === "{") depth++;
    if (css[i] === "}" && --depth === 0) return css.slice(open + 1, i);
  }
  throw new Error(`unclosed ${marker}`);
}

function vars(body: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of body.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/--([\w-]+)\s*:\s*([^;]+);/g)) {
    out[m[1]] = m[2].trim().toLowerCase().replace(/\s+/g, " ");
  }
  return out;
}

const light = vars(block("@theme"));
const darkMedia = vars(block(':root:not([data-theme="light"])'));
const darkAttr = vars(block(':root[data-theme="dark"]'));
const dark = { ...light, ...darkMedia };

type Rgba = [number, number, number, number];
function parse(color: string): Rgba {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/.exec(color);
  if (hex) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join("") : hex[1];
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(1) as Rgba;
  }
  const rgba = /^rgba?\(([^)]+)\)$/.exec(color);
  if (rgba) {
    const [r, g, b, a = "1"] = rgba[1].split(",").map((s) => s.trim());
    return [Number(r), Number(g), Number(b), Number(a)];
  }
  throw new Error(`not a colour: ${color}`);
}
/** Paint `top` over an opaque `under`. */
function over(top: Rgba, under: Rgba): Rgba {
  const a = top[3];
  return [0, 1, 2].map((i) => top[i] * a + under[i] * (1 - a)).concat(1) as Rgba;
}
function luminance([r, g, b]: Rgba): number {
  const lin = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}
function contrast(fg: Rgba, bg: Rgba): number {
  const [hi, lo] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe("palette tokens (palette-refresh 1.1)", () => {
  it("has no interim rail tokens left (2.1 moved the sidebar to side-*)", () => {
    expect(Object.keys(light).filter((k) => k.startsWith("color-rail"))).toEqual([]);
    expect(Object.keys(darkMedia).filter((k) => k.startsWith("color-rail"))).toEqual([]);
  });

  it("keeps the two dark blocks identical", () => {
    expect(darkAttr).toEqual(darkMedia);
  });

  it("uses the agreed A values (proposal)", () => {
    expect(light).toMatchObject({
      "color-ink": "#24283b",
      "color-ink-3": "#6a6f7f",
      "color-paper": "#f5f6f8",
      "color-sheet": "#ffffff",
      "color-danger": "#cf3338",
      "color-ok": "#1f7f4b",
      "color-warn": "#aa5a01",
      "color-duck": "#ffc83d",
    });
    expect(darkMedia).toMatchObject({
      "color-ink": "#eceef4",
      "color-ink-2": "#a9adbb",
      "color-ink-3": "#8b90a1",
      "color-paper": "#1d1e23",
      "color-sheet": "#27282f",
      "color-danger": "#ff6b6f",
      "color-ok": "#4cc38a",
      "color-warn": "#f0a24a",
    });
  });

  it.each(["color-side", "color-side-ink", "color-side-active", "color-well", "color-warn-soft", "color-scrim", "color-term", "shadow-e1", "shadow-e2", "shadow-e3", "select-chevron"])(
    "defines %s in light and dark",
    (name) => {
      expect(light[name]).toBeTruthy();
      expect(darkMedia[name]).toBeTruthy();
    },
  );

  it("has no navy left in the surface tokens", () => {
    const navy = ["#222854", "#10132b", "#151935", "#1c2143", "#0b0e22", "#343a68", "#2c3158"];
    const all = [...Object.values(light), ...Object.values(darkMedia), ...Object.values(darkAttr)];
    for (const v of navy) expect(all).not.toContain(v);
  });

  describe.each([
    ["light", light],
    ["dark", dark],
  ] as const)("%s contrast ≥ 4.5:1", (_theme, t) => {
    const text = ["color-ink", "color-ink-2", "color-ink-3", "color-danger", "color-ok", "color-warn"];
    const grounds = ["color-sheet", "color-paper"];
    for (const fg of text) {
      for (const bg of grounds) {
        it(`${fg} on ${bg}`, () => {
          expect(contrast(parse(t[fg]), parse(t[bg]))).toBeGreaterThanOrEqual(4.5);
        });
      }
    }

    it("news adopt tags: text on its tint over sheet and paper (news-apply B1)", () => {
      const tags: [string, Rgba][] = [
        ["color-ok-strong", [...parse(t["color-ok"]).slice(0, 3), 0.1] as Rgba],
        ["color-warn", parse(t["color-warn-soft"])],
        ["color-ink-2", [...parse(t["color-ink"]).slice(0, 3), 0.06] as Rgba],
      ];
      for (const [fg, tint] of tags) {
        for (const ground of ["color-sheet", "color-paper"]) {
          expect(contrast(parse(t[fg]), over(tint, parse(t[ground]))), `${fg} on ${ground}`).toBeGreaterThanOrEqual(4.5);
        }
      }
    });

    it("side-ink on the translucent sidebar and on its selected row", () => {
      const side = over(parse(t["color-side"]), parse(t["color-paper"]));
      const active = over(parse(t["color-side-active"]), side);
      expect(contrast(parse(t["color-side-ink"]), side)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(parse(t["color-ink"]), active)).toBeGreaterThanOrEqual(4.5);
    });

    it("sidebar text (badges, sync line) over the sidebar laid on paper, on hover and on the selected row", () => {
      // Sidebar.tsx: side-ink on the sidebar and on a hovered row (side-active at half strength); the selected row's
      // label and badge switch to ink. ink-3 is not used on the sidebar (sidebar source check below).
      const side = over(parse(t["color-side"]), parse(t["color-paper"]));
      const [r, g, b, a] = parse(t["color-side-active"]);
      const hover = over([r, g, b, a / 2], side);
      const active = over([r, g, b, a], side);
      expect(contrast(parse(t["color-side-ink"]), side)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(parse(t["color-side-ink"]), hover)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(parse(t["color-ink"]), active)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(parse(t["color-ink"]), side)).toBeGreaterThanOrEqual(4.5);
    });

    it("ink-fixed on the duck yellow and on every role colour (chips, waiting badge)", () => {
      expect(contrast(parse(t["color-ink-fixed"]), parse(t["color-duck"]))).toBeGreaterThanOrEqual(4.5);
    });

    it("ink and warn on the amber notice tint", () => {
      expect(contrast(parse(t["color-ink"]), parse(t["color-warn-soft"]))).toBeGreaterThanOrEqual(4.5);
      expect(contrast(parse(t["color-warn"]), parse(t["color-warn-soft"]))).toBeGreaterThanOrEqual(4.5);
    });

    it("terminal text on the terminal ground", () => {
      expect(contrast(parse(t["color-term-ink"]), parse(t["color-term"]))).toBeGreaterThanOrEqual(4.5);
    });
  });
});

describe("sidebar text tokens (palette-refresh 3.2 QA)", () => {
  const src: string = readFileSync("src/components/Sidebar.tsx", "utf8");
  it("puts no ink-3 text on the translucent sidebar", () => {
    expect(src).not.toMatch(/\btext-ink-3\b/);
  });
  it("keeps badges at 12px and switches the selected row's badge to ink", () => {
    expect(src).not.toMatch(/text-\[11px\][^"]*rounded-full|rounded-full[^"]*text-\[11px\]/);
    expect(src).toMatch(/active \? "text-ink" : "text-side-ink"/);
  });
});
