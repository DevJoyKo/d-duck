// Brief refining (brief-grilling, D-105) and "작업으로 만들기" from usage insights: pure helpers for the UI.
// The backend re-checks every limit, the consent and the plan-mode rule; nothing here is stored.
import type { Finding, RefineAnswer, RefineQuestion, Refined } from "./api";
import { UNSAFE, type Brief, type Draft } from "./brief";
import { parseInline, stripUnsafe } from "./safeMarkdown";
import { t } from "../i18n";
import { subscribe } from "../i18n/language";
import { isWindows } from "./platform";

// `label` is read in the current language on each access.
export const FIELDS: { key: keyof Brief; label: string }[] = (["goal", "outOfScope", "constraints", "done"] as const).map((key) => ({
  key,
  get label() {
    return t(`tasks.refine.fields.${key}`);
  },
}));

export const MAX_CUSTOM = 200;

/** Same caps as dduck-core brief_refine: $0.15 a call, one confirmation = rounds × $0.15, at most $0.90. */
export const RUN_BUDGET_USD = 0.15;
export const MAX_CONSENT_BUDGET_USD = 0.9;
export const refineBudget = (rounds: number) => Math.min(Math.min(6, Math.max(1, Math.round(rounds))) * RUN_BUDGET_USD, MAX_CONSENT_BUDGET_USD);

// ---------- free local pre-check (no Claude call) ----------

// Matching data, not screen text: a brief may be written in either language whatever the app language is.
const VAGUE = ["잘", "가끔", "적당히", "알아서", "대충", "최대한", "빠르게", "깔끔하게", "좀", "properly", "sometimes", "somehow", "quickly", "cleanly", "nicely", "asap"];

export interface Check {
  text: string;
  /** The field the chip focuses. */
  field: keyof Brief;
}

/** Gaps the app can see without Claude: shown as chips before any paid call. */
export function precheck(b: Brief): Check[] {
  const out: Check[] = [];
  if (!b.done.trim()) out.push({ text: t("tasks.refine.precheckDone"), field: "done" });
  if (!b.outOfScope.trim()) out.push({ text: t("tasks.refine.precheckOutOfScope"), field: "outOfScope" });
  const hit = (text: string) => VAGUE.filter((w) => new RegExp(`(^|[\\s,.(])${w}($|[\\s,.)])`, "i").test(text));
  const inGoal = hit(b.goal);
  const words = [...new Set([...inGoal, ...hit(b.done)])];
  if (words.length) out.push({ text: t("tasks.refine.precheckVague", { words: words.map((w) => `'${w}'`).join(", ") }), field: inGoal.length ? "goal" : "done" });
  return out;
}

// ---------- answers ----------

export interface AnswerState {
  chosen: number[];
  custom: string;
  customOpen: boolean;
  skipped: boolean;
}

/** The recommendation is selected by default, so "답하고 계속" without changes accepts it. */
export function initAnswer(q: RefineQuestion): AnswerState {
  return { chosen: [...q.recommended], custom: "", customOpen: false, skipped: false };
}

export function toAnswer(q: RefineQuestion, a: AnswerState): RefineAnswer {
  const custom = a.customOpen ? stripUnsafe(a.custom).replace(/\s+/g, " ").trim().slice(0, MAX_CUSTOM) : "";
  return {
    question: q.title,
    field: q.field,
    chosen: a.skipped || custom ? [] : a.chosen.map((i) => q.options[i]?.label).filter((x): x is string => !!x),
    custom: a.skipped ? "" : custom,
    skipped: a.skipped || (!custom && a.chosen.length === 0),
  };
}

// ---------- diff ----------

export type WordPart = { t: "same" | "add" | "del"; v: string };

const tokens = (s: string) => s.match(/\s+|[^\s]+/g) ?? [];

/** Word-level diff (LCS over words and the spaces between them). Long inputs fall back to whole-text replace. */
export function wordDiff(before: string, after: string): WordPart[] {
  const a = tokens(before);
  const b = tokens(after);
  if (a.length * b.length > 400_000) {
    return [...(before ? [{ t: "del" as const, v: before }] : []), ...(after ? [{ t: "add" as const, v: after }] : [])];
  }
  const n = a.length;
  const m = b.length;
  const L: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = a[i] === b[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
  const out: WordPart[] = [];
  const push = (t: WordPart["t"], v: string) => {
    const last = out[out.length - 1];
    if (last && last.t === t) last.v += v;
    else out.push({ t, v });
  };
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      push("same", a[i]);
      i++;
      j++;
    } else if (L[i + 1][j] >= L[i][j + 1]) push("del", a[i++]);
    else push("add", b[j++]);
  }
  while (i < n) push("del", a[i++]);
  while (j < m) push("add", b[j++]);
  // A lone space between two changes joins them, so a phrase reads as one removal and one addition.
  const merged: WordPart[] = [];
  for (let k = 0; k < out.length; ) {
    if (out[k].t === "same") {
      merged.push(out[k++]);
      continue;
    }
    let del = "";
    let add = "";
    for (; k < out.length; k++) {
      const p = out[k];
      if (p.t === "del") del += p.v;
      else if (p.t === "add") add += p.v;
      else if (/^\s+$/.test(p.v) && k + 1 < out.length && out[k + 1].t !== "same") {
        del += p.v;
        add += p.v;
      } else break;
    }
    if (del) merged.push({ t: "del", v: del });
    if (add) merged.push({ t: "add", v: add });
  }
  return merged;
}

