// "모두 업데이트" / "선택한 것 업데이트" (plugin-bulk-update, D-203): the bar above the 확장 list, the row checkbox
// and the row's one-line batch status. State lives in lib/pluginBulk.ts.
import { type Plugin, shown } from "../lib/api";
import { allTargets, bulkStore, MAX_BULK, progressOf, type RowState, selectable, SUMMARY_ORDER, useBulk } from "../lib/pluginBulk";
import { useT } from "../i18n/react";
import { noteFor } from "../lib/pluginNotes";
import { Button } from "./ui";

/** Buttons, progress and the summary line. Hidden when nothing can be updated and no batch has run. */
export function PluginBulkBar({ plugins, busy, onSettled }: { plugins: Plugin[]; busy: boolean; onSettled: () => void }) {
  const t = useT();
  const s = useBulk();
  const all = allTargets(plugins);
  const allIds = all.slice(0, MAX_BULK);
  // A selection can outlive a plugin (uninstalled, now up to date): count only what is still offered.
  // Runs in the order the list shows (not the click order).
  const offered = new Set(plugins.filter(selectable).map((p) => p.id));
  const picked = plugins.filter((p) => offered.has(p.id) && s.selected.includes(p.id)).map((p) => p.id);
  const idle = s.phase === "idle";
  const summary = s.counts ? SUMMARY_ORDER.filter((k) => s.counts![k] > 0).map((k) => t(`extensions.plugins.bulk.summary.${k}`, { count: s.counts![k] })) : [];
  if (offered.size === 0 && idle && summary.length === 0 && !s.error) return null;
  const { done, total } = progressOf(s);
  const status = s.phase === "confirming" ? t("extensions.plugins.bulk.confirming") : !idle ? t("extensions.plugins.bulk.progress", { done: String(Math.min(done + 1, total)), total: String(total) }) : summary.join(", ");
  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      {status ? (
        <p role="status" className="mr-auto min-w-0 truncate text-xs text-ink-2">
          {status}
        </p>
      ) : (
        <span className="mr-auto" />
      )}
      {s.error && (
        <p role="alert" className="w-full text-right text-xs text-danger">
          {shown(s.error)}
        </p>
      )}
      {(all.length > MAX_BULK || picked.length >= MAX_BULK) && idle && <span className="text-xs text-ink-3">{t("extensions.plugins.bulk.cap", { count: MAX_BULK })}</span>}
      {idle ? (
        <>
          {allIds.length > 0 && (
            <Button size="sm" disabled={busy} onClick={() => void bulkStore.start(allIds, onSettled)}>
              {t("extensions.plugins.bulk.all", { count: allIds.length })}
            </Button>
          )}
          <Button size="sm" disabled={busy || picked.length === 0} onClick={() => void bulkStore.start(picked, onSettled)}>
            {t("extensions.plugins.bulk.selected", { count: picked.length })}
          </Button>
        </>
      ) : (
        <Button size="sm" disabled={s.phase !== "running"} onClick={() => bulkStore.stop()}>
          {s.phase === "stopping" ? t("extensions.plugins.bulk.stopping") : t("extensions.plugins.bulk.stop")}
        </Button>
      )}
    </div>
  );
}

/** The row checkbox, or a same-width blank for plugins that can't be picked (up to date, locked). */
export function PluginBulkCheck({ p, busy }: { p: Plugin; busy: boolean }) {
  const t = useT();
  const s = useBulk();
  if (!selectable(p)) return <span className="w-4 shrink-0" aria-hidden />;
  const on = s.selected.includes(p.id);
  const full = !on && s.selected.length >= MAX_BULK;
  return (
    <input
      type="checkbox"
      className="h-4 w-4 shrink-0 accent-ink"
      aria-label={t("extensions.plugins.bulk.select", { name: p.name })}
      title={full ? t("extensions.plugins.bulk.cap", { count: MAX_BULK }) : undefined}
      checked={on}
      disabled={busy || s.phase !== "idle" || full}
      onChange={(e) => bulkStore.toggle(p.id, e.target.checked)}
    />
  );
}

/** One line under the row while a batch runs or after it ran; null when the batch never touched this plugin. */
export function useBulkLine(p: Plugin): { tone: "ok" | "warn" | "error" | "quiet"; text: string } | null {
  const t = useT();
  const r: RowState | undefined = useBulk().rows[p.id];
  if (!r) return null;
  switch (r.kind) {
    case "waiting":
      return { tone: "quiet", text: t("extensions.plugins.bulk.waiting") };
    case "running":
      return { tone: "quiet", text: t("extensions.plugins.bulk.running") };
    case "stopped":
      return { tone: "quiet", text: t("extensions.plugins.bulk.stopped") };
    case "skipped":
      return { tone: "warn", text: t(`extensions.plugins.bulk.skip.${r.reason}`) };
    case "finished": {
      const n = noteFor(p, r.outcome, "update");
      if (!n) return null;
      const text = shown(n.text);
      return n.tone === "error" ? { tone: "error", text: t("extensions.plugins.bulk.failed", { reason: text }) } : { tone: n.tone, text };
    }
  }
}
