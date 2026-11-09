// tasks.md 3.4: the extensions page (with the add sheet), the MCP tab and the memory tab (memory, rules, CLAUDE.md)
// render without app-added Korean in English. Mock data plays the user's data (names, descriptions, paths) and the
// text Rust sends (read-only reasons, warnings; tasks.md 4.1), so every string the mock returns is removed first.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";
import { invoke as mockInvoke } from "../mock/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args ?? {}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { ExtensionsPage } = await import("./ExtensionsPage");
const { McpPage } = await import("./McpPage");
const { MemoryPage } = await import("./MemoryPage");
const { scopeTitle, summaryText, globWarnings, patternProblem } = await import("../lib/rules");
const { nameError, targetPath } = await import("../lib/addItems");

const HANGUL = /[가-힣]/;
/** Not this group's text: the shared scope bar (ScopeBar.tsx) and the claude.ai login line (lib/auth COPY, 3.7). */
const OUTSIDE = ["범위", "프로젝트 바꾸기 (⌘P)", "프로젝트 고르기", "전역", "이 범위"];
const DATA_CMDS: [string, Record<string, unknown>][] = [
  ["skill_catalog", {}],
  ["plugin_list", {}],
  ["authoring_info", {}],
  ["authoring_jobs", {}],
  ["agent_catalog", { cwd: null }],
  ["mcp_servers", { project: null }],
  ["mcp_status", { project: null }],
  ["auto_memory_setting", {}],
  ["rules_list", { project: null }],
  ["instruction_files", { project: null }],
];

function leaves(v: unknown, out: string[] = []): string[] {
  if (typeof v === "string") out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => leaves(x, out));
  else if (v && typeof v === "object") Object.values(v).forEach((x) => leaves(x, out));
  return out;
}

let data: string[] = [];
function appText(): string {
  const attrs = [...document.body.querySelectorAll("[aria-label],[title],[placeholder]")].flatMap((el) => ["aria-label", "title", "placeholder"].map((a) => el.getAttribute(a) ?? ""));
  let text = [document.body.textContent ?? "", ...attrs].join("\n");
  for (const d of data) text = text.split(d).join("");
  return text;
}
const korean = () => appText().split("\n").filter((l) => HANGUL.test(l));

const wrap = (ui: React.ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);

beforeEach(async () => {
  applyLanguage("en");
  const all = await Promise.all(DATA_CMDS.map(([c, a]) => mockInvoke(c, a).catch(() => null)));
  // Multi-line leaves (the bundled authoring templates Rust sends) are matched line by line, since the form fills them.
  const lines = leaves(all).flatMap((s) => [s, ...s.split("\n")]);
  data = [...new Set([...lines, ...OUTSIDE].filter((s) => HANGUL.test(s)))].sort((a, b) => b.length - a.length);
});

afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("extensions group in English", () => {
  it("extensions page, a plugin's parts and the three add panes have no app-added Korean", async () => {
    wrap(<ExtensionsPage />);
    await screen.findByRole("heading", { level: 2, name: /^Skills/ });
    await screen.findByRole("button", { name: "pr-writer" });
    expect(screen.getByRole("button", { name: /^All/ })).toBeTruthy();
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getAllByRole("button", { name: /Expand .* parts/ })[0]);
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getByRole("button", { name: /^Add/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Install from link/ }));
    await screen.findByRole("dialog", { name: "Add" });
    expect(korean()).toEqual([]);
    fireEvent.click(screen.getByRole("radio", { name: "Create" }));
    expect(screen.getByRole("button", { name: "Create skill" })).toBeTruthy();
    expect(korean()).toEqual([]);
    fireEvent.click(screen.getByRole("radio", { name: "Ask Claude" }));
    await screen.findByText("Requests");
    expect(korean()).toEqual([]);
  });

  it("MCP tab has no app-added Korean", async () => {
    wrap(<McpPage />);
    await screen.findByRole("heading", { level: 2, name: /^Global · All projects/ }, { timeout: 3000 });
    expect(korean()).toEqual([]);
  });

  it("memory tab: auto memory, rules and CLAUDE.md have no app-added Korean", async () => {
    wrap(<MemoryPage />);
    await screen.findByText("Each project has its own auto memory");
    expect(korean()).toEqual([]);
    fireEvent.click(screen.getByRole("radio", { name: "Rules" }));
    await screen.findByRole("button", { name: /New rule/ });
    expect(korean()).toEqual([]);
    fireEvent.click(screen.getByRole("radio", { name: "Instruction files (CLAUDE.md)" }));
    await screen.findByRole("button", { name: "View rules" });
    expect(korean()).toEqual([]);
  });

  it("lib helpers speak English", () => {
    expect(scopeTitle("Always")).toBe("Read in every conversation");
    expect(scopeTitle("src/api/**/*.ts")).toBe("Only when working on TS files in src/api");
    expect(summaryText({ alwaysCount: 1, alwaysBytes: 61 })).toBe("1 always-on rule · 61B total · Read in every conversation");
    expect(globWarnings(["~/x"]).join()).not.toMatch(HANGUL);
    expect(patternProblem(" x")).toBe("Remove leading and trailing spaces");
    expect(nameError("가나")).not.toMatch(HANGUL);
    expect(targetPath("workflow", null, "")).toBe("~/.claude/workflows/<name>.js");
  });
});
