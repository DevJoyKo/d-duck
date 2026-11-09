import { describe, expect, it } from "vitest";

// palette-refresh 1.2 and 2.2 (D-176·D-177): screens take colours from tokens only, and the duck's yellow appears only
// where lib/yellow.ts allows it. The duck drawing and third-party brand icons are the exceptions.
// @ts-expect-error no @types/node in this project (same as paletteTokens.test.ts)
const { readFileSync, readdirSync, statSync } = await import("node:fs");

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name: string) => {
    const p = `${dir}/${name}`;
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

/** App source that draws screens: no tests, no mock data (it imitates other apps' pages), no copy. */
const SOURCES = walk("src")
  .filter((p) => /\.(tsx?|css)$/.test(p) && !/\.test\.tsx?$/.test(p))
  .filter((p) => !p.startsWith("src/mock/") && !p.startsWith("src/i18n/") && !p.startsWith("src/test/"));

/** Files allowed to hold raw colours: the duck drawing and role colours (D-177), brand icons, the token sheet. */
const RAW_COLOUR_OK = ["src/components/DuckAvatar.tsx", "src/lib/roles.ts", "src/lib/connectorIcons.tsx", "src/index.css"];
/** Sidebar pieces that used to paint white text on navy (design decision 4, target 3). */
const SIDEBAR = ["src/components/Sidebar.tsx", "src/components/StartChecklist.tsx", "src/components/LanguageMenu.tsx"];

const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
const read = (p: string) => strip(readFileSync(p, "utf8") as string);
const hits = (re: RegExp, files: string[]) => files.flatMap((p) => [...read(p).matchAll(re)].map((m) => `${p}: ${m[0]}`));

const NAVY = ["#222854", "#10132b", "#151935", "#1c2143", "#0b0e22", "#343a68", "#2c3158", "#61668a"];
const TAILWIND_PALETTE =
  /\b(?:bg|text|border|ring|from|to|via|fill|stroke|outline|divide|decoration|accent|caret|shadow)-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}\b/g;

describe("no hard-coded colours in screens (1.2)", () => {
  it("finds the sources", () => {
    expect(SOURCES.length).toBeGreaterThan(50);
    expect(SOURCES).toContain("src/components/LogView.tsx");
  });

  it("has no old navy anywhere, the token sheet included", () => {
    const all = SOURCES.filter((p) => p !== "src/components/DuckAvatar.tsx");
    for (const v of NAVY) expect(hits(new RegExp(v, "gi"), all), v).toEqual([]);
    expect(hits(/rgba?\(\s*15\s*,\s*18\s*,\s*40/g, all)).toEqual([]);
  });

  it("has no hex or rgb() colours outside the token sheet and the duck", () => {
    const files = SOURCES.filter((p) => !RAW_COLOUR_OK.includes(p));
    expect(hits(/#[0-9a-f]{6}\b|#[0-9a-f]{3}\b(?![-\w])|rgba?\(/gi, files)).toEqual([]);
  });

  it("has no Tailwind default palette colours", () => {
    expect(hits(TAILWIND_PALETTE, SOURCES)).toEqual([]);
  });

  it("has no white or black in the sidebar (it is light and translucent now)", () => {
    expect(hits(/\b(?:bg|text|border|ring)-(?:white|black)\b/g, SIDEBAR)).toEqual([]);
    expect(hits(/\brail(?:-ink)?\b/g, SOURCES)).toEqual([]);
  });

  it("builds the terminal theme from tokens", () => {
    const log = read("src/components/LogView.tsx");
    expect(log).toContain("--color-term");
    expect(log).toContain("bg-term");
  });
});

describe("the duck's yellow stays on the allow list (2.2)", () => {
  const YELLOW = /\b(?:bg|text|border|ring|outline|fill|stroke|shadow|from|to|via|decoration)-duck(?!-stage|-ground)(?:-soft)?\b|var\(--color-duck\)|var\(--color-duck-soft\)/g;

  it("uses *-duck classes only in lib/yellow.ts", () => {
    const files = SOURCES.filter((p) => p !== "src/lib/yellow.ts" && p !== "src/index.css");
    expect(hits(YELLOW, files)).toEqual([]);
  });

  it("allows only the waiting card, the sidebar badge and the duck's own foot", async () => {
    const { YELLOW: allowed } = await import("./yellow");
    expect(Object.keys(allowed).sort()).toEqual(["badge", "duckFoot", "waitingCard"]);
    expect(allowed.waitingCard).toContain("ring-2 ring-duck");
    expect(allowed.waitingCard).toContain("shadow-e2");
  });
});

describe("Button sizes come from the size prop (3.2 review, HIGH 1)", () => {
  // A height, padding or type size in a Button's className competes with ui.tsx's own by stylesheet order, so the
  // override silently lost (h-8 won over h-7, 13px over 12px). Sizes are a prop now; className is for layout only.
  const tsx = SOURCES.filter((p) => p.endsWith(".tsx"));
  function buttonClasses(src: string): string[] {
    const out: string[] = [];
    for (const m of src.matchAll(/<Button\b/g)) {
      // The opening tag ends at the first ">" outside braces and double quotes.
      let depth = 0;
      let quote = false;
      let i = m.index! + m[0].length;
      for (; i < src.length; i++) {
        const c = src[i];
        if (c === '"' && depth === 0) quote = !quote;
        else if (!quote && c === "{") depth++;
        else if (!quote && c === "}") depth--;
        else if (!quote && depth === 0 && c === ">") break;
      }
      const tag = src.slice(m.index!, i);
      const at = tag.indexOf("className=");
      if (at < 0) continue;
      const v = tag.slice(at + "className=".length);
      if (v.startsWith('"')) out.push(v.slice(0, v.indexOf('"', 1) + 1));
      else if (v.startsWith("{")) {
        // The whole expression, so a conditional (`x ? "h-7" : ""`) is read as well as a template.
        let d = 0;
        let j = 0;
        for (; j < v.length; j++) {
          if (v[j] === "{") d++;
          else if (v[j] === "}" && --d === 0) break;
        }
        out.push(v.slice(0, j + 1));
      }
    }
    return out;
  }
  const SIZE = /(^|[\s"'`])!?(h-|px-|text-(\[\d|xs|sm))/;
  it("reads a conditional className as well as a plain or template one", () => {
    const got = buttonClasses('<Button className={on ? "h-7" : "flex-1"}>a</Button><Button className={`a ${b ? "px-2" : ""}`}>b</Button><Button className="flex-1">c</Button>');
    expect(got.map((c) => SIZE.test(c))).toEqual([true, true, false]);
  });
  it("has no h-*, px-* or text-size classes on <Button>", () => {
    const bad = tsx.flatMap((p) => buttonClasses(read(p)).filter((c) => SIZE.test(c)).map((c) => `${p}: ${c}`));
    expect(bad).toEqual([]);
  });
});
