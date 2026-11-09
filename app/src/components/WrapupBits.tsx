// 리뷰 대기·정리 칸 pieces (task-lifecycle): record cards for tasks the CLI no longer lists, the 정리 checklist (local
// git facts only), "worktree 정리" behind the backend's native confirm, and the 5-second "정리 끝 · 되돌리기" bar.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, FileText, GitBranch, Lock } from "lucide-react";
import { useEffect, useId, useState, type ReactNode } from "react";
import { CardDuck } from "./RoleDuck";
import { api, type BoardCard, type WrapupCheck, type WrapupRefusal, failure, messageOf, shown, type Shown } from "../lib/api";
import { taskTypeLabel } from "../lib/agents";
import { useTeamConfig } from "../lib/team";
import { Button } from "./ui";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import { localized } from "../lib/format";

const REFUSAL_TEXT = localized({ running: "tasks.wrapup.refusalRunning", changes: "tasks.wrapup.refusalChanges", notChecked: "tasks.wrapup.refusalNotChecked" });

export const REFUSAL_LINE: Record<WrapupRefusal, string | null> = {
  get running() {
    return REFUSAL_TEXT.running;
  },
  get changes() {
    return REFUSAL_TEXT.changes;
  },
  get notChecked() {
    return REFUSAL_TEXT.notChecked;
  },
  noWorktree: null,
};

/** One line per local fact, in the spec's order. */
export function checkLines(c: WrapupCheck): string[] {
  const out: string[] = [];
  if (c.worktree?.kind === "present") out.push([t("tasks.wrapup.worktreePresent"), c.worktree.folder, c.worktree.branch].filter(Boolean).join(" · "));
  if (c.worktree?.kind === "gone") out.push(t("tasks.wrapup.worktreeGone"));
  if (c.worktree?.kind === "unknown") out.push(t("tasks.wrapup.worktreeUnknown"));
  if (c.changes !== null) out.push(c.changes === 0 ? t("tasks.wrapup.noChanges") : t("tasks.wrapup.changes", { count: c.changes }));
  return out;
}

/** A 리뷰 대기·정리 card for a D.Duck record the CLI no longer lists (no live session to draw). */
export function RecordCard({ c, line, onOpen, expanded, children }: { c: BoardCard; line: string; onOpen: () => void; expanded?: boolean; children?: ReactNode }) {
  const t = useT();
  const typeLabel = taskTypeLabel(c.taskType, useTeamConfig().data?.config.types);
  const id = useId();
  return (
    // Same lines as a live card (palette-refresh 2.3): duck + title + report icon, role, folder + where it stands.
    <li data-board-record={c.bgId} className="rounded-[10px] bg-sheet shadow-e1 transition-shadow hover:shadow-e2">
      <button onClick={onOpen} aria-expanded={expanded} aria-labelledby={`${id}-title`} aria-describedby={`${id}-line`} className="w-full rounded-[10px] px-3 py-3 text-left">
        <div className="flex items-center gap-2">
          <CardDuck taskType={c.taskType} pose="done" />
          <span id={`${id}-title`} className="min-w-0 flex-1 truncate text-[14px] font-semibold">
            {c.name || c.bgId}
          </span>
          <FileText size={14} aria-hidden className="shrink-0 text-ink-3" />
        </div>
        {typeLabel && <p className="mt-0.5 truncate pl-[30px] text-[12px] text-ink-3">{typeLabel}</p>}
        <div className="mt-1.5 flex items-center gap-2 text-[12px] text-ink-3">
          <span className="min-w-16 flex-1 truncate font-mono" title={c.folder ? `${c.folder} · ${t("tasks.wrapup.recordGone")}` : t("tasks.wrapup.recordGone")}>
            {c.folder || t("tasks.wrapup.recordGone")}
          </span>
          <span id={`${id}-line`} className="shrink-0 whitespace-nowrap">
            {line}
          </span>
        </div>
      </button>
      {children}
    </li>
  );
}

