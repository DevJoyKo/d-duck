import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LoadoutSection, type LoadoutValue } from "./LoadoutSection";

const H = "a".repeat(32);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => ({
    skills: [
      { name: "frontend-design", source: "user", plugin: null, hash: H, bytes: 9800, description: null },
      { name: "expo:expo-ui", source: "plugin:expo", plugin: "expo", hash: H, bytes: 8000, description: null },
      { name: "expo:expo-router", source: "plugin:expo", plugin: "expo", hash: H, bytes: 6000, description: null },
      { name: "empty", source: "user", plugin: null, hash: H, bytes: 0, description: null },
      { name: "huge", source: "user", plugin: null, hash: H, bytes: 90_000, description: null },
    ],
    unreadableSkills: ["shadowed"],
    mcp: [
      { name: "figma", transport: "http", scope: "local", host: null },
      { name: "notion", transport: "http", scope: "user", host: null },
      { name: "reports__v2", transport: "http", scope: "user", host: null },
    ],
    unreadable: [],
  })),
}));

let last: LoadoutValue | null = null;
function Harness({ start, toolList, strict }: { start: LoadoutValue; toolList?: boolean; strict?: boolean }) {
  const [v, setV] = useState(start);
  return (
    <LoadoutSection
      value={v}
      toolList={toolList}
      strict={strict}
      onChange={(n) => {
        last = n;
        setV(n);
      }}
    />
  );
}
const show = (start: LoadoutValue, toolList = false, strict = false) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <Harness start={start} toolList={toolList} strict={strict} />
    </QueryClientProvider>,
  );

afterEach(() => {
  cleanup();
  last = null;
});

describe("LoadoutSection", () => {
  it("adds a plugin bundle (skills only) and shows the preload size", async () => {
    show({ skills: [], mcp: [] });
    fireEvent.click(await screen.findByRole("button", { name: /스킬 고르기/ }));
    fireEvent.click(await screen.findByRole("button", { name: /묶음 담기/ }));
    expect(last?.skills.map((s) => s.name)).toEqual(["expo:expo-ui", "expo:expo-router"]);
    expect(last?.mcp).toEqual([]);
    expect(screen.getAllByText("약 4천 토큰").length).toBeGreaterThan(0);
    expect(screen.getAllByText("스킬만 담아요").length).toBe(1);
    // Pinned (security review): servers outside the static list are not withheld, and the screen says so.
    expect(screen.getByText("목록 밖 서버(claude.ai 커넥터·플러그인 MCP)는 막지 못해요")).toBeTruthy();
    expect(screen.getByText("지금: 모든 서버")).toBeTruthy();
    expect(screen.getByText("서버를 더 고르면 저장할 때 확인해요 · 엄격 유형은 스킬 추가도 확인해요")).toBeTruthy();
  });

  it("hides a 0-byte size, says 1천 미만 and warns above 20K tokens", async () => {
    show({ skills: [{ name: "empty", source: "user", hash: H }], mcp: [] });
    await screen.findByText("empty");
    await waitFor(() => expect(screen.queryByText(/토큰/)).toBeNull());
    cleanup();
    show({ skills: [{ name: "huge", source: "user", hash: H }], mcp: [] });
    const size = await screen.findByText("약 2만 3천 토큰");
    expect(size.className).toContain("text-warn");
    expect(screen.getByText("많이 넣으면 작업 공간이 줄어요")).toBeTruthy();
  });

  it("closes the skill panel on Escape without closing the sheet and returns focus", async () => {
    const sheet = vi.fn();
    window.addEventListener("keydown", sheet);
    show({ skills: [], mcp: [] });
    const btn = await screen.findByRole("button", { name: /스킬 고르기/ });
    fireEvent.click(btn);
    const box = screen.getAllByRole("checkbox")[0];
    box.focus();
    fireEvent.keyDown(box, { key: "Escape" });
    expect(screen.queryByLabelText("스킬 찾기")).toBeNull();
    expect(sheet).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(btn);
    window.removeEventListener("keydown", sheet);
  });

  it("marks changed items 바뀜, missing ones 없음 and tidies them", async () => {
    show({ skills: [{ name: "frontend-design", source: "user", hash: "b".repeat(32) }, { name: "gone", source: "user", hash: H }, { name: "shadowed", source: "user", hash: H }], mcp: ["old-server"] });
    await waitFor(() => expect(screen.getAllByText("없음")).toHaveLength(2));
    expect(screen.getAllByText("바뀜")).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "없는 항목 정리" }));
    expect(last).toEqual({ skills: [], mcp: [] });
  });

  it("chooses servers, blocks __ names and explains an empty choice", async () => {
    show({ skills: [], mcp: [] }, true);
    expect(await screen.findByText("고른 서버만 써요 · 안 고르면 MCP를 쓰지 않아요")).toBeTruthy();
    await screen.findByText("figma");
    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    const reports = boxes.find((b) => b.closest("label")?.textContent?.includes("reports__v2"));
    expect(reports?.disabled).toBe(true);
    expect(screen.getByText("reports__v2: 이름 때문에 따로 막을 수 없어요")).toBeTruthy();
    expect(screen.getByText("지금: MCP 안 씀")).toBeTruthy();
    expect(screen.queryByText(/목록 밖 서버/)).toBeNull();
    const figma = boxes.find((b) => b.closest("label")?.textContent?.includes("figma"))!;
    expect(figma.closest("label")?.className).toContain("has-[:focus-visible]:ring-2");
    expect(figma.closest("label")?.textContent).toContain("이 프로젝트 · 개인");
    fireEvent.click(figma);
    expect(last?.mcp).toEqual(["figma"]);
    expect(screen.getByText("지금: figma만")).toBeTruthy();
    expect(figma.closest("label")?.querySelector("svg.lucide-check")).toBeTruthy();
  });
});
