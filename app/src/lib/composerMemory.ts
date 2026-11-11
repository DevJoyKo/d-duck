// The task composer's unsent input, kept in memory while the app runs (QA FAIL: a trip to Settings unmounted the
// tasks page and the typed brief was gone). Never written to disk or localStorage: it is a draft, not a record
// (the stored task prompt of D-069 is only what was actually started).
import type { Brief, EffortId, TaskTypeId } from "./brief";

export interface ComposerInput {
  cwd: string;
  taskType: TaskTypeId | null;
  brief: Brief;
  name: string;
  modeChoice: string | null;
  effortChoice: EffortId | null;
  sensitive: boolean;
  worktreeChoice: boolean | null;
  persona: string;
  remoteChoice: boolean | null;
  reportChoice: boolean | null;
  limitChoice: boolean | null;
  modelChoice?: string | null;
}

let kept: { key: string; input: ComposerInput } | null = null;

/** The input kept for this composer opening (`key` names the prefilled draft it was opened with). */
export function recallComposer(key: string): ComposerInput | null {
  return kept && kept.key === key ? kept.input : null;
}

export function keepComposer(key: string, input: ComposerInput) {
  kept = { key, input };
}

/** Closed on purpose or started: nothing to bring back. */
export function forgetComposer() {
  kept = null;
}
