#!/usr/bin/env node
// sheet-focus 3.2: keyboard walk of the six modal sheets in the mock UI with native Tab (Playwright).
// For each sheet: open it from its control, record the initially focused element, press Tab ×25 then Shift+Tab ×25
// checking after every press that focus is inside the sheet, take a screenshot (light 1440, dark 900), then press
// Escape and check that focus went back to the expected control. Native confirm dialogs are dismissed.
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

const STEPS = 25;
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
    let outside = 0;
    for (let i = 0; i < STEPS * 2; i++) {
      await page.keyboard.press(i < STEPS ? "Tab" : "Shift+Tab");
      if (!(await page.evaluate(probe)).inside) outside++;
    }
    const shot = `${s.id}-${theme}-${width}.png`;
    await page.screenshot({ path: join(HERE, shot) });
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
    const after = await page.evaluate(probe);
    const back = s.back ? s.back(page) : s.id === "news" ? page.locator("[data-news-row]").first() : opener;
    const returned = (await back.count()) > 0 && (await back.evaluate((el) => el === document.activeElement));
    const initialOk = (s.initial ?? /^button "닫기"$/).test(initial.el);
    const ok = initial.inside && initialOk && outside === 0 && !after.open && returned;
    if (!ok) failures++;
    rows.push({ sheet: s.id, theme, width, initial: initial.el, initialInside: initial.inside, initialOk, presses: STEPS * 2, outside, closed: !after.open, focusAfter: after.el, returned, shot, ok });
    console.log(`${ok ? "PASS" : "FAIL"} ${s.id} ${theme}/${width} initial=${initial.el} outside=${outside}/${STEPS * 2} closed=${!after.open} after=${after.el} returned=${returned}`);
    await ctx.close();
  }
}
await browser.close();
writeFileSync(join(HERE, "walk.json"), JSON.stringify({ base: BASE, rows }, null, 2) + "\n");
process.exit(failures ? 1 : 0);
