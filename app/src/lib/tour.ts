// First-run tour, version 2 (tour-v2, D-113·D-116): a short story instead of a tab-by-tab list. Ducks are agents with
// roles, never mascots: five cards (one session for everything → ducks with roles → typed brief and refine → who is
// doing what → each duck's report), then the "first task" action. Card copy lives in the `tour` catalog: one key line
// + one supporting line. No remote images or links here (tour.test.ts).
import { t } from "../i18n";
import type { OnboardingState } from "./api";
import type { Draft } from "./brief";
import type { NavTarget as Page } from "../components/Sidebar"; // old ids are normalized by App (nav-ia)

/** Bump when cards change; new cards carry the version that introduced them in `since`. Rust mirrors it. */
export const TOUR_VERSION = 2;

/** Version 2 ids, in order. Rust also accepts the version 1 ids so older stored states stay valid. */
export type CardId = "solo" | "roles" | "brief" | "board" | "report" | "start";

export interface TourCard {
  id: CardId;
  since: number;
  title: string;
  /** One supporting line. */
  body: string;
  /** Three key points, one short line each (D-137). */
  points: string[];
  /** Read by screen readers in place of the demo (the demo itself is aria-hidden). */
  demoAlt: string;
}

/** Card text is read from the `tour` catalog in the current language each time (getters), never frozen at load. */
function card(id: CardId): TourCard {
  return {
    id,
    since: 2,
    get title() {
      return t(`tour.cards.${id}.title`);
    },
    get body() {
      return t(`tour.cards.${id}.body`);
    },
    get points() {
      return [t(`tour.cards.${id}.point1`), t(`tour.cards.${id}.point2`), t(`tour.cards.${id}.point3`)];
    },
    get demoAlt() {
      return t(`tour.cards.${id}.demoAlt`);
    },
  };
}

export const TOUR_CARDS: TourCard[] = (["solo", "roles", "brief", "board", "report", "start"] as const).map(card);

/** How each role works, shown on card 2 and card 3 (tour-v2 test review). Display only: the real permission presets
 *  stay as they are (dev keeps plan mode as a safety default); these lines describe the role, not its permission. */
export type DemoRole = "architecture" | "design" | "dev" | "review" | "qa";
export const ROLE_STYLE: Readonly<Record<DemoRole, string>> = Object.freeze({
  get architecture() {
    return t("tour.roleStyle.architecture");
  },
  get design() {
    return t("tour.roleStyle.design");
  },
  get dev() {
    return t("tour.roleStyle.dev");
  },
  get review() {
    return t("tour.roleStyle.review");
  },
  get qa() {
    return t("tour.roleStyle.qa");
  },
});

/** Cards counted in "n/5"; the last one is the action. */
export const STORY_CARDS = TOUR_CARDS.filter((c) => c.id !== "start").length;

/** 구성 links on the action card, one line. */
export const SETUP_LINKS: { page: Page; readonly label: string }[] = (["extensions", "mcp", "permissions", "memory"] as const).map((page) => ({
  page,
  get label() {
    return t(`tour.setup.${page}`);
  },
}));

/** Goal examples on the composer the action card opens, in the current language. Picking one only fills the goal;
 *  nothing runs. */
export function tourGoalExamples(): string[] {
  return [t("tour.goalExamples.fixTest"), t("tour.goalExamples.readme"), t("tour.goalExamples.structure")];
}

/** The composer draft the action card opens: empty goal, example chips, the refine button highlighted. */
export function tourDraft(cwd: string): Draft {
  return { cwd, taskType: null, brief: { goal: "", outOfScope: "", constraints: "", done: "" }, refineHint: true, goalExamples: tourGoalExamples(), fromTour: true };
}

/** Every stored key (v1 keys still tick, they are just not listed). */
export type ChecklistKey = keyof OnboardingState["checklist"];
export type ChecklistId = "firstTask" | "refineUsed" | "reportOpened";

export const CHECKLIST: { id: ChecklistId; readonly label: string; page: Page }[] = (["firstTask", "refineUsed", "reportOpened"] as const).map((id) => ({
  id,
  page: "tasks",
  get label() {
    return t(`tour.checklist.${id}`);
  },
}));

export function emptyState(): OnboardingState {
  return { version: TOUR_VERSION, seen: [], dismissed: false, checklist: { firstTask: false, timelineOpen: false, notifyOn: false, refineUsed: false, reportOpened: false } };
}

/** A stored state from a build before tour-v2 has no v2 keys; fill them (Rust does the same with serde defaults). */
export function migrate(state: OnboardingState | null): OnboardingState | null {
  if (!state) return null;
  return { ...state, checklist: { ...emptyState().checklist, ...state.checklist } };
}

/** Someone who went through an older tour: a one-time toast offers the new deck; it never opens by itself. */
export function wantsTourToast(state: OnboardingState | null, cards: TourCard[] = TOUR_CARDS): boolean {
  if (!state || state.seen.length === 0 || state.version >= TOUR_VERSION) return false;
  return cards.some((c) => !state.seen.includes(c.id));
}

/** Fresh install (nothing seen yet) = every card. An older-tour user = none (the toast offers them). Later, only the
 *  story cards added since (the action card is not shown alone). */
export function pendingCards(state: OnboardingState | null, cards: TourCard[] = TOUR_CARDS): TourCard[] {
  if (!state || state.seen.length === 0) return cards;
  if (state.version < TOUR_VERSION) return [];
  return cards.filter((c) => c.id !== "start" && !state.seen.includes(c.id));
}

/** State after closing the deck: every current card counts as seen, whether finished or dismissed. */
export function closedState(state: OnboardingState | null, how: "done" | "dismissed", cards: TourCard[] = TOUR_CARDS): OnboardingState {
  const base = migrate(state) ?? emptyState();
  return { ...base, version: TOUR_VERSION, seen: [...new Set([...base.seen, ...cards.map((c) => c.id)])], dismissed: how === "dismissed" };
}

export function checklistVisible(state: OnboardingState | null | undefined): boolean {
  if (!state || state.seen.length === 0) return false;
  return !CHECKLIST.every((c) => state.checklist[c.id]);
}
