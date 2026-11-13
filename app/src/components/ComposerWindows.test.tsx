// windows-support 7.13·7.14: the composer on Windows shows the Git Bash note with its fix and link only when Git for
// Windows is missing, and a local-only folder cannot start (Mac only). On macOS nothing of this shows.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setPlatformForTest } from "../lib/platform";
import { Composer, GIT_FOR_WINDOWS_URL } from "./Composer";

let guard = "draft_pr";
vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return {
    invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "delivery_guard_preview") return { guard, ticked: guard === "local_only", canTick: true, forcedWorktree: false };
      if (cmd === "open_insight_link") return true;
      return real.invoke(cmd, args);
    }),
  };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

const draft = { cwd: "C:\\Users\\me\\work\\app", taskType: "dev" as const, brief: { goal: "버그 고치기", outOfScope: "", constraints: "", done: "" } };

function show() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <Composer draft={draft as never} onClose={() => {}} status={{ cli: true, permissionModes: ["manual", "plan"] } as never} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  setPlatformForTest(null);
  guard = "draft_pr";
});

describe("Git Bash note", () => {
  it("shows the fix and a link that opens the download page after confirmation", async () => {
    setPlatformForTest({ os: "windows", gitBash: false });
    show();
    const note = await waitFor(() => document.querySelector("[data-git-bash-missing]") as HTMLElement);
    expect(note.textContent).toContain("Git for Windows를 설치해 주세요");
    expect(note.className).toContain("bg-warn-soft");
    fireEvent.click(screen.getByRole("button", { name: "설치 페이지 열기" }));
    const { invoke } = await import("@tauri-apps/api/core");
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_insight_link", { url: GIT_FOR_WINDOWS_URL }));
  });
  it("is absent with Git Bash and on macOS", async () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    show();
    await screen.findByRole("combobox", { name: "권한" });
    expect(document.querySelector("[data-git-bash-missing]")).toBeNull();
    cleanup();
    setPlatformForTest(null);
    show();
    await screen.findByRole("combobox", { name: "권한" });
    expect(document.querySelector("[data-git-bash-missing]")).toBeNull();
  });
});

describe("local-only folder on Windows", () => {
  it("disables 작업 시작 with the Mac-only reason", async () => {
    guard = "local_only";
    setPlatformForTest({ os: "windows", gitBash: true });
    show();
    await waitFor(() => expect(screen.getByText("로컬만 작업은 Windows 미리보기에서 아직 쓸 수 없어요", { selector: "[data-start-hint]" })).toBeTruthy());
    expect((screen.getByRole("button", { name: "작업 시작" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
