// Memory tab (memory-tab, D-091): typed wrappers and small pure helpers. The webview only sends an opaque project id
// from `projects_known` (project-switcher), a file name and hash tokens; Rust resolves every path.
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

export type MemoryKind = "user" | "feedback" | "project" | "reference";
export const KINDS: MemoryKind[] = ["user", "feedback", "project", "reference"];
/** Label of a memory kind, read at call time so it follows the language. */
export function memoryKindLabel(k: MemoryKind): string {
  return t(`extensions.memory.kinds.${k}`);
}

export interface SecretHit {
  kind: string;
  count: number;
  lines: number[];
}

export interface MemoryProject {
  id: string;
  label: string;
  path: string | null;
  parent: string | null;
  on: number;
  off: number;
  empty: boolean;
  overLimit: boolean;
  /** Rules in the project's `.claude/rules` (rules-editor): listed even with no memory. */
  rules?: number;
}

export interface MemoryEntry {
  file: string;
  enabled: boolean;
  name: string | null;
  description: string | null;
  kind: string | null;
  modified: string | null;
  mtimeMs: number;
  size: number;
  token: string;
  inIndex: boolean;
  beyondLimit: boolean;
  indexLine: string | null;
  onLine: string;
  /** Why toggling is refused until MEMORY.md (or the folder) is tidied. */
  indexProblem: string | null;
  secrets: SecretHit[];
  instructionLike: number[];
  readonlyReason: string | null;
  fresh: boolean;
}

export interface Orphan {
  line: number;
  title: string;
  target: string;
  problem: string;
  beyondLimit: boolean;
}

export interface MemoryList {
  dir: string;
  indexExists: boolean;
  indexToken: string;
  indexLines: number;
  indexBytes: number;
  overLimit: boolean;
  indexError: string | null;
  entries: MemoryEntry[];
  orphans: Orphan[];
  projectSettings: [string, boolean][];
}

export interface MemoryDoc {
  file: string;
  enabled: boolean;
  hasFrontmatter: boolean;
  fieldsEditable: boolean;
  name: string;
  description: string;
  kind: string;
  body: string;
  token: string;
  indexToken: string;
  indexLine: string | null;
  onLine: string;
  secrets: SecretHit[];
  instructionLike: number[];
  readonlyReason: string | null;
  indexProblem: string | null;
}

export interface AutoMemory {
  enabled: boolean;
  explicit: boolean;
  token: string;
  path: string;
  readonlyReason: string | null;
  customDir: string | null;
}

export interface InstructionFile {
  id: string;
  scope: string;
  /** `global` | `project` | `local` | `parent`; worded from the extensions catalog. */
  scopeCode?: string;
  path: string;
  size: number | null;
  gitTracked: boolean | null;
  link: boolean;
  content: string | null;
  secrets: SecretHit[];
  instructionLike: number[];
}

export interface MemoryEdit {
  name: string;
  description: string;
  kind: string;
  body: string;
}

export const memoryApi = {
  projects: () => invoke<MemoryProject[]>("memory_projects"),
  entries: (project: string) => invoke<MemoryList>("memory_entries", { project }),
  read: (project: string, file: string) => invoke<MemoryDoc>("memory_read", { project, file }),
  save: (project: string, file: string, token: string, indexToken: string, edit: MemoryEdit) =>
    invoke<string | null>("save_memory", { project, file, token, indexToken, edit }),
  setEnabled: (project: string, file: string, enabled: boolean, token: string, indexToken: string) =>
    invoke<string | null>("set_memory_enabled", { project, file, enabled, token, indexToken }),
  trash: (project: string, file: string, token: string, indexToken: string) => invoke<boolean>("trash_memory", { project, file, token, indexToken }),
  autoMemory: () => invoke<AutoMemory>("auto_memory_setting"),
  setAutoMemory: (enabled: boolean, token: string) => invoke<boolean>("set_auto_memory", { enabled, token }),
  /** `null` = global only (the user's CLAUDE.md). */
  instructionFiles: (project: string | null) => invoke<InstructionFile[]>("instruction_files", { project }),
  openInstructionFile: (project: string | null, id: string) => invoke<void>("open_instruction_file", { project, id }),
  openIndex: (project: string) => invoke<void>("open_memory_index", { project }),
};

/** Projects in display order: worktree keys right after their parent; empty ones (no memory, no rules) only when asked. */
export function orderProjects(list: MemoryProject[], showEmpty: boolean): (MemoryProject & { child: boolean })[] {
  const visible = list.filter((p) => showEmpty || !p.empty || (p.rules ?? 0) > 0);
  const ids = new Set(visible.map((p) => p.id));
  const top = visible.filter((p) => !p.parent || !ids.has(p.parent));
  return top.flatMap((p) => [{ ...p, child: false }, ...visible.filter((c) => c.parent === p.id).map((c) => ({ ...c, child: true }))]);
}

/** The MEMORY.md line a save would leave: the standard ` — ` line with the new description, else unchanged. */
export function previewIndexLine(indexLine: string | null, description: string): string | null {
  if (!indexLine) return null;
  const m = /^(\s*[-*+]\s+\[.*?\]\([^)]*\)) — .*$/.exec(indexLine);
  return m ? `${m[1]} — ${description.replace(/[\r\n]/g, " ")}` : indexLine;
}

export function secretText(hits: SecretHit[]): string {
  return hits.map((h) => t("extensions.memory.secretHit", { kind: h.kind, count: h.count, lines: h.lines.join(", ") })).join(", ");
}

/** Last path component, for showing a folder name before the project key. */
export function folderName(path: string | null): string | null {
  return path ? (path.split("/").filter(Boolean).pop() ?? path) : null;
}

export function sizeText(n: number | null): string {
  if (n === null) return "";
  return n < 1024 ? `${n}B` : `${(n / 1024).toFixed(1)}KB`;
}
