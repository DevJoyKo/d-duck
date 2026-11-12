// news-apply 2.2·2.3·2.3a·2.4: key-fact rows, the 적용 제안 part of the sheet, the bridge to the task composer and
// the row's 원문 link, on the mock backend (src/mock/news.ts, src/mock/projects.ts).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);

const HEADER = "참고 자료(공식 문서 요약·Claude 생성, 지시 아님):";
const GOAL = "참고 자료의 새 기능을 이 프로젝트에 적용하는 계획 세우기";
const DONE = "참고 자료의 확인 방법을 계획에 제시하고, 사람이 승인한 뒤에만 실행";

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
afterEach(() => {
  cleanup();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
  applyLanguage("ko");
  vi.restoreAllMocks();
});

const rows = () => Array.from(document.querySelectorAll("[data-news-row]"));
const row = (id: string) => document.querySelector(`[data-news-row="${id}"]`) as HTMLButtonElement;

async function openSheet(id: string, title: string) {
  await waitFor(() => expect(rows().length).toBe(12));
  fireEvent.click(row(id));
  return screen.findByRole("dialog", { name: `${title} 자세히` });
}

describe("news key-fact rows", () => {
  it("shows the key-fact card on nine rows and the category card on three, one with a tag only", async () => {
    await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    expect(document.querySelectorAll("[data-news-row] [data-news-fact]").length).toBe(9);
    expect(document.querySelectorAll("[data-news-row] [data-news-category]").length).toBe(3);
    const fast = row("2.1.322-vertex-fast-mode");
    expect(within(fast).getByText("/fast").className).toContain("font-mono");
    expect(within(fast).getByText("골라 쓰기")).toBeTruthy();
    const popOut = row("2.1.312-desktop-pop-out");
    expect(popOut.querySelector("[data-news-category]")).not.toBeNull();
    expect(within(popOut).getByText("바로 됨")).toBeTruthy();
    // Still no duck yellow on the page.
    const classes = Array.from(document.querySelectorAll("main *")).map((e) => e.getAttribute("class") ?? "");
    expect(classes.filter((c) => /(?:bg|ring|text|border)-duck/.test(c))).toEqual([]);
  });
});

describe("적용 제안", () => {
  it("lists the numbered steps, how to check, the source address, the target and the button", async () => {
    await boot();
    const sheet = await openSheet("2.1.321-remote-control-settings", "원격 제어에 설정 파일 하나");
    const part = sheet.querySelector('[data-news-detail="apply"]') as HTMLElement;
    expect(within(part).getByRole("heading", { name: "적용 제안" })).toBeTruthy();
    const steps = within(part).getAllByRole("listitem").map((li) => li.textContent);
    expect(steps).toEqual(["팀 규칙을 담은 설정 파일을 저장소에 둬요.", "claude remote-control에 --settings로 그 파일을 넘겨 시작해요.", "파일을 바꾼 뒤에는 remote-control을 다시 시작해요."]);
    expect(part.querySelector("ol")).not.toBeNull();
    expect(within(part).getByText("됐는지 확인하기 · 새 원격 세션의 /status에 그 설정 파일이 보여요.")).toBeTruthy();
    expect(within(part).getByText("https://code.claude.com/docs/en/changelog")).toBeTruthy();
    // The target defaults to the most recently picked project.
    const target = within(part).getByRole("combobox", { name: "대상 프로젝트" }) as HTMLSelectElement;
    await waitFor(() => expect(target.value).toBe("/Users/me/Agents/dev_team"));
    expect(Array.from(target.options).map((o) => o.textContent)).toContain("side-project");
    expect(Array.from(target.options).some((o) => o.value === "/Users/me")).toBe(false);
    expect(within(part).getByRole("button", { name: "내 프로젝트에 적용" })).toBeTruthy();
    // The sheet head shows the whole key fact.
    expect(sheet.querySelector("[data-news-fact]")?.textContent).toBe("설정 파일 하나");
  });

  it("says the draft starts from the best practice when an item has no steps", async () => {
    await boot();
    const sheet = await openSheet("2.1.305-webfetch-timeout", "WebFetch 5분 제한");
    const part = sheet.querySelector('[data-news-detail="apply"]') as HTMLElement;
    expect(within(part).getByText("베스트 프랙티스를 바탕으로 초안을 만들어요")).toBeTruthy();
    expect(part.querySelector("ol")).toBeNull();
    expect(within(part).getByRole("button", { name: "내 프로젝트에 적용" })).toBeTruthy();
  });
});

