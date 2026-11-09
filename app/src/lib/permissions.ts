// Tool permissions tab (D-076, D-079): IPC wrappers and the pure toggle/JSON logic. Rust re-validates every save.
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { AgentDef } from "./api";
import { liveLabels } from "./team";

export type Risk = "low" | "medium" | "high";
export type ListName = "allow" | "ask" | "deny";
export type Toggle = ListName | "none";
export const LISTS: ListName[] = ["allow", "ask", "deny"];
export const LIST_LABEL: Record<Toggle, string> = liveLabels(["allow", "ask", "deny", "none"], (k) => t(`agents.perm.list.${k}`));
export const RISK_LABEL: Record<Risk, string> = liveLabels(["high", "medium", "low"], (k) => t(`agents.perm.risk.${k}`));

export interface CatalogItem {
  group: string;
  rule: string;
  risk: Risk;
  recommended: ListName;
  recommendedGlobal?: ListName | null;
  description: string;
  reason: string;
}

export interface Catalog {
  groups: { id: string; label: string }[];
  items: CatalogItem[];
}

export interface ScopeView {
  path: string;
  exists: boolean;
  token: string;
  allow: string[];
  ask: string[];
  deny: string[];
  defaultMode: string | null;
  other: [string, unknown][];
  /** Member order of `permissions` in the file. */
  keys: string[];
  readonlyReason: string | null;
  managedOnly: boolean;
  error: string | null;
}

export interface PermAgent extends AgentDef {
  /** Lists exactly as written in the file (the catalog copy is cut for display). */
  toolsRaw: string[];
  disallowedRaw: string[];
}

export interface RowEffect {
  rule: string;
  decision: Explanation["decision"];
  byRule: string | null;
  byScope: string | null;
  /** `managed` | `local` | `shared` | `user` | `agent` | `other` (Rust `scope_code`). */
  byScopeCode?: string | null;
}

export interface PermissionScopes {
  catalog: Catalog;
  /** Opaque project id from `projects_known` (project-switcher), echoed back, and its folder name. */
  project: string | null;
  projectName: string | null;
  user: ScopeView;
  shared: ScopeView | null;
  local: ScopeView | null;
  managed: { sources: ScopeView[]; mdm: string | null; managedOnly: boolean };
  agents: PermAgent[];
  modes: string[];
}

export interface PermEdit {
  allow: string[];
  ask: string[];
  deny: string[];
  defaultMode: string | null;
}

export interface Explanation {
  decision: "allow" | "ask" | "deny" | "none" | "unavailable";
  rule: string | null;
  scope: string | null;
  /** `managed` | `local` | `shared` | `user` | `agent` | `other` (Rust `scope_code`). */
  scopeCode?: string | null;
  list: string | null;
  notes: string[];
}

export type SettingsScope = "user" | "shared" | "local";

export const permApi = {
  scopes: (project: string | null) => invoke<PermissionScopes>("permission_scopes", { project }),
  save: (scope: SettingsScope, project: string | null, edit: PermEdit, token: string) => invoke<void>("save_permissions", { scope, project, edit, token }),
  saveAgentTools: (id: string, project: string | null, tools: string[], disallowed: string[], token: string) =>
    invoke<void>("save_agent_tools", { id, project, tools, disallowed, token }),
  effective: (project: string | null, scope: SettingsScope, edit: PermEdit) => invoke<{ rows: RowEffect[]; mdmNote: string | null }>("permission_effective", { project, scope, edit }),
  ruleCovers: (rule: string) => invoke<string[]>("permission_rule_covers", { rule }),
  explain: (project: string | null, agentId: string | null, call: string) => invoke<Explanation>("explain_permission", { project, agentId, call }),
};

export const editOf = (v: ScopeView): PermEdit => ({ allow: [...v.allow], ask: [...v.ask], deny: [...v.deny], defaultMode: v.defaultMode });

export function sameEdit(a: PermEdit, b: PermEdit): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Toggle state from exact matches only. A rule in several lists shows the strongest (deny > ask > allow). */
export function stateOf(edit: PermEdit, rule: string): Toggle {
  if (edit.deny.includes(rule)) return "deny";
  if (edit.ask.includes(rule)) return "ask";
  if (edit.allow.includes(rule)) return "allow";
  return "none";
}

/** Moves one exact rule: out of every list, then to the end of the target. Other rules keep their places. */
export function setToggle(edit: PermEdit, rule: string, to: Toggle): PermEdit {
  const next: PermEdit = { ...edit, allow: edit.allow.filter((r) => r !== rule), ask: edit.ask.filter((r) => r !== rule), deny: edit.deny.filter((r) => r !== rule) };
  if (to !== "none") next[to] = [...next[to], rule];
  return next;
}

export const recommendedFor = (item: CatalogItem, scope: SettingsScope): ListName => (scope === "user" && item.recommendedGlobal ? item.recommendedGlobal : item.recommended);

export interface RecChange {
  rule: string;
  risk: Risk;
  from: Toggle;
  to: ListName;
}

