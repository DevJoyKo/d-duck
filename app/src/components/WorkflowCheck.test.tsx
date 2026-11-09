// workflow-meta-check (D-183): the create and edit panes ask Rust (`check_workflow_meta`) 400 ms after the last edit,
// show the reason and turn the save button off; the list marks workflows Claude Code can't read. The mock stands in
// for the Rust verdict here; the rules themselves are tested in authoring.rs.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";
import { invoke as mockInvoke } from "../mock/core";

const viaMock = (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args ?? {});
const invoke = vi.fn(viaMock);
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { ExtensionsPage } = await import("./ExtensionsPage");
const { AuthoringEditor } = await import("./AuthoringEditor");
const { addApi } = await import("../lib/addItems");
const { useWorkflowCheck, CHECK_DELAY_MS } = await import("../lib/workflowCheck");

const wrap = (ui: React.ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);
const checks = () => invoke.mock.calls.filter(([c]) => c === "check_workflow_meta");
const last = () => checks()[checks().length - 1];

async function openCreateWorkflow() {
  wrap(<ExtensionsPage />);
  await screen.findByRole("button", { name: "pr-writer" });
  fireEvent.click(screen.getByRole("button", { name: /^추가/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: /직접 만들기/ }));
  const sheet = await screen.findByRole("dialog", { name: "추가" });
  fireEvent.click(within(sheet).getByRole("radio", { name: "워크플로" }));
  return sheet;
}

beforeEach(() => {
  invoke.mockClear();
  invoke.mockImplementation(viaMock);
});
afterEach(() => {
  vi.useRealTimers();
  cleanup();
  vi.restoreAllMocks();
  applyLanguage("ko");
});

describe("live meta check in the create pane", () => {
  it("waits 400 ms, then shows the reason and turns the button off; a renamed item asks to match meta.name", async () => {
    const sheet = await openCreateWorkflow();
    fireEvent.change(within(sheet).getByPlaceholderText("triage"), { target: { value: "digest" } });
    expect(checks()).toHaveLength(0);
    await within(sheet).findByText("Claude Code가 읽을 수 있어요");
    expect(last()?.[1]).toMatchObject({ name: "digest" });
    expect(within(sheet).getByText("meta만 검사해요 · 본문 문법은 검사하지 않아요")).toBeTruthy();
    const submit = within(sheet).getByRole("button", { name: "워크플로 만들기" }) as HTMLButtonElement;
    expect(submit.disabled).toBe(false);

    const body = within(sheet).getAllByRole("textbox").find((e) => e.tagName === "TEXTAREA") as HTMLTextAreaElement;
    fireEvent.change(body, { target: { value: body.value.replace("description: '", "description: `${x}") } });
    // The reason says how to fix it, then the line.
    await within(sheet).findByText("meta 값은 리터럴이어야 해요 · 변수·함수 호출 대신 값을 직접 써 주세요 · 3번째 줄");
    expect(submit.disabled).toBe(true);

    // The body is now the user's own text: renaming leaves meta.name behind.
    fireEvent.change(body, { target: { value: body.value.replace("`${x}", "'") } });
    await within(sheet).findByText("Claude Code가 읽을 수 있어요");
    fireEvent.change(within(sheet).getByPlaceholderText("triage"), { target: { value: "digest-v2" } });
    await within(sheet).findByText("meta.name을 digest-v2로 맞춰 주세요");
    expect(submit.disabled).toBe(true);
  });

  it("makes no call before 400 ms and restarts the wait on each edit (fake timers)", async () => {
    vi.useFakeTimers();
    const { rerender } = renderHook(({ text }) => useWorkflowCheck(text, "a", true), { initialProps: { text: "x" } });
    expect(CHECK_DELAY_MS).toBe(400);
    await act(async () => void vi.advanceTimersByTime(399));
    expect(checks()).toHaveLength(0);
    rerender({ text: "xy" });
    await act(async () => void vi.advanceTimersByTime(399));
    expect(checks()).toHaveLength(0);
    await act(async () => void vi.advanceTimersByTime(1));
    expect(checks()).toHaveLength(1);
    expect(checks()[0][1]).toEqual({ text: "xy", name: "a" });
  });

  it("asks once per pause, not per keystroke", async () => {
    const sheet = await openCreateWorkflow();
    const input = within(sheet).getByPlaceholderText("triage");
    for (const v of ["d", "di", "dig"]) fireEvent.change(input, { target: { value: v } });
    await waitFor(() => expect(checks()).toHaveLength(1), { timeout: 1500 });
    expect(checks()[0][1]).toMatchObject({ name: "dig" });
  });

  it("speaks English", async () => {
    applyLanguage("en");
    wrap(<ExtensionsPage />);
    await screen.findByRole("button", { name: "pr-writer" });
    expect(screen.getAllByText("Claude Code can't read this").length).toBeGreaterThan(0);
    expect(screen.getByText("meta values must be literals · Write the value itself, not a variable or a call")).toBeTruthy();
    expect(screen.getByText("Claude Code registers it as sync-notes")).toBeTruthy();
  });
});

