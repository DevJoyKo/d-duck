// "먼저 살펴볼 스킬·에이전트" (preferred-skills, D-198). The backend lists what is installed, checks every item again
// and builds the line at start; this file only shapes data for the picker.
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { baseName } from "./platform";

export type PreferredKind = "plugin" | "skill" | "agent";

export interface PreferredItem {
  kind: PreferredKind;
  name: string;
  /** `user` · `project` · `plugin:<name>` */
  source: string;
  /** Canonical folder of a project item. */
  scope?: string | null;
  /** Plugin-sourced items: the marketplace it was picked from. Missing = any, while the name is in one only. */
  marketplace?: string | null;
}

/** One installed item (`preferred_options`). */
export interface PreferredOption extends PreferredItem {
  description: string | null;
  /** Plugins: short names of what they ship. */
  members: string[];
  marketplace: string | null;
  /** Plugin-sourced: recorded as installed and turned on in Claude Code settings. */
  enabled: boolean;
}

/** Why a chosen item does or doesn't go into the start line. */
export type PreferredStatus = "ok" | "missing" | "off" | "ambiguous";

export const MAX_PREFERRED = 10;
export const KINDS: PreferredKind[] = ["plugin", "skill", "agent"];

export const preferredApi = { options: () => invoke<PreferredOption[]>("preferred_options") };

export function usePreferredOptions() {
  return useQuery({ queryKey: ["preferred-options"], queryFn: preferredApi.options, staleTime: 30_000 });
}

/** Same rule as Rust `Available::matches` (`a` = the chosen item): kind, name and source, the folder for project
 * items, and the marketplace when the item names one. */
export function sameItem(a: PreferredItem, b: PreferredItem): boolean {
  return (
    a.kind === b.kind &&
    a.name === b.name &&
    a.source === b.source &&
    (a.source !== "project" || (a.scope ?? null) === (b.scope ?? null)) &&
    (a.marketplace == null || a.marketplace === (b.marketplace ?? null))
  );
}

export const itemKey = (i: PreferredItem) => `${i.kind}|${i.source}|${i.scope ?? ""}|${i.marketplace ?? ""}|${i.name}`;

/** The stored shape only (the webview never sends descriptions or members back). */
export const toItem = (o: PreferredItem): PreferredItem => ({
  kind: o.kind,
  name: o.name,
  source: o.source,
  ...(o.source === "project" && o.scope ? { scope: o.scope } : {}),
  ...(o.source.startsWith("plugin:") && o.marketplace ? { marketplace: o.marketplace } : {}),
});

/** The one installed option for a chosen item, or `null` (none, or several marketplaces). `undefined` = still unknown. */
export function installedOf(item: PreferredItem, options: PreferredOption[] | undefined): PreferredOption | null | undefined {
  if (!options) return undefined;
  const found = options.filter((o) => sameItem(item, o));
  return found.length === 1 ? found[0] : null;
}

/** Same outcome as Rust `line` for one item: left out when missing, ambiguous or turned off. */
export function statusOf(item: PreferredItem, options: PreferredOption[] | undefined): PreferredStatus | undefined {
  if (!options) return undefined;
  const found = options.filter((o) => sameItem(item, o));
  if (found.length === 0) return "missing";
  if (found.length > 1) return "ambiguous";
  return found[0].enabled ? "ok" : "off";
}

/** Options not chosen yet that match `query` (name or description, any case). */
export function addable(options: PreferredOption[], chosen: PreferredItem[], query: string): PreferredOption[] {
  const q = query.trim().toLowerCase();
  return options.filter((o) => !chosen.some((c) => sameItem(c, o)) && (!q || o.name.toLowerCase().includes(q) || (o.description ?? "").toLowerCase().includes(q)));
}

/** Last part of a project folder, for the row. */
export const folderName = (scope: string | null | undefined) => (scope ? baseName(scope) : null);
