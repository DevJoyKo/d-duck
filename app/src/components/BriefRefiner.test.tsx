import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RefineRequest, RefineRun } from "../lib/api";
import { EMPTY_BRIEF, type Brief } from "../lib/brief";

const round = (n: number): RefineRun => ({
  kind: "done",
  token: "abcd",
  costUsd: 0.02,
  spentUsd: 0.02 * n,
  runsLeft: 3 - n,
  result: {
    questions:
      n === 1
        ? [
            {
              id: "q1",
              title: "재현 테스트를 먼저 만들까요?",
              body: "확인할 수 있어야 해요",
              field: "done",
              multi: false,
              options: [
                { label: "예", detail: "" },
                { label: "아니오", detail: "" },
              ],
              recommended: [0],
            },
            {
              id: "q2",
              title: "어떤 검증을 넣을까요?",
              body: "",
              field: "done",
              multi: true,
              options: [
                { label: "20회 통과", detail: "" },
                { label: "원인 3줄", detail: "" },
                { label: "수동 확인", detail: "" },
              ],
              recommended: [0, 1],
            },
          ]
        : [],
    rewrite: { goal: "로그인 실패를 재현 후 고친다", outOfScope: "회원가입", constraints: "새 의존성 금지", done: "20회 통과" },
    finished: n > 1,
    note: "",
  },
});

const calls: RefineRequest[] = [];
let next: (req: RefineRequest) => RefineRun = () => round(calls.length);

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      refineBrief: vi.fn(async (req: RefineRequest) => {
        calls.push(structuredClone(req));
        return next(req);
      }),
      refineCancel: vi.fn(async () => undefined),
    },
  };
});

const { BriefRefiner } = await import("./BriefRefiner");
const { api } = await import("../lib/api");

function Host({ initial, onBrief, cliReady = true, onFocusField }: { initial: Brief; onBrief?: (b: Brief) => void; cliReady?: boolean; onFocusField?: (k: keyof Brief) => void }) {
  const [brief, setBrief] = useState(initial);
  return (
    <>
      <output data-testid="form">{JSON.stringify(brief)}</output>
      <BriefRefiner
        brief={brief}
        taskType="dev"
        strict={false}
        cliReady={cliReady}
        onFocusField={onFocusField}
        submitRef={submitRef}
        localNames={["acme-mcp"]}
        maxRounds={3}
        onApply={(b) => {
          setBrief(b);
          onBrief?.(b);
        }}
      />
    </>
  );
}

const submitRef: { current: (() => boolean) | null } = { current: null };
const start = { ...EMPTY_BRIEF, goal: "로그인 가끔 실패 고치기", constraints: "새 의존성 금지\n배포 금지" };
const form = () => JSON.parse(screen.getByTestId("form").textContent ?? "{}") as Brief;

afterEach(() => {
  cleanup();
  vi.mocked(api.refineBrief).mockClear();
  calls.length = 0;
  next = () => round(calls.length);
});

