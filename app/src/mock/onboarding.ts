// Mock tour state for design review. `?tour=first` = fresh install, `?tour=nocli` = fresh install without the CLI,
// `?tour=checklist` = tour seen with one checklist item done, `?tour=v1` = saw the version 1 tour (gets the one-time
// toast). Default: everything seen, so the tour stays out of the way.
const param = (() => {
  try {
    return new URLSearchParams(window.location.search).get("tour");
  } catch {
    return null;
  }
})();

export const mockNoCli = param === "nocli";

const V1 = ["welcome", "agents", "tasks", "outputs", "extend", "permissions", "memory", "usage", "finish"];
const ALL = [...V1, "solo", "roles", "brief", "board", "report", "start"];
const done = { firstTask: true, timelineOpen: true, notifyOn: true, refineUsed: true, reportOpened: true };
let state: unknown =
  param === "first" || param === "nocli"
    ? null
    : param === "v1"
      ? { version: 1, seen: V1, dismissed: false, checklist: { firstTask: true, timelineOpen: true, notifyOn: true } }
      : param === "checklist"
        ? { version: 2, seen: ALL, dismissed: false, checklist: { ...done, refineUsed: false, reportOpened: false } }
        : { version: 2, seen: ALL, dismissed: false, checklist: done };

export const onboardingHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  get_onboarding_state: () => state,
  set_onboarding_state: (a) => {
    state = JSON.parse(String(a.json));
    return undefined;
  },
};
