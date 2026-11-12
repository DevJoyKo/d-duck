// sheet-focus 2.1 (and 2.2 for the Usage session sheet): the sheets that were aria-modal without a focus trap. Each one takes focus on open (the
// initial-focus table in design.md 6), keeps Tab inside, and gives focus back on close. The dirty-guarded ones ask
// before Escape throws edits away, the same confirm the × button uses.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke as mockInvoke } from "../mock/core";
import type { ItemText } from "../lib/addItems";

vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args ?? {}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { AgentsPage } = await import("./AgentsPage");
const { AuthoringEditor } = await import("./AuthoringEditor");
const { AddMenu, AddSheet } = await import("./AddSheet");
const { addApi } = await import("../lib/addItems");
const { UsagePage } = await import("./UsagePage");

const wrap = (ui: ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);
const tab = (shift = false) => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Tab", shiftKey: shift });
const escape = () => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

/** Tab more times than there are stops (and Shift+Tab now and then): focus never leaves the dialog. */
function walk(dialog: HTMLElement) {
  const n = dialog.querySelectorAll("button, input, textarea, select, a[href]").length + 3;
  for (let i = 0; i < n; i++) {
    tab(i % 4 === 3);
    expect(dialog.contains(document.activeElement)).toBe(true);
  }
}

