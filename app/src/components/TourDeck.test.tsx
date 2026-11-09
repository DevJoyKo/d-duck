import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OnboardingState, Status } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const { api } = await import("../lib/api");
const onboarding = await import("../lib/onboarding");
const { TourDeck, TourToast } = await import("./TourDeck");
const { StartChecklist } = await import("./StartChecklist");
const { checklistVisible, emptyState, TOUR_CARDS } = await import("../lib/tour");
const auth = await import("../lib/auth");
// @ts-expect-error no @types/node in this project (same as vite.config.ts)
const { readFileSync } = await import("node:fs");
const cssSource: string = readFileSync("src/index.css", "utf8");

const status = (cli: boolean): Status =>
  ({
    cli: cli ? { path: "/secret/place/bin/claude", version: "2.1.319", supported: true } : null,
    configDir: "/secret/place/.claude",
  }) as Status;

const V1 = ["welcome", "agents", "tasks", "outputs", "extend", "permissions", "memory", "usage", "finish"];
const V2 = TOUR_CARDS.map((c) => c.id as string);
let stored: OnboardingState | null = null;
const allSeen = (): OnboardingState => ({ ...emptyState(), seen: [...V1, ...V2], checklist: { firstTask: true, timelineOpen: true, notifyOn: true, refineUsed: true, reportOpened: true } });
const navigate = vi.fn();
const onStart = vi.fn();

/** What App.tsx renders for the tour, plus a button that had focus before. */
function Layer({ cli = true }: { cli?: boolean }) {
  const o = onboarding.useOnboarding();
  return (
    <>
      <div inert={o.deck !== null}>
        <button>before</button>
      </div>
      {o.deck && (
        <TourDeck
          cards={o.deck}
          status={status(cli)}
          onClose={onboarding.closeTour}
          onNavigate={navigate}
          onStart={() => {
            onboarding.closeTour("done");
            onStart();
          }}
        />
      )}
      {o.toast && !o.deck && <TourToast onOpen={onboarding.openTour} onClose={onboarding.dismissToast} />}
      {!o.deck && o.state && checklistVisible(o.state) && <StartChecklist state={o.state} onNavigate={navigate} />}
    </>
  );
}

async function start(initial: OnboardingState | null, cli = true) {
  stored = initial;
  render(
    <StrictMode>
      <Layer cli={cli} />
    </StrictMode>,
  );
  await act(() => onboarding.loadOnboarding());
}

const title = () => screen.queryByRole("dialog") && document.getElementById("tour-title")?.textContent;
const card = () => screen.getByTestId("tour-card");

