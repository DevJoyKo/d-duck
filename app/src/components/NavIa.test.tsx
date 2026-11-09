// nav-ia: 확장 (skills + plugins), MCP tab, sidebar groups and old page ids — whole app on the mock backend.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { extRows, filterRows, copyName, copyTarget, countFor } from "./ExtensionsPage";
import type { Plugin, Skill } from "../lib/api";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);

async function boot(page: string | null, search = ""): Promise<Invoke> {
  if (page) localStorage.setItem("dduck.page", page);
  window.history.replaceState(null, "", `/${search}`);
  const core = await import("@tauri-apps/api/core");
  const { default: App } = await import("../App");
  render(<App />);
  return core.invoke as unknown as Invoke;
}

beforeEach(async () => {
  vi.resetModules();
  // The mocked invoke outlives resetModules; start every test with no recorded calls.
  ((await import("@tauri-apps/api/core")).invoke as unknown as Invoke).mockClear();
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
});

const mcpStatusCalls = (invoke: Invoke) => invoke.mock.calls.filter(([c]) => c === "mcp_status").length;

describe("MCP tab", () => {
  it("lists servers from the settings files at once and loads the connectors from home when the tab opens (D-138)", async () => {
    const invoke = await boot("mcp");
    expect(await screen.findByText("notion")).toBeTruthy();
    // Opening the tab checks once, from the home folder (no project), never an unconfirmed project's .mcp.json.
    await waitFor(() => expect(mcpStatusCalls(invoke)).toBe(1));
    expect(invoke).toHaveBeenCalledWith("mcp_status", { project: null });
    expect((await screen.findAllByText("연결됨")).length).toBeGreaterThan(0);
    // 전역 leaves out this project's servers (figma is local scope); the project side shows them.
    expect(screen.queryByText("figma")).toBeNull();
    fireEvent.click(await screen.findByRole("radio", { name: /dev_team/ }));
    fireEvent.click(await screen.findByRole("option", { name: /^dev_team/ }));
    expect(await screen.findByText("figma")).toBeTruthy();
    expect(screen.queryByText("notion")).toBeNull();
    // 다시 확인 checks in the current project.
    fireEvent.click(screen.getByRole("button", { name: "다시 확인" }));
    await waitFor(() => expect(mcpStatusCalls(invoke)).toBe(2));
    expect(invoke).toHaveBeenLastCalledWith("mcp_status", { project: "1f3a5c7e9b2d4f60" });
  });

  it("lists every claude.ai connector in its own group on both sides and keeps the result after leaving the tab", async () => {
    const invoke = await boot("mcp");
    const head = await screen.findByRole("heading", { name: /^claude\.ai 커넥터 9/ }, { timeout: 3000 });
    const sheet = head.parentElement as HTMLElement;
    expect(within(sheet).getAllByRole("listitem")).toHaveLength(9);
    expect(within(sheet.querySelector("ul") as HTMLElement).queryByText(/^claude\.ai /)).toBeNull(); // the group says it; rows drop the prefix
    expect(screen.queryByTestId("claude-ai-hint")).toBeNull();
    // The project side lists them too: they come with the account, not a settings file.
    fireEvent.click(await screen.findByRole("radio", { name: /dev_team/ }));
    fireEvent.click(await screen.findByRole("option", { name: /^dev_team/ }));
    expect(await screen.findByRole("heading", { name: /^claude\.ai 커넥터/ })).toBeTruthy();
    // Leave and come back: the result is still there and nothing ran again.
    fireEvent.click(screen.getByRole("button", { name: "메모리" }));
    fireEvent.click(await screen.findByRole("button", { name: "MCP" }));
    expect(await screen.findByRole("heading", { name: /^claude\.ai 커넥터/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "다시 확인" })).toBeTruthy();
    expect(mcpStatusCalls(invoke)).toBe(1);
  });

  it("does not run again when the tab is left mid-check and opened again", async () => {
    const invoke = await boot("mcp");
    expect(await screen.findByTestId("claude-ai-hint")).toBeTruthy(); // loading card while the first run is out
    fireEvent.click(screen.getByRole("button", { name: "메모리" }));
    fireEvent.click(await screen.findByRole("button", { name: "MCP" }));
    await screen.findByRole("heading", { name: /^claude\.ai 커넥터 9/ }, { timeout: 3000 });
    expect(mcpStatusCalls(invoke)).toBe(1);
  });

  it("settings links to the MCP tab", async () => {
    const invoke = await boot("settings");
    await new Promise((r) => setTimeout(r, 150));
    expect(mcpStatusCalls(invoke)).toBe(0);
    fireEvent.click(await screen.findByRole("button", { name: "MCP 탭 열기" }));
    expect(await screen.findByRole("heading", { name: "MCP" })).toBeTruthy();
  });
});