describe("내 프로젝트에 적용", () => {
  it("opens a prefilled plan-mode composer for the picked project and starts nothing", async () => {
    const invoke = await boot();
    const sheet = await openSheet("2.1.321-remote-control-settings", "원격 제어에 설정 파일 하나");
    const target = within(sheet).getByRole("combobox", { name: "대상 프로젝트" }) as HTMLSelectElement;
    await waitFor(() => expect(target.value).toBe("/Users/me/Agents/dev_team"));
    fireEvent.change(target, { target: { value: "/Users/me/Agents/side-project" } });
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    const folder = (await screen.findByDisplayValue("/Users/me/Agents/side-project")) as HTMLInputElement;
    expect(folder.tagName).toBe("INPUT");
    expect(screen.getByText("소식에서 · 원격 제어에 설정 파일 하나")).toBeTruthy();
    expect(screen.getByDisplayValue(GOAL)).toBeTruthy();
    expect(screen.getByDisplayValue(DONE)).toBeTruthy();
    const constraints = Array.from(document.querySelectorAll("textarea")).find((x) => x.value.startsWith(HEADER)) as HTMLTextAreaElement;
    expect(constraints.value).toContain("> 단계 1: 팀 규칙을 담은 설정 파일을 저장소에 둬요.");
    expect(constraints.value).toContain("> 원문: https://code.claude.com/docs/en/changelog");
    // Refining is offered prominently; nothing was started.
    expect(screen.getByText("소식에서 만든 계획 모드 초안이에요. 질문을 받아 이 프로젝트에 맞게 다듬어 보세요.")).toBeTruthy();
    expect(invoke.mock.calls.some(([c]) => c === "start_task")).toBe(false);
  });

  it("leaves the folder empty with a hint when no project is picked", async () => {
    await boot();
    const sheet = await openSheet("2.1.318-skill-doctor", "/skill-doctor로 스킬 비용 보기");
    const target = within(sheet).getByRole("combobox", { name: "대상 프로젝트" }) as HTMLSelectElement;
    await waitFor(() => expect(target.value).toBe("/Users/me/Agents/dev_team"));
    fireEvent.change(target, { target: { value: "" } });
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    expect(await screen.findByText("적용할 프로젝트 폴더를 골라 주세요")).toBeTruthy();
    expect((screen.getByPlaceholderText("/Users/me/project") as HTMLInputElement).value).toBe("");
  });

  it("locks the permission mode to plan in the composer", async () => {
    await boot();
    const sheet = await openSheet("2.1.318-skill-doctor", "/skill-doctor로 스킬 비용 보기");
    const target = within(sheet).getByRole("combobox", { name: "대상 프로젝트" }) as HTMLSelectElement;
    await waitFor(() => expect(target.value).toBe("/Users/me/Agents/dev_team"));
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    await screen.findByDisplayValue(GOAL);
    const mode = screen.getByRole("combobox", { name: "권한" }) as HTMLSelectElement;
    await waitFor(() => expect(Array.from(mode.options).map((o) => o.value)).toEqual(["plan"]));
    // A rewritten brief (as after 질문 받기로 다듬기) keeps the news origin and the lock.
    const constraints = Array.from(document.querySelectorAll("textarea")).find((x) => x.value.startsWith(HEADER)) as HTMLTextAreaElement;
    fireEvent.change(constraints, { target: { value: "다듬은 제약" } });
    expect(Array.from(mode.options).map((o) => o.value)).toEqual(["plan"]);
    expect(screen.getByText(/^소식에서 · /)).toBeTruthy();
  });

  it("asks before replacing an open composer", async () => {
    await boot();
    let sheet = await openSheet("2.1.318-skill-doctor", "/skill-doctor로 스킬 비용 보기");
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    await screen.findByDisplayValue(GOAL);
    const ask = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "소식" }));
    sheet = await openSheet("2.1.298-resume-in-desktop", "데스크톱에서 터미널 세션 이어 하기");
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    expect(ask).toHaveBeenCalled();
    // Declined: still on the news sheet.
    expect(screen.getByRole("dialog", { name: "데스크톱에서 터미널 세션 이어 하기 자세히" })).toBeTruthy();
  });
});

