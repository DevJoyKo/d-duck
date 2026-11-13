// Rules (rules-editor, D-106): typed wrappers and small pure helpers. The webview sends a memory project id, an
// opaque rule id from `rules_list` (or a new file name) and hash tokens; Rust resolves every path.
import { invoke } from "@tauri-apps/api/core";
import type { Trashed } from "./api";
import { t } from "../i18n";
import { subscribe } from "../i18n/language";
import type { SecretHit } from "./memory";
import { splitPath } from "./platform";

export type RuleScope = "global" | "project";
export type FmState = "none" | "ok" | "complex" | "broken";

export interface RuleRow {
  id: string;
  scope: RuleScope;
  rel: string;
  name: string;
  folder: string | null;
  paths: string[];
  fm: FmState;
  enabled: boolean;
  offBy: "app" | "other" | null;
  gitTracked: boolean | null;
  link: boolean;
  dirLink: boolean;
  linkOutside: boolean;
  /** Where a linked rule points, shown read-only. */
  linkTarget: string | null;
  size: number | null;
  token: string;
  readonlyReason: string | null;
  hasSecrets: boolean;
  warnings: string[];
}

export interface RuleGroup {
  scope: RuleScope;
  dir: string;
  exists: boolean;
  rules: RuleRow[];
  truncated: boolean;
  settingsPath: string;
  settingsToken: string;
  settingsReadonly: string | null;
  alwaysCount: number;
  alwaysBytes: number;
  readonlyReason: string | null;
}

export interface RuleTemplate {
  id: string;
  label: string;
  name: string;
  paths: string[];
  body: string;
}

export interface RulesList {
  global: RuleGroup | null;
  project: RuleGroup | null;
  templates: RuleTemplate[];
}

export interface RuleDoc extends RuleRow {
  body: string;
  pathsEditable: boolean;
  secrets: SecretHit[];
  instructionLike: number[];
}

export interface RuleSave {
  id?: string;
  token?: string;
  scope?: RuleScope;
  name?: string;
  body: string;
  /** Omitted: keep the frontmatter as it is. */
  paths?: string[];
}

export const rulesApi = {
  list: (project: string | null) => invoke<RulesList>("rules_list", { project }),
  read: (project: string | null, id: string) => invoke<RuleDoc>("rule_read", { project, id }),
  save: (project: string | null, rule: RuleSave) => invoke<string>("save_rule", { project, rule }),
  setEnabled: (project: string | null, id: string, enabled: boolean, settingsToken: string) => invoke<void>("set_rule_enabled", { project, id, enabled, settingsToken }),
  trash: (project: string | null, id: string, token: string) => invoke<Trashed>("trash_rule", { project, id, token }),
};

/** Same pattern as Rust `valid_new_name`: letters, digits, `-`, `_`, one optional subfolder, `.md`. */
export function validRuleName(n: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}(\/[A-Za-z0-9_-]{1,64})?\.md$/.test(n);
}

/** `api` → `api.md`; leaves a name that already ends with `.md`. */
export function ruleFileName(n: string): string {
  const t = n.trim();
  return t.endsWith(".md") ? t : `${t}.md`;
}

function expansions(p: string): number {
  const s = [...p];
  let i = 0;
  const seq = (stop: boolean): number => {
    let n = 1;
    while (i < s.length) {
      const c = s[i];
      if (c === "\\") i += 2;
      else if (stop && (c === "," || c === "}")) return n;
      else if (c === "{") {
        const save = i;
        i += 1;
        const alts = [seq(true)];
        while (i < s.length && s[i] === ",") {
          i += 1;
          alts.push(seq(true));
        }
        if (i < s.length && s[i] === "}") {
          i += 1;
          n *= alts.length > 1 ? alts.reduce((a, b) => a + b, 0) : alts[0];
        } else i = save + 1;
      } else i += 1;
    }
    return n;
  };
  return seq(false);
}

function badBracket(p: string): boolean {
  const c = [...p];
  for (let i = 0; i < c.length; ) {
    if (c[i] === "\\") i += 2;
    else if (c[i] === "[") {
      let j = i + 1;
      if (c[j] === "!" || c[j] === "^") j += 1;
      if (c[j] === "]") j += 1;
      while (j < c.length && c[j] !== "]" && c[j] !== "/") j += 1;
      if (c[j] !== "]") return true;
      i = j + 1;
    } else i += 1;
  }
  return false;
}

/** Live warnings while typing (same rules as Rust `glob_warnings`). Never blocks saving. */
export function globWarnings(paths: string[]): string[] {
  const out: string[] = [];
  let total = 0;
  let bytes = 0;
  for (const p of paths) {
    const n = expansions(p);
    if (n > 1) {
      total += n;
      bytes += n * new TextEncoder().encode(p).length;
    }
    if (badBracket(p)) out.push(t("extensions.rules.glob.bracket", { pattern: p }));
    if (p.startsWith("~") || p.split("/").includes("..") || /^[A-Za-z]:/.test(p)) out.push(t("extensions.rules.glob.outside", { pattern: p }));
  }
  if (total > 1000 || bytes > 4 * 1024 * 1024) out.push(t("extensions.rules.glob.tooMany"));
  return out;
}

