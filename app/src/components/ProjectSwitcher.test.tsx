// project-switcher: the whole app on the mock backend (per-tab picker in the ScopeBar, unsaved-edit guard).
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

async function boot(page: string, before?: (invoke: (c: string, a?: Record<string, unknown>) => Promise<unknown>) => Promise<void>) {
  localStorage.setItem("dduck.page", page);
  const core = await import("@tauri-apps/api/core");
  const invoke = core.invoke as unknown as ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);
  if (before) await before(invoke);
  const { default: App } = await import("../App");
  render(<App />);
  return invoke;
}

beforeEach(() => {
  vi.resetModules();
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

/** The project side of the open tab's ScopeBar (the picker trigger). */
const pickerButton = () => screen.findByRole("radio", { name: /dev_team|side-project|프로젝트 고르기|lint-sweep|me$/ });

describe("per-tab project picker (D-127)", () => {
  it("has no sidebar picker; pages without a project get no bar", async () => {
    await boot("routines");
    await screen.findByRole("heading", { name: "루틴" });
    expect(within(screen.getByRole("navigation")).queryByText("현재 프로젝트")).toBeNull();
    expect(screen.queryByRole("radiogroup", { name: "범위" })).toBeNull();
    expect(screen.queryByText("모든 프로젝트 화면이에요")).toBeNull();
  });

  it("marks 원격 push 막기 projects with a 로컬만 chip (delivery-guard)", async () => {
    await boot("permissions");
    await waitFor(async () => expect((await pickerButton()).textContent).toContain("dev_team"));
    act(() => {
      fireEvent.keyDown(window, { key: "p", metaKey: true });
    });
    const list = await screen.findByRole("listbox");
    const side = within(list).getAllByRole("option").find((o) => o.textContent?.includes("side-project"))!;
    const dev = within(list).getAllByRole("option").find((o) => o.textContent?.includes("dev_team"))!;
    expect(side.textContent).toContain("로컬만");
    expect(dev.textContent).not.toContain("로컬만");
  });

  it("opens with ⌘P on the tab, searches, picks with Enter and saves the id for that tab only", async () => {
    const invoke = await boot("permissions");
    await waitFor(async () => expect((await pickerButton()).textContent).toContain("dev_team"));
    act(() => {
      fireEvent.keyDown(window, { key: "p", metaKey: true });
    });
    const box = await screen.findByRole("combobox", { name: "프로젝트 찾기" });
    await waitFor(() => expect(document.activeElement).toBe(box));
    const list = screen.getByRole("listbox");
    expect(within(list).getAllByRole("option")[0].textContent).toContain("프로젝트 없음(전역만)");
    fireEvent.change(box, { target: { value: "side" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("set_page_project", { page: "permissions", id: "2b4d6f8a0c1e3a5b" }));
    // Only the opaque id goes over IPC, never a path.
    expect(invoke.mock.calls.filter(([c]) => c === "set_page_project").every(([, a]) => !String((a as { id: string }).id).includes("/"))).toBe(true);
    await waitFor(async () => expect((await pickerButton()).textContent).toContain("side-project"));
    expect(document.activeElement).toBe(await pickerButton());
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("permission_scopes", { project: "2b4d6f8a0c1e3a5b" }));
  });

  it("each tab keeps its own: a tab with its own pick stays, a tab without one follows the most recent", async () => {
    await boot("mcp", async (invoke) => {
      await invoke("set_page_project", { page: "permissions", id: "1f3a5c7e9b2d4f60" });
      await invoke("set_page_project", { page: "memory", id: "2b4d6f8a0c1e3a5b" });
    });
    // MCP has no pick of its own → most recent (side-project).
    await waitFor(async () => expect((await pickerButton()).textContent).toContain("side-project"));
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "권한" }));
    await waitFor(async () => expect((await pickerButton()).textContent).toContain("dev_team"));
  });

  it("Esc closes and returns focus to the trigger", async () => {
    await boot("memory");
    fireEvent.click(await pickerButton());
    const box = await screen.findByRole("combobox");
    fireEvent.keyDown(box, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(await pickerButton());
  });

  it("Tab closes and focus goes back to where it was before ⌘P", async () => {
    await boot("memory");
    const before = within(await screen.findByRole("navigation")).getByRole("button", { name: "메모리" });
    before.focus();
    act(() => {
      fireEvent.keyDown(window, { key: "p", metaKey: true });
    });
    const box = await screen.findByRole("combobox");
    fireEvent.keyDown(box, { key: "Tab" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(before);
  });

  it("→ unfolds worktrees on the active row and ← folds them", async () => {
    await boot("memory");
    fireEvent.click(await pickerButton());
    const box = await screen.findByRole("combobox");
    const parent = await waitFor(() => {
      const o = screen.getAllByRole("option").find((x) => x.getAttribute("aria-expanded") === "false");
      if (!o) throw new Error("no row with worktrees");
      return o;
    });
    fireEvent.mouseEnter(parent);
    const before = screen.getAllByRole("option").length;
    fireEvent.keyDown(box, { key: "ArrowRight" });
    await waitFor(() => expect(screen.getAllByRole("option").length).toBeGreaterThan(before));
    fireEvent.keyDown(box, { key: "ArrowLeft" });
    await waitFor(() => expect(screen.getAllByRole("option").length).toBe(before));
  });
});

describe("ScopeBar", () => {
  it("without a project shows [전역 | 프로젝트 고르기], and the project side opens the picker", async () => {
    await boot("permissions", async (invoke) => {
      await invoke("set_page_project", { page: "permissions", id: null });
    });
    const pick = await screen.findByRole("radio", { name: /프로젝트 고르기/ });
    expect(screen.getByRole("radio", { name: "전역" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(pick);
    expect(await screen.findByRole("combobox", { name: "프로젝트 찾기" })).toBeTruthy();
  });

  it("home as project: rules show the global side only, with the home line", async () => {
    await boot("memory", async (invoke) => {
      await invoke("set_page_project", { page: "memory", id: "3c5e7a9b1d2f4a6c" });
    });
    fireEvent.click(await screen.findByRole("radio", { name: "규칙" }));
    expect((await screen.findAllByText("홈 폴더는 전역과 같아요")).length).toBeGreaterThan(0);
    expect(await screen.findByText("korean")).toBeTruthy();
    expect(screen.queryByText("testing")).toBeNull();
  });
});

describe("unsaved edits (rules review H1)", () => {
  it("asks before a sidebar item drops an unsaved rule edit", async () => {
    await boot("memory", async (invoke) => {
      await invoke("set_page_project", { page: "memory", id: "1f3a5c7e9b2d4f60" });
    });
    fireEvent.click(await screen.findByRole("radio", { name: "규칙" }));
    fireEvent.click(await screen.findByText("testing", undefined, { timeout: 3000 }));
    const body = await screen.findByLabelText("본문");
    fireEvent.change(body, { target: { value: "# 바뀐 본문\n" } });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByRole("button", { name: "루틴" }));
    expect(confirm).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
    expect(screen.getByRole("heading", { name: "메모리" })).toBeTruthy();
    // Switching this tab's project asks too.
    fireEvent.click(await pickerButton());
    fireEvent.click(await screen.findByRole("option", { name: /side-project/ }));
    expect(confirm).toHaveBeenCalledTimes(2);
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "루틴" }));
    expect(await screen.findByRole("heading", { name: "루틴" })).toBeTruthy();
  });
});