export function recommendedDiff(edit: PermEdit, catalog: Catalog, scope: SettingsScope): RecChange[] {
  return catalog.items.flatMap((i) => {
    const from = stateOf(edit, i.rule);
    const to = recommendedFor(i, scope);
    return from === to ? [] : [{ rule: i.rule, risk: i.risk, from, to }];
  });
}

export function applyChanges(edit: PermEdit, changes: RecChange[]): PermEdit {
  return changes.reduce((e, c) => setToggle(e, c.rule, c.to), edit);
}

/** Rules that are not an exact catalog entry: shown as "직접 추가한 규칙". */
export function customRules(edit: PermEdit, catalog: Catalog): { list: ListName; rule: string }[] {
  const known = new Set(catalog.items.map((i) => i.rule));
  return LISTS.flatMap((list) => edit[list].filter((r) => !known.has(r)).map((rule) => ({ list, rule })));
}

// Control characters are exactly what this rejects (mirrors Rust `is_unsafe_char`).
// eslint-disable-next-line no-control-regex
const UNSAFE_G = new RegExp("[\\u0000-\\u001f\\u007f-\\u009f\\u00ad\\u061c\\u180e\\u200b-\\u200f\\u2028-\\u202e\\u2060-\\u206f\\ufeff\\ufff9-\\ufffb]", "g");
const INVISIBLE_G = new RegExp("[\\u007f-\\u009f\\u00ad\\u061c\\u180e\\u200b-\\u200f\\u2028-\\u202e\\u2060-\\u206f\\ufeff\\ufff9-\\ufffb]", "g");
// eslint-disable-next-line no-control-regex
const UNSAFE = new RegExp("[\\u0000-\\u001f\\u007f-\\u009f\\u00ad\\u061c\\u180e\\u200b-\\u200f\\u2028-\\u202e\\u2060-\\u206f\\ufeff\\ufff9-\\ufffb]");

const hex = (c: string) => `\\u${(c.codePointAt(0) ?? 0).toString(16).padStart(4, "0")}`;

/** Display copy of a raw rule: invisible/control characters become visible `\uXXXX`, long text is cut. Never saved. */
export function shown(s: string, max = 300): string {
  const v = s.replace(UNSAFE_G, hex);
  return v.length > max ? `${v.slice(0, max)}…` : v;
}

export const hasHidden = (s: string) => UNSAFE.test(s);

/** Mirrors the Rust parser. `null` = valid. */
export function checkRule(s: string): string | null {
  if (!s || [...s].length > 600) return t("agents.perm.ruleCheck.empty");
  if (UNSAFE.test(s)) return t("agents.perm.ruleCheck.unsafe");
  if (s.trim() !== s) return t("agents.perm.ruleCheck.spaces");
  let tool = s;
  const i = s.indexOf("(");
  if (i >= 0) {
    if (!s.endsWith(")") || i === 0) return t("agents.perm.ruleCheck.parens");
    if (s.length - 1 === i + 1) return t("agents.perm.ruleCheck.emptyParens");
    tool = s.slice(0, i);
  }
  if (!/^[A-Za-z*][A-Za-z0-9_.*-]*$/.test(tool)) return t("agents.perm.ruleCheck.toolName");
  if (tool.includes("*") && !tool.startsWith("mcp__") && tool !== "*") return t("agents.perm.ruleCheck.wildcard");
  return null;
}

export function ruleWarnings(s: string, list: ListName): string[] {
  if (checkRule(s)) return [];
  const i = s.indexOf("(");
  const tool = i >= 0 ? s.slice(0, i) : s;
  const spec = i >= 0 ? s.slice(i + 1, -1) : null;
  const w: string[] = [];
  if (tool.startsWith("mcp__") && spec !== null) w.push(t("agents.perm.ruleCheck.mcpParens"));
  if (list === "allow" && (tool === "*" || tool.startsWith("mcp__*"))) w.push(t("agents.perm.ruleCheck.allowWildcard"));
  if (tool === "Bash" && spec?.includes(":*") && !spec.endsWith(":*")) w.push(t("agents.perm.ruleCheck.bashSuffix"));
  return w;
}

/**
 * The scope's `permissions` object as it would be written: file key order, lists the file omits stay omitted while
 * empty, other keys read-only. Invisible characters are written as `\uXXXX` escapes (same string once parsed).
 */
export function toJson(edit: PermEdit, base: Pick<ScopeView, "other" | "keys">): string {
  const obj: Record<string, unknown> = {};
  const other = new Map(base.other);
  const ours = (k: string) => (k === "defaultMode" ? edit.defaultMode : edit[k as ListName]);
  for (const k of base.keys) {
    if (["allow", "ask", "deny", "defaultMode"].includes(k)) {
      const v = ours(k);
      if (v !== null) obj[k] = v;
    } else if (other.has(k)) obj[k] = other.get(k);
  }
  for (const k of LISTS) if (!(k in obj) && edit[k].length) obj[k] = edit[k];
  if (!("defaultMode" in obj) && edit.defaultMode) obj.defaultMode = edit.defaultMode;
  // Control characters are already escaped by stringify (and its line breaks must stay); only invisibles remain.
  return JSON.stringify(obj, null, 2).replace(INVISIBLE_G, hex);
}

