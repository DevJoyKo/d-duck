import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PreferredSection } from "./PreferredSection";
import { addable, installedOf, sameItem, toItem, type PreferredItem } from "../lib/preferred";

const OPTIONS = [
  { kind: "plugin", name: "kit", source: "plugin:kit", scope: null, description: null, members: ["grill", "tdd"], marketplace: "mk", enabled: true },
  { kind: "skill", name: "tdd", source: "user", scope: null, description: "테스트 먼저", members: [], marketplace: null, enabled: true },
  { kind: "agent", name: "reviewer", source: "project", scope: "/work/a", description: "리뷰", members: [], marketplace: null, enabled: true },
  { kind: "plugin", name: "dup", source: "plugin:dup", scope: null, description: null, members: ["a"], marketplace: "one", enabled: true },
  { kind: "plugin", name: "dup", source: "plugin:dup", scope: null, description: null, members: ["b"], marketplace: "two", enabled: true },
  { kind: "plugin", name: "sleepy", source: "plugin:sleepy", scope: null, description: null, members: ["c"], marketplace: "mk", enabled: false },
  { kind: "plugin", name: "mattpocock-skills", source: "plugin:mattpocock-skills", scope: null, description: null, members: ["x"], marketplace: "elsewhere", enabled: true },
];
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => OPTIONS) }));

const DEFAULT: PreferredItem[] = [{ kind: "plugin", name: "mattpocock-skills", source: "plugin:mattpocock-skills", marketplace: "claude-plugins-official" }];
let last: PreferredItem[] | null = null;
function Harness({ start, defaults = DEFAULT }: { start: PreferredItem[]; defaults?: PreferredItem[] }) {
  const [v, setV] = useState(start);
  return (
    <PreferredSection
      value={v}
      defaults={defaults}
      onChange={(n) => {
        last = n;
        setV(n);
      }}
    />
  );
}
const show = (start: PreferredItem[], defaults?: PreferredItem[]) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <Harness start={start} defaults={defaults} />
    </QueryClientProvider>,
  );

afterEach(() => {
  cleanup();
  last = null;
});

describe("PreferredSection", () => {
  it("adds an installed item and removes it again", async () => {
    show([]);
    expect(screen.getByText("아직 고른 항목이 없어요.")).toBeTruthy();
    const pick = await screen.findByLabelText("추가할 항목");
    await waitFor(() => expect((pick as HTMLSelectElement).disabled).toBe(false));
    fireEvent.change(pick, { target: { value: "plugin|plugin:kit||mk|kit" } });
    fireEvent.click(screen.getByRole("button", { name: "추가" }));
    expect(last).toEqual([{ kind: "plugin", name: "kit", source: "plugin:kit", marketplace: "mk" }]);
    const list = screen.getByRole("list", { name: "먼저 살펴볼 스킬·에이전트" });
    expect(within(list).getByText("kit")).toBeTruthy();
    expect(within(list).getByText("스킬·에이전트 2개")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "kit 빼기" }));
    expect(last).toEqual([]);
  });

  it("marks a chosen item that is not installed", async () => {
    show([{ kind: "agent", name: "old-helper", source: "user" }, { kind: "skill", name: "tdd", source: "user" }]);
    expect(await screen.findByText("설치되지 않아 빠져요")).toBeTruthy();
    expect(screen.getAllByText("설치되지 않아 빠져요")).toHaveLength(1);
    expect(screen.getByText("테스트 먼저")).toBeTruthy();
  });

  it("stops at ten items", async () => {
    const ten = Array.from({ length: 10 }, (_, n) => ({ kind: "skill" as const, name: `s${n}`, source: "user" }));
    show(ten);
    const pick = await screen.findByLabelText("추가할 항목");
    expect((pick as HTMLSelectElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "추가" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("10개까지 고를 수 있어요")).toBeTruthy();
  });

  it("shows the default plugin and says it is left out when not installed", async () => {
    show(DEFAULT);
    const list = screen.getByRole("list", { name: "먼저 살펴볼 스킬·에이전트" });
    expect(within(list).getByText("mattpocock-skills")).toBeTruthy();
    expect(within(list).getByText("플러그인")).toBeTruthy();
    expect(await within(list).findByText("설치되지 않아 빠져요")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "목록 기본값으로" }), "no reset while it is the default").toBeNull();
  });

  it("leaves out a turned-off plugin and an unnamed pick found in two marketplaces", async () => {
    show([
      { kind: "plugin", name: "sleepy", source: "plugin:sleepy", marketplace: "mk" },
      { kind: "plugin", name: "dup", source: "plugin:dup" },
      { kind: "plugin", name: "dup", source: "plugin:dup", marketplace: "two" },
    ]);
    expect(await screen.findByText("꺼져 있어 빠져요")).toBeTruthy();
    expect(screen.getByText("같은 이름이 여러 마켓플레이스에 있어 빠져요")).toBeTruthy();
    expect(screen.getByText("스킬·에이전트 1개"), "the named marketplace is found").toBeTruthy();
    expect(screen.queryByText("설치되지 않아 빠져요")).toBeNull();
  });

  it("goes back to the default after the list was cleared", async () => {
    show(DEFAULT);
    fireEvent.click(screen.getByRole("button", { name: "목록 비우기" }));
    expect(last).toEqual([]);
    expect(screen.getByText("아직 고른 항목이 없어요.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "목록 기본값으로" }));
    expect(last).toEqual(DEFAULT);
    expect(screen.getByText("mattpocock-skills")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "목록 기본값으로" })).toBeNull();
  });

  it("says it is guidance, not an enforced guardrail", () => {
    show([]);
    expect(screen.getByText(/강제되지는 않아요/)).toBeTruthy();
  });
});

describe("preferred helpers", () => {
  const opts = OPTIONS as Parameters<typeof addable>[0];
  it("matches project items by folder too", () => {
    const a = { kind: "agent" as const, name: "reviewer", source: "project", scope: "/work/a" };
    expect(sameItem(a, opts[2])).toBe(true);
    expect(sameItem({ ...a, scope: "/work/b" }, opts[2])).toBe(false);
    expect(installedOf({ ...a, source: "user", scope: null }, opts)).toBeNull();
    expect(installedOf(a, undefined)).toBeUndefined();
  });
  it("filters by name or description and leaves out chosen items", () => {
    expect(addable(opts, [toItem(opts[1])], "").map((o) => o.name)).toEqual(["kit", "reviewer", "dup", "dup", "sleepy", "mattpocock-skills"]);
    expect(addable(opts, [], "리뷰").map((o) => o.name)).toEqual(["reviewer"]);
    expect(toItem(opts[2])).toEqual({ kind: "agent", name: "reviewer", source: "project", scope: "/work/a" });
  });
});