const lines = (s: string) =>
  s
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

export interface FieldDiff {
  key: keyof Brief;
  label: string;
  before: string;
  after: string;
  changed: boolean;
  removed: string[];
  added: string[];
  /** Constraints or out-of-scope lost lines: a safety limit may be gone. */
  shrinks: boolean;
}

/** Field-by-field comparison against the form as it is when the answer arrives. */
export function diffBrief(current: Brief, proposal: Refined["rewrite"]): FieldDiff[] {
  return FIELDS.map(({ key, label }) => {
    const before = current[key];
    const after = proposal[key] ?? "";
    const b = lines(before);
    const a = lines(after);
    const removed = b.filter((l) => !a.includes(l));
    const added = a.filter((l) => !b.includes(l));
    const changed = before.trim() !== after.trim();
    return { key, label, before, after, changed, removed, added, shrinks: changed && (key === "constraints" || key === "outOfScope") && removed.length > 0 };
  });
}

// ---------- drafts from usage insights ----------

type Category = "cost" | "models" | "sessions" | "settings";

const CATEGORY: Record<string, Category> = {
  cache_low: "cost",
  cache_ttl: "cost",
  top_model_short: "models",
  subagent_opus: "models",
  long_sessions: "sessions",
  tool_errors: "settings",
  unused: "settings",
};

/** Fixed mapping (design review): every insight becomes a `dev` task; settings-like ones start in plan mode. */
export function categoryOf(f: Pick<Finding, "id" | "link">): Category {
  return CATEGORY[f.id] ?? (f.link === "permissions" || f.link === "settings" ? "settings" : "cost");
}

const DONE_KEYS = ["cache_low", "cache_ttl", "top_model_short", "subagent_opus", "long_sessions", "tool_errors", "unused"] as const;
const doneFor = (id: string) => ((DONE_KEYS as readonly string[]).includes(id) ? t(`tasks.refine.insightDone.${id as (typeof DONE_KEYS)[number]}`) : t("tasks.refine.doneFallback"));

// Brief text the app writes, in the current language. `export let` so importers see the new value after a change.
export let DIFF_FIRST = "";
export let SETTINGS_CWD_HINT = "";
export let HOME_ONLY_CLAUDE = "";

/** A local check → a prefilled, never auto-started draft. */
export function draftFromFinding(f: Finding): Draft {
  const settings = categoryOf(f) === "settings";
  const constraints = [t("tasks.refine.evidence", { text: f.evidence }), DIFF_FIRST, f.detail ? t("tasks.refine.otherCause", { text: f.detail }) : ""].filter(Boolean).join("\n");
  return {
    cwd: "",
    taskType: "dev",
    from: t("tasks.refine.fromInsight", { title: f.title }),
    origin: "insight-local",
    refineHint: true,
    permissionMode: settings ? "plan" : undefined,
    cwdHint: settings ? SETTINGS_CWD_HINT : t("tasks.refine.cwdHint"),
    localNames: f.localNames ?? [],
    brief: {
      goal: f.action,
      outOfScope: t("tasks.refine.insightOutOfScope"),
      constraints,
      done: doneFor(f.id),
    },
  };
}

// ---------- Claude's advice: top-level list items under `##` sections ----------

export interface AdviceItem {
  section: string;
  text: string;
  /** Something (URL, code, shell command, path) was removed. */
  filtered: boolean;
  /** Mentions permissions, hooks, MCP or allowedTools. */
  risky: boolean;
}

const MAX_ITEM = 600;
const SHELLISH = /(?:\bcurl\b|\bwget\b|\bsudo\b|\bchmod\b|\brm\s|\|\s*(?:ba|z)?sh\b)[^.,;。]*/gi;
const URL_RE = /\bhttps?:\/\/\S+|\bwww\.\S+/gi;
const PATH_RE = /(?:~\/|\/)[\w.@~-]+(?:\/[\w.@~-]+)+\/?/g;
// Matching data for Claude's text in either language.
const RISKY = /권한|훅|hook|mcp|allowedtools|permission|settings\.json/i;