/** The opened 정리 card: checklist (read on open only) and its two actions. */
export function WrapupPanel({ bgId, onWrap, onDetail, busy }: { bgId: string | null; onWrap: () => void; onDetail?: () => void; busy?: boolean }) {
  const t = useT();
  const qc = useQueryClient();
  const check = useQuery({ queryKey: ["wrapup-check", bgId], queryFn: () => api.wrapupCheck(bgId as string), enabled: !!bgId, gcTime: 0, retry: false });
  const [linkError, setLinkError] = useState<Shown | null>(null);
  const remove = useMutation({
    mutationFn: () => api.removeWorktree(bgId as string),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["wrapup-check", bgId] }),
  });
  const c = check.data;
  const present = c?.worktree?.kind === "present";
  const reason = c?.refusal ? REFUSAL_LINE[c.refusal] : null;
  return (
    <div className="mt-1 space-y-2 rounded-lg border border-line bg-paper px-3 py-2.5" data-wrapup-panel onClick={(e) => e.stopPropagation()}>
      {bgId && (
        <ul aria-label={t("tasks.wrapup.checks")} className="space-y-1 text-[12px] text-ink-2">
          {check.isPending && <li className="text-ink-3">{t("tasks.wrapup.checking")}</li>}
          {check.isError && <li className="text-ink-3">{t("tasks.wrapup.checkFailed")}</li>}
          {c && checkLines(c).map((l, i) => (
            <li key={l} className="flex items-center gap-1.5">
              {i === 0 && present && <GitBranch size={12} aria-hidden className="shrink-0 text-ink-3" />}
              <span className="min-w-0 truncate">{l}</span>
            </li>
          ))}
          {c?.pr && (
            <li>
              <button
                className="inline-flex items-center gap-1 underline underline-offset-2 hover:text-ink"
                aria-label={t("tasks.wrapup.prLabel", { number: String(c.pr.number) })}
                onClick={() => api.openInsightLink((c.pr as { url: string }).url).then(() => setLinkError(null), (e) => setLinkError(failure(e)))}
              >
                {t("tasks.wrapup.pr", { number: String(c.pr.number) })} <ExternalLink size={11} aria-hidden />
              </button>
            </li>
          )}
          {c?.guard && (
            <li className="flex items-center gap-1.5">
              {c.guard === "local_only" && <Lock size={12} aria-hidden className="shrink-0 text-ink-3" />}
              {t(c.guard === "local_only" ? "tasks.delivery.chip" : "tasks.delivery.chipDraft")}
            </li>
          )}
        </ul>
      )}
      {linkError && <p className="text-[11px] text-danger">{shown(linkError)}</p>}
      {present && reason && <p className="text-[11px] text-ink-3">{reason}</p>}
      {remove.isError && <p className="text-[11px] text-danger">{messageOf(remove.error)}</p>}
      <div className="flex flex-wrap gap-1.5">
        {present && (
          <Button size="xs" disabled={!!c?.refusal || remove.isPending} onClick={() => remove.mutate()}>
            {t("tasks.wrapup.removeWorktree")}
          </Button>
        )}
        <Button size="xs" disabled={busy} onClick={onWrap}>
          {t("tasks.wrapup.wrap")}
        </Button>
        {onDetail && (
          <button className="self-center text-[11px] text-ink-3 underline underline-offset-2 hover:text-ink" onClick={onDetail}>
            {t("tasks.wrapup.detail")}
          </button>
        )}
      </div>
    </div>
  );
}

/** "정리 끝 · 되돌리기" for 5 seconds. */
export function UndoBar({ onUndo, onDone }: { onUndo: () => void; onDone: () => void }) {
  const t = useT();
  useEffect(() => {
    const timer = window.setTimeout(onDone, 5_000);
    return () => window.clearTimeout(timer);
  }, [onDone]);
  return (
    <div role="status" data-undo-bar className="fixed bottom-6 left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-xl bg-sheet px-4 py-2.5 text-[13px] shadow-e3">
      <span>{t("tasks.wrapup.wrapped")}</span>
      <button onClick={onUndo} className="font-medium underline underline-offset-2">
        {t("tasks.wrapup.undo")}
      </button>
    </div>
  );
}
