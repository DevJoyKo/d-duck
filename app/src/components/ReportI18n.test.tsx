// app-language (tasks.md 3.2): the report screen, evidence and attachments in English, plus the shared save state of
// an attachment's inline header and full view (report-copy-save review).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskReportView } from "../lib/api";
import { applyLanguage } from "../i18n/language";
import { evidenceHandlers, mockEvidenceReport } from "../mock/evidence";
import { mockTaskReport } from "../mock/report";

const save = vi.fn<(id: string, index: number) => Promise<boolean>>();
vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      teamConfig: vi.fn(async () => null),
      reportEvidence: vi.fn(async (id: string, criterion: number, item: number) => evidenceHandlers.report_evidence({ id, criterion, item })),
      reportAttachment: vi.fn(async (id: string, index: number) => evidenceHandlers.report_attachment({ id, index })),
      saveReportAttachment: (id: string, index: number) => save(id, index),
    },
  };
});
const { ReportBody } = await import("./TaskReport");

const HANGUL = /[가-힣]/;

/** The view with every piece of user/agent text (all Korean in the mocks) swapped for ASCII, so any Hangul left on
 *  screen is app text. */
function asciiView(v: unknown): TaskReportView {
  const walk = (x: unknown): unknown => {
    if (typeof x === "string") return HANGUL.test(x) ? "x" : x;
    if (Array.isArray(x)) return x.map(walk);
    if (x && typeof x === "object") return Object.fromEntries(Object.entries(x).map(([k, val]) => [k, walk(val)]));
    return x;
  };
  const out = walk(v) as TaskReportView & { delegated: { taskType: string | null; permissionMode: string | null } | null };
  // Task type, permission mode and the delivery guard have labels in other namespaces (agents, tasks).
  if (out.delegated) out.delegated = { ...out.delegated, taskType: null, permissionMode: null, deliveryGuard: null, deliveryAlert: null } as typeof out.delegated;
  return out;
}

const show = (view: TaskReportView, bgId?: string) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ReportBody view={view} session="done" bgId={bgId} />
    </QueryClientProvider>,
  );

beforeEach(() => save.mockReset());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  applyLanguage("ko");
});

describe("report screen in English", () => {
  it("has no Korean app text on a full report", () => {
    applyLanguage("en");
    show(asciiView(mockTaskReport("9988aabb")));
    const order = Array.from(document.querySelectorAll("section[aria-label]")).map((s) => s.getAttribute("aria-label"));
    expect(order).toEqual(["Brief", "Criteria check", "Cautions", "Flow", "Records"]);
    expect(screen.getByRole("button", { name: "4 compared below" })).toBeTruthy();
    expect(document.body.textContent).not.toMatch(HANGUL);
  });

  it("has no Korean app text on evidence chips and the attachments block", async () => {
    applyLanguage("en");
    show(asciiView(mockEvidenceReport("e2e2e2e2")), "e2e2e2e2");
    await waitFor(() => expect(document.querySelector("iframe[data-attachment-frame]")).toBeTruthy());
    expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Enlarge" })).toBeTruthy();
    expect(document.querySelector("iframe")?.getAttribute("title")).toMatch(/^Agent-made document: /);
    expect(document.body.textContent).not.toMatch(HANGUL);
    show(asciiView(mockEvidenceReport("e1e1e1e1")), "e1e1e1e1");
    expect(document.body.textContent).not.toMatch(HANGUL);
  });
});

describe("attachment save state", () => {
  it("the inline header and the full view share one result line, and 저장했어요 goes away after a few seconds", async () => {
    save.mockResolvedValue(true);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    show(mockEvidenceReport("e2e2e2e2") as TaskReportView, "e2e2e2e2");
    await waitFor(() => expect(screen.getByRole("button", { name: "저장" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect((await screen.findByRole("status")).textContent).toBe("저장했어요");
    fireEvent.click(screen.getByRole("button", { name: "크게 보기" }));
    const full = screen.getByRole("dialog", { name: "에이전트가 만든 문서 크게 보기" });
    expect(within(full).getByRole("status").textContent).toBe("저장했어요");
    act(() => vi.advanceTimersByTime(3_100));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("an error stays, shows in both places, and a missing original has its own line", async () => {
    show(mockEvidenceReport("e2e2e2e2") as TaskReportView, "e2e2e2e2");
    await waitFor(() => expect(screen.getByRole("button", { name: "저장" })).toBeTruthy());
    save.mockRejectedValueOnce("missing");
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect((await screen.findByRole("status")).textContent).toBe("원본 파일이 없어요");
    fireEvent.click(screen.getByRole("button", { name: "크게 보기" }));
    expect(screen.getAllByRole("status").map((s) => s.textContent)).toEqual(["원본 파일이 없어요", "원본 파일이 없어요"]);
  });
});
