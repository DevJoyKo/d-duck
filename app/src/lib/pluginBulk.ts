// Update several plugins in one go (plugin-bulk-update, D-203). The rules that matter for safety live in the shell
// (plugin_bulk.rs: re-read list, one confirmation naming every target, re-check before each, one at a time); this
// module picks what the screen offers and keeps the batch state outside any component, so leaving the 확장 tab and
// coming back still shows the progress (design review HIGH).
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import {
  api,
  failure,
  type Plugin,
  type PluginBulkItem,
  type PluginBulkStatus,
  type Shown,
} from "./api";

/** Same cap as plugin_bulk.rs `MAX_RUN`: every target fits in the confirmation. */
export const MAX_BULK = 15;

/** A checkbox is offered when the single update button would be enabled: global scope, not up to date. */
export const selectable = (p: Plugin) =>
  !p.locked && p.scope === "user" && p.update.state !== "latest";

/** "모두 업데이트": only plugins with a known newer version (unknown ones may be up to date; pick them by hand). */
export const allTargets = (plugins: Plugin[]) =>
  plugins
    .filter((p) => selectable(p) && p.update.state === "available")
    .map((p) => p.id);

export type RowState = { kind: "waiting" } | PluginBulkStatus;

export interface Counts {
  updated: number;
  failed: number;
  approval: number;
  unchanged: number;
  stopped: number;
  skipped: number;
}

export function countItems(items: PluginBulkItem[]): Counts {
  const c: Counts = {
    updated: 0,
    failed: 0,
    approval: 0,
    unchanged: 0,
    stopped: 0,
    skipped: 0,
  };
  for (const { status: s } of items) {
    if (s.kind === "stopped") c.stopped++;
    else if (s.kind === "skipped") c.skipped++;
    else if (s.kind === "finished") {
      const k = s.outcome.kind;
      if (k === "done") c.updated++;
      else if (k === "needsApproval") c.approval++;
      else if (k === "unchanged") c.unchanged++;
      else c.failed++;
    }
  }
  return c;
}

/** Non-zero parts in a fixed order; the screen joins their catalog lines with ", ". */
export const SUMMARY_ORDER: (keyof Counts)[] = [
  "updated",
  "failed",
  "approval",
  "unchanged",
  "stopped",
  "skipped",
];

export type Phase = "idle" | "confirming" | "running" | "stopping";

export interface BulkState {
  phase: Phase;
  selected: string[];
  /** The ids sent with the running batch (progress "done/total" counts these). */
  ids: string[];
  rows: Record<string, RowState>;
  counts: Counts | null;
  error: Shown | null;
}

const initial: BulkState = {
  phase: "idle",
  selected: [],
  ids: [],
  rows: {},
  counts: null,
  error: null,
};
let state: BulkState = initial;
const subs = new Set<() => void>();

function set(next: Partial<BulkState>) {
  state = { ...state, ...next };
  subs.forEach((f) => f());
}

export const bulkStore = {
  get: () => state,
  subscribe(f: () => void) {
    subs.add(f);
    return () => subs.delete(f);
  },
  /** Tests only. */
  reset() {
    set(initial);
  },
  toggle(id: string, on: boolean) {
    if (state.phase !== "idle") return;
    const rest = state.selected.filter((x) => x !== id);
    if (on && rest.length >= MAX_BULK) return;
    set({ selected: on ? [...rest, id] : rest });
  },
  stop() {
    if (state.phase !== "running") return;
    set({ phase: "stopping" });
    api.stopPluginUpdates().catch((e: unknown) => set({ error: failure(e) }));
  },
  /** A single update or toggle of this plugin starts: its old batch line must not hide the new note. */
  clearRow(id: string) {
    if (!(id in state.rows)) return;
    const rows = { ...state.rows };
    delete rows[id];
    set({ rows });
  },
  /** Drops picks that are no longer offered (uninstalled, now up to date), so the cap counts real ones. */
  prune(offered: Set<string>) {
    if (state.phase !== "idle" || state.selected.every((id) => offered.has(id)))
      return;
    set({ selected: state.selected.filter((id) => offered.has(id)) });
  },
  /** Runs one batch; `onSettled` re-reads the plugin list. A declined confirmation changes nothing. */
  async start(ids: string[], onSettled: () => void) {
    if (state.phase !== "idle" || ids.length === 0) return;
    const before = { rows: state.rows, counts: state.counts };
    set({ phase: "confirming", ids, error: null });
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<PluginBulkItem>("plugin-update-progress", (e) => {
        const it = e.payload;
        // The first event means the confirmation was accepted: everything not reported yet is waiting.
        const rows: Record<string, RowState> =
          state.phase === "confirming"
            ? Object.fromEntries(
                state.ids.map((id) => [id, { kind: "waiting" } as RowState]),
              )
            : { ...state.rows };
        rows[it.id] = it.status;
        set({
          rows,
          counts: null,
          phase: state.phase === "confirming" ? "running" : state.phase,
        });
      });
      const r = await api.updatePlugins(ids);
      if (!r.confirmed && r.items.length === 0) {
        set({ ...before });
      } else {
        const rows: Record<string, RowState> = {};
        for (const it of r.items) if (it.id) rows[it.id] = it.status;
        const done = new Set(
          r.items
            .filter(
              (it) =>
                it.status.kind === "finished" &&
                it.status.outcome.kind === "done",
            )
            .map((it) => it.id),
        );
        set({
          rows,
          counts: countItems(r.items),
          selected: state.selected.filter((id) => !done.has(id)),
        });
      }
    } catch (e) {
      set({ ...before, error: failure(e) });
    } finally {
      unlisten?.();
      set({ phase: "idle", ids: [] });
      onSettled();
    }
  },
};

export function useBulk(): BulkState {
  return useSyncExternalStore(bulkStore.subscribe, bulkStore.get);
}

/** Finished, skipped or stopped rows of the running batch (for "업데이트 중 2/3"). */
export function progressOf(s: BulkState): { done: number; total: number } {
  const done = s.ids.filter((id) => {
    const r = s.rows[id];
    return r && r.kind !== "waiting" && r.kind !== "running";
  }).length;
  return { done, total: s.ids.length };
}
