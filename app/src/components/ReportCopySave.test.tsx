// report-copy-save (D-174): readable report text opts into selection, attachments get 저장 and (Markdown) 복사.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskReportView } from "../lib/api";
import { evidenceHandlers, mockEvidenceReport } from "../mock/evidence";

const save = vi.fn<(id: string, index: number) => Promise<boolean>>();
vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      teamConfig: vi.fn(async () => null),
      reportEvidence: vi.fn(async (id: string, criterion: number, item: number) => evidenceHandlers.report_evidence({ id, criterion, item })),
      reportAttachment: vi.fn(async (id: string, index: number) => {
        if (id === "bidi0000") return { kind: "markdown", text: "# 제목\n\n- 항목‮숨김​\n", images: [] };
        return evidenceHandlers.report_attachment({ id, index });
      }),
      saveReportAttachment: (id: string, index: number) => save(id, index),
    },
  };
});
const { ReportBody } = await import("./TaskReport");
const { opensOnClick } = await import("./ReviewPage");

const show = (id: string, view?: unknown) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ReportBody view={(view ?? mockEvidenceReport(id)) as TaskReportView} session="done" bgId={id} />
    </QueryClientProvider>,
  );

const writeText = vi.fn<(s: string) => Promise<void>>();
beforeEach(() => {
  save.mockReset();
  writeText.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const head = () => document.querySelector("[data-attachment-head]") as HTMLElement;
const saveBtn = () => screen.getByRole("button", { name: "저장" });

describe("보고서 내용 선택", () => {
  it("report sections are selectable; their headings, chips and marks are not", () => {
    show("e1e1e1e1");
    const crit = screen.getByRole("region", { name: "완료 기준 대조" });
    const body = crit.querySelector(".selectable") as HTMLElement;
    expect(body).toBeTruthy();
    // The heading sits outside the selectable body.
    expect(body.contains(crit.querySelector("h3"))).toBe(false);
    expect(body.querySelector("[data-mark]")?.hasAttribute("data-no-select")).toBe(true);
    // Evidence chips are buttons: the CSS rule keeps them unselectable inside .selectable.
    expect(body.querySelector("button[data-evidence-chip]")).toBeTruthy();
  });

  it("a card body click opens only without a selection and away from controls", () => {
    const li = document.createElement("li");
    const article = li.appendChild(document.createElement("article"));
    article.appendChild(Object.assign(document.createElement("span"), { id: "t", textContent: "목표" }));
    article.appendChild(Object.assign(document.createElement("button"), { id: "b", textContent: "x" }));
    document.body.append(li);
    expect(opensOnClick(li.querySelector("#t"), "")).toBe(true);
    expect(opensOnClick(li.querySelector("#t"), "목표")).toBe(false);
    expect(opensOnClick(li.querySelector("#b"), "")).toBe(false);
    li.remove();
  });
});

describe("첨부 저장·복사", () => {
  it("HTML: 저장 sends the id and index only and says 저장했어요; no 복사", async () => {
    save.mockResolvedValue(true);
    show("e2e2e2e2");
    await waitFor(() => expect(document.querySelector("iframe[data-attachment-frame]")).toBeTruthy());
    expect(head().contains(saveBtn())).toBe(true);
    expect(screen.queryByRole("button", { name: "복사" })).toBeNull();
    fireEvent.click(saveBtn());
    expect(save).toHaveBeenCalledWith("e2e2e2e2", 0);
    expect(await screen.findByRole("status")).toHaveProperty("textContent", "저장했어요");
  });

  it("cancel shows nothing; fixed codes become one line", async () => {
    show("e2e2e2e2");
    await waitFor(() => expect(saveBtn()).toBeTruthy());
    save.mockResolvedValueOnce(false);
    fireEvent.click(saveBtn());
    await waitFor(() => expect((saveBtn() as HTMLButtonElement).disabled).toBe(false));
    expect(document.querySelector("[data-attachment-result]")).toBeNull();
    save.mockRejectedValueOnce("refused");
    fireEvent.click(saveBtn());
    expect((await screen.findByRole("status")).textContent).toBe("원본이 바뀌어 저장하지 않았어요");
    save.mockRejectedValueOnce("busy");
    fireEvent.click(saveBtn());
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("저장 창이 이미 열려 있어요"));
    save.mockRejectedValueOnce("Permission denied (os error 13) /Users/me/x");
    fireEvent.click(saveBtn());
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("저장하지 못했어요"));
  });

  it("Markdown: 복사 puts the text with its marks, minus bidi and invisible characters, then 복사했어요 for 1.5 s", async () => {
    show("bidi0000", mockEvidenceReport("e2e2e2e2"));
    fireEvent.click(screen.getAllByRole("tab")[1]);
    const copy = await screen.findByRole("button", { name: "복사" });
    expect(document.querySelector("[data-attachment-text]")?.classList.contains("selectable")).toBe(true);
    vi.useFakeTimers();
    await act(async () => {
      fireEvent.click(copy);
    });
    expect(writeText).toHaveBeenCalledWith("# 제목\n\n- 항목숨김\n");
    expect(screen.getByRole("button", { name: "복사했어요" })).toBeTruthy();
    act(() => vi.advanceTimersByTime(1_600));
    expect(screen.getByRole("button", { name: "복사" })).toBeTruthy();
  });

  it("a document that could not be read has no 저장", async () => {
    show("e2e2e2e2");
    fireEvent.click(screen.getAllByRole("tab")[2]);
    await waitFor(() => expect(head().textContent).toContain("원본 메모"));
    await waitFor(() => expect(screen.queryByRole("button", { name: "저장" })).toBeNull());
  });
});