describe("sidebar groups", () => {
  it("shows 작업 공간 and Claude 구성 in order, then usage and settings, and no project picker", async () => {
    await boot("agents");
    const work = await screen.findByRole("group", { name: "작업 공간" });
    expect(within(work).getAllByRole("button").map((b) => b.textContent?.trim())).toEqual(["에이전트", "작업", "리뷰", "루틴", "아티팩트"]);
    const config = screen.getByRole("group", { name: "Claude 구성" });
    expect(within(config).getAllByRole("button").map((b) => b.textContent)).toEqual(["확장", "MCP", "권한", "메모리"]);
    const nav = screen.getByRole("navigation");
    const order = ["사용 통계", "설정"].map((n) => within(nav).getByRole("button", { name: n }));
    expect(within(nav).queryByRole("button", { name: /현재 프로젝트/ })).toBeNull();
    expect(config.compareDocumentPosition(order[0]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

describe("old page ids", () => {
  it("a remembered `plugins` opens 확장 with the 플러그인 chip", async () => {
    await boot("plugins");
    expect(await screen.findByRole("heading", { name: "확장" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /^플러그인/, pressed: true })).toBeTruthy();
  });
});

describe("확장 tab", () => {
  it("스킬 chip + search finds my skill and, inside its plugin, the plugin's", async () => {
    await boot("extensions");
    fireEvent.click(await screen.findByRole("button", { name: /^스킬/ }));
    fireEvent.change(screen.getByPlaceholderText("이름·설명 검색"), { target: { value: "design" } });
    expect(await screen.findByText("design-review")).toBeTruthy();
    // The plugin row opens by itself and lists its skill by the short name, once.
    expect(screen.getByRole("button", { name: /frontend-design 구성 요소 접기/ })).toBeTruthy();
    expect(screen.getAllByTitle("frontend-design:frontend-design — 보기")).toHaveLength(1);
    expect(screen.queryByText("standup")).toBeNull();
  });

  it("전체 is grouped by kind with plugins last, and plugin items are not repeated outside", async () => {
    await boot("extensions");
    const heads = await waitFor(() => {
      const h = screen.getAllByRole("heading", { level: 2 }).map((x) => x.textContent?.replace(/\d+$/, "").trim());
      if (!h.includes("플러그인")) throw new Error("not loaded");
      return h;
    });
    expect(heads).toEqual(["스킬", "커맨드", "워크플로", "플러그인"]);
    expect(screen.queryByText("cloudflare:wrangler")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /cloudflare 구성 요소 펼치기/ }));
    expect(await screen.findByTitle("cloudflare:wrangler — 보기")).toBeTruthy();
  });

  it("project side: chip counts follow the scope, project plugins count, empty line when nothing", async () => {
    await boot("extensions");
    await screen.findByText("design-review");
    const allCount = Number(screen.getByRole("button", { name: /^전체/ }).textContent?.replace(/\D/g, ""));
    // The project side opens the picker; picking the project switches to it.
    fireEvent.click(await screen.findByRole("radio", { name: /dev_team/ }));
    fireEvent.click(await screen.findByRole("option", { name: /^dev_team/ }));
    await waitFor(() => expect(Number(screen.getByRole("button", { name: /^전체/ }).textContent?.replace(/\D/g, ""))).toBeLessThan(allCount));
    expect(screen.queryByText("design-review")).toBeNull();
    expect(screen.getByText("team-lint")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^워크플로/ }));
    fireEvent.change(screen.getByPlaceholderText("이름·설명 검색"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: /^플러그인/ }));
    expect(screen.queryByText("이 프로젝트에 둔 항목이 없어요")).toBeNull();
  });

  it("every chip's count equals the sum of its section header numbers, on both scope sides", async () => {
    await boot("extensions");
    await screen.findByText("design-review");
    const sectionSum = () =>
      screen
        .queryAllByRole("heading", { level: 2 })
        .map((h) => Number(h.querySelector("span")?.textContent ?? NaN))
        .filter((n) => !Number.isNaN(n))
        .reduce((a, b) => a + b, 0);
    const check = async (side: string) => {
      for (const name of ["전체", "스킬", "커맨드", "워크플로", "플러그인"]) {
        const chip = screen.getByRole("button", { name: new RegExp(`^${name}`) });
        fireEvent.click(chip);
        await waitFor(() => expect(chip.getAttribute("aria-pressed")).toBe("true"));
        const count = Number(chip.textContent?.replace(/\D/g, ""));
        expect(sectionSum(), `${side} ${name}`).toBe(count);
      }
    };
    await check("전역");
    expect(Number(screen.getByRole("button", { name: /^전체/ }).textContent?.replace(/\D/g, ""))).toBeGreaterThan(0);
    fireEvent.click(await screen.findByRole("radio", { name: /dev_team/ }));
    fireEvent.click(await screen.findByRole("option", { name: /^dev_team/ }));
    await screen.findByText("team-lint");
    await check("프로젝트");
  });

  it("copies a locked plugin skill to mine from inside its plugin and opens the editor", async () => {
    const invoke = await boot("extensions");
    fireEvent.change(await screen.findByPlaceholderText("이름·설명 검색"), { target: { value: "frontend-design:" } });
    fireEvent.click(await screen.findByRole("button", { name: /복사해서 내 것으로/ }));
    const panel = await screen.findByRole("group", { name: "복사해서 내 것으로" });
    expect((within(panel).getByLabelText("새 이름") as HTMLInputElement).value).toBe("frontend-design-custom");
    expect(within(panel).getByText("→ ~/.claude/skills/frontend-design-custom")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "복사" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("copy_item", { itemId: "s-frontenddesignfrontenddesignskill", dest: "user", name: "frontend-design-custom", project: null }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("read_authoring_item", expect.anything()));
  });
});

describe("list model", () => {
  const skill = (name: string, source: Skill["source"], extra: Partial<Skill> = {}): Skill => ({ name, source, scope: null, kind: "skill", description: null, uses: 0, lastUsed: null, defined: true, ...extra });
  const plugin = { id: "kit@m", name: "kit", marketplace: "m", enabled: false, projectEnabled: false } as Plugin;

  it("puts a plugin's skills inside it, marks the current project's and project plugins, enabled first", () => {
    const kit = { ...plugin, components: { skills: [{ name: "kit:a", description: null, uses: 0, lastUsed: null }], commands: [], agents: [], hooks: false, lsp: false, unreadable: false } } as unknown as Plugin;
    const team = { ...kit, id: "team@m", name: "team", scope: "project", enabled: true, components: { ...kit.components, skills: [] } } as unknown as Plugin;
    const rows = extRows([skill("kit:a", "plugin", { id: "x", readonly: "잠김" }), skill("b", "project", { scope: "/p/dev" }), skill("c", "project", { scope: "/p/other" }), skill("gone:z", "plugin")], [kit, team], "/p/dev");
    expect(rows.find((r) => r.name === "kit:a")).toBeUndefined();
    const k = rows.find((r) => r.name === "kit");
    expect(k?.pluginOff).toBe(true);
    expect(k?.inner?.map((i) => [i.name, i.skill?.id])).toEqual([["kit:a", "x"]]);
    // A plugin that is no longer installed keeps its skill as a plain row.
    expect(rows.find((r) => r.name === "gone:z")?.source).toBe("gone");
    expect(rows.find((r) => r.name === "b")?.current).toBe(true);
    expect(filterRows(rows, "all", "", "project").map((r) => r.name)).toEqual(["b", "team"]);
    // 전역 leaves out project skills and project-scope plugins.
    expect(filterRows(rows, "all", "", "global").map((r) => r.name)).toEqual(["gone:z", "kit"]);
    expect(filterRows(rows, "plugin", "", "global").map((r) => r.name)).toEqual(["kit"]);
    // Chip counts are rows, the same basis as the section headers (kit is one row however many skills it brings).
    expect(countFor(rows, "skill", "", "global")).toBe(2);
    expect(countFor(rows, "skill", "", "project")).toBe(1);
    expect(filterRows(rows, "skill", "kit:", "global").map((r) => r.name)).toEqual(["kit"]);
  });

  it("counts a parent folder's items as the current project's, but not a sibling prefix (D-169)", () => {
    const rows = extRows([skill("up", "project", { scope: "/ws" }), skill("near", "project", { scope: "/ws/tri" })], [], "/ws/trials");
    expect(rows.find((r) => r.name === "up")?.current).toBe(true);
    expect(rows.find((r) => r.name === "near")?.current).toBe(false);
  });

  it("keeps plugin rows where they were loaded after a toggle", () => {
    const a = { ...plugin, id: "a@m", name: "a", enabled: true, components: { skills: [], commands: [], agents: [], hooks: false, lsp: false, unreadable: false } } as unknown as Plugin;
    const b = { ...a, id: "b@m", name: "b", enabled: false } as Plugin;
    const order = extRows([], [a, b], null).map((r) => r.plugin?.id ?? "");
    expect(extRows([], [{ ...a, enabled: false }, { ...b, enabled: true }], null, order).map((r) => r.name)).toEqual(["a", "b"]);
  });

  it("shows where a copy lands", () => {
    expect(copyTarget("skill", "user", "x", null)).toBe("~/.claude/skills/x");
    expect(copyTarget("command", "project", "y", "/p/dev/")).toBe("/p/dev/.claude/commands/y.md");
  });

  it("names copies <name>-custom, then -2", () => {
    expect(copyName("frontend-design", [])).toBe("frontend-design-custom");
    expect(copyName("frontend-design", ["frontend-design-custom"])).toBe("frontend-design-custom-2");
  });
});
