#!/usr/bin/env node
// Re-takes every screen capture listed in docs/assets/shots.json from the mock UI (oss-publish-prep, design decision 5).
//
// The mock server is not started here. Run it first: `cd app && npm run dev:mock` (http://localhost:1430).
//
// Usage:
//   node docs/assets/capture.mjs                       # every capture shot, written to its `file` path
//   node docs/assets/capture.mjs --only 'readme/en/*'  # glob on shot id or file (repeatable, comma-separated)
//   node docs/assets/capture.mjs --dry                 # list what would be taken, take nothing
//   node docs/assets/capture.mjs --out /tmp/shots      # write under another root instead of the repo (same relative paths)
//   node docs/assets/capture.mjs --base http://localhost:1430/ --headed
//   PLAYWRIGHT=/path/to/playwright/index.mjs node docs/assets/capture.mjs
//
// Shot fields: id, file, lang (ko|en), theme (light|dark), size ("WxH@scale"), page, query (object → ?k=v),
// actions (below), wait (ms after actions), clip ({x,y,width,height} in CSS px), kind (capture|copy|video).
// kind "copy" copies `from` (not a screenshot); kind "video" belongs to the video task and is only listed.
//
// Actions, run in order (a failing step fails the shot unless the step has "optional": true):
//   {"do":"click", "role":"button", "name":"…", "exact":true, "regex":false, "last":false, "nth":0}
//   {"do":"click", "text":"…"}                 first element containing the text
//   {"do":"click", "selector":"css or playwright selector"}
//   {"do":"fill", "selector":"…", "value":"…"}
//   {"do":"press", "key":"Enter"}
//   {"do":"hover", "text"|"selector":"…"}
//   {"do":"scrollTo", "text"|"selector":"…", "exact":false, "block":"start|center|end"}
//   {"do":"scroll", "y":600}                   scrolls the main content area
//   {"do":"waitFor", "text":"…"} | {"do":"waitFor", "role":"dialog"} | {"do":"waitFor", "selector":"…"}
//   {"do":"wait", "ms":500}
//
// Common setup per shot: fresh context, tour/checklist state left to the mock default (tour already seen),
// localStorage `dduck.theme` = theme and `dduck.tour.skip` = 1, remembered page and agent order cleared,
// prefers-color-scheme = theme, reduced motion, carets hidden, native confirm dialogs accepted.

import { readFileSync, mkdirSync, copyFileSync, existsSync } from "node:fs";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "../..");
// Playwright module to import: set PLAYWRIGHT_MODULE to a local install path, or install `playwright` next to this repo.
const DEFAULT_PW = process.env.PLAYWRIGHT_MODULE || "playwright";

function parseArgs(argv) {
  const o = { only: [], dry: false, out: ROOT, base: null, headed: false, list: join(HERE, "shots.json") };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--only") o.only.push(...argv[++i].split(",").filter(Boolean));
    else if (a === "--dry") o.dry = true;
    else if (a === "--out") o.out = resolve(argv[++i]);
    else if (a === "--base") o.base = argv[++i];
    else if (a === "--list") o.list = resolve(argv[++i]);
    else if (a === "--headed") o.headed = true;
    else if (a === "-h" || a === "--help") {
      console.log(readFileSync(fileURLToPath(import.meta.url), "utf8").split("\n").filter((l) => l.startsWith("//")).join("\n"));
      process.exit(0);
    } else throw new Error(`unknown option ${a}`);
  }
  return o;
}

