// The tour demos are display only: the whole api module (and the Tauri bridge) throws if anything calls it.
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string) => {
    calls.push(cmd);
    throw new Error(`invoke(${cmd}) from a tour demo`);
  },
}));
vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  const thrower = new Proxy({}, { get: (_t, k) => () => { calls.push(`api.${String(k)}`); throw new Error(`api.${String(k)} from a tour demo`); } });
  return { ...real, api: thrower };
});

const { TourDemo, ROLE_KEYS, REPORT_CRITERIA } = await import("./TourDemos");
const { ROLE_STYLE, TOUR_CARDS, tourGoalExamples } = await import("../lib/tour");
const { ROLE_COLOR_INDEX } = await import("../lib/roleColors");
const { DUCK_COLORS } = await import("./DuckAvatar");
const demoSource = Object.values(import.meta.glob("./TourDemos.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];

afterEach(() => {
  cleanup();
  calls.length = 0;
});

const ducks = (root: HTMLElement) => [...root.querySelectorAll("svg[data-pose]")] as SVGElement[];

describe("tour demos", () => {
  it("render every card without calling the api or the bridge", () => {
    for (const c of TOUR_CARDS) {
      const { unmount } = render(<TourDemo id={c.id} examples={tourGoalExamples()} />);
      unmount();
    }
    expect(calls).toEqual([]);
  });

  it("every duck has a role label next to it (never a mascot)", () => {
    for (const c of TOUR_CARDS) {
      const { container, unmount } = render(<TourDemo id={c.id} />);
      const list = ducks(container);
      expect(list.length > 0, c.id).toBe(["solo", "roles", "board", "report", "start"].includes(c.id));
      for (const d of list) expect(d.getAttribute("aria-label"), c.id).toMatch(/오리/);
      unmount();
    }
  });

  it("card 2 shows the five role ducks awake in their fixed colours, each with how that role works", () => {
    const { container } = render(<TourDemo id="roles" />);
    const rows = [...container.querySelectorAll("[data-role]")] as HTMLElement[];
    expect(rows.map((r) => r.dataset.role)).toEqual([...ROLE_KEYS]);
    expect(rows.map((r) => r.querySelector("svg")?.getAttribute("aria-label"))).toEqual(["설계 오리", "디자인 오리", "개발 오리", "리뷰 오리", "QA 오리"]);
    for (const r of rows) {
      const body = DUCK_COLORS[ROLE_COLOR_INDEX[r.dataset.role!]].body;
      const fills = [...r.querySelectorAll("svg [style]")].map((e) => e.getAttribute("style")).join(" ");
      expect(fills, r.dataset.role).toContain(`var(--duck-body, ${body})`);
      expect(within(r).getByText(ROLE_STYLE[r.dataset.role as keyof typeof ROLE_STYLE])).toBeTruthy();
    }
    // The permission presets are not on this card (dev keeps plan mode as a safety default; the card shows its style).
    expect(container.textContent).not.toMatch(/계획만 세우기|노력 /);
    expect(within(rows[2]).getByText("격리된 곳에서 코드 고치기")).toBeTruthy();
  });

  it("card 2 ducks are awake (not idle, eyes open) and large enough for the role item to read", () => {
    const { container } = render(<TourDemo id="roles" />);
    const list = ducks(container);
    expect(list).toHaveLength(5);
    for (const d of list) {
      expect(d.dataset.pose, d.getAttribute("aria-label")!).not.toBe("idle");
      expect(["working", "waiting", "done"]).toContain(d.dataset.pose);
      expect(d.querySelector(".duck-z"), "no sleeping z").toBeNull();
      expect(Number(d.getAttribute("width"))).toBeGreaterThanOrEqual(56);
    }
  });

  it("card 3 shows one question card; the picked answer fills the brief's 완료 기준 line, and the role line matches card 2", () => {
    const { container } = render(<TourDemo id="brief" />);
    expect(screen.getAllByText("추천")).toHaveLength(1);
    const picked = screen.getAllByText("같은 테스트 20번 연속 통과");
    expect(picked).toHaveLength(2);
    expect(picked[0].closest(".border-ink")).toBeTruthy();
    expect(container.querySelector("[data-brief-line]")?.textContent).toContain("완료 기준: 같은 테스트 20번 연속 통과");
    expect(container.textContent).toContain(`개발 담당 · ${ROLE_STYLE.dev}`);
    expect(container.textContent).not.toContain("계획만 세우기");
  });

  it("card 4 tells who is doing what and who waits for an answer, the waiting row with the board's yellow ring", () => {
    const { container } = render(<TourDemo id="board" />);
    expect(screen.getByText("일하는 중")).toBeTruthy();
    expect(screen.getByText("답을 기다려요")).toBeTruthy();
    const waiting = screen.getByText("답을 기다려요").closest("li")!;
    expect(waiting.className).toContain("ring-duck");
    expect(container.querySelectorAll('[class*="ring-duck"]')).toHaveLength(1);
    expect(container.querySelectorAll(`[class*="inset_3px"]`)).toHaveLength(0);
  });

  it("card 5 reads in the report's order: 맡긴 내용, then the criteria (2 ✓ and 1 △), then the caution", () => {
    const { container } = render(<TourDemo id="report" />);
    const order = [...container.querySelectorAll("[data-report-section]")].map((e) => (e as HTMLElement).dataset.reportSection);
    expect(order).toEqual(["delegated", "criteria", "caution", "verify"]);
    // The supporting line promises a second pair of eyes: the review duck's verdict is on the demo.
    expect(container.querySelector('[data-report-section="verify"]')?.textContent).toBe("리뷰다른 눈으로 본 판정: 통과 · 1개 확인 필요");
    expect(REPORT_CRITERIA.map((c) => c.met)).toEqual(["yes", "yes", "partial"]);
    expect(screen.getAllByText("✓")).toHaveLength(2);
    expect(screen.queryAllByText("✕")).toHaveLength(0);
    expect(screen.getByText("기준 3개 중 2개 충족")).toBeTruthy();
    expect(screen.getByText("묻지 않고 정함")).toBeTruthy();
  });

  it("cards 3 and 6 offer the same five types as card 2's roles", () => {
    for (const id of ["brief", "start"] as const) {
      const { container, unmount } = render(<TourDemo id={id} />);
      const chips = [...container.querySelectorAll("span > span.inline-flex.h-full")].map((e) => e.textContent);
      expect(chips, id).toEqual(["설계", "디자인", "개발", "리뷰", "QA"]);
      unmount();
    }
  });

  it("the action card has a waving duck that has no role yet", () => {
    const { container } = render(<TourDemo id="start" />);
    const list = ducks(container);
    expect(list).toHaveLength(1);
    expect(list[0].dataset.pose).toBe("waiting");
    expect(screen.getByText("역할 고르기 전")).toBeTruthy();
  });

  it("shows real-size cut-outs (no scaled-down pages, ducks 36px or larger) and moves in one demo only", () => {
    expect(demoSource).not.toMatch(/scale\(|scale-\[|\bscale-\d/);
    for (const c of TOUR_CARDS) {
      const { container, unmount } = render(<TourDemo id={c.id} />);
      for (const d of ducks(container)) expect(Number(d.getAttribute("width")), c.id).toBeGreaterThanOrEqual(36);
      const still = container.querySelector(".tour-still");
      expect(!!still, c.id).toBe(c.id !== "board");
      unmount();
    }
  });
});
