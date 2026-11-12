// sheet-focus 1.2 (test review): the report dialog keeps its Escape order on the shared useModal hook. Escape is
// caught in the capture phase so the task detail panel under it (TasksPage, a plain window listener) stays open, and
// an open image or document overlay closes before the dialog.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { evidenceHandlers, mockEvidenceReport } from "../mock/evidence";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      teamConfig: vi.fn(async () => null),
      taskReport: vi.fn(async (id: string) => mockEvidenceReport(id)),
      reportEvidence: vi.fn(async (id: string, criterion: number, item: number) => evidenceHandlers.report_evidence({ id, criterion, item })),
      reportAttachment: vi.fn(async (id: string, index: number) => evidenceHandlers.report_attachment({ id, index })),
    },
  };
});
const { TaskReportDialog } = await import("./TaskReport");

/** The task detail panel as TasksPage draws it: Escape on a window listener closes it; it opens the report. */
function Detail() {
  const [panel, setPanel] = useState(true);
  const [report, setReport] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setPanel(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  if (!panel) return null;
  return (
    <aside aria-label="작업 상세">
      <button onClick={() => setReport(true)}>보고서 보기</button>
      {report && <TaskReportDialog id="e1e1e1e1" title="list-dark-mode" session="done" onClose={() => setReport(false)} />}
    </aside>
  );
}

async function openReport() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } })}>
      <Detail />
    </QueryClientProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "보고서 보기" }));
  await screen.findByRole("dialog", { name: /list-dark-mode/ });
}

const escape = () => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

afterEach(cleanup);

describe("report dialog Escape", () => {
  it("closes only the report; the task detail panel under it stays open", async () => {
    await openReport();
    escape();
    expect(screen.queryByRole("dialog", { name: /list-dark-mode/ })).toBeNull();
    expect(screen.getByRole("complementary", { name: "작업 상세" })).toBeTruthy();
  });

  it("closes an open screenshot first and leaves the report open", async () => {
    await openReport();
    fireEvent.click(await screen.findByRole("button", { name: /스크린샷 list-dark\.png/ }));
    fireEvent.click(await screen.findByRole("button", { name: "list-dark.png 크게 보기" }));
    expect(screen.getByRole("dialog", { name: "스크린샷 크게 보기" })).toBeTruthy();
    escape();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "스크린샷 크게 보기" })).toBeNull());
    expect(screen.getByRole("dialog", { name: /list-dark-mode/ })).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "작업 상세" })).toBeTruthy();
    // The next Escape closes the report itself.
    escape();
    expect(screen.queryByRole("dialog", { name: /list-dark-mode/ })).toBeNull();
  });
});
