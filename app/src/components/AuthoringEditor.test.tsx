// AuthoringEditor (D-132 (5)): unsaved text registers with the app-wide guard, and the backdrop, Esc and X ask before
// throwing it away. A clean editor closes without asking.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ItemText } from "../lib/addItems";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const { addApi } = await import("../lib/addItems");
const { anyDirty } = await import("../lib/projects");
const { AuthoringEditor } = await import("./AuthoringEditor");

const ITEM: ItemText = { content: "# hello\n", token: "t1", kind: "skill", name: "hello", location: "전역", readonly: null, warnings: [], extraFiles: [] };
const onClose = vi.fn();
let confirm: ReturnType<typeof vi.spyOn>;

async function open() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <AuthoringEditor id="s-hello" onClose={onClose} />
    </QueryClientProvider>,
  );
  return (await screen.findByLabelText("내용")) as HTMLTextAreaElement;
}
const backdrop = () => screen.getByRole("dialog").parentElement!;

beforeEach(() => {
  onClose.mockReset();
  vi.spyOn(addApi, "read").mockResolvedValue(ITEM);
  confirm = vi.spyOn(window, "confirm");
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("authoring editor close", () => {
  it("closes at once when nothing changed (X, Esc, backdrop)", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "닫기" }));
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(backdrop());
    expect(onClose).toHaveBeenCalledTimes(3);
    expect(confirm).not.toHaveBeenCalled();
  });

  it("asks before discarding unsaved text and stays open when the user says no", async () => {
    const box = await open();
    fireEvent.change(box, { target: { value: "# changed\n" } });
    expect(anyDirty()).toBe(true);
    confirm.mockReturnValue(false);
    fireEvent.click(screen.getByRole("button", { name: "닫기" }));
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(backdrop());
    expect(confirm).toHaveBeenCalledTimes(3);
    expect(confirm).toHaveBeenLastCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
    expect(onClose).not.toHaveBeenCalled();
    // Clicks inside the sheet are not the backdrop.
    fireEvent.click(screen.getByRole("dialog"));
    expect(confirm).toHaveBeenCalledTimes(3);
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "닫기" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not ask while an IME composition is still open on Esc", async () => {
    const box = await open();
    fireEvent.change(box, { target: { value: "# 한" } });
    fireEvent.keyDown(document, { key: "Escape", isComposing: true });
    expect(confirm).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("clears the dirty guard when it unmounts", async () => {
    const box = await open();
    fireEvent.change(box, { target: { value: "x" } });
    expect(anyDirty()).toBe(true);
    cleanup();
    expect(anyDirty()).toBe(false);
  });
});
