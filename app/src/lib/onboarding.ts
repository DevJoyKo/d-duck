// Tour state shared by the app shell, the checklist and the places that tick it (onboarding-tour).
// Loaded once from the dedicated settings command; changes are written back whole (booleans and card ids only).
import { useSyncExternalStore } from "react";
import { api, type OnboardingState } from "./api";
import { closedState, emptyState, migrate, pendingCards, TOUR_CARDS, wantsTourToast, type ChecklistKey, type TourCard } from "./tour";

interface Store {
  /** undefined = not loaded yet, null = fresh install. */
  state: OnboardingState | null | undefined;
  /** Cards on screen, or null when the deck is closed. */
  deck: TourCard[] | null;
  /** "새로워진 안내 보기" toast for someone who saw an older tour (shown once, never the deck by itself). */
  toast: boolean;
}

let store: Store = { state: undefined, deck: null, toast: false };
let loading: Promise<void> | null = null;
const listeners = new Set<() => void>();
/** What had focus before the deck opened. Taken before the deck renders, because the shell turns inert then. */
let returnFocus: HTMLElement | null = null;

function rememberFocus() {
  const at = document.activeElement;
  returnFocus = at instanceof HTMLElement && at !== document.body ? at : null;
}

/** The deck calls this when it unmounts. Deferred so the shell's inert is gone first, and skipped while the deck is
 *  still open (React StrictMode runs a mount/unmount/mount cycle in development). */
export function restoreFocus() {
  setTimeout(() => {
    if (store.deck) return;
    const el = returnFocus;
    returnFocus = null;
    if (el?.isConnected) el.focus();
  }, 0);
}

function set(next: Partial<Store>) {
  store = { ...store, ...next };
  listeners.forEach((l) => l());
}

function save(state: OnboardingState) {
  set({ state });
  // Best effort: if saving fails the tour may show once more, which is harmless.
  api.setOnboardingState(state).catch(() => undefined);
}

/** Loads the state once and opens the deck if there are cards to show. */
export function loadOnboarding(): Promise<void> {
  loading ??= api
    .onboardingState()
    .then((raw) => {
      const state = migrate(raw ?? null);
      if (state && wantsTourToast(state)) {
        // Once: the new cards count as seen right away, so the deck never opens by itself and the toast never repeats.
        set({ toast: true, deck: null });
        save({ ...closedState(state, "done"), dismissed: state.dismissed });
        return;
      }
      const cards = pendingCards(state);
      if (cards.length) rememberFocus();
      set({ state, deck: cards.length ? cards : null });
    })
    .catch(() => set({ state: null, deck: null }));
  return loading;
}

/** Replay from Settings, the sidebar "?" or the toast: every card. */
export function openTour() {
  rememberFocus();
  set({ deck: TOUR_CARDS, toast: false });
}

export function dismissToast() {
  set({ toast: false });
}

export function closeTour(how: "done" | "dismissed") {
  if (!store.deck) return;
  const state = closedState(store.state ?? null, how);
  set({ deck: null });
  save(state);
}

/** Ticks one checklist item after the user really did it. Stores only the boolean. */
export async function markDone(id: ChecklistKey) {
  await loadOnboarding();
  const state = store.state ?? emptyState();
  if (state.checklist[id]) return;
  save({ ...state, checklist: { ...state.checklist, [id]: true } });
}

export function useOnboarding(): Store {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => store,
  );
}

/** Tests only. */
export function resetOnboardingForTests() {
  store = { state: undefined, deck: null, toast: false };
  loading = null;
  returnFocus = null;
}
