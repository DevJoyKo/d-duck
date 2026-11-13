// windows-support 4.2–4.4: the Windows preview disables Mac-only controls with a "Mac 전용" line, accepts C:\ paths and
// drops the macOS title-bar strip. Every case is also checked on macOS, which must look as before.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { setPlatformForTest } from "../lib/platform";
import { DeliveryGuardLine, LocalOnlyList } from "./LocalOnlySection";
import { Sidebar } from "./Sidebar";
import { messageOf } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "local_only_projects") return [{ id: "b", name: "app", parentDir: "C:\\work", checked: false, missing: false, worktreeOf: null, offReason: null }];
    if (cmd === "delivery_guard_preview") return { guard: "local_only", ticked: true, canTick: true, forcedWorktree: false };
    return null;
  }),
}));

const wrap = (ui: ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);
const win = () => setPlatformForTest({ os: "windows", gitBash: true });

afterEach(() => {
  cleanup();
  vi.mocked(invoke).mockClear();
  setPlatformForTest(null);
});

describe("Windows preview: local-only is Mac only", () => {
  it("settings list: ticks are shown but cannot change, with the Mac 전용 line", async () => {
    win();
    wrap(<LocalOnlyList />);
    const box = (await screen.findByLabelText(/^app/)) as HTMLInputElement;
    expect(box.disabled).toBe(true);
    expect(screen.getByText("Mac 전용")).toBeTruthy();
    expect(screen.getByText("로컬만 작업은 Windows 미리보기에서 아직 쓸 수 없어요")).toBeTruthy();
    fireEvent.click(box);
    expect(invoke).not.toHaveBeenCalledWith("set_local_only", expect.anything());
  });

  it("composer line: a C:\\ folder is looked up, no tick button, Mac 전용 for a local-only project", async () => {
    win();
    wrap(<DeliveryGuardLine cwd="C:\Users\me\work\app" worktree={false} />);
    expect(await screen.findByText("로컬 작업만 · push하지 않아요")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("delivery_guard_preview", { cwd: "C:\\Users\\me\\work\\app" });
    expect(screen.queryByRole("button", { name: "로컬만으로 바꾸기" })).toBeNull();
    expect(screen.getByText("Mac 전용")).toBeTruthy();
  });

  it("macOS: same rows stay editable, no Mac 전용 line, C:\\ is not a folder", async () => {
    wrap(<LocalOnlyList />);
    expect(((await screen.findByLabelText(/^app/)) as HTMLInputElement).disabled).toBe(false);
    expect(screen.queryByText("Mac 전용")).toBeNull();
    cleanup();
    wrap(<DeliveryGuardLine cwd="C:\Users\me\work\app" worktree={false} />);
    expect(screen.queryByTestId("delivery-guard")).toBeNull();
  });
});

describe("Windows preview: backend reasons", () => {
  it("words MacOnly and WindowsCommandTooLong from the catalog", () => {
    expect(messageOf({ kind: "invalidInput", code: "macOnly", params: {}, text: "x", reason: "x" }, "ko")).toBe("Mac 전용이에요 · Windows 미리보기에서는 아직 쓸 수 없어요.");
    expect(messageOf({ kind: "invalidInput", code: "windowsCommandTooLong", params: {}, text: "x", reason: "x" }, "en")).toBe("The instructions and settings are too long for Windows · Shorten them and try again.");
  });
  // The Git Bash note is rendered by the composer itself (ComposerWindows.test.tsx).
});

describe("window chrome per OS", () => {
  const side = () =>
    wrap(<Sidebar page="agents" onNavigate={() => undefined} waiting={0} ingest={null} />);
  it("macOS: traffic-light padding and the drag strip", () => {
    side();
    const nav = screen.getByRole("navigation");
    expect(nav.className).toContain("pt-11");
    expect(nav.querySelector("[data-tauri-drag-region]")).not.toBeNull();
  });
  it("Windows: native title bar, so no padding strip", async () => {
    win();
    side();
    const nav = screen.getByRole("navigation");
    expect(nav.className).not.toContain("pt-11");
    expect(nav.querySelector("[data-tauri-drag-region]")).toBeNull();
    await waitFor(() => expect(nav.textContent).toContain("D.Duck"));
  });
});
