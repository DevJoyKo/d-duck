import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DeliveryGuardLine, LocalOnlyList } from "./LocalOnlySection";
import { alertLines, guardChip, guardLine } from "../lib/delivery";

const rows = [
  { id: "a", name: "client", parentDir: "/work", checked: true, missing: false, worktreeOf: null, offReason: null },
  { id: "w", name: "fix-1", parentDir: "/work/client/.claude/worktrees", checked: false, missing: false, worktreeOf: "a", offReason: null },
  { id: "b", name: "app", parentDir: "/work", checked: false, missing: false, worktreeOf: null, offReason: null },
  { id: "h", name: "me", parentDir: "/Users", checked: false, missing: false, worktreeOf: null, offReason: "홈 폴더는 고를 수 없어요", offCode: "homeNotAllowed" },
  { id: "g", name: "gone", parentDir: "/work", checked: true, missing: true, worktreeOf: null, offReason: null },
];
let preview: Record<string, unknown> = { guard: "draft_pr", ticked: false, canTick: true, forcedWorktree: false };
let declineUntick = false;
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: Record<string, unknown>) => {
    if (cmd === "local_only_projects") return rows;
    if (cmd === "set_local_only") return args.on ? true : declineUntick;
    if (cmd === "delivery_guard_preview") return preview;
    if (cmd === "set_local_only_cwd") {
      preview = { guard: "local_only", ticked: true, canTick: false, forcedWorktree: false };
      return null;
    }
    throw new Error(cmd);
  }),
}));

const wrap = (ui: ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);

afterEach(() => {
  cleanup();
  vi.mocked(invoke).mockClear();
  preview = { guard: "draft_pr", ticked: false, canTick: true, forcedWorktree: false };
  declineUntick = false;
});

describe("원격 push 막기", () => {
  it("lists projects with their ticks; worktrees fold into the parent; home cannot be ticked", async () => {
    wrap(<LocalOnlyList />);
    const client = await screen.findByLabelText(/client/);
    expect((client as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText(/^app/) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByLabelText(/fix-1/)).toBeNull();
    expect(screen.getByText(/worktree 1개도 따라요/)).toBeTruthy();
    expect((screen.getByLabelText(/^me/) as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText(/폴더가 없어졌어요/)).toBeTruthy();
    expect(screen.getByText("실행 중인 작업은 그대로예요")).toBeTruthy();
  });

  it("ticking saves at once; unticking only calls the backend (which asks natively)", async () => {
    wrap(<LocalOnlyList />);
    fireEvent.click(await screen.findByLabelText(/^app/));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("set_local_only", { id: "b", on: true }));
    fireEvent.click(screen.getByLabelText(/client/));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("set_local_only", { id: "a", on: false }));
    // No optimistic update: the row reflects what the backend returns on refetch (still ticked here).
    expect((screen.getByLabelText(/client/) as HTMLInputElement).checked).toBe(true);
  });

  it("Composer line: draft PR by default, the tick button switches to local-only", async () => {
    wrap(<DeliveryGuardLine cwd="/work/new-client" worktree={false} />);
    expect(await screen.findByText("PR 초안까지 · 머지는 사람이 해요")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "로컬만으로 바꾸기" }));
    expect(await screen.findByText("로컬 작업만 · push하지 않아요")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("set_local_only_cwd", { cwd: "/work/new-client" });
    expect(screen.queryByRole("button", { name: "로컬만으로 바꾸기" })).toBeNull();
  });

  it("Composer line: forced worktree note, nothing for a relative path", async () => {
    preview = { guard: "draft_pr", ticked: false, canTick: false, forcedWorktree: true };
    wrap(<DeliveryGuardLine cwd="/work/app" worktree={false} />);
    expect(await screen.findByText("기본 브랜치라 worktree에서 해요")).toBeTruthy();
    cleanup();
    wrap(<DeliveryGuardLine cwd="work" worktree={false} />);
    expect(screen.queryByTestId("delivery-guard")).toBeNull();
  });

  it("copy", () => {
    expect(guardLine("draft_pr")).toBe("PR 초안까지 · 머지는 사람이 해요");
    expect(guardLine("local_only")).toBe("로컬 작업만 · push하지 않아요");
    expect(guardChip("local_only")).toBe("로컬만");
    expect(guardChip("draft_pr")).toBe("PR 초안까지");
    expect(guardChip(null)).toBeNull();
    expect(guardChip(undefined)).toBeNull();
    expect(alertLines(null)).toEqual([]);
    expect(alertLines(0)).toEqual([]);
    expect(alertLines(3)).toEqual(["원격으로 push한 기록이 있어요", "기본 브랜치에 새 커밋이 생겼어요"]);
  });
});
