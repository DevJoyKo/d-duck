// Editable team rules, task types and agent on/off (team-presets-editable, D-081).
// The backend owns every default and every safety check; this file only shapes data for the UI.
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { t as tr } from "../i18n";
import type { AgentDef } from "./api";

/** A label record read at access time, so it follows the current language (never frozen at module load). */
export function liveLabels<K extends string>(keys: readonly K[], text: (k: K) => string): Record<K, string> {
  const out = {} as Record<K, string>;
  for (const k of keys) Object.defineProperty(out, k, { get: () => text(k), enumerable: true });
  return out;
}

export interface SkillPin {
  name: string;
  /** `user` · `project` · `plugin:<name>` */
  source: string;
  hash: string;
}

export interface TeamRule {
  id: string;
  text: string;
  extra: string;
  locked: boolean;
  bundled: boolean;
  defaultText: string | null;
  changed: boolean;
  defaultChanged: boolean;
}

export interface TeamType {
  key: string;
  label: string;
  role: string;
  roleLabel: string;
  agent: string | null;
  rolePrompt: string;
  /** `null` = the role inherits every tool. */
  roleTools: string[] | null;
  mode: string;
  modes: string[];
  effort: string;
  review: boolean;
  strict: boolean;
  hidden: boolean;
  autoAllowed: string[];
  note: string;
  /** What "요구사항 다듬기" should probe for this type (≤ 300 characters). */
  refineFocus: string;
  /** "기본 장비" skills, pinned by source and SKILL.md hash (agent-loadout). */
  skills: SkillPin[];
  /** "기본 장비" MCP servers: only these are used. Empty = every server (or none with a tool list). */
  mcpAllow: string[];
  bundled: boolean;
  changed: boolean;
  defaultChanged: boolean;
  fixedStrict: boolean;
  /** Rust-owned "[필수]" evidence rule (review, QA): shown read-only, never saved. */
  lockedRule?: string | null;
  /** "free" types may hand in attachments only (D-156). */
  reportShape?: "free" | "criteria";
}

export interface RefineSettings {
  guidance: string;
  maxQuestions: number;
  rounds: number;
}

export interface TeamConfig {
  header: string;
  rules: TeamRule[];
  types: TeamType[];
  refine: RefineSettings;
  errors: string[];
}

export interface TeamConfigDto {
  config: TeamConfig;
  defaults: TeamConfig;
  usedTypes: string[];
  autoAllowed: string[];
}

export interface AgentControls {
  userOff: string[];
  projectOff: [string, string[]][];
  elsewhereOff: string[];
  hidden: string[];
}

export interface TrashResult {
  name: string;
  project: string | null;
  off: boolean;
  hidden: boolean;
}

export const teamApi = {
  config: () => invoke<TeamConfigDto>("team_config"),
  save: (input: { rules: { id: string; text: string; extra: string }[]; types: TeamType[]; refine: RefineSettings }) => invoke<void>("save_team_config", { input }),
  reset: (item: string | null) => invoke<void>("reset_team_config", { item }),
  controls: () => invoke<AgentControls>("agent_controls"),
  setEnabled: (id: string, enabled: boolean) => invoke<string | null>("set_agent_enabled", { id, enabled }),
  setHidden: (name: string, hidden: boolean) => invoke<void>("set_agent_hidden", { name, hidden }),
  trash: (id: string) => invoke<TrashResult>("trash_agent", { id }),
  forget: (name: string, project: string | null) => invoke<void>("forget_agent", { name, project }),
};

export function useTeamConfig() {
  return useQuery({ queryKey: ["team-config"], queryFn: teamApi.config, staleTime: 30_000 });
}

export function useAgentControls() {
  return useQuery({ queryKey: ["agent-controls"], queryFn: teamApi.controls, staleTime: 15_000 });
}

/** Label for a recorded type key; a key no longer in the config stays visible with "삭제된 유형". */
export function typeLabel(types: TeamType[] | undefined, key: string | null | undefined): string | null {
  if (!key) return null;
  if (!types) return key;
  return types.find((t) => t.key === key)?.label ?? tr("agents.deletedType", { key });
}

/** Counts toward review capacity. Without the config (loading, error) the bundled code-changing types count. */
export function isReviewType(types: TeamType[] | undefined, key: string | null | undefined): boolean {
  if (!key) return false;
  const t = types?.find((x) => x.key === key);
  return t ? t.review : key === "dev" || key === "deploy" || key === "design";
}

export type OffState = "off" | "elsewhere" | null;

const OPEN_KEY = "dduck.openAgentSettings";

