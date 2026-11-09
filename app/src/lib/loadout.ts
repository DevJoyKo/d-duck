// "기본 장비" (agent-loadout, D-120): skills a role preloads and the MCP servers it may use. Rust pins skills by
// source and SKILL.md hash, checks them again at every start and turns "only the chosen servers" into
// `disallowedTools: mcp__<server>__*`. This file only shapes data for the picker.
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AgentDef, McpServer } from "./api";
import { currentLang, t } from "../i18n";
import { liveLabels, type SkillPin, type TeamType } from "./team";

export const MAX_SKILLS = 12;
export const MAX_MCP = 8;

export interface SkillOption {
  name: string;
  /** `user` · `project` · `plugin:<name>` */
  source: string;
  plugin: string | null;
  hash: string;
  bytes: number;
  description: string | null;
}

export interface LoadoutOptions {
  skills: SkillOption[];
  /** Names with a definition Rust could not read: a pin of that name counts as changed. */
  unreadableSkills?: string[];
  mcp: McpServer[];
  /** Settings files with MCP servers that could not be read. */
  unreadable: string[];
}

export const loadoutApi = {
  options: () => invoke<LoadoutOptions>("loadout_options"),
  saveAgent: (id: string, project: string | null, skills: string[], tools: string[], disallowed: string[], token: string) => invoke<void>("save_agent_loadout", { id, project, skills, tools, disallowed, token }),
};

export function useLoadoutOptions(enabled = true) {
  return useQuery({ queryKey: ["loadout-options"], queryFn: loadoutApi.options, staleTime: 30_000, enabled });
}

/** Source badge text: 전역 · 프로젝트 · 플러그인 이름. */
export function sourceLabel(source: string): string {
  if (source === "user") return t("agents.loadout.sourceUser");
  if (source === "project") return t("agents.loadout.sourceProject");
  return source.replace(/^plugin:/, "");
}

/** Same labels as the MCP tab's scope line. */
export const MCP_SCOPE_LABEL: Record<string, string> = liveLabels(["user", "local", "project"] as const, (k) => t(k === "user" ? "agents.loadout.scopeUser" : k === "local" ? "agents.loadout.scopeLocal" : "agents.loadout.scopeProject"));

/** Above this many tokens the preload eats noticeably into the context window. */
export const HEAVY_TOKENS = 20_000;

/** SKILL.md bytes → "약 N천 토큰" (about 4 bytes a token). 0 → "" (hidden), under 1천 → "1천 토큰 미만". */
export function tokenText(bytes: number): string {
  if (bytes <= 0) return "";
  const tokens = bytes / 4;
  if (tokens < 1000) return t("agents.loadout.tokensSmall");
  const k = Math.round(tokens / 1000);
  // Korean counts in 천·만; English in thousands ("12k").
  const amount = currentLang() === "en" ? `${k.toLocaleString("en")}k` : k < 10 ? `${k}천` : `${Math.floor(k / 10).toLocaleString()}만${k % 10 ? ` ${k % 10}천` : ""}`;
  return t("agents.loadout.tokensAbout", { amount });
}

export const heavyTokens = (bytes: number) => bytes / 4 > HEAVY_TOKENS;

export const pinOf = (o: SkillOption): SkillPin => ({ name: o.name, source: o.source, hash: o.hash });

/** "changed" (바뀜): same name and source but other content, or a same-name definition Rust could not read.
 * "missing" (없음): no definition of that name and source. */
export function pinState(p: SkillPin, options: SkillOption[], unreadable: string[] = []): "ok" | "changed" | "missing" {
  const same = options.filter((o) => o.name === p.name);
  if (unreadable.includes(p.name)) return "changed";
  if (same.some((o) => o.source === p.source && o.hash === p.hash)) return "ok";
  return same.some((o) => o.source === p.source) ? "changed" : "missing";
}

/** Total preload size of the pins that still match. */
export function pinnedBytes(pins: SkillPin[], options: SkillOption[]): number {
  return pins.reduce((n, p) => n + (options.find((o) => o.name === p.name && o.source === p.source && o.hash === p.hash)?.bytes ?? 0), 0);
}

/** Skills grouped by plugin for "플러그인 묶음" (skills only: a plugin's MCP is on whenever the plugin is). */
export function pluginBundles(options: SkillOption[]): { plugin: string; skills: SkillOption[] }[] {
  const out = new Map<string, SkillOption[]>();
  for (const o of options) if (o.plugin) out.set(o.plugin, [...(out.get(o.plugin) ?? []), o]);
  return [...out.entries()].map(([plugin, skills]) => ({ plugin, skills }));
}

/** Adds options to the pins (one per name, up to MAX_SKILLS). */
export function addPins(pins: SkillPin[], add: SkillOption[]): SkillPin[] {
  const out = [...pins];
  for (const o of add) if (out.length < MAX_SKILLS && !out.some((p) => p.name === o.name)) out.push(pinOf(o));
  return out;
}

/** `a__b` cannot be withheld on its own (`mcp__a__*` would match it too), so it cannot be picked. */
export const separable = (server: string) => !server.includes("__");

/** Servers a file agent's `disallowedTools` withholds as a whole (`mcp__x` or `mcp__x__*`). */
export function withheldServers(disallowed: string[]): string[] {
  return disallowed.flatMap((d) => {
    const m = /^mcp__(.+?)(?:__\*)?$/.exec(d);
    return m && separable(m[1]) ? [m[1]] : [];
  });
}

