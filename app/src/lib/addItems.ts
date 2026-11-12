// "추가 ▾" (install-from-link, skill-authoring): IPC wrappers and small pure helpers. The webview sends tokens, ids,
// names and a project picked from Rust's list — never a path; Rust re-checks everything.
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { PluginOutcome } from "./api";
import { isWindows } from "./platform";

export type AuthorKind = "skill" | "command" | "workflow";
/** Label of one item's kind ("스킬"), read at call time so it follows the language. */
export function kindLabel(k: AuthorKind): string {
  return t(`extensions.kindOne.${k}`);
}
/** The kind inside a sentence ("커맨드 만들기" / "Create command"). */
export function kindNoun(k: AuthorKind): string {
  return t(`extensions.kindNoun.${k}`);
}

export interface MarketPlugin {
  name: string;
  description: string | null;
  version: string | null;
  origin: string;
  declares: string[];
}

export interface SkillRisk {
  allowedTools: string | null;
  hooks: boolean;
  shellLines: number;
  danger: boolean;
}

export interface SkillFile {
  path: string;
  size: number;
  executable: boolean;
}

export type LinkPreview = { token: string; installed: boolean } & (
  | { kind: "marketplace"; source: string; sha: string; market: string; description: string | null; plugins: MarketPlugin[]; skipped: number; alreadyAdded: boolean | null }
  | { kind: "plugin"; id: string; plugin: MarketPlugin | null }
  | {
      kind: "skill";
      source: string;
      sha: string;
      name: string;
      description: string | null;
      files: SkillFile[];
      executables: number;
      totalBytes: number;
      skipped: string[];
      skillMd: string;
      risk: SkillRisk;
    }
);

export interface PluginInstall {
  outcome: PluginOutcome;
  marketplaceAdded: boolean;
  approvalCommand: string | null;
  removeCommand: string | null;
}

export interface Made {
  kind: AuthorKind;
  name: string;
  project: string | null;
  atMs: number;
}

export interface AuthoringInfo {
  workflowsDisabled: boolean;
  dduckNew: "off" | "ours" | "other";
  made: Made[];
  templates: Record<AuthorKind, string>;
}

export interface ItemText {
  content: string;
  token: string;
  kind: AuthorKind;
  name: string;
  location: string;
  readonly: string | null;
  warnings: string[];
  extraFiles: string[];
}

export interface Produced {
  kind: AuthorKind;
  name: string;
  content: string;
  files: string[];
  problem: string | null;
  warnings: string[];
  risk: SkillRisk;
}

export interface Job {
  token: string;
  kind: AuthorKind;
  createdMs: number;
  bgId: string | null;
  title: string;
  produced: Produced[];
  stray: string[];
}

export const addApi = {
  inspectLink: (input: string) => invoke<LinkPreview>("inspect_link", { input }),
  discardLink: (token: string) => invoke<void>("discard_link", { token }),
  installPlugin: (token: string, plugin: string | null) => invoke<PluginInstall>("install_plugin", { token, plugin }),
  addMarketplace: (token: string) => invoke<PluginOutcome>("add_marketplace", { token }),
  installSkill: (token: string) => invoke<string>("install_skill", { token }),
  info: () => invoke<AuthoringInfo>("authoring_info"),
  create: (kind: AuthorKind, project: string | null, name: string, content: string) => invoke<void>("create_authoring_item", { kind, project, name, content }),
  read: (id: string) => invoke<ItemText>("read_authoring_item", { id }),
  save: (id: string, content: string, token: string) => invoke<void>("save_authoring_item", { id, content, token }),
  startJob: (kind: AuthorKind, request: string) => invoke<{ token: string; bgId: string }>("start_authoring_task", { kind, request }),
  jobs: () => invoke<Job[]>("authoring_jobs"),
  installResult: (token: string, kind: AuthorKind, name: string, project: string | null) => invoke<void>("install_authoring_result", { token, kind, name, project }),
  discardJob: (token: string) => invoke<void>("discard_authoring_job", { token }),
  installCommand: () => invoke<void>("install_authoring_command"),
  removeCommand: () => invoke<void>("remove_authoring_command"),
};

/** Same rule as Rust (`authoring::valid_name`). */
export function nameError(name: string): string | null {
  if (!name) return null;
  return /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(name) ? null : t("extensions.add.create.nameError");
}

/** Fills a bundled template. The description is kept to one line without quotes or backslashes (it lands in YAML or a JS string). */
export function fillTemplate(template: string, name: string, description: string): string {
  const d = Array.from(description, (c) => (c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127 || `'"\\`.includes(c) ? " " : c)).join("").trim();
  return template.split("{{name}}").join(name || "my-item").split("{{description}}").join(d || t("extensions.add.create.templateDescription"));
}

/** Where a new item lands, for display. */
export function targetPath(kind: AuthorKind, project: string | null, name: string): string {
  const root = project ? `${project}/.claude` : "~/.claude";
  const n = name || t("extensions.add.create.namePlaceholder");
  const path = kind === "skill" ? `${root}/skills/${n}/SKILL.md` : kind === "command" ? `${root}/commands/${n}.md` : `${root}/workflows/${n}.js`;
  return isWindows() ? path.replace(/\//g, "\\") : path;
}

/** "새로 만듦": made through D.Duck in the last 7 days, matched by kind, name and folder. */
export function isNew(made: Made[] | undefined, kind: string, name: string, scope: string | null, nowMs: number): boolean {
  return !!made?.some((m) => m.kind === kind && m.name === name && (m.project ?? null) === (scope ?? null) && nowMs - m.atMs < 7 * 86_400_000);
}

export function shortSha(sha: string): string {
  return sha.slice(0, 7);
}

/** "리뷰 대기": items Claude produced that have no problem and were not installed since the job started. */
export function pendingReviews(jobs: Job[] | undefined, made: Made[] | undefined): number {
  return (jobs ?? []).reduce((n, j) => n + j.produced.filter((p) => !p.problem && !made?.some((m) => m.kind === p.kind && m.name === p.name && m.atMs >= j.createdMs)).length, 0);
}
