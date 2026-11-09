import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactRow } from "../lib/api";
import { PAGE_SIZE, PREVIEW_CSP, clampPage, fitPageSize, limiter, pageCount, pageForAnchor, pageNumbers, pageSlice, previewReason, previewSrcdoc } from "../lib/artifactCards";

const rows: ArtifactRow[] = Array.from({ length: 23 }, (_, i) => ({
  artifactId: `id-${i + 1}`,
  url: `https://claude.ai/artifact/id-${i + 1}`,
  title: i < 12 ? `대시보드 ${i + 1}` : `보고서 ${i + 1}`,
  kind: i === 1 ? "typed" : "page",
  seq: 1,
  firstSeenTs: 1,
  lastTs: 1000 - i,
  projectName: "p",
}));

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      artifacts: vi.fn(async (q: string) => rows.filter((r) => (r.title ?? "").includes(q))),
      artifactPreview: vi.fn(async (id: string) => {
        if (id === "id-1") return { kind: "html", text: "<p>본문</p><script>alert(1)</script>", truncated: false, scriptOnly: false };
        if (id === "id-3") return { kind: "markdown", text: "# 제목", truncated: false };
        if (id === "id-4") throw new Error("/Users/x/secret.html: denied");
        if (id === "id-5") return { kind: "html", text: '<div id="root"></div><script src="app.js"></script>', truncated: false, scriptOnly: true };
        if (id === "id-6") return { kind: "html", text: "<p>긴 본문</p>", truncated: true, scriptOnly: false };
        throw "missing";
      }),
    },
  };
});

const { ArtifactsPage } = await import("./ArtifactsPage");
const { api } = await import("../lib/api");

const cardTitles = () => within(screen.getAllByRole("list")[0]).getAllByRole("listitem").map((li) => li.querySelector("h3")?.textContent);

describe("pagination helpers", () => {
  it("splits 23 rows into 10 / 10 / 3 and clamps out-of-range pages", () => {
    expect(pageCount(23)).toBe(3);
    expect(pageCount(0)).toBe(1);
    expect(pageSlice(rows, 1)).toHaveLength(10);
    expect(pageSlice(rows, 2)[0].artifactId).toBe("id-11");
    expect(pageSlice(rows, 3)).toHaveLength(3);
    expect(clampPage(9, 23)).toBe(3);
    expect(clampPage(0, 23)).toBe(1);
    expect(clampPage(3, 4)).toBe(1);
    expect(pageSlice(rows, 9).map((r) => r.artifactId)).toEqual(["id-21", "id-22", "id-23"]);
  });

  it("fits columns by the card's minimum width and only rows that are not cut off (D-184)", () => {
    // 1160×700 list area, 16px gap, 240px minimum card, preview 16:10 + 120px text: 4 columns of 278px, rows of 294px.
    expect(fitPageSize({ width: 1160, height: 700, minCard: 240, gap: 16, body: 120 })).toEqual({ cols: 4, rows: 2, size: 8 });
    // One pixel short of the second row: that row would be cut, so only one fits.
    expect(fitPageSize({ width: 1160, height: 603, minCard: 240, gap: 16, body: 120 }).rows).toBe(1);
    expect(fitPageSize({ width: 1160, height: 604, minCard: 240, gap: 16, body: 120 }).rows).toBe(2);
    // 900px window: two columns, fewer cards than the big window.
    expect(fitPageSize({ width: 620, height: 640, minCard: 240, gap: 16, body: 120 })).toEqual({ cols: 2, rows: 2, size: 4 });
  });

  it("never goes below one column and one row, and falls back before the area is measured", () => {
    expect(fitPageSize({ width: 150, height: 80, minCard: 240, gap: 16, body: 120 })).toEqual({ cols: 1, rows: 1, size: 1 });
    expect(fitPageSize({ width: 0, height: 0, minCard: 240, gap: 16, body: 120 }).size).toBe(PAGE_SIZE);
  });

  it("keeps the first card in view when the page size changes", () => {
    // Page 2 of 8 starts at card index 8; at 4 per page that card is on page 3, at 12 per page on page 1.
    expect(pageForAnchor(8, 4)).toBe(3);
    expect(pageForAnchor(8, 12)).toBe(1);
    expect(pageForAnchor(0, 5)).toBe(1);
  });

  it("shows every page up to 7, otherwise a window with gaps", () => {
    expect(pageNumbers(1, 3)).toEqual([1, 2, 3]);
    expect(pageNumbers(5, 20)).toEqual([1, "gap", 4, 5, 6, "gap", 20]);
    expect(pageNumbers(1, 20)).toEqual([1, 2, "gap", 20]);
  });
});