function lineOf(text: string, message: string): string {
  const line = /line (\d+)/.exec(message);
  if (line) return t("agents.perm.json.line", { n: line[1] });
  const pos = /position (\d+)/.exec(message);
  return pos ? t("agents.perm.json.line", { n: text.slice(0, Number(pos[1])).split("\n").length }) : t("agents.perm.json.end");
}

function jsonErrorText(message: string): string {
  if (/Unexpected end/i.test(message)) return t("agents.perm.json.unexpectedEnd");
  if (/Expected ',' or/i.test(message)) return t("agents.perm.json.comma");
  if (/Expected property name|Expected double-quoted/i.test(message)) return t("agents.perm.json.propertyName");
  if (/Bad control character|Bad escaped/i.test(message)) return t("agents.perm.json.badChar");
  return t("agents.perm.json.notJson");
}

/**
 * Parses the JSON editor. Only allow/ask/deny/defaultMode may change; new rules must pass `checkRule`, existing
 * rules are kept even when unknown (security review).
 */
export function parseJson(text: string, base: ScopeView): { edit: PermEdit } | { error: string } {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return { error: t("agents.perm.json.syntax", { where: lineOf(text, msg), why: jsonErrorText(msg) }) };
  }
  if (typeof v !== "object" || v === null || Array.isArray(v)) return { error: t("agents.perm.json.notObject", { braces: "{ }" }) };
  const o = v as Record<string, unknown>;
  const known = new Set([...base.allow, ...base.ask, ...base.deny]);
  const edit: PermEdit = { allow: [], ask: [], deny: [], defaultMode: null };
  for (const list of LISTS) {
    const arr = o[list] ?? [];
    if (!Array.isArray(arr) || arr.some((x) => typeof x !== "string")) return { error: t("agents.perm.json.notArray", { list }) };
    if (arr.length > 500) return { error: t("agents.perm.json.tooMany", { list }) };
    for (const r of arr as string[]) {
      const bad = known.has(r) ? null : checkRule(r);
      if (bad) return { error: t("agents.perm.json.badRule", { list, rule: r.slice(0, 60), why: bad }) };
    }
    edit[list] = arr as string[];
  }
  if (o.defaultMode !== undefined && typeof o.defaultMode !== "string") return { error: t("agents.perm.json.modeString") };
  edit.defaultMode = (o.defaultMode as string | undefined) ?? null;
  if (edit.defaultMode === "bypassPermissions" && base.defaultMode !== "bypassPermissions") return { error: t("agents.perm.json.bypass") };
  const otherKeys = Object.keys(o).filter((k) => !["allow", "ask", "deny", "defaultMode"].includes(k));
  const baseOther = new Map(base.other);
  for (const k of otherKeys) {
    if (!baseOther.has(k) || JSON.stringify(baseOther.get(k)) !== JSON.stringify(o[k])) return { error: t("agents.perm.json.readOnlyKey", { key: k }) };
  }
  if (otherKeys.length !== baseOther.size) return { error: t("agents.perm.json.keepKeys") };
  return { edit };
}

/** Where another scope decides the same exact rule differently, e.g. "전역: 거부". */
export const MODE_LABEL: Record<string, string> = liveLabels(["default", "acceptEdits", "plan", "auto", "dontAsk", "bypassPermissions"] as const, (m) => t(`agents.perm.modeName.${m}`));
export const MODE_HINT: Record<string, string> = liveLabels(["acceptEdits", "auto", "dontAsk"] as const, (m) => t(`agents.perm.modeHint.${m}`));

/** Count of rule/mode changes against the file, for the save bar. */
export function changeCount(base: PermEdit, edit: PermEdit): number {
  let n = edit.defaultMode !== base.defaultMode ? 1 : 0;
  for (const l of LISTS) {
    n += edit[l].filter((r) => !base[l].includes(r)).length + base[l].filter((r) => !edit[l].includes(r)).length;
  }
  return n;
}

export function inheritedNote(rule: string, current: Toggle, others: { label: string; edit: PermEdit }[]): string | null {
  const notes = others.map((o) => ({ label: o.label, s: stateOf(o.edit, rule) })).filter((x) => x.s !== "none" && x.s !== current);
  return notes.length ? notes.map((n) => `${n.label}: ${LIST_LABEL[n.s]}`).join(" · ") : null;
}

const SCOPE_CODES = ["managed", "local", "shared", "user", "agent"] as const;

/** A rule-set label in the app language from its code; `other` or no code shows Rust's label as sent. The agent
 *  label carries the agent name after Rust's fixed "에이전트 " prefix. */
export function scopeLabel(code: string | null | undefined, raw: string | null): string {
  if (!code || !(SCOPE_CODES as readonly string[]).includes(code)) return raw ?? "";
  if (code === "agent") return t("agents.perm.scopeName.agent", { name: raw?.startsWith("에이전트 ") ? raw.slice("에이전트 ".length) : (raw ?? "") });
  return t(`agents.perm.scopeName.${code as Exclude<(typeof SCOPE_CODES)[number], "agent">}`);
}
