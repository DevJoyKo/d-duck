// review-inbox (D-140): the 리뷰 tab on the mock backend (src/mock/review.ts).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);

async function boot(search = ""): Promise<Invoke> {
  localStorage.setItem("dduck.page", "review");
  window.history.replaceState(null, "", `/${search}`);
  const core = await import("@tauri-apps/api/core");
  const { default: App } = await import("../App");
  render(<App />);
  return core.invoke as unknown as Invoke;
}

beforeEach(async () => {
  vi.resetModules();
  ((await import("@tauri-apps/api/core")).invoke as unknown as Invoke).mockClear();
});
afterEach(() => {
  cleanup();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
});

const groups = () => Array.from(document.querySelectorAll("[data-review-group]")).map((g) => g.getAttribute("data-review-group"));
const cards = () => Array.from(document.querySelectorAll("[data-review-card]")).map((c) => c.getAttribute("data-review-card"));

describe("리뷰 tab", () => {
  it("sits after 작업 with its own badge label, and groups 리뷰 전 cards by role in team order", async () => {
    await boot();
    const nav = screen.getByRole("navigation");
    expect(await within(nav).findByLabelText("리뷰 전 5개")).toBeTruthy();
    // task-lifecycle: the failed migrate-db has a report, so it waits for review instead of 확인 필요.
    expect(within(nav).getByLabelText("확인 필요 1개")).toBeTruthy();
    await waitFor(() => expect(groups()).toEqual(["dev", "design", "comms", "qa"]));
    // Starts on 모든 프로젝트; old, reviewed, running and waiting tasks are left out.
    expect(screen.getByRole("radio", { name: "모든 프로젝트" }).getAttribute("aria-checked")).toBe("true");
    expect(cards()).not.toContain("cc33dd44");
    expect(cards()).not.toContain("aa11bb22");
    expect(cards()).not.toContain("f3e5ff71");
    expect(cards()).not.toContain("ab12cd34");
    const lint = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
    expect(lint.textContent).toContain("린트 경고와 오류를 정리한다");
    expect(lint.textContent).toContain("합의와 다름 1 · 일부 충족 1 · 되돌리기 어려워요 1 → 확인이 필요해요");
    expect(lint.textContent).toContain("충족 3 · 일부 충족 1");
    expect(lint.textContent).toContain("공용 유틸 이름 변경");
    expect(document.body.textContent).not.toMatch(/검토|리뷰함/);
    // Primary once: only the first card in the queue; the rest use the normal button. No yellow (palette-refresh 2.2:
    // yellow means "waiting for you").
    const marks = screen.getAllByRole("button", { name: "리뷰했어요" });
    const primary = (b: HTMLElement) => b.className.includes("bg-ink text-paper");
    expect(marks.filter(primary).length).toBe(1);
    expect(primary(within(lint).getByRole("button", { name: "리뷰했어요" }))).toBe(true);
    expect(marks.some((b) => b.className.includes("bg-duck"))).toBe(false);
    // The QA group header counts every matching card.
    expect(document.querySelector('[data-review-group="qa"] [data-group-total]')?.textContent).toBe("2개");
  });

  it("전체 shows running, waiting, reviewed and older tasks; running ones have no mark button", async () => {
    await boot();
    await waitFor(() => expect(cards().length).toBe(5));
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    // 9 with a report + the finished task without one (D-196).
    await waitFor(() => expect(cards().length).toBe(10));
    const card = (id: string) => document.querySelector(`[data-review-card="${id}"]`) as HTMLElement;
    expect(card("f3e5ff71").textContent).toContain("답을 기다려요");
    expect(card("ab12cd34").textContent).toContain("일하는 중이에요");
    for (const id of ["f3e5ff71", "ab12cd34"]) expect(within(card(id)).queryByRole("button", { name: "리뷰했어요" })).toBeNull();
    expect(card("cc33dd44").textContent).toContain("보고서가 아직 없어요");
    expect(card("aa11bb22").textContent).toContain("리뷰했어요");
    expect(within(card("aa11bb22")).queryByRole("button", { name: "리뷰했어요" })).toBeNull();
    expect(within(card("aa11bb22")).getByRole("button", { name: "되돌리기" })).toBeTruthy();
    // A format error is a warning, not red.
    expect(card("77aa55cc").querySelector("[data-tone]")?.getAttribute("data-tone")).toBe("warn");
  });

  it("리뷰했어요 takes the card out, moves focus to the next one and lowers the badge; 되돌리기 brings it back", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards()[0]).toBe("9988aabb"));
    const first = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
    fireEvent.click(within(first).getByRole("button", { name: "리뷰했어요" }));
    await waitFor(() => expect(cards()).not.toContain("9988aabb"));
    expect(invoke).toHaveBeenCalledWith("inbox_mark_reviewed", { id: "9988aabb" });
    await waitFor(() => expect(within(screen.getByRole("navigation")).getByLabelText("리뷰 전 4개")).toBeTruthy());
    await waitFor(() => expect((document.activeElement as HTMLElement).closest("[data-review-card]")?.getAttribute("data-review-card")).toBe("4b4b4b4b"));
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    const back = await waitFor(() => {
      const c = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
      return within(c).getByRole("button", { name: "되돌리기" });
    });
    fireEvent.click(back);
    await waitFor(() => expect(within(screen.getByRole("navigation")).getByLabelText("리뷰 전 5개")).toBeTruthy());
  });

  it("marking shows a five-second undo that brings the card back", async () => {
    await boot();
    await waitFor(() => expect(cards()[0]).toBe("9988aabb"));
    fireEvent.click(within(document.querySelector('[data-review-card="9988aabb"]') as HTMLElement).getByRole("button", { name: "리뷰했어요" }));
    const toast = await screen.findByRole("status");
    expect(toast.textContent).toContain("리뷰했어요");
    fireEvent.click(within(toast).getByRole("button", { name: "되돌리기" }));
    await waitFor(() => expect(cards()).toContain("9988aabb"));
  });

  it("report-copy-save: the card is an article with selectable text; a body click opens, a selection does not", async () => {
    await boot();
    await waitFor(() => expect(cards()[0]).toBe("9988aabb"));
    const card = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
    const body = card.querySelector("article") as HTMLElement;
    expect(body).toBeTruthy();
    expect(body.closest("button")).toBeNull();
    const title = body.querySelector("[data-card-title]") as HTMLElement;
    expect(title.classList.contains("selectable")).toBe(true);
    // A drag that leaves a selection is not a click on the card.
    const sel = vi.spyOn(window, "getSelection").mockReturnValue({ toString: () => "린트" } as Selection);
    fireEvent.click(title);
    expect(screen.queryByRole("dialog")).toBeNull();
    sel.mockRestore();
    fireEvent.click(title);
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("the report dialog walks the queue: 리뷰했어요 · 다음 marks and opens the next card", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards()[0]).toBe("9988aabb"));
    fireEvent.click(within(document.querySelector('[data-review-card="9988aabb"]') as HTMLElement).getByRole("button", { name: /보고서 열기/ }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "리뷰했어요 · 다음" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("inbox_mark_reviewed", { id: "9988aabb" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("task_report", { id: "4b4b4b4b" }));
    expect(within(screen.getByRole("dialog")).getByRole("button", { name: "닫기" })).toBeTruthy();
    // Put the mock back for the next tests.
    await invoke("unmark_reviewed", { id: "9988aabb" });
  });

  it("a project that cannot be resolved shows the error, never every project", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards().length).toBe(5));
    const real = invoke.getMockImplementation() as (c: string, a?: Record<string, unknown>) => Promise<unknown>;
    invoke.mockImplementation((c: string, a?: Record<string, unknown>) => (c === "review_inbox" && a?.project ? Promise.reject("이 프로젝트 폴더를 찾지 못했어요") : real(c, a)));
    try {
      fireEvent.click(screen.getByRole("radio", { name: /dev_team/ }));
      const box = await screen.findByRole("combobox", { name: "프로젝트 찾기" });
      fireEvent.change(box, { target: { value: "dev_team" } });
      fireEvent.keyDown(box, { key: "Enter" });
      expect(await screen.findByText("리뷰 목록을 불러오지 못했어요")).toBeTruthy();
      expect(screen.getByText("이 프로젝트 폴더를 찾지 못했어요")).toBeTruthy();
      expect(cards()).toEqual([]);
    } finally {
      invoke.mockImplementation(real);
    }
  });

  it("cards preview up to three criteria with one line of evidence", async () => {
    await boot();
    await waitFor(() => expect(cards()).toContain("9988aabb"));
    const lint = document.querySelector('[data-review-card="9988aabb"] [data-preview]') as HTMLElement;
    expect(lint.textContent).toContain("eslint 오류 0개");
    expect(lint.textContent).toContain("npx eslint . → 0 errors");
    expect(lint.textContent).not.toContain("공개 API 그대로");
    expect(lint.textContent).toContain("+1개 더");
  });

  it("review-routine-runs: a routine run's report is a card in 기본, says 루틴, counts on the badge and can be marked", async () => {
    const invoke = await boot("?routine=1");
    const nav = screen.getByRole("navigation");
    expect(await within(nav).findByLabelText("리뷰 전 6개")).toBeTruthy();
    await waitFor(() => expect(cards()).toContain("7a7a7a7a"));
    const card = document.querySelector('[data-review-card="7a7a7a7a"]') as HTMLElement;
    expect(card.closest("[data-review-group]")?.getAttribute("data-review-group")).toBe("");
    expect(card.querySelector("[data-card-title]")?.textContent).toBe("아침 공고 확인");
    expect(card.textContent).toMatch(/루틴 · notices · /);
    fireEvent.click(within(card).getByRole("button", { name: /보고서 열기/ }));
    const dialog = await screen.findByRole("dialog");
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("task_report", { id: "7a7a7a7a" }));
    // The report screen's 맡긴 내용 carries the routine chip.
    await waitFor(() => expect(within(dialog).getAllByText("루틴").length).toBeGreaterThan(0));
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(within(card).getByRole("button", { name: "리뷰했어요" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("inbox_mark_reviewed", { id: "7a7a7a7a" }));
    await waitFor(() => expect(within(nav).getByLabelText("리뷰 전 5개")).toBeTruthy());
  });

  it("D-196: a finished task without a report is in 전체 only, says 보고서 없음 and opens the task, not a report", async () => {
    const invoke = await boot();
    const nav = screen.getByRole("navigation");
    expect(await within(nav).findByLabelText("리뷰 전 5개")).toBeTruthy();
    await waitFor(() => expect(cards().length).toBe(5));
    expect(cards()).not.toContain("ee0c0d22");
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    await waitFor(() => expect(cards()).toContain("ee0c0d22"));
    // The badge does not move.
    expect(within(nav).getByLabelText("리뷰 전 5개")).toBeTruthy();
    const card = document.querySelector('[data-review-card="ee0c0d22"]') as HTMLElement;
    expect(card.closest("[data-review-group]")?.getAttribute("data-review-group")).toBe("research");
    expect(card.querySelector("[data-card-title]")?.textContent).toBe("market-research");
    expect(card.querySelector("[data-no-report]")?.textContent).toBe("보고서 없음");
    expect(card.textContent).toContain("끝냈어요");
    expect(card.textContent).toMatch(/dev_team · /);
    expect(within(card).queryByRole("button", { name: "리뷰했어요" })).toBeNull();
    expect(within(card).queryByRole("button", { name: /보고서 열기/ })).toBeNull();
    // Not listed by the CLI any more: 작업 보기 lands on its 지난 작업 row, expanded.
    fireEvent.click(within(card).getByRole("button", { name: /작업 보기/ }));
    await waitFor(() => expect(document.querySelector('[data-record-row="ee0c0d22"]')).toBeTruthy());
    expect(invoke).not.toHaveBeenCalledWith("task_report", { id: "ee0c0d22" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("task_record", { id: "ee0c0d22" }));
    expect(window.location.hash).toBe("");
  });

  it("D-196: 작업 보기 on a task the CLI still lists opens its board drawer", async () => {
    const invoke = (await import("@tauri-apps/api/core")).invoke as unknown as Invoke;
    const real = invoke.getMockImplementation() as (c: string, a?: Record<string, unknown>) => Promise<unknown>;
    invoke.mockImplementation(async (c: string, a?: Record<string, unknown>) => {
      const v = await real(c, a);
      if (c !== "list_agents") return v;
      const p = v as { sessions: { id: string | null; key: string; status: string }[] };
      return { ...p, sessions: [...p.sessions, { ...p.sessions[0], key: "listed-key-9", id: "ee0c0d22", status: "done", name: "market-research" }] };
    });
    await boot();
    const opened: string[] = [];
    const on = (e: Event) => opened.push(String((e as CustomEvent).detail));
    window.addEventListener("dduck-open-task", on);
    try {
      fireEvent.click(await screen.findByRole("radio", { name: "전체" }));
      await waitFor(() => expect(cards()).toContain("ee0c0d22"));
      const card = document.querySelector('[data-review-card="ee0c0d22"]') as HTMLElement;
      await waitFor(() => {
        fireEvent.click(within(card).getByRole("button", { name: /작업 보기/ }));
        expect(opened).toContain("listed-key-9");
      });
      expect(window.location.hash).toBe("");
    } finally {
      window.removeEventListener("dduck-open-task", on);
      invoke.mockImplementation(real);
    }
  });

  it("D-196: with reports off and only tasks without a report, the filters and the settings hint stay", async () => {
    await boot("?report=off&review=plain");
    expect(await screen.findByText(/작업 보고서가 꺼져 있어요/)).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    await waitFor(() => expect(cards()).toEqual(["ee0c0d22"]));
  });

  it("a task card does not say 루틴", async () => {
    await boot();
    await waitFor(() => expect(cards()).toContain("9988aabb"));
    expect(document.querySelector('[data-review-card="9988aabb"]')?.textContent).not.toContain("루틴");
  });

  it("pages hold ten cards like the 아티팩트 tab; a new filter goes back to page 1", async () => {
    const invoke = await boot("?review=many");
    await waitFor(() => expect(cards().length).toBe(10));
    await waitFor(() => expect(within(screen.getAllByRole("navigation")[0]).getByLabelText("리뷰 전 20개")).toBeTruthy());
    const pager = screen.getByRole("navigation", { name: "페이지" });
    fireEvent.click(within(pager).getByRole("button", { name: "2쪽" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("review_inbox", { project: null, filter: "pending", offset: 10, limit: 10 }));
    await waitFor(() => expect(within(screen.getByRole("navigation", { name: "페이지" })).getByRole("button", { name: "2쪽" }).getAttribute("aria-current")).toBe("page"));
    expect(cards().length).toBe(10);
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("review_inbox", { project: null, filter: "all", offset: 0, limit: 10 }));
  });

  it("리뷰했어요 · 다음 on the last card of a page goes on to the next page's card", async () => {
    const invoke = await boot("?review=many");
    await waitFor(() => expect(cards().length).toBe(10));
    const firstPage = cards();
    const lastId = firstPage[firstPage.length - 1];
    const last = document.querySelector(`[data-review-card="${lastId}"]`) as HTMLElement;
    // Cards without a goal lead with their start time.
    const untitled = Array.from(document.querySelectorAll("[data-review-card]")).find((c) => c.textContent?.includes("목표를 남기지 않은 작업이에요"));
    expect(untitled?.textContent).toMatch(/^\d{2}\. \d{2}\..*목표를 남기지 않은 작업이에요/);
    fireEvent.click(within(last).getByRole("button", { name: /보고서 열기/ }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "리뷰했어요 · 다음" }));
    await waitFor(() => {
      const opened = invoke.mock.calls.filter(([c]) => c === "task_report").map(([, a]) => (a as { id: string }).id);
      expect(opened.some((id) => !firstPage.includes(id))).toBe(true);
    });
    expect(within(screen.getByRole("dialog")).getByRole("status").textContent).toContain("앞 작업 리뷰했어요");
    await invoke("unmark_reviewed", { id: lastId });
  });

  it("in 전체, 리뷰했어요 · 다음 reads the list again and opens the next card after the marked one", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards().length).toBe(5));
    fireEvent.click(screen.getByRole("radio", { name: "전체" }));
    // 9 with a report + the finished task without one (D-196).
    await waitFor(() => expect(cards().length).toBe(10));
    fireEvent.click(within(document.querySelector('[data-review-card="9988aabb"]') as HTMLElement).getByRole("button", { name: /보고서 열기/ }));
    const dialog = await screen.findByRole("dialog");
    const listCalls = () => invoke.mock.calls.filter(([c]) => c === "review_inbox").length;
    const before = listCalls();
    fireEvent.click(within(dialog).getByRole("button", { name: "리뷰했어요 · 다음" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("task_report", { id: "4b4b4b4b" }));
    expect(listCalls()).toBeGreaterThan(before);
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "닫기" }));
    await waitFor(() => expect(within(document.querySelector('[data-review-card="9988aabb"]') as HTMLElement).getByRole("button", { name: "되돌리기" })).toBeTruthy());
    expect((document.querySelector('[data-review-card="9988aabb"]') as HTMLElement).textContent).toContain("리뷰했어요");
    await invoke("unmark_reviewed", { id: "9988aabb" });
  });

  it("before a good poll, cards say 실행 여부 확인 중, cannot be marked, and the badge hides", async () => {
    await boot("?poll=none");
    await waitFor(() => expect(cards().length).toBeGreaterThan(0));
    const lint = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
    expect(lint.textContent).toContain("실행 여부 확인 중");
    expect(within(lint).queryByRole("button", { name: "리뷰했어요" })).toBeNull();
    expect(within(screen.getAllByRole("navigation")[0]).queryByLabelText(/리뷰 전/)).toBeNull();
  });

  it("opening a card shows the full report screen", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards()).toContain("5a5a5a5a"));
    const qa = document.querySelector('[data-review-card="5a5a5a5a"]') as HTMLElement;
    fireEvent.click(within(qa).getByRole("button", { name: /보고서 열기/ }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("task_report", { id: "5a5a5a5a" }));
  });

  it("picking a project narrows the list and says how many are elsewhere", async () => {
    const invoke = await boot();
    await waitFor(() => expect(cards().length).toBe(5));
    // The mock's most recent project is dev_team; the project side opens the picker, picking it narrows the list.
    fireEvent.click(screen.getByRole("radio", { name: /dev_team/ }));
    const box = await screen.findByRole("combobox", { name: "프로젝트 찾기" });
    fireEvent.change(box, { target: { value: "dev_team" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("review_inbox", { project: "1f3a5c7e9b2d4f60", filter: "pending", offset: 0, limit: 10 }));
    await waitFor(() => expect(cards().sort()).toEqual(["5a5a5a5a", "66bb44dd", "9988aabb"]));
    expect(screen.getByText("다른 프로젝트에 2개 더")).toBeTruthy();
    expect(within(screen.getByRole("navigation")).getByLabelText("리뷰 전 5개")).toBeTruthy();
  });
});

describe("빈 화면", () => {
  it("no records with the report setting off: one line and a way to settings, no scope bar or filters", async () => {
    await boot("?review=empty&report=off");
    expect(await screen.findByText("보고서를 켜면 오리들이 결과를 모아 와요")).toBeTruthy();
    expect(screen.queryByRole("radio", { name: "모든 프로젝트" })).toBeNull();
    expect(screen.queryByRole("radio", { name: "리뷰 전" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "설정 열기" }));
    await waitFor(() => expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("설정"));
  });

  it("everything reviewed while some still run: says how many are working", async () => {
    await boot("?review=done");
    expect(await screen.findByText("모두 리뷰했어요 · 일하는 중 2개")).toBeTruthy();
  });

  it("no records: says there is nothing yet", async () => {
    await boot("?review=empty");
    expect(await screen.findByText("아직 보고서가 있는 작업이 없어요")).toBeTruthy();
  });
});
