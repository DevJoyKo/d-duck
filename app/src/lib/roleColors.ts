// One fixed duck colour per task type (D-116): the duck is an agent with a role, and the same role always wears the
// same colour, on the report, in the tour and anywhere else a task's duck is drawn. Colours come from DUCK_COLORS and
// match the agents page's role colours (roles.ts) where the two overlap.
import { DUCK_COLORS, PLAIN_DUCK, type DuckColor } from "../components/DuckAvatar";

/** Bundled task type keys (brief.ts) → index into DUCK_COLORS. Each bundled type has its own colour. */
export const ROLE_COLOR_INDEX: Record<string, number> = {
  security: 0, // coral
  dev: 1, // sky
  architecture: 2, // lime
  review: 3, // lavender
  deploy: 4, // orange
  research: 5, // teal
  comms: 6, // pink
  qa: 7, // mint
  design: 8, // lilac
  learn: 9, // slate
};

function hash(s: string): number {
  let h = 2166136261;
  for (const ch of s) h = Math.imul(h ^ ch.codePointAt(0)!, 16777619);
  return h >>> 0;
}

/** The duck colour for a task type. A custom type gets a stable colour from its key; no type is the plain duck. */
export function roleColor(taskType: string | null | undefined): DuckColor {
  if (!taskType) return PLAIN_DUCK;
  const i = ROLE_COLOR_INDEX[taskType];
  return DUCK_COLORS[i ?? hash(taskType) % DUCK_COLORS.length];
}
