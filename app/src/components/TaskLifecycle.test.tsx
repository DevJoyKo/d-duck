// task-lifecycle (D-149 (7)·D-152): the 작업 board's 리뷰 대기·정리 columns on the mock backend (src/mock/lifecycle.ts).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));
// The detail drawer's terminal view needs a real browser (matchMedia); this file tests the lifecycle around it.
vi.mock("./LogView", () => ({ LogView: () => null }));

async function boot(search = "") {
  localStorage.setItem("dduck.page", "tasks");
  window.history.replaceState(null, "", `/${search}`);
  const { default: App } = await import("../App");
  render(<App />);
}

beforeEach(() => {
  vi.resetModules();
});
afterEach(() => {
  cleanup();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
});

const column = (label: string) => screen.getByRole("region", { name: label });
const total = (label: string) => column(label).querySelector("[data-column-total]")?.textContent;

// The mock keeps its marks across tests in this file (as in ReviewPage.test.tsx): tests that read counts run first.
describe("작업 board lifecycle", () => {
  it("has 대기 중 → 진행 중 → 확인 필요 → 리뷰 대기 → 정리 and no 완료 column", async () => {
    await boot();
    await screen.findByRole("region", { name: "리뷰 대기" });
    const names = Array.from(document.querySelectorAll("section[id^='column-']")).map((s) => s.getAttribute("aria-label"));
    expect(names).toEqual(["대기 중", "진행 중", "확인 필요", "리뷰 대기", "정리"]);
    expect(within(column("리뷰 대기")).getByText("오리가 끝냈고 리뷰를 기다려요")).toBeTruthy();
    expect(within(column("정리")).getByText("리뷰한 작업의 마무리를 확인해요")).toBeTruthy();
  });

  it("리뷰 대기 holds the review badge's rows, failed and stopped report tasks included, and never says 완료", async () => {
    await boot();
    const nav = screen.getByRole("navigation");
    await within(nav).findByLabelText("리뷰 전 5개");
    await waitFor(() => expect(total("리뷰 대기")).toBe("5"));
    const rw = column("리뷰 대기");
    // The column names the lane, so cards say only how the task ended (3.2 review: the path kept its width at 1440).
    const migrate = within(rw).getByText("migrate-db").closest("li") as HTMLElement;
    expect(within(migrate).getByText("실패")).toBeTruthy();
    expect(within(rw).getByText("멈췄어요")).toBeTruthy();
    expect(rw.textContent).not.toContain("· 리뷰 대기");
    expect(within(rw).getByText("migrate-db")).toBeTruthy();
    expect(within(column("확인 필요")).queryByText("migrate-db")).toBeNull();
    // Records the CLI no longer lists still show; tasks from before review_since do not.
    expect(within(rw).getByText("settings-layout")).toBeTruthy();
    expect(document.querySelector("[data-board-record='cc33dd44']")).toBeNull();
    expect(within(nav).getByLabelText("확인 필요 1개")).toBeTruthy();
    const board = document.querySelector("section[id='column-reviewWait']")?.parentElement as HTMLElement;
    expect(board.textContent).not.toMatch(/완료(?! 기준)/);
    // A finished dev task in 리뷰 대기 can be handed to QA and review.
    const lint = within(rw).getByText("lint-sweep").closest("li") as HTMLElement;
    expect(within(lint).getByRole("button", { name: "QA 맡기기" })).toBeTruthy();
    expect(within(lint).getByRole("button", { name: "리뷰 맡기기" })).toBeTruthy();
  });

  it("with no live session the board still shows 리뷰 대기 and 정리", async () => {
    await boot("?board=nolive");
    await waitFor(() => expect(total("리뷰 대기")).toBe("4"));
    expect(screen.queryByText("실행 중인 Claude 세션이 없어요")).toBeNull();
    expect(within(column("리뷰 대기")).getByText("migrate-db")).toBeTruthy();
    expect(within(column("정리")).getByText("payment-refactor")).toBeTruthy();
  });

  it("record cards keep how the task ended: a failed task that left the list stays 실패, a reviewed failure stays 실패", async () => {
    await boot();
    await waitFor(() => expect(total("리뷰 대기")).toBe("5"));
    const line = (id: string) => (document.querySelector(`[data-board-record='${id}']`) as HTMLElement).querySelector("[id$='-line']")?.textContent;
    expect(line("4b4b4b4b")).toBe("끝냈어요");
    expect(line("5a5a5a5a")).toBe("실패");
    expect(within(column("리뷰 대기")).getByText("qa-lint-sweep")).toBeTruthy();
    await waitFor(() => expect(document.querySelector("[data-board-record='aa11bb22']")).toBeTruthy());
    expect(line("aa11bb22")).toBe("실패 · 리뷰했어요");
  });

  it("a record never seen ended says 목록에서 사라진 작업, not 끝냈어요", async () => {
    await boot("?ended=none");
    await waitFor(() => expect(document.querySelector("[data-board-record='5a5a5a5a']")).toBeTruthy());
    const line = (id: string) => (document.querySelector(`[data-board-record='${id}']`) as HTMLElement).querySelector("[id$='-line']")?.textContent;
    expect(line("5a5a5a5a")).toBe("목록에서 사라진 작업");
    expect(line("4b4b4b4b")).toBe("목록에서 사라진 작업");
  });

  it("the usage-limit banner counts planned resumes (limit_resumes), not live sessions or board lanes", async () => {
    await boot("?board=nolive");
    await waitFor(() => expect(total("리뷰 대기")).toBe("4"));
    // No live session, yet the planned resumes still show; the 리뷰 대기 count is not mixed with them.
    expect(await screen.findByText(/사용량 한도 · .* · 2개는 이어서 할게요 · 1개는 직접 이어야 해요/)).toBeTruthy();
    cleanup();
    vi.resetModules();
    await boot("?board=nolive&limits=none");
    await waitFor(() => expect(total("리뷰 대기")).toBe("4"));
    expect(document.body.textContent).not.toContain("사용량 한도");
  });

  it("끝내고 넘기기 on a task with an unreviewed report sends it to 리뷰 대기 and says so", async () => {
    await boot();
    fireEvent.click(await within(await screen.findByRole("region", { name: "확인 필요" })).findByText("flaky-test-fix"));
    const drawer = await waitFor(() => {
      const el = document.querySelector("aside") as HTMLElement | null;
      expect(el).toBeTruthy();
      return el as HTMLElement;
    });
    fireEvent.click(within(drawer).getByRole("button", { name: "끝내고 넘기기" }));
    fireEvent.click(within(drawer).getByRole("button", { name: "끝내고 넘기기" }));
    expect(await within(drawer).findByText("정상 종료하고 리뷰 대기로 옮겼어요. 리뷰하면 정리 칸으로 가요.")).toBeTruthy();
  });

  it("리뷰했어요 from the report moves the card to 정리 and lowers the badge and the column together", async () => {
    await boot();
    const nav = screen.getByRole("navigation");
    await within(nav).findByLabelText("리뷰 전 5개");
    await waitFor(() => expect(total("리뷰 대기")).toBe("5"));
    fireEvent.click(within(column("리뷰 대기")).getByText("settings-layout"));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("리뷰 대기")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "리뷰했다고 표시" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(total("리뷰 대기")).toBe("4"));
    await within(nav).findByLabelText("리뷰 전 4개");
    expect(within(column("정리")).getByText("settings-layout")).toBeTruthy();
  });

  it("an opened 정리 card shows local facts; worktree 정리 is off with uncommitted changes", async () => {
    await boot("?board=demo&limits=none");
    await waitFor(() => expect(within(column("정리")).getByText("lint-sweep")).toBeTruthy());
    const wu = column("정리");
    expect(within(wu).getAllByText("리뷰했어요").length).toBeGreaterThan(0);
    expect(within(wu).getByText("리뷰 없음")).toBeTruthy();
    expect(wu.textContent).not.toContain("정리 남음");
    fireEvent.click(within(wu).getByText("lint-sweep"));
    expect(await within(wu).findByText("worktree가 남아 있어요 · lint-sweep · dduck/lint-sweep")).toBeTruthy();
    expect(within(wu).getByText("커밋 안 된 변경 없어요")).toBeTruthy();
    expect(within(wu).getByText(/보고서에 적힌 PR · #42/)).toBeTruthy();
    expect(within(wu).getByText("로컬만")).toBeTruthy();
    expect((within(wu).getByRole("button", { name: "worktree 정리" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(within(wu).getByText("payment-refactor"));
    expect(await within(wu).findByText("커밋 안 된 변경 2개")).toBeTruthy();
    expect(within(wu).getByText("커밋 안 된 변경이 있어 worktree를 지울 수 없어요")).toBeTruthy();
    expect((within(wu).getByRole("button", { name: "worktree 정리" }) as HTMLButtonElement).disabled).toBe(true);
    // Only the worktree is blocked: 정리 끝 stays on.
    const pay = within(wu).getByText("payment-refactor").closest("li") as HTMLElement;
    expect((within(pay).getByRole("button", { name: "정리 끝" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("a planned usage-limit resume blocks 정리 끝 and worktree 정리 with one line", async () => {
    await boot("?board=demo");
    await waitFor(() => expect(within(column("정리")).getByText("lint-sweep")).toBeTruthy());
    const wu = column("정리");
    fireEvent.click(within(wu).getByText("lint-sweep"));
    expect(await within(wu).findByText("한도 뒤 이어서 하기가 예약돼 있어 정리할 수 없어요")).toBeTruthy();
    expect((within(wu).getByRole("button", { name: "정리 끝" }) as HTMLButtonElement).disabled).toBe(true);
    expect((within(wu).getByRole("button", { name: "worktree 정리" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("a worktree it cannot locate says so and offers no worktree 정리", async () => {
    await boot("?check=unknown");
    await waitFor(() => expect(within(column("정리")).getByText("payment-refactor")).toBeTruthy());
    const wu = column("정리");
    fireEvent.click(within(wu).getByText("payment-refactor"));
    expect(await within(wu).findByText("worktree 위치를 확인하지 못했어요")).toBeTruthy();
    expect(within(wu).queryByRole("button", { name: "worktree 정리" })).toBeNull();
    expect(within(wu).getByRole("button", { name: "정리 끝" })).toBeTruthy();
  });

  it("changes it could not count block worktree 정리 with the not-checked line", async () => {
    await boot("?check=notchecked");
    await waitFor(() => expect(within(column("정리")).getByText("payment-refactor")).toBeTruthy());
    const wu = column("정리");
    fireEvent.click(within(wu).getByText("payment-refactor"));
    expect(await within(wu).findByText("변경을 확인하지 못해 worktree를 지울 수 없어요")).toBeTruthy();
    expect((within(wu).getByRole("button", { name: "worktree 정리" }) as HTMLButtonElement).disabled).toBe(true);
    expect(wu.textContent).not.toContain("worktree를 지우지 못했어요");
  });


  it("리뷰했다고 표시 only on ended tasks, and a reviewed card says 리뷰했어요 once", async () => {
    await boot("?board=demo&limits=none");
    await waitFor(() => expect(within(column("정리")).getByText("lint-sweep")).toBeTruthy());
    for (const label of ["진행 중", "확인 필요", "대기 중"]) expect(within(column(label)).queryByRole("button", { name: "리뷰했다고 표시" })).toBeNull();
    const lint = within(column("정리")).getByText("lint-sweep").closest("li") as HTMLElement;
    expect(within(lint).getAllByText("리뷰했어요")).toHaveLength(1);
  });

  it("정리 shows 40 record cards, then 더 보기 shows the rest (45 records)", async () => {
    await boot("?wrapup=many");
    await waitFor(() => expect(within(column("정리")).getByText("cleanup-1")).toBeTruthy());
    const records = () => column("정리").querySelectorAll("[data-board-record]").length;
    expect(records()).toBe(40);
    fireEvent.click(within(column("정리")).getByRole("button", { name: "더 보기 (5개 남음)" }));
    await waitFor(() => expect(records()).toBe(45));
    expect(within(column("정리")).queryByRole("button", { name: /더 보기/ })).toBeNull();
  });


  it("a second 정리 끝 restarts the 5-second undo, which then closes on time", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      await boot("?board=demo&limits=none");
      await waitFor(() => expect(within(column("정리")).getByText("payment-refactor")).toBeTruthy());
      fireEvent.click(within(column("정리")).getByText("payment-refactor"));
      fireEvent.click(await within(column("정리")).findByRole("button", { name: "정리 끝" }));
      await waitFor(() => expect(document.querySelector("[data-undo-bar]")).toBeTruthy());
      await vi.advanceTimersByTimeAsync(3_000);
      fireEvent.click(within(column("정리")).getByText("lint-sweep"));
      fireEvent.click(await within(column("정리")).findByRole("button", { name: "정리 끝" }));
      await waitFor(() => expect(within(column("정리")).queryByText("lint-sweep")).toBeNull());
      // 3 s into the second bar (6 s after the first): still open.
      await vi.advanceTimersByTimeAsync(3_000);
      expect(document.querySelector("[data-undo-bar]")).toBeTruthy();
      await vi.advanceTimersByTimeAsync(2_100);
      expect(document.querySelector("[data-undo-bar]")).toBeNull();
    } finally {
      vi.useRealTimers();
      const { invoke } = await import("@tauri-apps/api/core");
      for (const id of ["aa11bb22", "9988aabb"]) await invoke("unwrap_task", { id });
    }
  });

  it("정리 끝 takes the card off with a 5-second undo, and the review tab shows 완료", async () => {
    await boot("?board=demo&limits=none");
    await waitFor(() => expect(within(column("정리")).getByText("lint-sweep")).toBeTruthy());
    fireEvent.click(within(column("정리")).getByText("lint-sweep"));
    fireEvent.click(await within(column("정리")).findByRole("button", { name: "정리 끝" }));
    const bar = await waitFor(() => {
      const el = document.querySelector("[data-undo-bar]") as HTMLElement | null;
      expect(el?.textContent).toContain("정리 끝");
      return el as HTMLElement;
    });
    expect(within(bar).getByRole("button", { name: "되돌리기" })).toBeTruthy();
    await waitFor(() => expect(within(column("정리")).queryByText("lint-sweep")).toBeNull());
    fireEvent.click(within(bar).getByRole("button", { name: "되돌리기" }));
    await waitFor(() => expect(within(column("정리")).getByText("lint-sweep")).toBeTruthy());
    // Wrapped up for good: the review tab's 전체 shows it with the 완료 chip.
    fireEvent.click(within(column("정리")).getByText("lint-sweep"));
    fireEvent.click(await within(column("정리")).findByRole("button", { name: "정리 끝" }));
    await waitFor(() => expect(within(column("정리")).queryByText("lint-sweep")).toBeNull());
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: /^리뷰/ }));
    fireEvent.click(await screen.findByRole("radio", { name: "전체" }));
    const card = await waitFor(() => {
      const el = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement | null;
      expect(el).toBeTruthy();
      return el as HTMLElement;
    });
    expect(await within(card).findByText("완료")).toBeTruthy();
    expect(within(card).getByRole("button", { name: "되돌리기" })).toBeTruthy();
  });

});