beforeEach(() => {
  onboarding.resetOnboardingForTests();
  navigate.mockReset();
  onStart.mockReset();
  vi.spyOn(api, "onboardingState").mockImplementation(async () => stored);
  vi.spyOn(api, "setOnboardingState").mockImplementation(async (s) => {
    stored = s;
  });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("tour v2", () => {
  it("opens on a fresh install with n/5, flips by click and keys, and Esc closes as dismissed", async () => {
    await start(null);
    expect(screen.getByRole("dialog").getAttribute("aria-modal")).toBe("true");
    expect(title()).toBe("세션 하나에 다 시키고 있나요?");
    expect(screen.getByText("1/5")).toBeTruthy();
    fireEvent.click(card());
    expect(title()).toBe("D.Duck에선 오리마다 역할이 있어요");
    fireEvent.keyDown(card(), { key: "ArrowRight" });
    fireEvent.keyDown(card(), { key: " " });
    expect(title()).toBe("누가 무엇을 하고 있는지 보여요");
    expect(screen.getByText("4/5")).toBeTruthy();
    fireEvent.keyDown(card(), { key: "ArrowLeft" });
    expect(screen.getByText("3/5")).toBeTruthy();
    fireEvent.keyDown(card(), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(stored?.dismissed).toBe(true);
    expect(stored?.version).toBe(2);
    expect(stored?.seen).toEqual(V2);
  });

  it("progress text and dots agree: five story dots n/5 and a separate marker for the action card", async () => {
    await start(null);
    const story = () => [...card().querySelectorAll('[data-progress="story"]')];
    const action = () => card().querySelectorAll('[data-progress="action"]');
    expect(story()).toHaveLength(5);
    expect(action()).toHaveLength(1);
    for (let k = 1; k <= 5; k += 1) {
      expect(screen.getByText(`${k}/5`)).toBeTruthy();
      expect(story()[k - 1].getAttribute("aria-current")).toBe("step");
      expect(story()[k - 1].getAttribute("aria-label")).toMatch(new RegExp(`^${k}/5: `));
      fireEvent.keyDown(card(), { key: "ArrowRight" });
    }
    expect(screen.getByText("직접 해 보기")).toBeTruthy();
    expect(action()[0].getAttribute("aria-current")).toBe("step");
    expect(story().every((d) => !d.hasAttribute("aria-current"))).toBe(true);
  });

  it("every card has the same fixed height with the bottom bar pinned to its foot (다음 does not move)", async () => {
    await start(null);
    const classes = new Set<string>();
    for (let k = 0; k < 6; k += 1) {
      classes.add(card().className);
      const bar = screen.getByTestId("tour-bar");
      expect(bar.parentElement).toBe(card());
      expect(card().lastElementChild?.previousElementSibling ?? card().lastElementChild).toBe(bar);
      expect(bar.className).toContain("shrink-0");
      fireEvent.keyDown(card(), { key: "ArrowRight" });
    }
    expect(classes.size).toBe(1);
    const cls = [...classes][0];
    expect(cls).toMatch(/(^| )h-\[480px\]( |$)/);
    expect(cls).toContain("flex-col");
  });

  it("the action card keeps the setup links to one quiet line under the text", async () => {
    await start(null);
    fireEvent.click(screen.getByLabelText(/^직접 해 보기:/));
    const line = screen.getByText(/규칙과 권한은 한곳에서/);
    expect(line.tagName).toBe("P");
    expect(line.className).toContain("text-ink-3");
    expect(line.querySelectorAll("button")).toHaveLength(4);
  });

  it("skip is there on every card but the last", async () => {
    await start(null);
    for (let k = 0; k < 5; k += 1) {
      expect(screen.getByText("건너뛰기")).toBeTruthy();
      fireEvent.keyDown(card(), { key: "ArrowRight" });
    }
    expect(title()).toBe("내 일로 첫 작업 맡겨 보기");
    expect(screen.queryByText("건너뛰기")).toBeNull();
    expect(screen.getByText("직접 해 보기")).toBeTruthy();
  });

  it("demos are inert, hidden from screen readers and carry a text alternative", async () => {
    await start(null);
    const demo = screen.getByTestId("tour-demo");
    expect(demo.hasAttribute("inert")).toBe(true);
    expect(demo.getAttribute("aria-hidden")).toBe("true");
    expect(screen.getByText(TOUR_CARDS[0].demoAlt)).toBeTruthy();
    // Clicking the demo flips the card like the rest of it.
    fireEvent.click(demo);
    expect(title()).toBe("D.Duck에선 오리마다 역할이 있어요");
  });

  it("the last card opens the composer and starts nothing; setup links navigate", async () => {
    await start(null);
    fireEvent.click(screen.getByLabelText(/^직접 해 보기:/));
    fireEvent.click(screen.getByText("첫 작업 맡겨 보기"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onStart).toHaveBeenCalledTimes(1);
    expect(stored?.dismissed).toBe(false);
    act(() => onboarding.openTour());
    fireEvent.click(screen.getByLabelText(/^직접 해 보기:/));
    fireEvent.click(screen.getByRole("button", { name: "권한" }));
    expect(navigate).toHaveBeenCalledWith("permissions");
  });

  it("does not open when everything was seen", async () => {
    await start(allSeen());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByLabelText("시작하기 체크리스트")).toBeNull();
  });

  it("a version 1 user gets a toast once instead of the deck; the toast opens the full deck", async () => {
    await start({ version: 1, seen: V1, dismissed: false, checklist: { firstTask: true, timelineOpen: true, notifyOn: false } as OnboardingState["checklist"] });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText("새로워진 안내 보기")).toBeTruthy();
    // Saved at once: v2, the new cards seen, the old checklist kept with the new keys.
    expect(stored?.version).toBe(2);
    expect(stored?.seen).toEqual(expect.arrayContaining([...V1, ...V2]));
    expect(stored?.checklist).toEqual({ firstTask: true, timelineOpen: true, notifyOn: false, refineUsed: false, reportOpened: false });
    fireEvent.click(screen.getByText("새로워진 안내 보기"));
    expect(title()).toBe("세션 하나에 다 시키고 있나요?");
    expect(screen.getAllByRole("button", { name: /^(\d\/5|직접 해 보기):/ })).toHaveLength(6);
    // Next launch: no toast, no deck.
    cleanup();
    onboarding.resetOnboardingForTests();
    await start(stored);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByText("새로워진 안내 보기")).toBeNull();
  });

  it("button, dot and selected-text clicks do not advance", async () => {
    await start(null);
    fireEvent.click(screen.getByLabelText(/^4\/5:/));
    expect(title()).toBe("누가 무엇을 하고 있는지 보여요");
    expect(screen.getByLabelText(/^4\/5:/).getAttribute("aria-current")).toBe("step");
    fireEvent.click(screen.getByText("이전"));
    expect(title()).toBe("틀에 맞춰 맡기고, 고르기만 하면 다듬어져요");
    const sel = vi.spyOn(window, "getSelection").mockReturnValue({ toString: () => "Claude" } as Selection);
    fireEvent.click(card());
    expect(title()).toBe("틀에 맞춰 맡기고, 고르기만 하면 다듬어져요");
    sel.mockRestore();
  });

  it("traps focus and restores it on close", async () => {
    stored = null;
    render(
      <StrictMode>
        <Layer />
      </StrictMode>,
    );
    const before = screen.getByText("before");
    before.focus();
    await act(() => onboarding.loadOnboarding());
    const dialog = screen.getByRole("dialog");
    expect(document.activeElement).toBe(dialog);
    const buttons = [...dialog.querySelectorAll("button")];
    buttons[buttons.length - 1].focus();
    fireEvent.keyDown(buttons[buttons.length - 1], { key: "Tab" });
    expect(document.activeElement).toBe(buttons[0]);
    fireEvent.keyDown(buttons[0], { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(buttons[buttons.length - 1]);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(before));
  });

  it("restores focus to the replay opener even though the shell blurs it when it turns inert", async () => {
    await start(allSeen());
    const before = screen.getByText("before");
    before.focus();
    act(() => {
      onboarding.openTour();
      // A real browser drops focus from an element that becomes inert; jsdom does not, so do it here.
      (document.activeElement as HTMLElement).blur();
    });
    expect(document.activeElement).toBe(screen.getByRole("dialog"));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(before));
  });

  it("offers the install command on the last card when the CLI is missing, never paths", async () => {
    await start(null, false);
    expect(document.body.textContent).not.toContain("/secret/place");
    fireEvent.click(screen.getByLabelText(/^직접 해 보기:/));
    expect(screen.getByText("Claude Code를 찾지 못했어요")).toBeTruthy();
    expect(screen.getByText("설치 명령 복사")).toBeTruthy();
  });

  it.each([
    [false, "loggedOut", "Claude Code를 찾지 못했어요"],
    [true, "loggedOut", "Claude에 로그인해야 이 기능을 쓸 수 있어요"],
    [true, "loggedIn", null],
  ] as const)("start card, cli %s and %s: the status line is %s (claude-login)", async (cli, login, line) => {
    auth.setAuthForTests({ view: { login, method: null, connection: null, plan: null, claudeAi: login === "loggedIn", expiresSoon: false } });
    await start(null, cli);
    fireEvent.click(screen.getByLabelText(/^직접 해 보기:/));
    const missing = screen.queryByText("Claude Code를 찾지 못했어요");
    const login_ = screen.queryByTestId("tour-login");
    if (line === null) {
      expect(missing).toBeNull();
      expect(login_).toBeNull();
    } else {
      expect(screen.getByText(line)).toBeTruthy();
      expect(missing !== null && login_ !== null).toBe(false);
    }
    expect(login_ ? login_.querySelector("button")?.textContent : null).toBe(cli && login === "loggedOut" ? "로그인" : null);
    auth.resetAuthForTests();
  });

  it("fades cards, keeps other demos still, and reduced motion turns motion off", () => {
    expect(cssSource).toMatch(/\.tour-fade\s*{\s*animation: tour-fade/);
    expect(cssSource).toMatch(/\.tour-still \*\s*{\s*animation: none !important/);
    expect(cssSource).toMatch(/prefers-reduced-motion: reduce\)\s*{\s*\*\s*{[^}]*animation: none !important/);
  });
});

describe("start checklist", () => {
  it("lists the v2 items, ticks them as they happen and disappears when all are done", async () => {
    await start({ ...allSeen(), checklist: { firstTask: false, timelineOpen: true, notifyOn: true, refineUsed: false, reportOpened: false } });
    expect(screen.getByText("0/3")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /시작하기/ }));
    expect(screen.getByText("다듬기 써 보기")).toBeTruthy();
    await act(() => onboarding.markDone("firstTask"));
    expect(screen.getByText("1/3")).toBeTruthy();
    fireEvent.click(screen.getByText("보고서 열어 보기"));
    expect(navigate).toHaveBeenCalledWith("tasks");
    await act(() => onboarding.markDone("refineUsed"));
    await act(() => onboarding.markDone("reportOpened"));
    await waitFor(() => expect(screen.queryByLabelText("시작하기 체크리스트")).toBeNull());
    expect(stored?.checklist).toEqual({ firstTask: true, timelineOpen: true, notifyOn: true, refineUsed: true, reportOpened: true });
    expect(JSON.stringify(stored)).not.toMatch(/sessionId|path|prompt/);
  });
});