/** Refused before sending (Rust refuses them too): blanks around, line breaks, invisible characters. */
export function patternProblem(p: string): string | null {
  if (!p.trim()) return t("extensions.rules.problem.empty");
  if (p.trim() !== p) return t("extensions.rules.problem.spaces");
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f-\u009f\u00ad\u200b-\u200f\u2028-\u202e\u2060-\u206f\ufeff]/.test(p)) return t("extensions.rules.problem.invisible");
  if (p.length > 300) return t("extensions.rules.problem.tooLong");
  return null;
}

/** The "항상" chip text in the current language. A chip equal to it is the always chip, not a pattern. */
export function alwaysChip(): string {
  return t("extensions.rules.scope.always");
}

/** Scope chips of a row: its patterns, "항상" when it loads every session, none when the app cannot tell. */
export function scopeChips(r: Pick<RuleRow, "paths" | "fm" | "link" | "dirLink">): string[] {
  if (r.dirLink) return [];
  if (r.fm === "broken") return [alwaysChip()];
  if (r.paths.length) return r.paths;
  return r.fm === "complex" ? [] : [alwaysChip()];
}

const hasGlob = (s: string) => /[*?[\]{}]/.test(s);

/** Plain-language title of one scope chip: `**\/*.{ts,tsx}` → "TS·TSX 파일을 다룰 때만". */
export function scopeTitle(p: string): string {
  if (p === alwaysChip()) return t("extensions.rules.scopeTitle.always");
  const segs = p.replace(/^\//, "").split("/");
  const last = segs[segs.length - 1] ?? "";
  const dir = segs.slice(0, -1);
  const fixed: string[] = [];
  for (const s of dir) {
    if (hasGlob(s)) break;
    fixed.push(s);
  }
  const ext = /^\*\.(?:\{([A-Za-z0-9_,]+)\}|([A-Za-z0-9_]+))$/.exec(last);
  const exts = ext ? (ext[1] ?? ext[2]).split(",").filter(Boolean).map((e) => e.toUpperCase()) : null;
  const folder = fixed.join("/");
  if (exts) return fixed.length ? t("extensions.rules.scopeTitle.folderExts", { folder, exts: exts.join("·") }) : t("extensions.rules.scopeTitle.exts", { exts: exts.join("·") });
  if (!hasGlob(p)) return t("extensions.rules.scopeTitle.file", { pattern: p });
  if (last === "**" || last === "*") return fixed.length ? t("extensions.rules.scopeTitle.folderFiles", { folder }) : t("extensions.rules.scopeTitle.allFiles");
  return t("extensions.rules.scopeTitle.pattern");
}

/** `api` when free in `taken` (rel paths like `api.md`), else `api-2`, `api-3`, … */
export function freeName(name: string, taken: string[]): string {
  const set = new Set(taken.map((t) => t.toLowerCase()));
  if (!name || !set.has(ruleFileName(name).toLowerCase())) return name;
  for (let i = 2; i < 1000; i += 1) if (!set.has(`${name}-${i}.md`.toLowerCase())) return `${name}-${i}`;
  return name;
}

/** Folder name of a project rules group (`/a/dev_team/.claude/rules` → `dev_team`). */
export function projectFolder(dir: string): string | null {
  const parts = splitPath(dir);
  return parts.length >= 3 && parts[parts.length - 1] === "rules" && parts[parts.length - 2] === ".claude" ? parts[parts.length - 3] : null;
}

export function filterRules(rules: RuleRow[], q: string): RuleRow[] {
  const t = q.trim().toLowerCase();
  return t ? rules.filter((r) => r.rel.toLowerCase().includes(t) || r.paths.some((p) => p.toLowerCase().includes(t))) : rules;
}

/** Rules grouped by subfolder (`""` = top level first), in listing order. */
export function byFolder(rules: RuleRow[]): [string, RuleRow[]][] {
  const m = new Map<string, RuleRow[]>();
  for (const r of rules) {
    const k = r.folder ?? "";
    m.set(k, [...(m.get(k) ?? []), r]);
  }
  return [...m.entries()].sort(([a], [b]) => (a === "" ? -1 : b === "" ? 1 : a.localeCompare(b)));
}

export function summaryText(g: Pick<RuleGroup, "alwaysCount" | "alwaysBytes">): string {
  const kb = g.alwaysBytes < 1024 ? `${g.alwaysBytes}B` : `${(g.alwaysBytes / 1024).toFixed(1)}KB`;
  return t("extensions.rules.summary", { count: g.alwaysCount, size: kb });
}

/** What "off" means, shown in the editor of an off rule and as the switch title. */
export function offText(): string {
  return t("extensions.rules.offText");
}

/** The unsaved-changes question, in the current language. */
export function discardText(): string {
  return t("extensions.rules.discard");
}

/** Kept for `lib/projects.ts`, which re-exports it: a live binding refreshed on every language change. New code calls
 * `discardText()`. */
export let DISCARD_TEXT = discardText();
subscribe(() => {
  DISCARD_TEXT = discardText();
});

/** True when it is fine to leave: nothing unsaved, or the user agreed to drop it. */
export function confirmDiscard(dirty: boolean, ask: (q: string) => boolean = (q) => window.confirm(q)): boolean {
  return !dirty || ask(discardText());
}