/** Glob → RegExp: `*` within a path segment, `**` across segments, `?` one char. */
export function globToRegExp(g) {
  let s = "";
  for (let i = 0; i < g.length; i++) {
    const c = g[i];
    if (c === "*" && g[i + 1] === "*") (s += ".*"), i++;
    else if (c === "*") s += "[^/]*";
    else if (c === "?") s += "[^/]";
    else s += c.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${s}$`);
}

const matches = (shot, globs) => globs.length === 0 || globs.some((g) => [shot.id, shot.file].some((v) => globToRegExp(g).test(v)));

function parseSize(size) {
  const m = /^(\d+)x(\d+)(?:@(\d+(?:\.\d+)?)x)?$/.exec(size ?? "");
  if (!m) throw new Error(`bad size "${size}"`);
  return { width: +m[1], height: +m[2], scale: m[3] ? +m[3] : 1 };
}

function urlOf(base, query) {
  const u = new URL(base);
  for (const [k, v] of Object.entries(query ?? {})) u.searchParams.set(k, String(v));
  return u.toString();
}

function locate(page, a) {
  if (a.role) {
    const name = a.regex ? new RegExp(a.name) : a.name;
    const loc = page.getByRole(a.role, { name, exact: a.regex ? undefined : (a.exact ?? false) });
    return a.last ? loc.last() : loc.nth(a.nth ?? 0);
  }
  if (a.text) return page.getByText(a.text, { exact: a.exact ?? false }).first();
  if (a.selector) return page.locator(a.selector).first();
  throw new Error(`action needs role, text or selector: ${JSON.stringify(a)}`);
}

async function run(page, a) {
  const t = { timeout: a.timeout ?? 5000 };
  switch (a.do) {
    case "click":
      return locate(page, a).click(t);
    case "fill":
      return locate(page, a).fill(a.value, t);
    case "press":
      return page.keyboard.press(a.key);
    case "hover":
      return locate(page, a).hover(t);
    case "scrollTo": {
      const el = locate(page, a);
      await el.waitFor({ state: "attached", ...t });
      return el.evaluate((n, block) => n.scrollIntoView({ block, inline: "nearest" }), a.block ?? "start");
    }
    case "scroll":
      return page.evaluate((y) => {
        const main = document.querySelector("main");
        const box = [main, ...(main ? main.querySelectorAll("*") : [])].find((n) => n && n.scrollHeight > n.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(n).overflowY));
        (box ?? document.scrollingElement).scrollBy(0, y);
      }, a.y ?? 600);
    case "waitFor":
      if (a.role) return page.getByRole(a.role).first().waitFor(t);
      return locate(page, a).waitFor(t);
    case "wait":
      return page.waitForTimeout(a.ms ?? 500);
    default:
      throw new Error(`unknown action "${a.do}"`);
  }
}

async function shoot(browser, shot, opts, defaults) {
  const theme = shot.theme ?? defaults.theme ?? "light";
  const { width, height, scale } = parseSize(shot.size ?? defaults.size);
  const ctx = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: scale,
    colorScheme: theme === "dark" ? "dark" : "light",
    reducedMotion: "reduce",
    locale: shot.lang === "en" ? "en-US" : "ko-KR",
  });
  try {
    await ctx.addInitScript((theme) => {
      try {
        localStorage.setItem("dduck.theme", theme);
        localStorage.setItem("dduck.tour.skip", "1");
        localStorage.removeItem("dduck.page");
        localStorage.removeItem("dduck.agentOrder.mode");
        localStorage.removeItem("dduck.agentOrder.custom");
      } catch {
        /* storage blocked: the mock defaults still apply */
      }
    }, theme);
    const page = await ctx.newPage();
    page.on("dialog", (d) => d.accept().catch(() => {}));
    await page.goto(urlOf(opts.base, shot.query ?? { page: shot.page, lang: shot.lang }), { waitUntil: "networkidle", timeout: 15000 });
    await page.addStyleTag({ content: "*{caret-color:transparent!important}" });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(500);
    for (const [i, a] of (shot.actions ?? []).entries()) {
      try {
        await run(page, a);
        await page.waitForTimeout(a.do === "wait" ? 0 : 250);
      } catch (e) {
        if (!a.optional) throw new Error(`step ${i + 1} ${JSON.stringify(a)}: ${String(e.message).split("\n")[0]}`);
      }
    }
    await page.waitForTimeout(shot.wait ?? defaults.wait ?? 600);
    const path = join(opts.out, shot.file);
    mkdirSync(dirname(path), { recursive: true });
    await page.screenshot({ path, clip: shot.clip, animations: "disabled" });
    return path;
  } finally {
    await ctx.close();
  }
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const list = JSON.parse(readFileSync(opts.list, "utf8"));
  opts.base ??= list.baseUrl ?? "http://localhost:1430/";
  const defaults = list.defaults ?? {};
  const picked = list.shots.filter((s) => matches(s, opts.only));
  const kind = (s) => s.kind ?? "capture";
  const todo = picked.filter((s) => kind(s) !== "video");
  const skipped = picked.filter((s) => kind(s) === "video");

  if (opts.dry) {
    for (const s of todo) {
      const how = kind(s) === "copy" ? `copy ${s.from}` : `${s.lang} ${s.theme ?? defaults.theme} ${s.size ?? defaults.size} ${urlOf(opts.base, s.query)} +${(s.actions ?? []).length} steps`;
      console.log(`${s.id.padEnd(36)} ${s.file}\n${"".padEnd(36)} ${how}`);
    }
    for (const s of skipped) console.log(`${s.id.padEnd(36)} ${s.file}  [skip: ${s.owner ?? "video"}]`);
    console.log(`\n${todo.length} to take, ${skipped.length} skipped (video), out=${opts.out}`);
    return;
  }

  try {
    const r = await fetch(opts.base);
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
  } catch (e) {
    console.error(`mock server not reachable at ${opts.base} (${e.message}). Start it: cd app && npm run dev:mock`);
    process.exit(2);
  }

  const pw = process.env.PLAYWRIGHT ?? DEFAULT_PW;
  const { chromium } = await import(pw.startsWith("/") || pw.startsWith(".") ? pathToFileURL(pw).href : pw);
  const browser = await chromium.launch({ headless: !opts.headed });
  const ok = [];
  const failed = [];
  try {
    for (const s of todo) {
      const t0 = Date.now();
      try {
        if (kind(s) === "copy") {
          const to = join(opts.out, s.file);
          mkdirSync(dirname(to), { recursive: true });
          copyFileSync(join(ROOT, s.from), to);
        } else await shoot(browser, s, opts, defaults);
        ok.push(s);
        console.log(`ok   ${s.id} (${Date.now() - t0} ms)`);
      } catch (e) {
        failed.push({ s, why: String(e.message).split("\n")[0] });
        console.log(`FAIL ${s.id}: ${String(e.message).split("\n")[0]}`);
      }
    }
  } finally {
    await browser.close();
  }
  console.log(`\n${ok.length} taken, ${failed.length} failed, ${skipped.length} skipped (video), out=${opts.out}`);
  if (failed.length) {
    console.log("Failed:");
    for (const { s, why } of failed) console.log(`  ${s.id}: ${why}`);
    process.exitCode = 1;
  }
  if (!existsSync(opts.out)) process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
