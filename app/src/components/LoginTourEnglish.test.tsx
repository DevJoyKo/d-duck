// app-language (tasks 2.3, 3.7): the first-run tour, the login banner and sheet, and the shared pieces in English.
// No Hangul may be left in what the app writes itself.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Status } from "../lib/api";
import type { AuthView } from "../lib/auth";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

const { applyLanguage } = await import("../i18n/language");
const auth = await import("../lib/auth");
const { THEME_LABEL } = await import("../lib/theme");
const { CHECKLIST, SETUP_LINKS, TOUR_CARDS, ROLE_STYLE, tourGoalExamples } = await import("../lib/tour");
const { TourDeck, TourToast } = await import("./TourDeck");
const { TourDemo } = await import("./TourDemos");
const { LoginBanner, GateLine, LoginSheet, AuthSettingsRow } = await import("./LoginBits");
const { MoreInfo } = await import("./ui");
const { Sidebar } = await import("./Sidebar");
const { SettingsPage } = await import("./SettingsPage");
const { GUARD_COPY } = await import("../lib/delivery");

const HANGUL = /[가-힣]/;
/** Each Korean run with a little context, so a failure names the text. */
const korean = (text: string) => (text.replace(OTHER_GROUPS, "").match(/.{0,16}[가-힣]+.{0,16}/g) ?? []);
/** Text from a shared piece another screen group still moves to the catalog: OptionLine's "추천" badge
 *  (BriefRefiner). Drop once its owner lands. */
const OTHER_GROUPS = /추천/g;
const view = (login: AuthView["login"], extra: Partial<AuthView> = {}): AuthView => ({ login, method: null, connection: null, plan: null, claudeAi: false, expiresSoon: false, ...extra });
const status = { cli: { path: "/usr/local/bin/claude", version: "2.1.0", supported: true }, configDir: "/Users/me/.claude" } as Status;
const wrap = (ui: ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);

/** Visible text plus the labels screen readers read. */
const allText = (root: HTMLElement) => [root.textContent ?? "", ...[...root.querySelectorAll("[aria-label],[title]")].flatMap((e) => [e.getAttribute("aria-label") ?? "", e.getAttribute("title") ?? ""])].join("\n");

beforeEach(() => {
  applyLanguage("en");
  auth.resetAuthForTests();
});
afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("tour in English", () => {
  it("every card, its demo and the bar have no Korean, and the start card shows the login line", () => {
    auth.setAuthForTests({ view: view("loggedOut") });
    const { container } = render(<TourDeck cards={TOUR_CARDS} status={status} onClose={() => {}} onNavigate={() => {}} onStart={() => {}} />);
    for (let i = 0; i < TOUR_CARDS.length; i++) {
      expect(korean(allText(container)), TOUR_CARDS[i].id).toEqual([]);
      if (i < TOUR_CARDS.length - 1) fireEvent.click(screen.getByRole("button", { name: /^Next/ }));
    }
    expect(screen.getByRole("heading", { name: "Hand off your first task" })).toBeTruthy();
    expect(screen.getByTestId("tour-login").textContent).toContain("Log in to Claude to use this");
    expect(screen.getByRole("button", { name: "Hand off a first task" })).toBeTruthy();
  });

  it("card text stays one line each: a short title, one sentence, three points", () => {
    for (const c of TOUR_CARDS) {
      expect(c.title.length, c.id).toBeLessThanOrEqual(48);
      expect(c.body.split(/[.?!]\s/).length, c.id).toBe(1);
      expect(c.points).toHaveLength(3);
      for (const p of c.points) expect(p.length, p).toBeLessThanOrEqual(56);
    }
  });

  it("demos, the toast, links, examples and checklist labels follow the language", () => {
    for (const c of TOUR_CARDS) {
      const { container, unmount } = render(<TourDemo id={c.id} examples={tourGoalExamples()} />);
      expect(korean(allText(container)), c.id).toEqual([]);
      unmount();
    }
    const toast = render(<TourToast onOpen={() => {}} onClose={() => {}} />);
    expect(korean(allText(toast.container))).toEqual([]);
    const labels = [...SETUP_LINKS.map((l) => l.label), ...CHECKLIST.map((c) => c.label), ...tourGoalExamples(), ...Object.values(ROLE_STYLE), ...Object.values(THEME_LABEL)];
    expect(labels.filter((l) => HANGUL.test(l))).toEqual([]);
    applyLanguage("ko");
    expect(SETUP_LINKS[3].label).toBe("메모리와 규칙");
    expect(THEME_LABEL.system).toBe("시스템 설정 따름");
  });
});

describe("login in English", () => {
  it("banner, gate line with its fold, settings row and the failed sheet have no Korean", () => {
    auth.setAuthForTests({ view: view("loggedOut"), sheet: true, login: { phase: "failed", reason: "timeout", hasLink: false, method: "claudeAi" } });
    const { container } = render(
      <>
        <LoginBanner cliMissing={false} />
        <GateLine force="claudeAi" more />
        <AuthSettingsRow />
        <LoginSheet />
        <MoreInfo>x</MoreInfo>
      </>,
    );
    expect(korean(allText(container))).toEqual([]);
    expect(screen.getByText("Log in to Claude to hand off tasks")).toBeTruthy();
    expect(screen.getByTestId("login-fail-reason").textContent).toBe("Stopped waiting after 5 minutes");
    expect(auth.COPY.gate).toBe("Log in to Claude to use this");
  });

  it("COPY reads the current language on every access", () => {
    expect(auth.COPY.banner).toBe("Log in to Claude to hand off tasks");
    applyLanguage("ko");
    expect(auth.COPY.banner).toBe("Claude에 로그인해야 작업을 맡길 수 있어요");
  });
});

describe("sidebar in English", () => {
  it("has no Korean apart from the language names", () => {
    const { container } = wrap(<Sidebar page="tasks" onNavigate={() => {}} waiting={0} status={status} ingest={null} />);
    expect(korean(allText(container).replace(/한국어/g, ""))).toEqual([]);
  });
});

describe("settings in English", () => {
  it("every row has no Korean apart from user data and the push-guard row", async () => {
    const { container } = wrap(<SettingsPage />);
    await waitFor(() => expect(screen.getByText("Version 2.1.319")).toBeTruthy());
    // The push-guard row (lib/delivery GUARD_COPY, LocalOnlySection) belongs to another screen group.
    screen.getByRole("heading", { name: GUARD_COPY.title }).closest("div.grid")?.remove();
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(korean(allText(container).replace(/한국어/g, ""))).toEqual([]);
  });
});
