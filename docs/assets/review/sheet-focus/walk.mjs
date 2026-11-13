#!/usr/bin/env node
// sheet-focus 3.2: keyboard walk of the six modal sheets in the mock UI with native Tab (Playwright).
// For each sheet: open it from its control, record the initially focused element, then go around once with Tab and
// once with Shift+Tab (stops + 2 presses each), checking after every press that focus is inside the sheet and moved,
// and at the end of each pass that every tab stop was visited. Then take a screenshot (light 1440, dark 900), press
// Escape and check that focus went back to the expected control. Native confirm dialogs are dismissed.
// The tab stops are counted in the page with the same rules as app/src/lib/useModal.ts `focusables` (kept in step).
//
// Usage: (cd app && npm run dev:mock) then
//   PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node docs/assets/review/sheet-focus/walk.mjs [--base http://localhost:1430/]
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const argBase = process.argv.indexOf("--base");
const BASE = argBase > 0 ? process.argv[argBase + 1] : "http://localhost:1430/";
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright");

// `opener`: the control activated with Enter to open the sheet. `back`: where focus must be after Escape (default: the
// opener). `initial`: the expected first focus (default: × "닫기"), from the initial-focus table in design.md.
const SHEETS = [
  { id: "agent-settings", page: "agents", opener: (p) => p.getByRole("button", { name: "에이전트 설정", exact: true }) },
  { id: "agent-settings-rules", page: "agents", opener: (p) => p.getByRole("button", { name: "에이전트 설정", exact: true }), after: (p) => p.getByRole("radio", { name: "팀 규칙" }).click() },
  { id: "agent-settings-types", page: "agents", opener: (p) => p.getByRole("button", { name: "에이전트 설정", exact: true }), after: (p) => p.getByRole("radio", { name: "작업 유형" }).click() },
  { id: "agent-definition", page: "agents", opener: (p) => p.getByRole("button", { name: "정의 고치기" }).first() },
  { id: "extension-editor", page: "extensions", opener: (p) => p.getByRole("button", { name: "수정", exact: true }).first() },
  {
    id: "add-sheet",
    page: "extensions",
    opener: (p) => p.getByRole("menuitem", { name: /^링크로 설치/ }),
    before: (p) => p.getByRole("button", { name: /^추가/ }).click(),
    back: (p) => p.locator("[data-add-trigger]"),
    initial: /^input /,
  },
  { id: "usage-session", page: "usage", opener: (p) => p.locator("table tbody button").first() },
  { id: "news", page: "news", opener: (p) => p.locator("[data-news-row]").first() },
];

// The focused element and whether it is inside the open modal sheet (the last aria-modal dialog).
const probe = () => {
  const a = document.activeElement;
  const dialogs = Array.from(document.querySelectorAll('[role="dialog"][aria-modal="true"]'));
  const d = dialogs[dialogs.length - 1] ?? null;
  const name = a ? (a.getAttribute("aria-label") || a.textContent || a.tagName).trim().replace(/\s+/g, " ").slice(0, 40) : "";
  return { open: !!d, inside: !!d && !!a && d.contains(a), el: a ? `${a.tagName.toLowerCase()} "${name}"` : "none" };
};

// Tab stops of the top sheet, marked with data-walk-stop; returns how many.
const markStops = () => {
  const dialogs = Array.from(document.querySelectorAll('[role="dialog"][aria-modal="true"]'));
  const d = dialogs[dialogs.length - 1];
  if (!d) return 0;
  const sel = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), details > summary:first-of-type, [tabindex]';
  const closedDetails = (el) => {
    for (let p = el.parentElement; p && p !== d; p = p.parentElement) {
      if (p.tagName === "DETAILS" && !p.open && !p.querySelector(":scope > summary")?.contains(el)) return true;
    }
    return false;
  };
  const all = Array.from(d.querySelectorAll(sel)).filter((el) => {
    if (el.getAttribute("tabindex") === "-1") return false;
    const off = el.closest('[hidden], [inert], [aria-hidden="true"]');
    if (off && d.contains(off)) return false;
    return !closedDetails(el) && (el.checkVisibility?.() ?? true);
  });
  const radios = new Map();
  for (const el of all) {
    if (el.tagName !== "INPUT" || el.type !== "radio" || !el.name) continue;
    const k = radios.get(el.name);
    if (!k || (!k.checked && el.checked)) radios.set(el.name, el);
  }
  const stops = all.filter((el) => !(el.tagName === "INPUT" && el.type === "radio" && el.name) || radios.get(el.name) === el);
  document.querySelectorAll("[data-walk-stop]").forEach((el) => el.removeAttribute("data-walk-stop"));
  stops.forEach((el) => el.setAttribute("data-walk-stop", ""));
  return stops.length;
};
// Marks the focused element as seen and returns a stable id for it.
const visit = () => {
  const a = document.activeElement;
  if (!a) return "";
  window.__walkId = (window.__walkId ?? 0) + 1;
  a.dataset.walkId ??= String(window.__walkId);
  a.setAttribute("data-walk-seen", "");
  return a.dataset.walkId;
};
const missed = () => Array.from(document.querySelectorAll("[data-walk-stop]:not([data-walk-seen])")).map((el) => (el.getAttribute("aria-label") || el.textContent || el.tagName).trim().slice(0, 30));
const clearSeen = () => document.querySelectorAll("[data-walk-seen]").forEach((el) => el.removeAttribute("data-walk-seen"));

