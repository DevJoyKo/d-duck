// app-language: the settings row and the sidebar quick switch share `ui_language`, apply at once, keep drafts.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPage } from "./SettingsPage";
import { Sidebar } from "./Sidebar";
import { applyLanguage, currentLang } from "../i18n/language";
import { useT } from "../i18n/react";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

const wrap = (ui: ReactNode) => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{ui}</QueryClientProvider>);
const languageCalls = () => vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_language").map(([, a]) => a);

afterEach(async () => {
  // Let a pending save answer (the mock waits 30 ms) before resetting, so it cannot flip the next test.
  await new Promise((r) => setTimeout(r, 60));
  cleanup();
  vi.mocked(invoke).mockClear();
  applyLanguage("ko");
});

/** Stands in for the task composer: its text lives in component state. */
function Draft() {
  const t = useT();
  const [goal, setGoal] = useState("");
  return <input aria-label={t("settings.language.title")} data-testid="goal" value={goal} onChange={(e) => setGoal(e.target.value)} />;
}

describe("설정 > 언어", () => {
  it("offers system / 한국어 / English, each language named in itself, and applies at once", async () => {
    wrap(<SettingsPage />);
    const select = screen.getByRole("combobox", { name: "언어" }) as HTMLSelectElement;
    expect(select.value).toBe("ko");
    expect([...select.options].map((o) => o.textContent)).toEqual(["시스템 언어 따름", "한국어", "English"]);
    fireEvent.change(select, { target: { value: "en" } });
    expect(currentLang()).toBe("en");
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(document.documentElement.lang).toBe("en");
    const after = screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement;
    expect([...after.options].map((o) => o.textContent)).toEqual(["System language", "한국어", "English"]);
    await waitFor(() => expect(languageCalls()).toEqual([{ choice: "en" }]));
  });

  it("keeps a draft typed before the change", () => {
    wrap(
      <>
        <Draft />
        <SettingsPage />
      </>,
    );
    fireEvent.change(screen.getByTestId("goal"), { target: { value: "로그인 버그 고치기" } });
    fireEvent.change(screen.getByRole("combobox", { name: "언어" }), { target: { value: "en" } });
    const goal = screen.getByTestId("goal") as HTMLInputElement;
    expect(goal.value).toBe("로그인 버그 고치기");
    expect(goal.getAttribute("aria-label")).toBe("Language");
  });
});

describe("사이드바 빠른 언어 전환", () => {
  const sidebar = () => wrap(<Sidebar page="settings" onNavigate={() => {}} waiting={0} ingest={null} />);

  it("switches from the menu, names itself in the current language, and saves the same value", async () => {
    sidebar();
    fireEvent.click(screen.getByRole("button", { name: "언어 바꾸기" }));
    const menu = screen.getByRole("menu");
    expect(within(menu).getAllByRole("menuitemradio").map((i) => i.textContent)).toEqual(["한국어", "English", "시스템 언어 따름"]);
    expect(within(menu).getByRole("menuitemradio", { name: "한국어" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(menu).getByRole("menuitemradio", { name: "English" }));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "Change language" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Workspace" })).toBeTruthy();
    await waitFor(() => expect(languageCalls()).toEqual([{ choice: "en" }]));
  });

  it("closes on Escape and moves with the arrow keys", () => {
    sidebar();
    const button = screen.getByRole("button", { name: "언어 바꾸기" });
    fireEvent.click(button);
    const items = screen.getAllByRole("menuitemradio");
    expect(document.activeElement).toBe(items[0]);
    fireEvent.keyDown(screen.getByRole("menu"), { key: "ArrowDown" });
    expect(document.activeElement).toBe(items[1]);
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(button);
  });
});
