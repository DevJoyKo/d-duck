import { describe, expect, it } from "vitest";
import type { OnboardingState } from "./api";
import { checklistVisible, closedState, emptyState, migrate, pendingCards, STORY_CARDS, TOUR_CARDS, tourDraft, TOUR_VERSION, wantsTourToast } from "./tour";

const mockSources = import.meta.glob("../mock/**/*.{ts,tsx,json}", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const tourSource = import.meta.glob("./tour.ts", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const demoSource = import.meta.glob("../components/Tour*.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const rustSource = import.meta.glob("../../crates/dduck-core/src/settings.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

const V1 = ["welcome", "agents", "tasks", "outputs", "extend", "permissions", "memory", "usage", "finish"];
const v1State = (): OnboardingState => ({ version: 1, seen: V1, dismissed: false, checklist: { firstTask: true, timelineOpen: true, notifyOn: true } as OnboardingState["checklist"] });
const seen = (ids: string[]): OnboardingState => ({ ...emptyState(), seen: ids });

describe("tour cards", () => {
  it("is five story cards and the action card, ids matching the Rust allowlist", () => {
    expect(TOUR_CARDS.map((c) => c.id)).toEqual(["solo", "roles", "brief", "board", "report", "start"]);
    expect(STORY_CARDS).toBe(5);
    expect(TOUR_VERSION).toBe(2);
    const rust = Object.values(rustSource)[0];
    expect(rust).toContain(`pub const TOUR_CARDS: &[&str] = &[${TOUR_CARDS.map((c) => `"${c.id}"`).join(", ")}];`);
    expect(rust).toContain(`pub const TOUR_VERSION: u32 = ${TOUR_VERSION};`);
    for (const id of V1) expect(rust).toContain(`"${id}"`);
  });

  it("keeps each card to one key line and one supporting line, with a text alternative for the demo", () => {
    for (const c of TOUR_CARDS) {
      expect(c.title.length, c.id).toBeLessThanOrEqual(26);
      expect(c.body.split(/[.?!]\s/).length, c.id).toBe(1);
      expect(c.title + c.body).not.toMatch(/\n/);
      expect(c.demoAlt.length).toBeGreaterThan(10);
    }
  });

  it("fresh install shows every card; an update without new cards shows none; a new card shows only itself", () => {
    expect(pendingCards(null)).toHaveLength(6);
    expect(pendingCards(emptyState())).toHaveLength(6);
    expect(pendingCards(closedState(null, "done"))).toEqual([]);
    expect(pendingCards(seen(TOUR_CARDS.map((c) => c.id).filter((id) => id !== "board"))).map((c) => c.id)).toEqual(["board"]);
  });

  it("a version 1 user gets a one-time toast, never the deck by itself", () => {
    const old = migrate(v1State());
    expect(old?.checklist).toEqual({ firstTask: true, timelineOpen: true, notifyOn: true, refineUsed: false, reportOpened: false });
    expect(wantsTourToast(old)).toBe(true);
    expect(pendingCards(old)).toEqual([]);
    // What the store saves once the toast was shown: no second toast, no deck.
    const after = closedState(old, "done");
    expect(after.version).toBe(2);
    expect(wantsTourToast(after)).toBe(false);
    expect(pendingCards(after)).toEqual([]);
    expect(after.seen).toEqual(expect.arrayContaining([...V1, "solo", "start"]));
    expect(wantsTourToast(null)).toBe(false);
    expect(wantsTourToast(emptyState())).toBe(false);
  });

  it("dismissing still counts every card as seen, keeps the checklist; the list is the v2 one", () => {
    const s = closedState({ ...emptyState(), checklist: { ...emptyState().checklist, firstTask: true } }, "dismissed");
    expect(s.dismissed).toBe(true);
    expect(s.checklist.firstTask).toBe(true);
    expect(checklistVisible(s)).toBe(true);
    // v1 items done do not hide the v2 list.
    expect(checklistVisible({ ...s, checklist: { ...s.checklist, timelineOpen: true, notifyOn: true } })).toBe(true);
    expect(checklistVisible({ ...s, checklist: { ...s.checklist, refineUsed: true, reportOpened: true } })).toBe(false);
    expect(checklistVisible(null)).toBe(false);
  });

  it("the action card's draft has an empty goal, example chips and the refine hint, and no origin", () => {
    const d = tourDraft("/p");
    expect(d.brief.goal).toBe("");
    expect(d.goalExamples?.length).toBeGreaterThanOrEqual(2);
    expect(d.refineHint && d.fromTour).toBe(true);
    expect(d.origin).toBeUndefined();
  });
});

describe("static checks", () => {
  it("tour content has no http(s) URLs", () => {
    const sources = [...Object.values(tourSource), ...Object.values(demoSource)];
    expect(sources.length).toBeGreaterThanOrEqual(3);
    for (const src of sources) expect(src).not.toMatch(/https?:\/\//);
  });

  it("mock data holds no personal items", () => {
    const files = Object.entries(mockSources);
    expect(files.length).toBeGreaterThan(3);
    // Only the placeholder home folders `/Users/me` and `/Users/x` may appear; no real e-mail addresses.
    const banned = /\/Users\/(?!(me|x)\b)\w|[\w.]+@[\w-]+\.(com|net|kr)/;
    for (const [name, src] of files) expect(src.match(banned)?.[0], name).toBeUndefined();
  });
});

describe("tour points (D-137)", () => {
  it("every card has three short one-line points", () => {
    for (const c of TOUR_CARDS) {
      expect(c.points).toHaveLength(3);
      for (const p of c.points) {
        expect(p.length).toBeLessThanOrEqual(26);
        expect(p).not.toMatch(/[.。]$/);
      }
    }
  });
});
