// app-language (tasks.md 3.1): the tasks board and the new-task composer in English on the mock backend. Korean may
// only come from user data (the mock plays that role: task names, prompts, folders, team presets), never from labels.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Draft } from "../lib/brief";
import { applyLanguage } from "../i18n/language";

vi.mock("@tauri-apps/api/core", () => import("../mock/core"));
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

// Mock and bundled sources: a Korean string found verbatim in them is user data, not app copy.
const DATA = Object.values(
  import.meta.glob(["../mock/*.ts", "../../crates/dduck-core/data/*.json"], { query: "?raw", import: "default", eager: true }) as Record<string, string>,
).join("\n");

/** Korean text nodes and labels inside `root` that do not come from the mock data. */
function koreanCopy(root: HTMLElement): string[] {
  const out: string[] = [];
  const check = (s: string | null | undefined) => {
    const v = (s ?? "").trim();
    if (/[가-힣]/.test(v) && !DATA.includes(v)) out.push(v);
  };
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) check(n.textContent);
  for (const el of root.querySelectorAll("*")) for (const a of ["aria-label", "title", "placeholder"]) check(el.getAttribute(a));
  return out;
}

async function renderBoard() {
  const { api } = await import("../lib/api");
  const { TasksPage } = await import("./TasksPage");
  const [status, agents] = await Promise.all([api.status(), api.agents()]);
  function Host() {
    const [draft, setDraft] = useState<Draft | null | undefined>(undefined);
    return <TasksPage status={status} agents={agents} selected={null} onSelect={() => undefined} draft={draft} onDraft={setDraft} />;
  }
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <Host />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  applyLanguage("en");
});
afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("tasks in English", () => {
  it("draws the board with English labels only", async () => {
    const { container } = await renderBoard();
    await screen.findByRole("heading", { name: "Tasks" });
    await waitFor(() => expect(screen.getByRole("heading", { name: "In progress" })).toBeTruthy());
    expect(screen.getByRole("heading", { name: "Wrap-up" })).toBeTruthy();
    expect(screen.getByText("Past tasks")).toBeTruthy();
    expect(koreanCopy(container)).toEqual([]);
  });

  it("opens the composer down to the start button with English labels only", async () => {
    const { container } = await renderBoard();
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    await screen.findByText("New background task");
    expect(screen.getByRole("radiogroup", { name: "Task type" })).toBeTruthy();
    expect(screen.getByText("Refine the brief")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start task" })).toBeTruthy();
    // Let the folder lookups (repository, push scope) settle so their lines are drawn too.
    await waitFor(() => expect(container.querySelector("[data-testid=delivery-guard]")).toBeTruthy());
    expect(koreanCopy(container)).toEqual([]);
  });
});