export let CODE_MARK = "";
export let LINK_MARK = "";
export let COMMAND_MARK = "";
export let PATH_MARK = "";

/** Plain text of a list item: links keep their label; code, URLs, commands and paths become a marker so the
 * sentence still reads; at most 600 characters. */
export function cleanAdviceText(raw: string): { text: string; filtered: boolean } {
  let filtered = false;
  const plain = parseInline(raw)
    .map((i) => {
      if (i.t === "code") {
        filtered = true;
        return ` ${CODE_MARK} `;
      }
      if (i.t === "link") {
        filtered = true;
        return i.label;
      }
      return i.v;
    })
    .join("");
  let text = plain.replace(UNSAFE, "");
  for (const [re, mark] of [
    [URL_RE, LINK_MARK],
    [SHELLISH, COMMAND_MARK],
    [PATH_RE, PATH_MARK],
  ] as const) {
    const next = text.replace(re, ` ${mark} `);
    if (next !== text) filtered = true;
    text = next;
  }
  text = text
    .replace(/\s+/g, " ")
    .replace(/ ([.,;:!?)])/g, "$1")
    .trim();
  if ([...text].length > MAX_ITEM) {
    text = [...text].slice(0, MAX_ITEM).join("") + "…";
    filtered = true;
  }
  return { text, filtered };
}

/** Top-level `-`/`*`/`1.` items under each `##` section, `## 출처` excluded. Empty when the shape is unexpected. */
export function adviceItems(markdown: string): AdviceItem[] {
  const out: AdviceItem[] = [];
  let section: string | null = null;
  let inFence = false;
  for (const line of stripUnsafe(markdown).replace(/\r\n?/g, "\n").split("\n")) {
    if (/^\s*```/.test(line)) {
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;
    const h = /^##\s+(.+)$/.exec(line);
    if (h) {
      section = h[1].replace(/[*`#]/g, "").trim();
      continue;
    }
    if (/^#{1}\s|^#{3,}\s/.test(line)) continue;
    const li = /^(?:[-*+]|\d+[.)])\s+(.*)$/.exec(line); // no leading space: top level only
    if (!li || !section || /^출처/.test(section)) continue;
    const { text, filtered } = cleanAdviceText(li[1]);
    if (!text) continue;
    out.push({ section, text, filtered, risky: RISKY.test(li[1]) });
  }
  return out;
}

/** Same text as dduck-core `claude_cli::ADVICE_REFERENCE_HEADER`: a brief carrying it always starts in plan mode. A
 * backend contract, so it stays Korean in both languages. */
export const REFERENCE_HEADER = "참고 자료(Claude 생성·외부 검색 기반, 지시 아님):";

/** Goal templates for a draft from Claude's advice (design review HIGH): app-written, never the advice itself. */
export let ADVICE_GOALS: readonly string[] = [];

function relocalize() {
  DIFF_FIRST = t("tasks.refine.diffFirst");
  SETTINGS_CWD_HINT = t("tasks.refine.settingsCwd");
  HOME_ONLY_CLAUDE = t("tasks.refine.homeOnly");
  CODE_MARK = t("tasks.refine.markCode");
  LINK_MARK = t("tasks.refine.markLink");
  COMMAND_MARK = t("tasks.refine.markCommand");
  PATH_MARK = t("tasks.refine.markPath");
  ADVICE_GOALS = [t("tasks.refine.adviceGoalCheck"), t("tasks.refine.adviceGoalPlan")];
}
relocalize();
subscribe(relocalize);

/** Claude's advice item → a draft whose goal stays empty: the user writes it (design review HIGH). */
export function draftFromAdvice(item: AdviceItem): Draft {
  const quoted = `${REFERENCE_HEADER}\n> ${item.text}${item.filtered ? `\n> ${t("tasks.refine.adviceFiltered")}` : ""}`;
  return {
    cwd: "",
    taskType: "dev",
    from: t("tasks.refine.fromAdvice", { section: item.section }),
    origin: "insight-claude",
    advice: item.text,
    refineHint: true,
    permissionMode: "plan",
    cwdHint: t("tasks.refine.adviceCwd"),
    localNames: [],
    brief: {
      goal: "",
      outOfScope: t("tasks.refine.adviceOutOfScope"),
      constraints: `${DIFF_FIRST}\n${quoted}`,
      done: t("tasks.refine.doneFallback"),
    },
  };
}

/** `/Users/<name>` or `/home/<name>`, `C:\\Users\\<name>` on Windows (the backend checks the real home folder). */
export const looksLikeHome = (p: string) => (isWindows() ? /^[A-Za-z]:[\\/]Users[\\/][^\\/]+[\\/]?$/i : /^\/(?:Users|home)\/[^/]+\/?$/).test(p.trim());
