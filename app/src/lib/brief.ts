// Tech-lead workflow on the webview side (D-061, D-063).
// These are defaults and habits only; the backend re-checks everything that matters (AG-09).
import type { AgentSession, AgentsPayload, TaskOrigin } from "./api";
import { endedStatus, groupOf } from "./format";
import { isReviewType, type TeamType } from "./team";
import { t } from "../i18n";

/** Task type key from the editable team config (D-081); bundled keys are dev, design, architecture, security,
 * deploy, research, comms, review, qa, learn. */
export type TaskTypeId = string;
export type EffortId = "low" | "medium" | "high" | "xhigh" | "max";

// `label` is read in the current language on each access.
export const EFFORTS: { id: EffortId; label: string }[] = (["low", "medium", "high", "xhigh", "max"] as const).map((id) => ({
  id,
  get label() {
    return t(`tasks.composer.effort.${id}`);
  },
}));

export interface Brief {
  goal: string;
  outOfScope: string;
  constraints: string;
  done: string;
}

export const EMPTY_BRIEF: Brief = { goal: "", outOfScope: "", constraints: "", done: "" };

/** Same headings as dduck-core team::compose_brief. Empty sections are left out. The headings are part of the prompt
 * contract (parseBrief, the backend), so they stay Korean in both languages until the prompt step of app-language. */
export function composeBrief(b: Brief): string {
  const parts: [string, string][] = [
    ["## 목표", b.goal],
    ["## 범위 밖 (손대지 말 것)", b.outOfScope],
    ["## 제약", b.constraints],
    ["## 완료 기준", b.done],
  ];
  return parts
    .filter(([, v]) => v.trim())
    .map(([h, v]) => `${h}\n${v.trim()}`)
    .join("\n\n");
}

/** What the composer opens with (follow-up checks, "다시 맡기기"). */
export interface Draft {
  cwd: string;
  taskType: TaskTypeId | null;
  brief: Brief;
  name?: string;
  /** Which task this follows up, shown as a badge in the composer. */
  from?: string;
  /** Drafts from usage insights ("작업으로 만들기"): shown as a source badge, enforced by the backend. */
  origin?: TaskOrigin;
  /** Offer "질문 받기로 다듬기" prominently (never runs by itself). */
  refineHint?: boolean;
  /** Preset permission mode (settings-type insights start in plan). */
  permissionMode?: string;
  /** Local names in the draft's evidence; the refiner offers to remove them before sending. */
  localNames?: string[];
  /** Shown under the folder field when the draft leaves the folder empty. */
  cwdHint?: string;
  /** Goal example chips above the goal (tour action card). Picking one fills the goal only. */
  goalExamples?: string[];
  /** Opened from the tour's last card: the refine banner explains the next step instead of an insight source. */
  fromTour?: boolean;
  /** Claude's advice text, shown read-only above the goal (it also stays quoted in the constraints). */
  advice?: string;
}

// Control, zero-width and bidi characters out; at most 64 characters (session names come from CLI output).
// eslint-disable-next-line no-control-regex -- stripping control characters is the point
export const UNSAFE = new RegExp("[\\u0000-\\u001f\\u007f-\\u009f\\u200b-\\u200f\\u202a-\\u202e\\u2066-\\u2069]", "g");
export const cleanName = (s: string) => [...s.replace(UNSAFE, "")].slice(0, 64).join("");

/** "QA 맡기기" / "리뷰 맡기기": a fresh session in the dev session's real folder, worktree off (AG-14). */
export function followUpDraft(s: AgentSession, kind: "qa" | "review"): Draft {
  const name = cleanName(s.name || s.key.slice(0, 8));
  return {
    from: name,
    cwd: s.cwd,
    taskType: kind,
    name: `${kind}-${name}`.slice(0, 64),
    brief:
      kind === "qa"
        ? {
            goal: t("tasks.followUp.qaBrief.goal", { name }),
            outOfScope: t("tasks.followUp.qaBrief.outOfScope"),
            constraints: t("tasks.followUp.qaBrief.constraints"),
            done: t("tasks.followUp.qaBrief.done"),
          }
        : {
            goal: t("tasks.followUp.reviewBrief.goal", { name }),
            outOfScope: t("tasks.followUp.reviewBrief.outOfScope"),
            // Dev sessions commit on their work branch, so plain `git diff` may be empty (design review MAJOR 5).
            constraints: t("tasks.followUp.reviewBrief.constraints"),
            done: t("tasks.followUp.reviewBrief.done"),
          },
  };
}

/** Review capacity from status: a number, or "unlimited" (no warning anywhere). */
export type ReviewCapacity = number | "unlimited";

/** True when another review-capacity task would go over the user's capacity. Never true for "unlimited". */
export function overCapacity(load: number, capacity: ReviewCapacity | undefined): boolean {
  return capacity !== "unlimited" && load >= (capacity ?? 3);
}

/** D.Duck tasks of review-capacity types not yet marked "리뷰했어요", including finished ones (AG-14). */
export function reviewLoad(agents: AgentsPayload | undefined, types?: TeamType[]): number {
  const tags = agents?.tags ?? {};
  return (agents?.sessions ?? []).filter((s) => {
    const t = tags[s.key];
    return !!t && isReviewType(types, t.taskType) && !t.reviewed && s.status !== "stopped";
  }).length;
}

/** 확인 필요: waiting for an answer, or failed without a report to review (the tray's number too). */
export function blockedCount(agents: AgentsPayload | undefined): number {
  return (agents?.sessions ?? []).filter((s) => groupOf(s.status, agents?.tags?.[s.key]?.lane) === "review").length;
}

/** "리뷰했어요" applies to every review-capacity task, whatever its state (AG-14). */
export function canMarkReviewed(taskType: string | null | undefined, types?: TeamType[]): boolean {
  return isReviewType(types, taskType);
}

/** QA·리뷰 맡기기 once a dev task is over: 확인 필요 (failed), 리뷰 대기 and 정리 (task-lifecycle). */
export function canFollowUp(s: AgentSession, taskType: string | null | undefined): boolean {
  return taskType === "dev" && endedStatus(s.status);
}

const HEADINGS: [keyof Brief, string][] = [
  ["goal", "## 목표"],
  ["outOfScope", "## 범위 밖 (손대지 말 것)"],
  ["constraints", "## 제약"],
  ["done", "## 완료 기준"],
];

/** Inverse of composeBrief; a prompt without the headings becomes the goal ("이 브리프로 다시 맡기기"). */
export function parseBrief(prompt: string): Brief {
  const out: Brief = { ...EMPTY_BRIEF };
  if (!prompt.includes("## 목표")) return { ...out, goal: prompt.trim() };
  let current: keyof Brief | null = null;
  let found = false;
  const buf: Record<keyof Brief, string[]> = { goal: [], outOfScope: [], constraints: [], done: [] };
  for (const line of prompt.split("\n")) {
    const h = HEADINGS.find(([, t]) => line.trim() === t);
    if (h) {
      current = h[0];
      found = true;
    } else if (current) buf[current].push(line);
  }
  // No heading line after all (the text merely mentions one): keep the whole prompt as the goal.
  if (!found) return { ...out, goal: prompt.trim() };
  for (const [k] of HEADINGS) out[k] = buf[k].join("\n").trim();
  return out;
}
