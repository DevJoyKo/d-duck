import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AgentSession, AgentsPayload } from "../lib/api";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: { ...real.api, sessionPrompt: vi.fn(async () => null), forceStop: vi.fn(async () => undefined), openTerminal: vi.fn() },
  };
});

const { TasksPage } = await import("./TasksPage");
const { api } = await import("../lib/api");

// Terminal sessions only: no log view (xterm needs a canvas jsdom lacks).
const session = (key: string, name: string): AgentSession => ({
  key,
  pid: 100,
  id: null,
  sessionId: key,
  name,
  cwd: "/w/" + name,
  kind: "interactive",
  startedAt: 1,
  status: "working",
});

describe("TasksPage detail", () => {
  it("does not carry a pending force-stop confirmation over to another session", async () => {
    const agents: AgentsPayload = { sessions: [session("a", "alpha"), session("b", "beta")], error: null };
    const { useState } = await import("react");
    function Stateful() {
      const [selected, setSelected] = useState<string | null>(null);
      return <TasksPage agents={agents} selected={selected} onSelect={setSelected} draft={undefined} onDraft={() => undefined} />;
    }
    render(
      <QueryClientProvider client={new QueryClient()}>
        <Stateful />
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByText("alpha"));
    fireEvent.click(screen.getByRole("button", { name: "강제 종료" }));
    expect(screen.getByRole("button", { name: "강제 종료하기" })).toBeTruthy();

    fireEvent.click(screen.getByText("beta"));
    expect(screen.queryByRole("button", { name: "강제 종료하기" })).toBeNull();
    expect(api.forceStop).not.toHaveBeenCalled();
  });
});

describe("TasksPage board look (palette-refresh 2.2)", () => {
  it("rings only the waiting card in yellow, marks the selection apart from it, and never nests buttons", () => {
    const waiting: AgentSession = { ...session("w", "waiter"), status: "waitingInput" };
    const agents: AgentsPayload = { sessions: [session("a", "alpha"), waiting], error: null };
    const { container } = render(
      <QueryClientProvider client={new QueryClient()}>
        <TasksPage agents={agents} selected="a" onSelect={() => undefined} draft={undefined} onDraft={() => undefined} />
      </QueryClientProvider>,
    );
    const cards = [...container.querySelectorAll("[data-task-card]")] as HTMLElement[];
    const ringed = cards.filter((c) => c.className.includes("ring-duck"));
    expect(ringed).toHaveLength(1);
    expect(ringed[0].textContent).toContain("waiter");
    expect(ringed[0].className).toContain("shadow-e2");
    // Selection: an inner ink outline, not a ring, so it never reads as the waiting ring or the focus outline.
    const picked = cards.find((c) => c.textContent?.includes("alpha"))!;
    expect(picked.className).toContain("outline-ink");
    expect(picked.className).not.toContain("ring-");
    // Columns sit on the well with no border; no yellow anywhere else on the board.
    const columns = container.querySelectorAll("[data-board-column]");
    expect(columns.length).toBeGreaterThan(0);
    for (const col of columns) expect(col.className).toContain("bg-well");
    expect(container.querySelectorAll('[class*="-duck"]:not([class*="duck-stage"])')).toHaveLength(1);
    expect(container.querySelectorAll("button button, button a, a button")).toHaveLength(0);
  });
});
