// plugin-bulk-update (D-203): checkboxes, "모두/선택한 것 업데이트 (N)", row progress, stop and the summary, on the mock
// backend. The confirmation itself is native (plugin_bulk.rs); here `update_plugins` stands in for it.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Plugin, PluginBulkResult } from "../lib/api";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);
let real: (c: string, a?: Record<string, unknown>) => Promise<unknown>;

async function boot(): Promise<Invoke> {
  localStorage.setItem("dduck.page", "extensions");
  const core = await import("@tauri-apps/api/core");
  const { default: App } = await import("../App");
  render(<App />);
  await screen.findByRole("button", { name: "cloudflare 구성 요소 펼치기" }, { timeout: 3000 });
  return core.invoke as unknown as Invoke;
}

/** The list item of a plugin row. */
const row = (name: string) => screen.getByRole("button", { name: `${name} 구성 요소 펼치기` }).closest("li") as HTMLElement;
const box = (name: string) => screen.queryByRole("checkbox", { name: `${name} 선택` }) as HTMLInputElement | null;

beforeEach(async () => {
  vi.resetModules();
  const core = (await import("@tauri-apps/api/core")).invoke as unknown as Invoke;
  core.mockClear();
  real = (await vi.importActual<typeof import("../mock/core")>("../mock/core")).invoke as typeof real;
  core.mockImplementation(real);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("what can be picked (3-state rule)", async () => {
  const { allTargets, selectable, countItems, bulkStore, MAX_BULK } = await import("../lib/pluginBulk");
  const pl = (id: string, state: "available" | "latest" | "unknown", extra: Partial<Plugin> = {}): Plugin =>
    ({ id, name: id, scope: "user", locked: null, update: state === "available" ? { state, latest: "2", marketplaceUpdated: null } : { state, marketplaceUpdated: null }, ...extra }) as Plugin;

  it("global plugins that are not up to date get a checkbox; 모두 takes only known updates", () => {
    const list = [pl("a@m", "available"), pl("u@m", "unknown"), pl("l@m", "latest"), pl("corp@m", "available", { scope: "managed", locked: "관리자" }), pl("team@m", "available", { scope: "project", locked: "프로젝트" })];
    expect(list.filter(selectable).map((p) => p.id)).toEqual(["a@m", "u@m"]);
    expect(allTargets(list)).toEqual(["a@m"]);
  });

  it("counts each outcome once for the summary", () => {
    const c = countItems([
      { id: "a", status: { kind: "finished", outcome: { kind: "done" } } },
      { id: "b", status: { kind: "finished", outcome: { kind: "failed" } } },
      { id: "c", status: { kind: "finished", outcome: { kind: "timeout" } } },
      { id: "d", status: { kind: "finished", outcome: { kind: "needsApproval" } } },
      { id: "e", status: { kind: "finished", outcome: { kind: "unchanged" } } },
      { id: "f", status: { kind: "stopped" } },
      { id: "g", status: { kind: "skipped", reason: "changed" } },
    ]);
    expect(c).toEqual({ updated: 1, failed: 2, approval: 1, unchanged: 1, stopped: 1, skipped: 1 });
  });

  it("selects at most fifteen", () => {
    bulkStore.reset();
    for (let i = 0; i < 20; i++) bulkStore.toggle(`p${i}@m`, true);
    expect(bulkStore.get().selected).toHaveLength(MAX_BULK);
    bulkStore.toggle("p0@m", false);
    expect(bulkStore.get().selected).toHaveLength(MAX_BULK - 1);
    bulkStore.reset();
  });
});

describe("확장 tab: update several plugins", () => {
  it("offers checkboxes only on plugins that can be updated here, and counts them", async () => {
    const invoke = await boot();
    for (const n of ["cloudflare", "expo", "frontend-design", "notes-kit"]) expect(box(n), n).toBeTruthy();
    for (const n of ["agent-skills", "corp-guard", "team-lint"]) expect(box(n), n).toBeNull();
    expect(screen.getByRole("button", { name: "모두 업데이트 (3)" })).toBeTruthy();
    const selected = screen.getByRole("button", { name: "선택한 것 업데이트 (0)" }) as HTMLButtonElement;
    expect(selected.disabled).toBe(true);
    fireEvent.click(box("notes-kit")!);
    fireEvent.click(box("cloudflare")!);
    const two = screen.getByRole("button", { name: "선택한 것 업데이트 (2)" }) as HTMLButtonElement;
    expect(two.disabled).toBe(false);
    fireEvent.click(two);
    // List order, not click order.
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_plugins", { ids: ["cloudflare@cloudflare", "notes-kit@personal"] }));
  });

  it("모두 업데이트 runs them one by one and ends with one summary line", async () => {
    const invoke = await boot();
    fireEvent.click(screen.getByRole("button", { name: "모두 업데이트 (3)" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_plugins", { ids: ["cloudflare@cloudflare", "frontend-design@claude-plugins-official", "expo@claude-plugins-official"] }));
    expect(await screen.findByText("1개 업데이트, 1개 실패, 1개 승인 필요", {}, { timeout: 3000 })).toBeTruthy();
    expect(within(row("cloudflare")).getByText("업데이트했어요. Claude Code를 다시 시작하면 적용돼요.")).toBeTruthy();
    expect(within(row("frontend-design")).getByText("실패했어요 · 터미널에서 claude plugin으로 확인해 주세요")).toBeTruthy();
    expect(within(row("expo")).getByText("승인이 필요해 실행하지 않았어요 · 터미널에서 업데이트해 주세요")).toBeTruthy();
    // The list was read again: cloudflare is on 1.0.3 and no longer offered.
    await waitFor(() => expect(box("cloudflare")).toBeNull());
    expect(invoke.mock.calls.filter(([c]) => c === "plugin_list").length).toBeGreaterThan(1);
  });

  it("shows waiting and updating per row, locks the other controls, and stop leaves the rest unrun", async () => {
    const invoke = await boot();
    // The same module the app listens on (the mocked @tauri-apps/api/event).
    const { emit } = (await import("@tauri-apps/api/event")) as unknown as typeof import("../mock/event");
    let release: () => void = () => {};
    let stopped = false;
    invoke.mockImplementation(async (c, a) => {
      if (c === "stop_plugin_updates") {
        stopped = true;
        return undefined;
      }
      if (c !== "update_plugins") return real(c, a);
      emit("plugin-update-progress", { id: "cloudflare@cloudflare", status: { kind: "running" } });
      await new Promise<void>((r) => (release = r));
      const first = { id: "cloudflare@cloudflare", status: { kind: "finished", outcome: { kind: "done" } } };
      const rest = ["expo@claude-plugins-official", "frontend-design@claude-plugins-official"].map((id) => ({ id, status: stopped ? { kind: "stopped" } : { kind: "finished", outcome: { kind: "done" } } }));
      return { confirmed: true, items: [first, ...rest] } as PluginBulkResult;
    });
    fireEvent.click(screen.getByRole("button", { name: "모두 업데이트 (3)" }));
    expect(await within(row("cloudflare")).findByText("업데이트하는 중이에요")).toBeTruthy();
    expect(within(row("expo")).getByText("기다리는 중이에요")).toBeTruthy();
    expect(document.querySelector("[data-bulk-status]")!.textContent).toBe("업데이트 중 1/3");
    expect((screen.getByRole("switch", { name: "cloudflare 끄기" }) as HTMLButtonElement).disabled).toBe(true);
    expect(box("notes-kit")!.disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "멈추기" }));
    expect(screen.getByRole("button", { name: "멈추는 중…" }).getAttribute("aria-disabled")).toBe("true");
    expect(invoke).toHaveBeenCalledWith("stop_plugin_updates");
    release();
    expect(await screen.findByText("1개 업데이트, 2개 멈춤")).toBeTruthy();
    expect(within(row("expo")).getByText("멈췄어요 · 실행하지 않았어요")).toBeTruthy();
    expect((screen.getByRole("switch", { name: "notes-kit 끄기" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("a declined confirmation changes nothing and keeps the selection", async () => {
    const invoke = await boot();
    invoke.mockImplementation((c, a) => (c === "update_plugins" ? Promise.resolve({ confirmed: false, items: [] }) : real(c, a)));
    fireEvent.click(box("notes-kit")!);
    fireEvent.click(screen.getByRole("button", { name: "선택한 것 업데이트 (1)" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_plugins", { ids: ["notes-kit@personal"] }));
    await waitFor(() => expect(box("notes-kit")!.disabled).toBe(false));
    expect(box("notes-kit")!.checked).toBe(true);
    expect(document.querySelector("[data-bulk-status]")!.textContent).toBe("");
    expect(screen.queryByText("기다리는 중이에요")).toBeNull();
  });

  it("a plugin skipped because it changed says so; a refused batch shows the error", async () => {
    const invoke = await boot();
    invoke.mockImplementation((c, a) =>
      c === "update_plugins" ? Promise.resolve({ confirmed: true, items: [{ id: "notes-kit@personal", status: { kind: "skipped", reason: "changed" } }] }) : real(c, a),
    );
    fireEvent.click(box("notes-kit")!);
    fireEvent.click(screen.getByRole("button", { name: "선택한 것 업데이트 (1)" }));
    expect(await within(row("notes-kit")).findByText("건너뜀 · 그사이 바뀌었어요")).toBeTruthy();
    expect(screen.getByText("1개 건너뜀")).toBeTruthy();
    invoke.mockImplementation((c, a) => (c === "update_plugins" ? Promise.reject("다른 플러그인 작업이 진행 중이에요") : real(c, a)));
    fireEvent.click(screen.getByRole("button", { name: "선택한 것 업데이트 (1)" }));
    expect((await screen.findByRole("alert")).textContent).toContain("다른 플러그인 작업이 진행 중이에요");
  });

  it("keeps the progress after leaving the tab and coming back", async () => {
    const invoke = await boot();
    const { emit } = (await import("@tauri-apps/api/event")) as unknown as typeof import("../mock/event");
    let release: () => void = () => {};
    invoke.mockImplementation(async (c, a) => {
      if (c !== "update_plugins") return real(c, a);
      emit("plugin-update-progress", { id: "cloudflare@cloudflare", status: { kind: "running" } });
      await new Promise<void>((r) => (release = r));
      return { confirmed: true, items: [{ id: "cloudflare@cloudflare", status: { kind: "finished", outcome: { kind: "done" } } }] };
    });
    fireEvent.click(box("cloudflare")!);
    fireEvent.click(screen.getByRole("button", { name: "선택한 것 업데이트 (1)" }));
    expect(await within(row("cloudflare")).findByText("업데이트하는 중이에요")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^MCP/ }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "cloudflare 구성 요소 펼치기" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /^확장/ }));
    expect(await within(await screen.findByRole("button", { name: "cloudflare 구성 요소 펼치기" }).then((b) => b.closest("li") as HTMLElement)).findByText("업데이트하는 중이에요")).toBeTruthy();
    expect(screen.getByRole("button", { name: "멈추기" })).toBeTruthy();
    release();
    expect(await screen.findByText("1개 업데이트")).toBeTruthy();
  });

  it("the bar stays when a search hides every plugin", async () => {
    await boot();
    fireEvent.change(screen.getByPlaceholderText(/검색/), { target: { value: "zzz-no-such-thing" } });
    await waitFor(() => expect(screen.queryByRole("button", { name: "cloudflare 구성 요소 펼치기" })).toBeNull());
    expect(screen.getByRole("button", { name: "모두 업데이트 (3)" })).toBeTruthy();
  });

  it("a single update after a batch replaces the old batch line", async () => {
    const invoke = await boot();
    invoke.mockImplementation((c, a) =>
      c === "update_plugins" ? Promise.resolve({ confirmed: true, items: [{ id: "notes-kit@personal", status: { kind: "finished", outcome: { kind: "failed" } } }] }) : real(c, a),
    );
    fireEvent.click(box("notes-kit")!);
    fireEvent.click(screen.getByRole("button", { name: "선택한 것 업데이트 (1)" }));
    expect(await within(row("notes-kit")).findByText(/^실패했어요/)).toBeTruthy();
    fireEvent.click(within(row("notes-kit")).getByRole("button", { name: "업데이트 시도" }));
    expect(await within(row("notes-kit")).findByText("바뀐 것이 없어요. 이미 최신일 수 있어요.")).toBeTruthy();
    expect(within(row("notes-kit")).queryByText(/^실패했어요/)).toBeNull();
  });
});

describe("fifteen at a time", async () => {
  const { PluginBulkBar } = await import("./PluginBulk");
  it("모두 counts at most fifteen and says so", () => {
    const many = Array.from({ length: 18 }, (_, i) => ({ id: `p${i}@m`, name: `p${i}`, scope: "user", locked: null, update: { state: "available", latest: "2", marketplaceUpdated: null } }) as unknown as Plugin);
    render(<PluginBulkBar plugins={many} busy={false} onSettled={() => {}} />);
    expect(screen.getByRole("button", { name: "모두 업데이트 (15)" })).toBeTruthy();
    expect(screen.getByText("한 번에 15개까지 해요")).toBeTruthy();
  });
});
