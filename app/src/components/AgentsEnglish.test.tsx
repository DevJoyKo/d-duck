// tasks.md 3.3: the agents page, the agent settings sheet and the permissions page render without Korean in English.
// Mock data plays the user's data (agent names, descriptions, rule text) and the bundled Rust data (team defaults,
// permission catalog, translated by group G7), so every string the mock returns is removed before the check.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";
import { invoke as mockInvoke } from "../mock/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args ?? {}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { AgentsPage } = await import("./AgentsPage");
const { PermissionsPage } = await import("./PermissionsPage");
const { tokenText } = await import("../lib/loadout");
const { statusLine } = await import("../lib/agents");
const { parseJson, checkRule } = await import("../lib/permissions");

const HANGUL = /[가-힣]/;
/** Not this group's text: the shared scope bar (ScopeBar.tsx) and the scope names the backend's effective check
 * returns (`byScope`, Rust permissions code, tasks.md 4.1). */
const OUTSIDE = ["범위", "프로젝트 바꾸기 (⌘P)", "프로젝트 고르기", "전역", "이 범위"];
const DATA_CMDS: [string, Record<string, unknown>][] = [
  ["agent_catalog", { cwd: null }],
  ["agent_activity", {}],
  ["team_config", {}],
  ["agent_controls", {}],
  ["loadout_options", {}],
  ["permission_scopes", { project: null }],
];

function leaves(v: unknown, out: string[] = []): string[] {
  if (typeof v === "string") out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => leaves(x, out));
  else if (v && typeof v === "object") Object.values(v).forEach((x) => leaves(x, out));
  return out;
}

let data: string[] = [];
/** Page text (and labels) left after removing every string the mock returned. */
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
  const all = await Promise.all(DATA_CMDS.map(([c, a]) => mockInvoke(c, a)));
  // Only Korean leaves matter; short English ones ("allow", "t") would cut into the app's own words.
  data = [...new Set([...leaves(all), ...OUTSIDE].filter((s) => HANGUL.test(s)))].sort((a, b) => b.length - a.length);
});

afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("agents group in English", () => {
  it("agents page and the settings sheet have no app-added Korean", async () => {
    wrap(<AgentsPage onOpenTask={() => {}} onOpenBlocked={() => {}} capacity={3} />);
    await screen.findByText("Default Claude");
    await screen.findByText("design-critic");
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getByRole("button", { name: "Agent settings" }));
    await screen.findByRole("dialog", { name: "Agent settings" });
    await screen.findByText("On applies to Claude Code, Shown applies only to D.Duck");
    await waitFor(() => expect(screen.getAllByRole("switch").length).toBeGreaterThan(0));
    fireEvent.click(screen.getAllByRole("button", { name: /Loadout/ })[0]);
    await screen.findByText("Skills");
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getByRole("radio", { name: "Team rules" }));
    await screen.findByText("Brief refining guidance");
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getByRole("radio", { name: "Task types" }));
    await screen.findByText("Default permission");
    expect(korean()).toEqual([]);
  });

  it("permissions page has no app-added Korean on either tab", async () => {
    wrap(<PermissionsPage />);
    await screen.findByText("Rules you added");
    await screen.findByText("Effective check");
    expect(korean()).toEqual([]);

    fireEvent.click(screen.getByRole("radio", { name: "Agents" }));
    await screen.findByText("tools (allow list)");
    expect(korean()).toEqual([]);
  });

  it("lib text follows the language", () => {
    expect(tokenText(48_000)).toBe("About 12k tokens");
    expect(tokenText(1000)).toBe("Under 1k tokens");
    expect(statusLine({ agent: "x", state: "working", count: 2, total: 3, detail: null, sessionKey: null, subagent: false } as never)).toMatchObject({ head: "Working · 2 tasks", detail: "1 other task" });
    expect(checkRule(" Read")).toBe("Has leading or trailing spaces");
    const base = { allow: [], ask: [], deny: [], defaultMode: null, other: [], keys: [] } as never;
    expect(parseJson("[]", base)).toEqual({ error: "permissions must be a { } object" });
    expect(parseJson("{", base)).toMatchObject({ error: expect.stringMatching(/^JSON syntax error/) });
  });
});