/** A file agent's chosen servers: none withheld = nothing chosen (every server); else the ones not withheld. */
export function chosenFromDisallowed(disallowed: string[], servers: string[]): string[] {
  const off = withheldServers(disallowed);
  if (off.length === 0) return [];
  return servers.filter((s) => separable(s) && !off.includes(s));
}

/** New `disallowedTools` for a file agent: other entries kept, MCP entries rebuilt from the chosen servers. */
export function disallowedFor(disallowed: string[], chosen: string[], servers: string[]): string[] {
  const known = new Set(servers);
  const keep = disallowed.filter((d) => {
    const m = /^mcp__(.+?)(?:__\*)?$/.exec(d);
    return !(m && known.has(m[1]));
  });
  if (chosen.length === 0) return keep;
  return [...keep, ...servers.filter((s) => !chosen.includes(s)).map((s) => `mcp__${s}__*`)];
}

/** `tools` lets a whole server through (`mcp__x` or `mcp__x__*`). */
const toolsAllow = (tools: string[], server: string) => tools.some((t) => t === `mcp__${server}` || t === `mcp__${server}__*`);

/** A file agent's chosen servers. With a `tools` line only servers listed there whole count (none = MCP 안 씀);
 * without one, the servers not withheld by `disallowedTools` (none withheld = nothing chosen = every server). */
export function chosenFromAgent(tools: string[], disallowed: string[], servers: string[]): string[] {
  if (tools.length === 0) return chosenFromDisallowed(disallowed, servers);
  const off = withheldServers(disallowed);
  return servers.filter((s) => separable(s) && toolsAllow(tools, s) && !off.includes(s));
}

/** New `tools` / `disallowedTools` for a file agent's loadout. Only MCP entries of servers in the list change (Rust
 * refuses anything else). A `tools` line gets `mcp__<server>__*` for each chosen server, else they could not be used;
 * a withheld entry of a chosen server goes. Without a `tools` line the unchosen servers are withheld. */
export function agentListsFor(tools: string[], disallowed: string[], chosen: string[], servers: string[]): { tools: string[]; disallowed: string[] } {
  if (tools.length === 0) return { tools, disallowed: disallowedFor(disallowed, chosen, servers) };
  const known = new Set(servers);
  const whole = (t: string) => {
    const m = /^mcp__(.+?)(?:__\*)?$/.exec(t);
    return m && known.has(m[1]) && separable(m[1]) ? m[1] : null;
  };
  const keptTools = tools.filter((t) => whole(t) === null || chosen.includes(whole(t)!));
  const add = chosen.filter((c) => !keptTools.some((t) => whole(t) === c)).map((c) => `mcp__${c}__*`);
  return { tools: [...keptTools, ...add], disallowed: disallowed.filter((d) => !chosen.includes(whole(d) ?? "")) };
}

/** True when a save would empty a non-empty `tools` line: the line would vanish and the agent would get every tool,
 * so Rust refuses it and the screen keeps the last server. */
export function wouldEmptyTools(tools: string[], chosen: string[], servers: string[]): boolean {
  return tools.length > 0 && agentListsFor(tools, [], chosen, servers).tools.length === 0;
}

/** One-line frontmatter list exactly as written (`tools: a, b` or `[a, b]`), like Rust's `tool_lists_raw`: the save
 * builds on these, not on the catalog's shortened display copy. */
export function frontmatterList(content: string, key: string): string[] {
  const lines = content.replace(/^\uFEFF/, "").split(/\r?\n/);
  if (lines[0] !== "---") return [];
  for (const l of lines.slice(1)) {
    if (l === "---") break;
    const m = /^([A-Za-z][A-Za-z0-9_-]*):(.*)$/.exec(l);
    if (m && m[1] === key) {
      return m[2].trim().replace(/^\[/, "").replace(/\]$/, "").split(",").map((t) => t.trim().replace(/^(["'])(.*)\1$/, "$2").trim()).filter(Boolean);
    }
  }
  return [];
}

/** Entries a save adds to a `tools` line (shown before saving). */
export const addedTools = (before: string[], after: string[]) => after.filter((t) => !before.includes(t));

/** Chip line for the tour's role duck cards (tour-v2): "frontend-design 스킬", "figma MCP". */
export function roleLoadoutChips(type: Pick<TeamType, "skills" | "mcpAllow" | "agent">): { kind: "skill" | "mcp"; label: string }[] {
  if (type.agent) return [];
  return [...type.skills.map((s) => ({ kind: "skill" as const, label: t("agents.loadout.skillChip", { name: s.name }) })), ...type.mcpAllow.map((m) => ({ kind: "mcp" as const, label: t("agents.loadout.mcpChip", { name: m }) }))];
}

/** Agents whose loadout cannot be edited here, and why (one line). */
export function agentLoadoutLock(d: Pick<AgentDef, "source" | "editable" | "readonlyReason">): string | null {
  if (d.source === "plugin") return t("agents.loadout.lockPlugin");
  if (d.source === "builtin") return t("agents.loadout.lockBuiltin");
  if (!d.editable) return d.readonlyReason ?? t("agents.loadout.readOnly");
  return null;
}
