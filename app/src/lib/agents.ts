// Display rules for agent cards: poses, the one-line status under the duck, task types and persona options.
import type { Activity, ActState, AgentDef, AgentSource } from "./api";
import { t, translate } from "../i18n";
import { liveLabels, typeLabel, type TeamType } from "./team";

export type Pose = "working" | "waiting" | "failed" | "done" | "idle";

export const POSE_OF: Record<ActState, Pose> = { working: "working", waiting: "waiting", failed: "failed", done: "done" };

const stateText = (p: Pose) => t(`agents.status.${p}`);

/** "일하는 중 · 설계 비판 검토", "일하는 중 2건 · …", or "쉬는 중". */
export function statusLine(a: Activity | undefined): { pose: Pose; head: string; detail: string | null } {
  if (!a) return { pose: "idle", head: stateText("idle"), detail: null };
  const pose = POSE_OF[a.state];
  const head = a.count > 1 ? t("agents.status.many", { state: stateText(pose), count: a.count }) : stateText(pose);
  // Other items in calmer states are counted, not described, so the line never mixes two meanings.
  const others = a.total - a.count;
  const detail = [a.detail, others > 0 ? t("agents.status.others", { count: others }) : null].filter(Boolean).join(" · ") || null;
  return { pose, head, detail };
}

export const SOURCE_LABEL: Record<AgentSource, string> = liveLabels(["project", "user", "plugin", "builtin"], (s) => t(`agents.source.${s}`));

export function sourceText(d: AgentDef): string {
  const scope = d.scope ? d.scope.split("/").filter(Boolean).pop() : null;
  return scope ? `${SOURCE_LABEL[d.source]} · ${scope}` : SOURCE_LABEL[d.source];
}

/** Task type label from the team config; recorded keys missing from it show "삭제된 유형" (D-081). */
export function taskTypeLabel(id: string | null | undefined, types?: TeamType[]): string | null {
  const label = typeLabel(types, id);
  // A bundled type still carrying its bundled name (in either language) follows the app language; a renamed one is
  // the user's text and stays as written (D-171, palette-refresh 2.3).
  if (id && label && Object.prototype.hasOwnProperty.call(TYPE_NAME_KEYS, id)) {
    const key = `agents.typeNames.${id as keyof typeof TYPE_NAME_KEYS}` as const;
    if (label === translate("ko", key) || label === translate("en", key)) return t(key);
  }
  return label;
}

const TYPE_NAME_KEYS = { dev: 1, design: 1, architecture: 1, security: 1, deploy: 1, research: 1, comms: 1, review: 1, qa: 1, learn: 1 } as const;

const ORDER: AgentSource[] = ["project", "user", "plugin", "builtin"];

/** One persona per name among the definitions the backend marked usable for this folder (canonical paths, D-057). */
export function personasFor(defs: AgentDef[]): AgentDef[] {
  const usable = defs.filter((d) => d.usable);
  const byName = new Map<string, AgentDef>();
  for (const d of usable) {
    const cur = byName.get(d.name);
    if (!cur || ORDER.indexOf(d.source) < ORDER.indexOf(cur.source) || (d.source === cur.source && (d.scope?.length ?? 0) > (cur.scope?.length ?? 0))) byName.set(d.name, d);
  }
  return [...byName.values()].sort((a, b) => ORDER.indexOf(a.source) - ORDER.indexOf(b.source) || a.name.localeCompare(b.name));
}

export function hasRisk(d: AgentDef): boolean {
  return d.risk.hooks || d.risk.mcpServers || d.risk.dangerMode || d.risk.unreadable;
}