describe("live meta check in the editor", () => {
  it("saves an existing file whose meta.name differs, warning which name Claude Code registers", async () => {
    // Rust's answer for a mismatch carries the registered name (the mock's rough stand-in does not).
    invoke.mockImplementation((cmd: string, args?: Record<string, unknown>) =>
      cmd === "check_workflow_meta" && String(args?.text).includes("'plan'")
        ? Promise.resolve({ ok: false, reason: "nameMismatch", line: null, blankDescription: false, registeredName: "plan" })
        : mockInvoke(cmd, args ?? {}),
    );
    vi.spyOn(addApi, "read").mockResolvedValue({
      content: "export const meta = { name: 'triage', description: 'd' }\n",
      token: "t",
      kind: "workflow",
      name: "triage",
      location: "전역",
      readonly: null,
      warnings: [],
      extraFiles: [],
    });
    wrap(<AuthoringEditor id="s-triageworkflow" onClose={() => {}} />);
    const box = (await screen.findByLabelText("내용")) as HTMLTextAreaElement;
    await screen.findByText("Claude Code가 읽을 수 있어요");
    fireEvent.change(box, { target: { value: "export const meta = { name: 'plan', description: 'd' }\n" } });
    await screen.findByText("Claude Code에는 plan으로 등록돼요");
    expect(screen.queryByText(/meta.name을/)).toBeNull();
    expect((screen.getByRole("button", { name: "저장" }) as HTMLButtonElement).disabled).toBe(false);
    expect(last()?.[1]).toMatchObject({ name: "triage" });
    // Other reasons still turn save off.
    fireEvent.change(box, { target: { value: "export const meta = { name: 'triage', description: f() }\n" } });
    await screen.findByText(/^meta 값은 리터럴이어야 해요 · /);
    expect((screen.getByRole("button", { name: "저장" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("list marks", () => {
  it("shows the mark, the reason and the registered name", async () => {
    wrap(<ExtensionsPage />);
    const row = (await screen.findByRole("button", { name: "weekly-digest" })).closest("div") as HTMLElement;
    expect(within(row).getByText("Claude Code가 못 읽어요")).toBeTruthy();
    expect(within(row).getByText("meta 값은 리터럴이어야 해요 · 변수·함수 호출 대신 값을 직접 써 주세요")).toBeTruthy();
    const notes = (await screen.findByRole("button", { name: "notes" })).closest("div") as HTMLElement;
    expect(within(notes).getByText("등록 이름 다름")).toBeTruthy();
    expect(within(notes).getByText("Claude Code에는 sync-notes로 등록돼요")).toBeTruthy();
    expect(within(notes).queryByText("Claude Code가 못 읽어요")).toBeNull();
  });
});
