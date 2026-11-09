import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TaskReportView } from "../lib/api";
import { mockTaskReport } from "../mock/report";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return { ...real, api: { ...real.api, teamConfig: vi.fn(async () => null), taskReport: vi.fn(async (id: string) => mockTaskReport(id)) } };
});
const { ReportBody, TaskReportDialog } = await import("./TaskReport");

// No retries and no background refetch: each test sees exactly one settled query.
const client = () => new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } });

const show = (id: string, session: Parameters<typeof ReportBody>[0]["session"], onRetry?: Parameters<typeof ReportBody>[0]["onRetry"]) =>
  render(
    <QueryClientProvider client={client()}>
      <ReportBody view={mockTaskReport(id) as TaskReportView} session={session} onRetry={onRetry} />
    </QueryClientProvider>,
  );

const chips = () => Array.from(document.querySelector('section[aria-label="주의"]')!.querySelectorAll("li > button[aria-label]")).map((b) => b.getAttribute("aria-label"));

describe("ReportBody", () => {
  afterEach(cleanup);
  it("shows the agreed brief first, then criteria, attention, flow and tables in order", () => {
    show("9988aabb", "done");
    const order = Array.from(document.querySelectorAll("section[aria-label]")).map((s) => s.getAttribute("aria-label"));
    expect(order).toEqual(["맡긴 내용", "완료 기준 대조", "주의", "흐름", "기록"]);
    expect(screen.getByText("린트 경고와 오류를 정리한다")).toBeTruthy();
    // 완료 기준 points to the comparison instead of repeating it.
    expect(screen.getByRole("button", { name: "아래 대조 4개" })).toBeTruthy();
    expect(screen.getByText("나머지 결정 2개는 아래 표에 있어요")).toBeTruthy();
    expect(screen.getAllByText(/묻지 않고 정함/).length).toBeGreaterThan(0);
  });

  it("orders the chips: a block that needs the user, unmet criteria, then flagged decisions", () => {
    show("66bb44dd", "stopped");
    expect(chips()).toEqual(["답이 필요해요: 태그가 없는 커밋 범위", "일부 충족: 사용자 변경 목록", "묻지 않고 정함: 분류 기준"]);
  });

  it("folds chips past five, merges a decision's flags, and jumps to the row", () => {
    show("aa11bb22", "done");
    expect(chips()).toEqual([
      "못 했어요: 화면 동작 그대로",
      "합의와 다름, 되돌리기 어려워요: 취소 상태를 실패와 합치기",
      "합의와 다름: 영수증 메일 발송 시점을 결제 완료 직후에서 정산 확정 이후로 옮기기",
      "일부 충족: 결제 실패 시 다시 시도 가능",
      "되돌리기 어려워요: 구 API 응답 필드 삭제",
    ]);
    fireEvent.click(screen.getByRole("button", { name: "외 2개" }));
    expect(chips()).toHaveLength(7);
    // ✕ criterion is drawn with its mark.
    expect(screen.getAllByText("못 했어요").length).toBeGreaterThan(0);
    // A chip moves focus to its (expanded) row.
    fireEvent.click(screen.getByRole("button", { name: /^합의와 다름: 영수증/ }));
    const row = document.activeElement as HTMLElement;
    expect(row.tagName).toBe("TR");
    expect(row.querySelector('button[aria-expanded="true"]')?.textContent).toBe("접기");
  });

  it("shows a format error without any report content", () => {
    show("77aa55cc", "failed");
    expect(screen.getByRole("alert").textContent).toBe("보고서 형식이 맞지 않아 보여 드릴 수 없어요");
    expect(document.querySelector('section[aria-label="흐름"]')).toBeNull();
    expect(screen.getByText("users 테이블에 last_seen 컬럼을 추가한다")).toBeTruthy();
  });

  it("shows the delivery scope chip under 맡긴 내용 (delivery-guard)", () => {
    show("9988aabb", "done");
    const delegated = () => document.querySelector('section[aria-label="맡긴 내용"]')!.textContent;
    expect(delegated()).toContain("PR 초안까지");
    cleanup();
    show("77aa55cc", "failed");
    expect(delegated()).toContain("로컬만");
    cleanup();
    show("ab12cd34", "working");
    expect(delegated()).not.toContain("PR 초안까지");
    expect(delegated()).not.toContain("로컬만");
  });

  it("shows the delivery post-check warnings (delivery-guard)", () => {
    show("77aa55cc", "failed");
    expect(screen.getByRole("list", { name: "전달 범위 점검" }).textContent).toContain("원격으로 push한 기록이 있어요");
    cleanup();
    show("9988aabb", "done");
    expect(screen.queryByRole("list", { name: "전달 범위 점검" })).toBeNull();
  });

  it("says when instructions were not stored", () => {
    show("ab12cd34", "working");
    expect(screen.getByText(/저장하지 않아/)).toBeTruthy();
    expect(screen.getByText("진행하면서 채워져요")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "다시 맡기기" })).toBeNull();
  });

  it("offers 다시 맡기기 with the same brief for a finished task without a report", () => {
    const retry = vi.fn();
    show("cc33dd44", null, retry);
    expect(screen.getByText("보고서 없음")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "다시 맡기기" }));
    expect(retry).toHaveBeenCalledWith(expect.objectContaining({ goal: "1.2 릴리스 노트를 쓴다" }));
  });
});

describe("TaskReportDialog", () => {
  afterEach(cleanup);

  function Page() {
    const [open, setOpen] = useState(false);
    return (
      <div>
        <textarea aria-label="터미널 입력" />
        <button onClick={() => setOpen(true)}>보고서 보기</button>
        {open && <TaskReportDialog id="9988aabb" title="lint-sweep" session="done" onClose={() => setOpen(false)} />}
      </div>
    );
  }

  it("keeps Tab inside the dialog, makes the page inert, and gives focus back on close", async () => {
    const { container } = render(
      <QueryClientProvider client={client()}>
        <Page />
      </QueryClientProvider>,
    );
    const opener = screen.getByRole("button", { name: "보고서 보기" });
    opener.focus();
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog");
    await screen.findByText("완료 기준 대조");
    const terminal = screen.getByLabelText("터미널 입력");
    expect(container.closest("[inert]")).not.toBeNull();
    expect(dialog.closest("[inert]")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "보고서 닫기" }));
    const count = dialog.querySelectorAll("button").length;
    for (let i = 0; i < count + 3; i++) {
      fireEvent.keyDown(document.activeElement ?? document.body, { key: "Tab", shiftKey: i % 5 === 4 });
      expect(document.activeElement).not.toBe(terminal);
      expect(dialog.contains(document.activeElement)).toBe(true);
    }
    // Focus that lands outside (a click on the backdrop page) is pulled back in.
    act(() => terminal.focus());
    expect(dialog.contains(document.activeElement)).toBe(true);
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(container.closest("[inert]")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });
});
