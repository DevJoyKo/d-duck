// report-evidence: evidence chips and previews, the legacy chip, attachments behind the app's own header.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TaskReportView } from "../lib/api";
import { evidenceHandlers, mockEvidenceReport } from "../mock/evidence";
import { mockTaskReport } from "../mock/report";

const calls: string[] = [];
vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      teamConfig: vi.fn(async () => null),
      reportEvidence: vi.fn(async (id: string, criterion: number, item: number) => {
        calls.push(`e:${id}:${criterion}:${item}`);
        return evidenceHandlers.report_evidence({ id, criterion, item });
      }),
      reportAttachment: vi.fn(async (id: string, index: number) => {
        calls.push(`a:${id}:${index}`);
        return evidenceHandlers.report_attachment({ id, index });
      }),
    },
  };
});
const { ReportBody } = await import("./TaskReport");

const show = (id: string, view?: unknown) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ReportBody view={(view ?? mockEvidenceReport(id)) as TaskReportView} session="done" bgId={id} />
    </QueryClientProvider>,
  );

describe("근거 칩과 첨부", () => {
  afterEach(() => {
    cleanup();
    calls.length = 0;
  });

  it("draws chips per criterion, loads a preview only when pressed, and marks missing evidence", async () => {
    show("e1e1e1e1");
    expect(screen.getByRole("button", { name: /스크린샷 list-dark\.png/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /테스트 vitest\.xml/ })).toBeTruthy();
    for (const t of ["형식이 맞지 않아요", "파일이 없어요", "읽지 않았어요(안전하지 않은 파일)", "너무 커서 읽지 않았어요", "근거 파일이 너무 많아요"]) expect(screen.getByText(t)).toBeTruthy();
    expect(document.querySelectorAll("[data-no-evidence]")).toHaveLength(2);
    expect(screen.getAllByText("테스트 실패 2").length).toBeGreaterThan(0);
    expect(calls).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: /테스트 vitest-all\.xml/ }));
    await waitFor(() => expect(document.querySelector("[data-test-counts]")?.textContent).toBe("통과 48 · 실패 2 · 건너뜀 1"));
    expect(screen.getByRole("list", { name: "실패한 테스트" }).children).toHaveLength(2);
    expect(calls).toEqual(["e:e1e1e1e1:1:0"]);
  });

  it("opens a screenshot large; Escape closes it and focus goes back to the chip", async () => {
    show("e1e1e1e1");
    const chip = screen.getByRole("button", { name: /스크린샷 list-dark\.png/ });
    fireEvent.click(chip);
    fireEvent.click(await screen.findByRole("button", { name: "list-dark.png 크게 보기" }));
    expect(screen.getByRole("dialog", { name: "스크린샷 크게 보기" })).toBeTruthy();
    // The report dialog's Escape handler asks the overlay first.
    const { overlayEscape } = await import("./ReportEvidence");
    overlayEscape.current?.();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "스크린샷 크게 보기" })).toBeNull());
    expect(document.activeElement).toBe(chip);
  });

  it("downgrades unbacked ✓ marks and dashes a kind the role does not accept", () => {
    show("e1e1e1e1");
    const marks = Array.from(document.querySelectorAll("[data-mark]")).map((m) => m.getAttribute("data-mark"));
    expect(marks.slice(0, 4)).toEqual(["yes", "testsFailed", "noEvidence", "noEvidence"]);
    cleanup();
    show("e3e3e3e3");
    expect(screen.getByText("QA는 파일을 근거로 안 써요")).toBeTruthy();
  });

  it("the lightbox names the image and its size and says Esc closes it", async () => {
    show("e1e1e1e1");
    fireEvent.click(screen.getByRole("button", { name: /스크린샷 list-dark\.png/ }));
    fireEvent.click(await screen.findByRole("button", { name: "list-dark.png 크게 보기" }));
    expect(document.querySelector("[data-lightbox-label]")?.textContent).toBe("list-dark.png · 640×400");
    expect(screen.getByText("Esc로 닫기")).toBeTruthy();
  });

  it("review file evidence shows the path and line range only", async () => {
    show("e4e4e4e4");
    fireEvent.click(screen.getByRole("button", { name: /파일 src\/a\.ts:10–42/ }));
    expect(await screen.findByText("src/a.ts:10–42")).toBeTruthy();
    expect(screen.getByRole("button", { name: /경로 복사/ })).toBeTruthy();
  });

  it("shows the legacy chip for a version 1 report and no missing-evidence marks", () => {
    show("9988aabb", mockTaskReport("9988aabb"));
    expect(screen.getByText("근거 형식 이전 보고서")).toBeTruthy();
    expect(document.querySelectorAll("[data-no-evidence]")).toHaveLength(0);
  });

  it("attachments: tabs, the app header outside a sandbox=\"\" frame, close returns focus to the tab", async () => {
    show("e2e2e2e2");
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(["DB 후보 비교HTML", "운영 비용 표Markdown", "원본 메모Markdown· 형식이 맞지 않아요", "빈 문서HTML"]);
    const frame = await waitFor(() => {
      const f = document.querySelector<HTMLIFrameElement>("iframe[data-attachment-frame]");
      expect(f).toBeTruthy();
      return f!;
    });
    expect(frame.getAttribute("sandbox")).toBe("");
    expect(frame.getAttribute("srcdoc")?.startsWith('<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy"')).toBe(true);
    const head = document.querySelector("[data-attachment-head]")!;
    expect(head.textContent).toContain("에이전트가 만든 문서 · DB 후보 비교");
    expect(head.contains(frame)).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "크게 보기" }));
    expect(screen.getByRole("dialog", { name: "에이전트가 만든 문서 크게 보기" })).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "닫기" }).slice(-1)[0]);
    expect(screen.queryByRole("dialog", { name: "에이전트가 만든 문서 크게 보기" })).toBeNull();
    expect(document.activeElement?.textContent).toContain("크게 보기");
    // Markdown with a table.
    fireEvent.click(tabs[1]);
    expect(await screen.findByRole("table")).toBeTruthy();
    expect(screen.getAllByRole("row")).toHaveLength(6);
    // A document with nothing left after cleaning.
    fireEvent.click(tabs[3]);
    expect(await screen.findByText("보여 드릴 내용이 없어요")).toBeTruthy();
  });
});