const browser = await chromium.launch();
const rows = [];
let failures = 0;
for (const theme of ["light", "dark"]) {
  const width = theme === "light" ? 1440 : 900;
  for (const s of SHEETS) {
    const ctx = await browser.newContext({ viewport: { width, height: 900 }, colorScheme: theme, reducedMotion: "reduce" });
    await ctx.addInitScript((th) => {
      try {
        localStorage.setItem("dduck.theme", th);
        localStorage.setItem("dduck.tour.skip", "1");
      } catch {
        /* storage unavailable: defaults */
      }
    }, theme);
    const page = await ctx.newPage();
    page.on("dialog", (d) => d.dismiss().catch(() => {}));
    await page.goto(`${BASE}?page=${s.page}&lang=ko`);
    await page.waitForLoadState("networkidle");
    if (s.before) await s.before(page);
    const opener = s.opener(page);
    await opener.focus();
    await opener.press("Enter");
    await page.waitForSelector('[role="dialog"][aria-modal="true"]');
    await page.waitForTimeout(300);
    const initial = await page.evaluate(probe);
    if (s.after) {
      await s.after(page);
      await page.waitForTimeout(300);
    }
    const stops = await page.evaluate(markStops);
    let outside = 0;
    let stuck = 0;
    let presses = 0;
    const notVisited = [];
    for (const key of ["Tab", "Shift+Tab"]) {
      await page.evaluate(clearSeen);
      let prev = await page.evaluate(visit);
      await page.evaluate(clearSeen);
      for (let i = 0; i < stops + 2; i++) {
        await page.keyboard.press(key);
        presses++;
        if (!(await page.evaluate(probe)).inside) outside++;
        const id = await page.evaluate(visit);
        if (id === prev) stuck++;
        prev = id;
      }
      notVisited.push(...(await page.evaluate(missed)).map((m) => `${key}: ${m}`));
    }
    const shot = `${s.id}-${theme}-${width}.png`;
    await page.screenshot({ path: join(HERE, shot) });
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
    const after = await page.evaluate(probe);
    const back = s.back ? s.back(page) : s.id === "news" ? page.locator("[data-news-row]").first() : opener;
    const returned = (await back.count()) > 0 && (await back.evaluate((el) => el === document.activeElement));
    const initialOk = (s.initial ?? /^button "닫기"$/).test(initial.el);
    const ok = initial.inside && initialOk && stops > 1 && outside === 0 && stuck === 0 && notVisited.length === 0 && !after.open && returned;
    if (!ok) failures++;
    rows.push({ sheet: s.id, theme, width, initial: initial.el, initialInside: initial.inside, initialOk, stops, presses, outside, stuck, notVisited, closed: !after.open, focusAfter: after.el, returned, shot, ok });
    console.log(`${ok ? "PASS" : "FAIL"} ${s.id} ${theme}/${width} initial=${initial.el} stops=${stops} outside=${outside}/${presses} stuck=${stuck} missed=${notVisited.length} closed=${!after.open} after=${after.el} returned=${returned}`);
    await ctx.close();
  }
}
await browser.close();
writeFileSync(join(HERE, "walk.json"), JSON.stringify({ base: BASE, rows }, null, 2) + "\n");
process.exit(failures ? 1 : 0);
