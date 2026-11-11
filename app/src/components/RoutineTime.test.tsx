// routine-time-select: local routine time is two 24-hour selects (new and edit), and a broken time never reaches save_routine.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";
import type { Status } from "../lib/api";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { RoutinesPage } = await import("./RoutinesPage");
const { validateRoutineForm } = await import("../lib/routines");

const status = { defaultCwd: "/work/app" } as unknown as Status;
const weekly = {
  id: 2,
  name: "의존성 점검",
  cwd: "/work/app",
  promptLen: 10,
  permissionMode: "plan",
  schedule: { kind: "weekly", weekdays: 0b10001, hour: 14, minute: 30 },
  scheduleText: "매주 월·금 14:30",
  enabled: true,
  nextRunAt: null,
  lastStatus: null,
  active: false,
};
let listed: unknown[] = [];

function show() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <RoutinesPage status={status} onOpenTask={() => undefined} />
    </QueryClientProvider>,
  );
}
const timeGroup = (name = "실행 시각") => screen.getByRole("group", { name });
const pick = (part: "시" | "분" | "Hour" | "Minute", value: string, group = timeGroup()) =>
  fireEvent.change(within(group).getByRole("combobox", { name: part }), { target: { value } });
const shown = (part: "시" | "분", group = timeGroup()) => (within(group).getByRole("combobox", { name: part }) as HTMLSelectElement).value;
const savedSchedule = () => (invoke.mock.calls.find(([c]) => c === "save_routine")![1] as { input: { schedule: unknown } }).input.schedule;

async function newForm() {
  fireEvent.click(await screen.findByRole("button", { name: /새 루틴/ }));
  fireEvent.change(await screen.findByPlaceholderText("예: 아침 점검"), { target: { value: "아침 점검" } });
  fireEvent.change(screen.getByPlaceholderText(/어제 이후/), { target: { value: "테스트 돌려 줘" } });
}

beforeEach(() => {
  applyLanguage("ko");
  listed = [];
  invoke.mockReset();
  invoke.mockImplementation(async (cmd) => {
    switch (cmd) {
      case "list_routines":
        return listed;
      case "routine_runs":
        return [];
      case "routine_prompt":
        return "의존성 버전을 확인해 줘";
      case "folder_trusted":
        return true;
      case "save_routine":
        return 7;
      default:
        return null;
    }
  });
});
afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("local routine time", () => {
  it("new routine: two 24-hour selects, saves daily 07:30 with both fields", async () => {
    const { container } = show();
    await newForm();
    expect(container.querySelector('input[type="time"]')).toBeNull();
    expect(screen.queryByText(/오전|오후|\bAM\b|\bPM\b/)).toBeNull();
    pick("시", "7");
    pick("분", "30");
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_routine", expect.anything()));
    expect(savedSchedule()).toEqual({ kind: "daily", hour: 7, minute: 30 });
  });

  it("switching daily → weekly keeps the time, turns on Mon–Fri and saves weekdays 31", async () => {
    show();
    await newForm();
    pick("시", "7");
    pick("분", "30");
    fireEvent.click(screen.getByRole("radio", { name: "매주" }));
    expect([shown("시"), shown("분")]).toEqual(["7", "30"]);
    const days = within(screen.getByRole("group", { name: "요일" })).getAllByRole("button");
    expect(days.map((d) => d.getAttribute("aria-pressed"))).toEqual(["true", "true", "true", "true", "true", "false", "false"]);
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_routine", expect.anything()));
    expect(savedSchedule()).toEqual({ kind: "weekly", weekdays: 31, hour: 7, minute: 30 });
  });

  it("coming back from every N hours starts at 09:00", async () => {
    show();
    await newForm();
    pick("시", "7");
    fireEvent.click(screen.getByRole("radio", { name: "N시간마다" }));
    expect(screen.queryByRole("group", { name: "실행 시각" })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "매일" }));
    expect([shown("시"), shown("분")]).toEqual(["9", "0"]);
  });

  it("editing a weekly 14:30 routine shows 14·30 and saves the new minute", async () => {
    listed = [weekly];
    show();
    fireEvent.click(await screen.findByRole("button", { name: "고치기" }));
    await screen.findByDisplayValue("의존성 버전을 확인해 줘");
    expect([shown("시"), shown("분")]).toEqual(["14", "30"]);
    pick("분", "45");
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_routine", expect.anything()));
    expect(savedSchedule()).toEqual({ kind: "weekly", weekdays: 17, hour: 14, minute: 45 });
    expect((invoke.mock.calls.find(([c]) => c === "save_routine")![1] as { id: number }).id).toBe(2);
  });

  it("English labels: Run time with Hour and Minute", async () => {
    applyLanguage("en");
    show();
    fireEvent.click(await screen.findByRole("button", { name: /New routine/ }));
    const group = await screen.findByRole("group", { name: "Run time" });
    pick("Hour", "18", group);
    expect((within(group).getByRole("combobox", { name: "Hour" }) as HTMLSelectElement).value).toBe("18");
    expect(within(group).getByRole("combobox", { name: "Minute" })).toBeTruthy();
  });

  it("the form check rejects a time that is not a whole hour/minute in range", () => {
    const ok = { name: "n", cwd: "/x", prompt: "p", schedule: { kind: "daily", hour: 9, minute: 0 } as const };
    expect(validateRoutineForm(ok)).toBeNull();
    for (const bad of [
      { kind: "daily", hour: 0, minute: undefined },
      { kind: "daily", hour: undefined, minute: 0 },
      { kind: "daily", hour: Number.NaN, minute: 0 },
      { kind: "daily", hour: -1, minute: 0 },
      { kind: "daily", hour: "7", minute: 0 },
      { kind: "weekly", weekdays: 1, hour: 9, minute: 1.5 },
    ]) {
      expect(validateRoutineForm({ ...ok, schedule: bad as never })).toBe("시각이 올바르지 않아요");
    }
  });
});