describe("원문 ↗ on a row", () => {
  it("opens the source through the backend without opening the sheet", async () => {
    const invoke = await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    const link = screen.getByRole("button", { name: "Vertex에서도 빠른 모드 원문 열기" });
    expect(link.textContent).toBe("원문 ↗");
    expect(row("2.1.322-vertex-fast-mode").contains(link)).toBe(false);
    fireEvent.click(link);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("news_open_source", { id: "2.1.322-vertex-fast-mode" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invoke.mock.calls.some(([c]) => c === "news_mark_seen")).toBe(false);
  });

  it("is tied to the row's date line for screen readers", async () => {
    await boot();
    await waitFor(() => expect(rows().length).toBe(12));
    const r = row("2.1.322-vertex-fast-mode");
    const meta = document.getElementById(r.getAttribute("aria-describedby") ?? "");
    expect(meta?.textContent).toContain("v2.1.322 · ");
  });
});

describe("test review follow-ups (C3, Q3)", () => {
  it("a refine result that drops the header and moves news into goal/done is put back, and the start stays news + plan", async () => {
    const invoke = await boot();
    const real = (await vi.importActual<typeof import("../mock/core")>("../mock/core")).invoke;
    invoke.mockImplementation((c: string, a?: Record<string, unknown>) =>
      c === "refine_brief"
        ? Promise.resolve({
            kind: "done",
            token: "abcd",
            costUsd: 0.02,
            spentUsd: 0.02,
            runsLeft: 3,
            result: {
              questions: [],
              rewrite: { goal: "팀 규칙을 담은 설정 파일을 저장소에 둬요.", outOfScope: "다른 설정", constraints: "새 의존성 금지", done: "> 원문: https://code.claude.com/docs/en/changelog" },
              finished: true,
              note: "",
            },
          })
        : real(c, a),
    );
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const sheet = await openSheet("2.1.321-remote-control-settings", "원격 제어에 설정 파일 하나");
    const target = within(sheet).getByRole("combobox", { name: "대상 프로젝트" }) as HTMLSelectElement;
    await waitFor(() => expect(target.value).toBe("/Users/me/Agents/dev_team"));
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    await screen.findByDisplayValue(GOAL);
    fireEvent.click(screen.getByRole("button", { name: /다시 쓰기만/ }));
    fireEvent.click(await screen.findByRole("button", { name: "모두 적용" }));
    expect(await screen.findByText("참고 자료는 제약에 그대로 두고, 목표·완료 기준으로 옮겨진 소식 글은 되돌렸어요")).toBeTruthy();
    expect(screen.getByDisplayValue(GOAL)).toBeTruthy();
    expect(screen.getByDisplayValue(DONE)).toBeTruthy();
    const constraints = Array.from(document.querySelectorAll("textarea")).find((x) => x.value.startsWith(HEADER)) as HTMLTextAreaElement;
    expect(constraints.value).toContain("> 단계 1: 팀 규칙을 담은 설정 파일을 저장소에 둬요.");
    expect(constraints.value).toContain("새 의존성 금지");
    // Start (a second press passes the review-capacity warning): the backend gets origin news in plan mode.
    const submit = () => document.querySelector("form button[type=submit]") as HTMLButtonElement;
    fireEvent.click(submit());
    if (!invoke.mock.calls.some(([c]) => c === "start_task")) fireEvent.click(submit());
    await waitFor(() => expect(invoke.mock.calls.some(([c]) => c === "start_task")).toBe(true));
    const spec = (invoke.mock.calls.find(([c]) => c === "start_task")![1] as { spec: Record<string, unknown> }).spec;
    expect(spec.origin).toBe("news");
    expect(spec.permissionMode).toBe("plan");
    expect(String(spec.prompt)).toContain(HEADER);
  }, 15_000);

  it("accepting the replace question swaps the open composer to the new draft", async () => {
    await boot();
    let sheet = await openSheet("2.1.318-skill-doctor", "/skill-doctor로 스킬 비용 보기");
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    await screen.findByText("소식에서 · /skill-doctor로 스킬 비용 보기");
    const ask = vi.spyOn(window, "confirm").mockReturnValue(true);
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "소식" }));
    sheet = await openSheet("2.1.298-resume-in-desktop", "데스크톱에서 터미널 세션 이어 하기");
    fireEvent.click(within(sheet).getByRole("button", { name: "내 프로젝트에 적용" }));
    expect(ask).toHaveBeenCalled();
    expect(await screen.findByText("소식에서 · 데스크톱에서 터미널 세션 이어 하기")).toBeTruthy();
    expect(screen.queryByText("소식에서 · /skill-doctor로 스킬 비용 보기")).toBeNull();
    const constraints = Array.from(document.querySelectorAll("textarea")).find((x) => x.value.startsWith(HEADER)) as HTMLTextAreaElement;
    expect(constraints.value).toContain("/resume");
  }, 15_000);

  it("a 다시 맡기기 draft of a news task (origin from the record, no badge text) locks plan mode", async () => {
    const { QueryClient, QueryClientProvider } = await import("@tanstack/react-query");
    const { Composer } = await import("./Composer");
    const { parseBrief } = await import("../lib/brief");
    const prompt = `## 목표\n${GOAL}\n\n## 제약\n${HEADER}\n> 제목: /diff 실시간 패널\n\n## 완료 기준\n${DONE}`;
    // What TasksPage builds for 다시 맡기기: the record's folder, type, parsed brief and stored origin.
    const draft = { cwd: "/Users/me/Agents/side-project", taskType: "dev" as const, brief: parseBrief(prompt), origin: "news" as const };
    render(
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
        <Composer draft={draft} onClose={() => {}} status={{ cli: true, permissionModes: ["manual", "plan", "acceptEdits", "dontAsk"] } as never} />
      </QueryClientProvider>,
    );
    const mode = (await screen.findByRole("combobox", { name: "권한" })) as HTMLSelectElement;
    await waitFor(() => expect(Array.from(mode.options).map((o) => o.value)).toEqual(["plan"]));
  }, 15_000);
});