describe("BriefRefiner", () => {
  it("is off until the goal has a line, and shows the free pre-check", () => {
    render(<Host initial={EMPTY_BRIEF} />);
    expect((screen.getByRole("button", { name: /질문 받기/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("목표를 한 줄 이상 적으면 켜져요")).toBeTruthy();
    cleanup();
    const focus = vi.fn();
    render(<Host initial={start} onFocusField={focus} />);
    expect((screen.getByRole("button", { name: /질문 받기/ }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "측정하기 어려운 말이 있어요: '가끔'" }));
    expect(focus).toHaveBeenCalledWith("goal");
    fireEvent.click(screen.getByRole("button", { name: "완료 기준이 비어 있어요" }));
    expect(focus).toHaveBeenLastCalledWith("done");
  });

  it("is off without the Claude CLI and says why", () => {
    render(<Host initial={start} cliReady={false} />);
    for (const name of [/질문 받기/, /다시 쓰기만/]) expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("Claude CLI가 없어 쓸 수 없어요")).toBeTruthy();
    expect(api.refineBrief).not.toHaveBeenCalled();
  });

  it("two rounds: recommended answers preselected, keyboard submit, then 다듬기 완료", async () => {
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    await screen.findByText("재현 테스트를 먼저 만들까요?");
    expect(calls[0]).toMatchObject({ mode: "grill", history: [], token: null, localNames: ["acme-mcp"], redactNames: true });
    expect((screen.getByRole("radio", { name: /예/ }) as HTMLInputElement).checked).toBe(true);
    // The first card's selected answer takes focus when the questions arrive.
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("radio", { name: /예/ })));
    expect((screen.getByRole("checkbox", { name: /20회 통과/ }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: /원인 3줄/ }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText("여러 개 골라도 돼요")).toBeTruthy();
    expect(screen.getByLabelText("비용").textContent).toBe("이번: $0.02 · 누적 $0.02 (예상 비용)");
    expect(screen.getByText("질문 1/3 · 2개")).toBeTruthy();
    // Form untouched until applied.
    expect(form()).toEqual(start);

    fireEvent.click(screen.getByRole("radio", { name: /아니오/ }));
    fireEvent.keyDown(screen.getByRole("radio", { name: /아니오/ }), { key: "Enter", metaKey: true });
    await screen.findByText("다듬기 끝");
    expect(calls[1].token).toBe("abcd");
    expect(calls[1].history).toEqual([
      [
        { question: "재현 테스트를 먼저 만들까요?", field: "done", chosen: ["아니오"], custom: "", skipped: false },
        { question: "어떤 검증을 넣을까요?", field: "done", chosen: ["20회 통과", "원인 3줄"], custom: "", skipped: false },
      ],
    ]);
    expect(screen.queryByRole("button", { name: /답하고 계속/ })).toBeNull();
  });

  it("applies one field, then reverts it", async () => {
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /다시 쓰기만/ }));
    await screen.findByText("고쳐 쓴 제안");
    expect(calls[0].mode).toBe("rewrite");
    const doneRow = screen.getByText("완료 기준", { selector: "span" }).parentElement as HTMLElement;
    fireEvent.click(doneRow.querySelector("button") as HTMLButtonElement);
    expect(form()).toEqual({ ...start, done: "20회 통과" });
    // Applied rows collapse to "적용됨" with the value behind a disclosure.
    expect(screen.getByText("적용한 내용 보기")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /되돌리기/ }));
    expect(form()).toEqual(start);
  });

  it("warns when constraints shrink and confirms before 모두 적용", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /다시 쓰기만/ }));
    await screen.findByText(/지워지는 줄이 있어요/);
    const lost = screen.getByText("배포 금지", { selector: "span.line-through" });
    expect(lost.previousElementSibling?.textContent).toBe("− ");
    expect(lost.previousElementSibling?.className ?? "").not.toContain("line-through");
    fireEvent.click(screen.getByRole("button", { name: "모두 적용" }));
    expect(confirm).toHaveBeenCalled();
    expect(form()).toEqual(start);
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "모두 적용" }));
    expect(form()).toEqual({ goal: "로그인 실패를 재현 후 고친다", outOfScope: "회원가입", constraints: "새 의존성 금지", done: "20회 통과" });
    confirm.mockRestore();
  });

  it("a malformed answer shows the fixed message and leaves the form alone", async () => {
    next = () => ({ kind: "failed", error: { kind: "badResponse" }, message: "Claude 응답을 읽지 못했어요. 다시 시도하세요", costUsd: 0.01, spentUsd: 0.01 });
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    expect((await screen.findByRole("alert")).textContent).toBe("Claude 응답을 읽지 못했어요. 다시 시도하세요");
    expect(form()).toEqual(start);
  });

  it("모두 건너뛰고 계속 submits every question as skipped at once", async () => {
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    await screen.findByText("재현 테스트를 먼저 만들까요?");
    fireEvent.click(screen.getByRole("button", { name: "모두 건너뛰고 계속" }));
    await screen.findByText("다듬기 끝");
    expect(calls[1].history[0].every((a) => a.skipped && a.chosen.length === 0)).toBe(true);
  });

  it("⌘↵ from the composer form submits the open answers; details show only on the picked option", async () => {
    next = (req) => {
      const r = round(calls.length) as Extract<RefineRun, { kind: "done" }>;
      if (req.history.length === 0) r.result.questions[0].options[1].detail = "원인을 이미 알 때";
      return r;
    };
    render(<Host initial={start} />);
    expect(submitRef.current).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    await screen.findByText("재현 테스트를 먼저 만들까요?");
    expect(screen.queryByText("원인을 이미 알 때")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: /아니오/ }));
    expect(screen.getByText("원인을 이미 알 때")).toBeTruthy();
    expect(submitRef.current?.()).toBe(true);
    await screen.findByText("다듬기 끝");
    expect(calls[1].history[0][0].chosen).toEqual(["아니오"]);
  });

  it("cancel and timeout copy: the cost chip says 취소됨, the message gives the cost", async () => {
    next = () => ({ kind: "failed", error: { kind: "cancelled" }, message: "취소했어요", costUsd: null, spentUsd: 0.04 });
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    expect((await screen.findByRole("alert")).textContent).toBe("취소했어요. 그때까지 쓴 사용량은 계정에 남을 수 있어요.");
    expect(screen.getByLabelText("비용").textContent).toBe("이번: 취소됨 · 누적 $0.04 (예상 비용)");
    next = () => ({ kind: "failed", error: { kind: "timeout" }, message: "90초 안에 끝나지 않아 중단했어요", costUsd: 0.05, spentUsd: 0.09 });
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("90초 안에 끝나지 않아 멈췄어요. 이번 예상 비용은 $0.05예요. 다시 해 보세요."));
    expect(screen.getByLabelText("비용").textContent).toBe("이번: $0.05 · 누적 $0.09 (예상 비용)");
  });

  it("closing mid-run cancels it", async () => {
    let release: (r: RefineRun) => void = () => undefined;
    next = () => round(1);
    vi.mocked(api.refineBrief).mockImplementationOnce(() => new Promise((r) => (release = r)));
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    await screen.findByText(/질문을 만드는 중/);
    expect(screen.getByRole("img", { name: "Claude가 질문을 만드는 중" })).toBeTruthy();
    cleanup();
    await waitFor(() => expect(api.refineCancel).toHaveBeenCalled());
    release(round(1));
  });

  it("취소 ends the wait at once, drops the late answer and restores the panel (D-159)", async () => {
    let release: (r: RefineRun) => void = () => undefined;
    vi.mocked(api.refineBrief).mockImplementationOnce(() => new Promise((r) => (release = r)));
    vi.mocked(api.refineCancel).mockClear();
    render(<Host initial={start} />);
    fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
    const progress = await screen.findByText(/질문을 만드는 중/);
    expect(progress.textContent).toMatch(/질문을 만드는 중 · \d+초/);
    fireEvent.click(screen.getByRole("button", { name: "취소" }));
    // Back to before: no progress row, buttons usable again, nothing opened, no error.
    expect(screen.queryByText(/질문을 만드는 중/)).toBeNull();
    expect((screen.getByRole("button", { name: /질문 받기/ }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(api.refineCancel).toHaveBeenCalledTimes(1);
    // The backend's answer that still arrives is thrown away.
    release(round(1));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByText("재현 테스트를 먼저 만들까요?")).toBeNull();
    expect(screen.queryByRole("tablist")).toBeNull();
  });

  it("shows seconds while waiting", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      vi.mocked(api.refineBrief).mockImplementationOnce(() => new Promise(() => undefined));
      render(<Host initial={start} />);
      fireEvent.click(screen.getByRole("button", { name: /질문 받기/ }));
      await screen.findByText(/질문을 만드는 중/);
      await vi.advanceTimersByTimeAsync(3_100);
      expect(screen.getByText(/질문을 만드는 중/).textContent).toMatch(/· [3-4]초/);
    } finally {
      vi.useRealTimers();
    }
  });
});
