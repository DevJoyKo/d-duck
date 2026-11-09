// A duck is an agent with a role, never a mascot (D-116): wherever a task's duck is drawn it carries its role label in
// the role's fixed colour (roleColors.ts) and item (DuckAvatar). Display only; used by the report header and the tour.
import type { Pose } from "../lib/agents";
import { roleColor } from "../lib/roleColors";
import { DuckAvatar, type DuckColor } from "./DuckAvatar";
import { useT } from "../i18n/react";

/** The role label on the duck's body colour, with an ink outline so light colours hold on light sheets. */
export function RoleChip({ label, color }: { label: string; color: DuckColor }) {
  return (
    <span className="inline-block rounded-full px-2 py-px text-[10px] font-semibold whitespace-nowrap text-ink-fixed ring-1 ring-ink-fixed/25 ring-inset" style={{ background: color.body }}>
      {label}
    </span>
  );
}

/** 20px role duck at the start of a board card's first line; the title next to it names the card, so it is hidden. */
export function CardDuck({ taskType, pose }: { taskType: string | null | undefined; pose: Pose }) {
  return (
    <span aria-hidden className="shrink-0 rounded-full bg-duck-stage-quiet p-px">
      <DuckAvatar pose={pose} size={20} color={roleColor(taskType ?? null)} />
    </span>
  );
}

/** "역할 · 페르소나" for a card's second line; null when the task has neither. */
export function roleLine(role: string | null, persona: string | null | undefined): string | null {
  return [role, persona].filter(Boolean).join(" · ") || null;
}

/** Duck in its role colour on the stage circle, role chip underneath. `taskType` null = the plain duck (no role). */
export function RoleDuck({ taskType, label, pose, size = 48 }: { taskType: string | null; label: string; pose: Pose; size?: number }) {
  const t = useT();
  const color = roleColor(taskType);
  return (
    <span className="flex shrink-0 flex-col items-center gap-1">
      <span className="rounded-full bg-duck-stage-quiet p-0.5">
        <DuckAvatar pose={pose} size={size} color={color} label={t("agents.roles.duck", { role: label })} />
      </span>
      <RoleChip label={label} color={color} />
    </span>
  );
}