/** The settings page asks the agents page to open its settings sheet once it mounts. */
export function requestAgentSettings() {
  try {
    sessionStorage.setItem(OPEN_KEY, "1");
  } catch {
    /* storage unavailable: the user opens the gear */
  }
}

export function takeAgentSettingsRequest(): boolean {
  try {
    const v = sessionStorage.getItem(OPEN_KEY) === "1";
    sessionStorage.removeItem(OPEN_KEY);
    return v;
  } catch {
    return false;
  }
}

const has = (list: string[], name: string) => list.includes(name) || list.includes("*");

/** "off" = this app can turn it back on (the exact rule in its own scope); "elsewhere" = denied by another file. */
export function offState(d: AgentDef, c: AgentControls | undefined): OffState {
  if (!c) return null;
  const own = d.source === "project" ? (c.projectOff.find(([p]) => p === d.scope)?.[1] ?? []) : c.userOff;
  if (own.includes(d.name)) return "off";
  // A global deny also blocks same-named project agents; a bare `Agent` rule ("*") blocks every agent.
  if (has(c.elsewhereOff, d.name) || has(own, d.name) || (d.source === "project" && has(c.userOff, d.name))) return "elsewhere";
  if (d.source !== "project" && c.projectOff.some(([, names]) => has(names, d.name))) return "elsewhere";
  return null;
}

/** Off for D.Duck: any deny anywhere refuses the start (the backend checks again). */
export function isOffAnywhere(name: string, c: AgentControls | undefined): boolean {
  if (!c) return false;
  return has(c.userOff, name) || has(c.elsewhereOff, name) || c.projectOff.some(([, n]) => has(n, name));
}

export const MODE_LABEL: Record<string, string> = liveLabels(["manual", "plan", "acceptEdits", "dontAsk", "auto", "bypassPermissions"] as const, (m) => tr(`agents.mode.${m}`));

export const EDITABLE_MODES = ["plan", "manual", "acceptEdits", "dontAsk"];
export const STRICT_MODES = ["plan", "manual"];
export const WRITE_TOOLS = ["Write", "Edit", "MultiEdit", "NotebookEdit", "Bash", "*"];
export const AUTO_ALLOWED_LABEL: Record<string, string> = liveLabels(["Read", "Glob", "Grep", "git-read"], (k) => (k === "git-read" ? tr("agents.autoAllowed.gitRead") : k));

/** MCP tools count as write tools, like in Rust (agent-loadout). */
export const isWriteTool = (rule: string) => WRITE_TOOLS.includes(rule.split("(")[0]) || rule.startsWith("mcp__");

/** An agent definition that can write or run commands (no `tools` = every tool). Strict types refuse these. */
export const writesFiles = (d: Pick<AgentDef, "tools">) => d.tools.length === 0 || d.tools.some(isWriteTool);

/** Client-side hints only; the backend validates again and decides what needs a confirmation. */
export function typeProblems(t: TeamType, bundled: TeamType | undefined, catalog?: AgentDef[]): string[] {
  const out: string[] = [];
  if (!t.label.trim() || [...t.label].length > 20) out.push(tr("agents.problems.label"));
  if (!/^[a-z][a-z0-9-]{1,23}$/.test(t.key)) out.push(tr("agents.problems.key"));
  if (!t.agent && !t.rolePrompt.trim()) out.push(tr("agents.problems.promptEmpty"));
  if (new TextEncoder().encode(t.rolePrompt).length > 8192) out.push(tr("agents.problems.promptSize"));
  if (!t.modes.includes(t.mode)) out.push(tr("agents.problems.modeMissing"));
  if (t.agent && t.fixedStrict) out.push(tr("agents.problems.fixedStrict"));
  if (t.agent && t.strict && catalog) {
    const d = catalog.find((x) => x.name === t.agent && x.primary);
    if (!d || writesFiles(d)) out.push(tr("agents.problems.strictWriter", { agent: t.agent }));
  }
  if (t.strict) {
    if (t.modes.some((m) => !STRICT_MODES.includes(m))) out.push(tr("agents.problems.strictModes"));
    if (!t.agent) {
      if (!t.roleTools || t.roleTools.length === 0) out.push(tr("agents.problems.strictNoTools"));
      const allowed = new Set((bundled?.roleTools ?? []).filter(isWriteTool));
      const extra = (t.roleTools ?? []).filter((x) => isWriteTool(x) && !allowed.has(x));
      if (extra.length) out.push(tr("agents.problems.strictExtra", { tools: extra.join(", ") }));
    }
  }
  return out;
}