/** Last stop → Tab → first stop, and back with Shift+Tab. */
function wraps(dialog: HTMLElement, first: HTMLElement) {
  act(() => first.focus());
  tab(true);
  const last = document.activeElement as HTMLElement;
  expect(dialog.contains(last)).toBe(true);
  expect(last).not.toBe(first);
  tab();
  expect(document.activeElement).toBe(first);
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Agent settings sheet", () => {
  async function open() {
    wrap(<AgentsPage onOpenTask={() => {}} onOpenBlocked={() => {}} capacity={3} />);
    await screen.findByText("design-critic");
    const opener = screen.getByRole("button", { name: "에이전트 설정" });
    act(() => opener.focus());
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog", { name: "에이전트 설정" });
    return { opener, dialog };
  }

  it("focuses ×, keeps Tab inside and returns focus to the settings button", async () => {
    const { opener, dialog } = await open();
    const close = within(dialog).getByRole("button", { name: "닫기" });
    expect(document.activeElement).toBe(close);
    await waitFor(() => expect(within(dialog).getAllByRole("switch").length).toBeGreaterThan(0));
    wraps(dialog, close);
    walk(dialog);
    escape();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("asks before Escape drops edited rules, and declining keeps the sheet open", async () => {
    const { dialog } = await open();
    fireEvent.click(within(dialog).getByRole("radio", { name: "팀 규칙" }));
    const rule = (await within(dialog).findAllByRole("textbox"))[0] as HTMLInputElement;
    act(() => rule.focus());
    fireEvent.change(rule, { target: { value: `${rule.value} 고침` } });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    escape();
    expect(confirm).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 닫을까요?");
    expect(screen.getByRole("dialog", { name: "에이전트 설정" })).toBeTruthy();
    expect(document.activeElement).toBe(rule);
  });
});

describe("agent definition editor", () => {
  async function open() {
    wrap(<AgentsPage onOpenTask={() => {}} onOpenBlocked={() => {}} capacity={3} />);
    await screen.findByText("design-critic");
    const opener = screen.getAllByRole("button", { name: "정의 고치기" })[0];
    act(() => opener.focus());
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog", { name: /정의$/ });
    return { opener, dialog };
  }

  it("focuses ×, keeps Tab inside and returns focus to the card button", async () => {
    const { opener, dialog } = await open();
    const close = within(dialog).getByRole("button", { name: "닫기" });
    expect(document.activeElement).toBe(close);
    await within(dialog).findByRole("textbox");
    wraps(dialog, close);
    walk(dialog);
    escape();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("asks before Escape drops an edit, and declining keeps the editor open", async () => {
    const { dialog } = await open();
    const box = (await within(dialog).findByRole("textbox")) as HTMLTextAreaElement;
    await waitFor(() => expect(box.value).not.toBe(""));
    act(() => box.focus());
    fireEvent.change(box, { target: { value: `${box.value}\n고침` } });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    escape();
    expect(confirm).toHaveBeenCalledWith("저장하지 않은 수정 내용이 있어요. 닫을까요?");
    expect(screen.getByRole("dialog", { name: /정의$/ })).toBeTruthy();
  });
});

describe("extension editor (AuthoringEditor)", () => {
  const ITEM: ItemText = { content: "# hello\n", token: "t1", kind: "skill", name: "hello", location: "전역", readonly: null, warnings: [], extraFiles: [] };
  function Page() {
    const [open, setOpen] = useState(false);
    return (
      <div>
        <button onClick={() => setOpen(true)}>hello 열기</button>
        {open && <AuthoringEditor id="s-hello" onClose={() => setOpen(false)} onDelete={() => {}} />}
      </div>
    );
  }
  async function open() {
    vi.spyOn(addApi, "read").mockResolvedValue(ITEM);
    wrap(<Page />);
    const opener = screen.getByRole("button", { name: "hello 열기" });
    act(() => opener.focus());
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog");
    return { opener, dialog };
  }

  it("focuses ×, not the delete button, keeps Tab inside and returns focus to the row", async () => {
    const { opener, dialog } = await open();
    const close = within(dialog).getByRole("button", { name: "닫기" });
    expect(document.activeElement).toBe(close);
    await within(dialog).findByLabelText("내용");
    expect(within(dialog).getByRole("button", { name: "삭제" })).not.toBe(document.activeElement);
    wraps(dialog, close);
    walk(dialog);
    escape();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("asks before Escape drops an edit, and declining keeps the editor open", async () => {
    const { dialog } = await open();
    const box = await within(dialog).findByLabelText("내용");
    act(() => box.focus());
    fireEvent.change(box, { target: { value: "# changed\n" } });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    escape();
    expect(confirm).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(document.activeElement).toBe(box);
  });
});

describe("add-extension sheet", () => {
  function Page() {
    const [add, setAdd] = useState<"link" | "create" | "ask" | null>(null);
    return (
      <div>
        <AddMenu onPick={setAdd} />
        {add && <AddSheet mode={add} onMode={setAdd} onClose={() => setAdd(null)} />}
      </div>
    );
  }
  function pick(name: RegExp) {
    wrap(<Page />);
    const trigger = screen.getByRole("button", { name: /^추가/ });
    act(() => trigger.focus());
    fireEvent.click(trigger);
    const item = screen.getByRole("menuitem", { name });
    act(() => item.focus());
    fireEvent.click(item);
    return { trigger, dialog: screen.getByRole("dialog") };
  }

  it("opened for a link: the link field keeps its autoFocus, Tab stays inside, close returns to the Add button", () => {
    const { trigger, dialog } = pick(/^링크로 설치/);
    expect(document.activeElement).toBe(within(dialog).getByRole("textbox"));
    wraps(dialog, within(dialog).getByRole("button", { name: "닫기" }));
    walk(dialog);
    escape();
    expect(screen.queryByRole("dialog")).toBeNull();
    // The menu item that opened the sheet is gone: the Add button takes focus.
    expect(document.activeElement).toBe(trigger);
  });

  it("opened to create: × takes focus", () => {
    const { dialog } = pick(/^직접 만들기/);
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "닫기" }));
  });
});

describe("Usage session sheet (2.2)", () => {
  it("focuses ×, lets the title editor take its own Escape, and returns focus to the session row", async () => {
    wrap(<UsagePage />);
    const row = await waitFor(() => {
      const b = document.querySelector<HTMLButtonElement>("table tbody button");
      if (!b) throw new Error("no sessions yet");
      return b;
    });
    act(() => row.focus());
    fireEvent.click(row);
    const dialog = await screen.findByRole("dialog");
    const close = within(dialog).getByRole("button", { name: "닫기" });
    expect(document.activeElement).toBe(close);
    fireEvent.click(await within(dialog).findByRole("button", { name: "제목 고치기" }));
    const field = within(dialog).getByRole("textbox");
    act(() => field.focus());
    fireEvent.keyDown(field, { key: "Escape" });
    expect(within(dialog).queryByRole("textbox")).toBeNull();
    expect(screen.getByRole("dialog")).toBe(dialog);
    wraps(dialog, close);
    walk(dialog);
    escape();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(row);
  });
});
