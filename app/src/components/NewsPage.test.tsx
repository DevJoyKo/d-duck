// news-tab (D-204): the 소식 tab on the mock backend (src/mock/news.ts).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { applyLanguage } from "../i18n/language";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);

async function boot(search = ""): Promise<Invoke> {
  localStorage.setItem("dduck.page", "news");
  window.history.replaceState(null, "", `/${search}`);
  const core = await import("@tauri-apps/api/core");
  await (core.invoke as unknown as Invoke)("mock_news_reset");
  (core.invoke as unknown as Invoke).mockClear();
  const { default: App } = await import("../App");
  render(<App />);
  return core.invoke as unknown as Invoke;
}

beforeEach(() => vi.resetModules());
afterEach(async () => {
  cleanup();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
  applyLanguage("ko");
  (await import("../i18n/language")).applyLanguage("ko");
});

const rows = () => Array.from(document.querySelectorAll("[data-news-row]")).map((r) => r.getAttribute("data-news-row"));
const section = (name: string) => document.querySelector(`[data-news-section="${name}"]`) as HTMLElement | null;

describe("소식 tab", () => {
  it("sits above 사용 통계 in the sidebar", async () => {
    await boot();
    const nav = screen.getByRole("navigation");
    const news = within(nav).getByRole("button", { name: "소식" });
    const usage = within(nav).getByRole("button", { name: "사용 통계" });
    expect(news.getAttribute("aria-current")).toBe("page");
    expect(news.compareDocumentPosition(usage) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("lists newest first, with older items under 이미 나온 기능 and new dots on unseen rows", async () => {
    await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    // Same date: the numeric version decides (2.1.321 fable before remote-control by id).
    expect(rows().slice(0, 3)).toEqual(["2.1.322-vertex-fast-mode", "2.1.321-fable-5-2", "2.1.321-remote-control-settings"]);
    // Each row is the row button plus its own "원문 ↗" button (news-apply 2.3a).
    const recent = Array.from(section("새로 나온 기능")!.querySelectorAll("[data-news-row]"));
    const earlier = Array.from(section("이미 나온 기능")!.querySelectorAll("[data-news-row]"));
    expect(recent.length).toBe(8);
    expect(earlier.map((b) => b.getAttribute("data-news-row"))).toEqual(["2.1.298-resume-in-desktop", "2.1.296-restricted-flag", "2.1.292-model-picker-setting", "2.1.288-agent-feedback"]);
    expect(document.querySelectorAll("[data-news-new]").length).toBe(7);
    const first = document.querySelector('[data-news-row="2.1.322-vertex-fast-mode"]')!.closest("li") as HTMLElement;
    expect(first.textContent).toContain("Vertex에서도 빠른 모드");
    expect(first.textContent).toContain("v2.1.322 · ");
    expect(first.textContent).toContain("새 소식");
    expect(screen.getByText("소식 12개")).toBeTruthy();
    // No yellow on this page (yellow means "waiting for you").
    const classes = Array.from(document.querySelectorAll("main *")).map((e) => e.getAttribute("class") ?? "");
    expect(classes.filter((c) => /(?:bg|ring|text|border)-duck/.test(c))).toEqual([]);
  });

  it("opens a detail sheet with the three parts, marks it seen and returns focus on Esc", async () => {
    const invoke = await boot();
    const row = (await screen.findByText("Vertex에서도 빠른 모드")).closest("button") as HTMLButtonElement;
    fireEvent.click(row);
    const sheet = await screen.findByRole("dialog", { name: "Vertex에서도 빠른 모드 자세히" });
    for (const h of ["무엇인지", "언제 쓰면 좋은지", "베스트 프랙티스"]) expect(within(sheet).getByRole("heading", { name: h })).toBeTruthy();
    expect(within(sheet).getByText(/Vertex가 거절하면 보통 속도로/)).toBeTruthy();
    expect(within(sheet).getByText("Claude가 공식 문서를 요약했어요 · 원문에서 확인해요")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("news_mark_seen", { id: "2.1.322-vertex-fast-mode" });
    fireEvent.click(within(sheet).getByRole("button", { name: /원문 열기/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("news_open_source", { id: "2.1.322-vertex-fast-mode" }));
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement?.getAttribute("data-news-row")).toBe("2.1.322-vertex-fast-mode"));
    await waitFor(() => expect(document.querySelector('[data-news-row="2.1.322-vertex-fast-mode"] [data-news-new]')).toBeNull());
  });

  it("shows an empty state with Fetch, and fetching fills the list and the status line", async () => {
    const invoke = await boot("?news=empty");
    const empty = await waitFor(() => {
      const e = document.querySelector("[data-news-empty]") as HTMLElement | null;
      if (!e) throw new Error("no empty state yet");
      return e;
    });
    expect(empty.textContent).toContain("아직 소식이 없어요");
    expect(empty.textContent).toContain("1회 약 $0.10–0.30");
    expect(screen.getByText("아직 가져온 적이 없어요")).toBeTruthy();
    fireEvent.click(within(empty).getByRole("button", { name: "가져오기" }));
    await waitFor(() => expect(rows().length).toBe(13));
    expect(invoke).toHaveBeenCalledWith("news_fetch");
    expect(rows()[0]).toBe("2.1.323-diff-panel");
    expect(document.querySelector("[data-news-status]")?.textContent).toMatch(/마지막 가져오기 .* · 추가 13개 · \$0\.17/);
  });

  it("changes nothing when the confirmation is cancelled", async () => {
    const invoke = await boot("?news=decline");
    await waitFor(() => expect(rows().length).toBe(12));
    const before = document.querySelector("[data-news-status]")?.textContent;
    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("news_fetch"));
    await waitFor(() => expect(document.querySelector("[data-news-status]")?.textContent).toBe(before));
    expect(rows().length).toBe(12);
    expect(document.querySelector("[data-news-status]")?.textContent).toBe(before);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps old items and shows the reason when a fetch fails", async () => {
    await boot("?news=fail");
    await waitFor(() => expect(rows().length).toBe(12));
    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "예산 상한($0.40)에 닿아 멈췄어요 · 쓴 비용 $0.41");
    expect(rows().length).toBe(12);
    await waitFor(() => expect(document.querySelector("[data-news-status]")?.textContent).toMatch(/가져오지 못했어요 · 예산 상한/));
    // One red place: the alert. The status line stays neutral while the alert is up.
    expect(document.querySelector("[data-news-status] .text-danger")).toBeNull();
  });

  it("shows the login line instead of a confirmation when logged out", async () => {
    const invoke = await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    // The shell's login gate answers before any dialog (claude-login).
    const real = (await vi.importActual<typeof import("../mock/core")>("../mock/core")).invoke;
    invoke.mockImplementation((c: string, a?: Record<string, unknown>) => (c === "news_fetch" ? Promise.reject({ kind: "login_required" }) : real(c, a)));
    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    expect(await screen.findAllByTestId("gate-line")).not.toHaveLength(0);
    expect(rows().length).toBe(12);
    invoke.mockImplementation(real);
  });

  it("refuses a second fetch while one is running", async () => {
    await boot("?news=busy");
    await waitFor(() => expect(rows().length).toBe(12));
    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "이미 소식을 가져오는 중이에요");
  });

  it("can cancel a running fetch", async () => {
    const invoke = await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    fireEvent.click(await screen.findByRole("button", { name: "취소" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("news_cancel"));
    await waitFor(() => expect(document.querySelector("[data-news-status]")?.textContent).toMatch(/가져오기를 취소했어요$/));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(rows().length).toBe(12);
  });

  it("turns the daily fetch on through the backend and shows the next time", async () => {
    const invoke = await boot();
    const sw = await screen.findByRole("switch", { name: "매일 가져오기" });
    expect(sw.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByText("하루 한 번, 회당 약 $0.40까지")).toBeTruthy();
    fireEvent.click(sw);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("news_set_daily", { enabled: true }));
    await waitFor(() => expect(screen.getByRole("switch", { name: "매일 가져오기" }).getAttribute("aria-checked")).toBe("true"));
    expect(screen.getByText(/^다음 가져오기 /)).toBeTruthy();
    fireEvent.click(screen.getByRole("switch", { name: "매일 가져오기" }));
    await waitFor(() => expect(screen.getByRole("switch", { name: "매일 가져오기" }).getAttribute("aria-checked")).toBe("false"));
  });

  it("shows English items in English and points at the other language when there are none", async () => {
    window.history.replaceState(null, "", "/?news=other");
    const core = await import("@tauri-apps/api/core");
    const invoke = core.invoke as unknown as Invoke;
    await invoke("set_language", { choice: "en" });
    await invoke("mock_news_reset");
    // Modules were reset: switch the language in the copy NewsPage will import.
    (await import("../i18n/language")).applyLanguage("en");
    const { NewsPage } = await import("./NewsPage");
    render(
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
        <NewsPage />
      </QueryClientProvider>,
    );
    const empty = await waitFor(() => {
      const e = document.querySelector("[data-news-empty]") as HTMLElement | null;
      if (!e) throw new Error("no empty state yet");
      return e;
    });
    expect(empty.textContent).toContain("No news yet");
    expect(empty.textContent).toContain("You have 12 items in another language");
    expect(within(empty).getByRole("button", { name: "Fetch" })).toBeTruthy();
    // Fetching in English stores English items.
    fireEvent.click(within(empty).getByRole("button", { name: "Fetch" }));
    expect(await screen.findByText("Fast mode on Vertex")).toBeTruthy();
    expect(screen.getByText("Earlier")).toBeTruthy();
    await invoke("set_language", { choice: "ko" });
  });

  it("Windows preview: 가져오기 is Mac only, saved items still show, and the backend refuses a fetch", async () => {
    localStorage.setItem("dduck.page", "news");
    window.history.replaceState(null, "", "/?os=win");
    const core = await import("@tauri-apps/api/core");
    const invoke = core.invoke as unknown as Invoke;
    await invoke("mock_news_reset");
    await (await import("../lib/platform")).loadPlatform();
    const { default: App } = await import("../App");
    render(<App />);
    await waitFor(() => expect(rows().length).toBe(12));
    const fetch = screen.getByRole("button", { name: /가져오기/ }) as HTMLButtonElement;
    expect(fetch.disabled).toBe(true);
    expect(screen.getByText("새 소식 가져오기는 Windows 미리보기에서 아직 쓸 수 없어요 · 저장된 소식은 볼 수 있어요")).toBeTruthy();
    await expect(invoke("news_fetch")).rejects.toMatchObject({ kind: "invalidInput", code: "macOnly" });
  });
});
