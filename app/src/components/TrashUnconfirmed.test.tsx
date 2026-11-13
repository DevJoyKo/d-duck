// windows-support 9.1: a Recycle Bin move Windows could not confirm is still a finished delete. The memory editor and
// the agent settings sheet take the success path (list refresh, editor closed, deny-rule/hidden prompt) and add one
// note; a cancelled dialog changes nothing. On the mock backend (real rules: Rust platform::shell::trash).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke as mockInvoke } from "../mock/core";

const calls: string[] = [];
let trashAnswer: unknown = { done: true, unconfirmed: true };
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => {
    calls.push(cmd);
    if (cmd === "trash_memory") return Promise.resolve(trashAnswer);
    if (cmd === "trash_agent") return Promise.resolve({ name: "doc-writer", project: null, off: false, hidden: false, unconfirmed: true });
    return mockInvoke(cmd, args ?? {});
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const { MemorySection } = await import("./MemoryPage");
const { AgentSettings } = await import("./AgentSettings");
const { MOCK_IDS } = await import("../mock/projects");
// jsdom has no scrollIntoView (the editor scrolls itself into view under 1100px).
Element.prototype.scrollIntoView = () => {};
const NOTE = "지웠지만 휴지통에서 찾지 못했어요 · 영구 삭제됐을 수 있어요";

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const spy = vi.spyOn(qc, "invalidateQueries");
  render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
  return spy;
}

afterEach(() => {
  cleanup();
  calls.length = 0;
  trashAnswer = { done: true, unconfirmed: true };
  vi.restoreAllMocks();
});

async function memoryList() {
  return (await mockInvoke("memory_entries", { project: MOCK_IDS.home })) as Parameters<typeof MemorySection>[0]["list"];
}

describe("memory editor", () => {
  it("unconfirmed: refreshes the list, closes the editor and shows the note", async () => {
    const list = await memoryList();
    const spy = wrap(<MemorySection project={MOCK_IDS.home} list={list} loading={false} globalOff={false} />);
    fireEvent.click(await screen.findByText("review-gates"));
    fireEvent.click(await screen.findByRole("button", { name: "휴지통으로" }));
    expect(await screen.findByText(NOTE)).toBeTruthy();
    expect(spy).toHaveBeenCalledWith({ queryKey: ["memory-entries", MOCK_IDS.home] });
    expect(screen.queryByRole("button", { name: "휴지통으로" })).toBeNull();
  });

  it("cancelled: nothing changes", async () => {
    trashAnswer = { done: false, unconfirmed: false };
    const list = await memoryList();
    const spy = wrap(<MemorySection project={MOCK_IDS.home} list={list} loading={false} globalOff={false} />);
    fireEvent.click(await screen.findByText("review-gates"));
    fireEvent.click(await screen.findByRole("button", { name: "휴지통으로" }));
    await waitFor(() => expect(calls).toContain("trash_memory"));
    expect(spy).not.toHaveBeenCalledWith({ queryKey: ["memory-entries", MOCK_IDS.home] });
    expect(screen.getByRole("button", { name: "휴지통으로" })).toBeTruthy();
    expect(screen.queryByText(NOTE)).toBeNull();
  });
});

describe("agent settings", () => {
  it("unconfirmed: the catalog refreshes and the note shows", async () => {
    const catalog = (await mockInvoke("agent_catalog", { cwd: null })) as Parameters<typeof AgentSettings>[0]["catalog"];
    const own = catalog.find((d) => (d.source === "user" || d.source === "project") && d.editable)!;
    const spy = wrap(<AgentSettings catalog={catalog} onClose={() => {}} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${own.name}.*삭제|삭제.*${own.name}`) }));
    expect(await screen.findByText(NOTE)).toBeTruthy();
    expect(calls).toContain("trash_agent");
    expect(spy).toHaveBeenCalledWith({ queryKey: ["agent-catalog"] });
  });
});
