// app-language test-stage review fixes (D-171): the composer draft survives a trip to Settings (QA FAIL 2), the
// sidebar language menu is drawn above the page (QA FAIL 1), the review badge says "task"/"tasks" (QA FAIL 3).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";

/** "새 작업" stays disabled until get_status says the CLI is there; clicking it earlier does nothing (flaky on slow CI). */
async function enabledButton(name: string) {
  return waitFor(() => {
    const b = screen.getAllByRole("button", { name }).find((x) => !(x as HTMLButtonElement).disabled);
    expect(b).toBeTruthy();
    return b!;
  });
}

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

async function boot(page: string) {
  localStorage.setItem("dduck.page", page);
  const { default: App } = await import("../App");
  render(<App />);
}

beforeEach(() => vi.resetModules());
afterEach(() => {
  cleanup();
  localStorage.clear();
  applyLanguage("ko");
});

const nav = (name: string) => within(screen.getByRole("navigation")).getByRole("button", { name: new RegExp(`^${name}`) });

describe("작업 시키기 draft", () => {
  it("keeps the typed brief after going to Settings and back, with or without a language change", async () => {
    await boot("tasks");
    fireEvent.click(await enabledButton("새 작업"));
    const goal = (await screen.findByLabelText("목표 (필수)")) as HTMLTextAreaElement;
    fireEvent.change(goal, { target: { value: "로그인 버그 고치기" } });
    fireEvent.click(nav("설정"));
    expect(await screen.findByRole("heading", { name: "설정" })).toBeTruthy();
    expect(screen.queryByLabelText("목표 (필수)")).toBeNull();
    fireEvent.click(nav("작업"));
    expect(((await screen.findByLabelText("목표 (필수)")) as HTMLTextAreaElement).value).toBe("로그인 버그 고치기");

    // Language changed on the way.
    fireEvent.click(nav("설정"));
    fireEvent.change(await screen.findByRole("combobox", { name: "언어" }), { target: { value: "en" } });
    fireEvent.click(nav("Tasks"));
    expect(((await screen.findByLabelText("Goal (required)")) as HTMLTextAreaElement).value).toBe("로그인 버그 고치기");

    // Closed on purpose: the next opening starts empty.
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    expect(((await screen.findByLabelText("Goal (required)")) as HTMLTextAreaElement).value).toBe("");
  });
});

describe("sidebar language menu", () => {
  it("is portaled to <body> with fixed placement above the page", async () => {
    await boot("tasks");
    fireEvent.click(await screen.findByRole("button", { name: "언어 바꾸기" }));
    const menu = await screen.findByRole("menu");
    expect(menu.parentElement).toBe(document.body);
    expect(menu.className).toContain("fixed");
    expect(menu.className).toContain("z-50");
    expect(screen.getByRole("navigation").contains(menu)).toBe(false);
    // Clicking an item inside the portal still counts as inside the menu.
    fireEvent.mouseDown(within(menu).getByRole("menuitemradio", { name: "English" }));
    fireEvent.click(within(menu).getByRole("menuitemradio", { name: "English" }));
    await waitFor(() => expect(document.documentElement.lang).toBe("en"));
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("review badge", () => {
  it("counts tasks in English: one and many", async () => {
    const { badgeLabel } = await import("../lib/review");
    const { applyLanguage: apply } = await import("../i18n/language");
    apply("en");
    expect(badgeLabel.review(1)).toBe("1 task to review");
    expect(badgeLabel.review(3)).toBe("3 tasks to review");
    expect(badgeLabel.review(5)).toBe("5 tasks to review");
    expect(badgeLabel.tasks(1)).toBe("1 needs you");
    expect(badgeLabel.tasks(3)).toBe("3 need you");
    apply("ko");
    expect(badgeLabel.review(3)).toBe("리뷰 전 3개");
  });
});