describe("preview document", () => {
  it("puts the CSP and dns-prefetch metas before any file content", () => {
    const doc = previewSrcdoc('<html><head><meta http-equiv="Content-Security-Policy" content="default-src *"></head></html>');
    expect(doc.startsWith(`<meta http-equiv="Content-Security-Policy" content="${PREVIEW_CSP}"><meta http-equiv="x-dns-prefetch-control" content="off">`)).toBe(true);
    expect(PREVIEW_CSP).toContain("default-src 'none'");
    expect(PREVIEW_CSP).not.toMatch(/script-src|connect-src|https?:/);
    expect(PREVIEW_CSP).toContain("base-uri 'none'");
    expect(PREVIEW_CSP).toContain("form-action 'none'");
    expect(PREVIEW_CSP).not.toContain("font-src");
  });

  it("maps only known reason codes", () => {
    expect(previewReason("missing")).toBe("missing");
    expect(previewReason("/Users/x/secret.html")).toBe("unknown");
    expect(previewReason(new Error("x"))).toBe("unknown");
    expect(previewReason("toString")).toBe("unknown"); // inherited keys are not reason codes
    expect(previewReason(undefined)).toBe("unknown");
  });

  it("limits concurrent tasks", async () => {
    const run = limiter(3);
    let active = 0;
    let peak = 0;
    await Promise.all(
      Array.from({ length: 8 }, () =>
        run(async () => {
          active++;
          peak = Math.max(peak, active);
          await new Promise((r) => setTimeout(r, 5));
          active--;
        }),
      ),
    );
    expect(peak).toBe(3);
  });
});

describe("ArtifactsPage", () => {
  afterEach(cleanup);
  const mount = () =>
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ArtifactsPage />
      </QueryClientProvider>,
    );

  it("pages 10 at a time and resets to page 1 on search", async () => {
    mount();
    await screen.findByText("1 / 3");
    expect(cardTitles()).toHaveLength(10);
    expect(screen.getByRole("button", { name: "1쪽" }).getAttribute("aria-current")).toBe("page");
    fireEvent.click(screen.getByRole("button", { name: "다음 페이지" }));
    expect(screen.getByText("2 / 3")).toBeTruthy();
    expect(cardTitles()[0]).toBe("대시보드 11");
    fireEvent.click(screen.getByRole("button", { name: "3쪽" }));
    expect(cardTitles()).toHaveLength(3);
    expect((screen.getByRole("button", { name: "다음 페이지" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByPlaceholderText("제목 검색"), { target: { value: "대시보드" } });
    await screen.findByText("1 / 2");
    expect(cardTitles()[0]).toBe("대시보드 1");
  });

  it("renders a sandboxed srcdoc frame, markdown as text and reasons", async () => {
    mount();
    const frame = await screen.findByTitle("대시보드 1 미리보기");
    expect(frame.tagName).toBe("IFRAME");
    expect(frame.getAttribute("sandbox")).toBe("");
    expect(frame.getAttribute("aria-hidden")).toBe("true");
    expect(frame.getAttribute("tabindex")).toBe("-1");
    expect(frame.getAttribute("srcdoc")?.startsWith('<meta http-equiv="Content-Security-Policy"')).toBe(true);
    expect(await screen.findByText("제목")).toBeTruthy();
    expect(screen.getByText("미리보기 없음 · 덱·디자인은 claude.ai에서 확인")).toBeTruthy();
    expect(await screen.findByText("스크립트 페이지 · 미리보기 제한")).toBeTruthy();
    expect(screen.queryByTitle("대시보드 5 미리보기")).toBeNull(); // script-only: no blank frame
    expect(await screen.findByText("앞부분만 표시(512KB)")).toBeTruthy();
    await waitFor(() => expect(screen.getAllByText("미리보기 없음 · 원본 파일이 없어요").length).toBeGreaterThan(0));
    expect(vi.mocked(api.artifactPreview).mock.calls.map((c) => c[0])).not.toContain("id-2"); // typed: not requested
    expect(vi.mocked(api.artifactPreview).mock.calls.map((c) => c[0])).not.toContain("id-11"); // other pages: not requested
  });

  it("resets to page 1 on search even when the results still span pages", async () => {
    mount();
    await screen.findByText("1 / 3");
    fireEvent.click(screen.getByRole("button", { name: "2쪽" }));
    expect(screen.getByText("2 / 3")).toBeTruthy();
    // 12 matches = 2 pages, so clamping alone would keep page 2.
    fireEvent.change(screen.getByPlaceholderText("제목 검색"), { target: { value: "대시보드" } });
    await screen.findByText("1 / 2");
    expect(cardTitles()[0]).toBe("대시보드 1");
    expect(screen.getByRole("button", { name: "1쪽" }).getAttribute("aria-current")).toBe("page");
  });

  it("announces the page and scrolls the list top into view", async () => {
    const scroll = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = scroll;
    mount();
    await screen.findByText("1 / 3");
    fireEvent.click(screen.getByRole("button", { name: "다음 페이지" }));
    const live = document.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toBe("2/3쪽");
    expect(scroll).toHaveBeenCalledWith({ block: "start" });
    Element.prototype.scrollIntoView = original;
  });

  it("maps a non-string rejection to the generic reason without leaking it", async () => {
    mount();
    await screen.findByText("1 / 3");
    await waitFor(() => expect(screen.getByText("미리보기 없음 · 미리보기를 불러오지 못했어요")).toBeTruthy());
    expect(screen.queryByText(/secret\.html/)).toBeNull();
  });

  it("names each card and its open button", async () => {
    mount();
    await screen.findByText("1 / 3");
    expect(screen.getByRole("heading", { level: 3, name: "대시보드 1" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "대시보드 1 열기" })).toBeTruthy();
  });
});
