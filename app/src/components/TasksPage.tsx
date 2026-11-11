import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, FileText, Plus, X } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { api, messageOf, type AgentSession, type AgentsPayload, type AttachResult, type BoardCard, type Status, type TaskRecord, type TaskTag } from "../lib/api";
import { taskTypeLabel, type Pose } from "../lib/agents";
import { boardSummary } from "../lib/boardSummary";
import { CardDuck, roleLine } from "./RoleDuck";
import { useTeamConfig } from "../lib/team";
import { canFollowUp, canMarkReviewed, followUpDraft, parseBrief, type Draft, type TaskTypeId } from "../lib/brief";
import { Composer } from "./Composer";
import {
  canControl,
  cliErrorMessage,
  dateTime,
  GROUP_LABEL,
  groupSessions,
  localized,
  shortPath,
  cardStatusLabel,
  recordStatusLabel,
  statusLabel,
  type Group,
} from "../lib/format";
import { LogView } from "./LogView";
import { CARD_BUTTON, LimitBanner, LimitCardButtons, LimitCardLine, LimitCardNote, useLimitCard } from "./LimitBits";
import { TaskReportDialog } from "./TaskReport";
import { RecordCard, UndoBar, WrapupPanel } from "./WrapupBits";
import { reportChip } from "../lib/report";
import { YELLOW } from "../lib/yellow";
import { Button, Empty, PageHeader, Sheet } from "./ui";
import { StartingCards } from "./StartingCards";
import { useT } from "../i18n/react";

const KIND_LABEL: Record<AgentSession["kind"], string> = localized({ background: "tasks.kind.background", interactive: "tasks.kind.interactive", other: "tasks.kind.misc" });


const COLUMN_HINT: Record<Group, string> = localized({
  queued: "tasks.columnHint.queued",
  running: "tasks.columnHint.running",
  review: "tasks.columnHint.review",
  reviewWait: "tasks.columnHint.reviewWait",
  wrapup: "tasks.columnHint.wrapup",
});

/** Filled dot for live states; an outline ring for ended ones, so the state is not color alone. */
function StatusDot({ status }: { status: AgentSession["status"] }) {
  if (status === "done" || status === "stopped" || status === "unknown") {
    return <span aria-hidden className="h-2 w-2 shrink-0 rounded-full border border-ink-3" />;
  }
  const color: Record<string, string> = { waitingInput: "bg-warn", working: "bg-ok animate-pulse", failed: "bg-danger", idle: "bg-ink-3" };
  return <span aria-hidden className={`h-2 w-2 shrink-0 rounded-full ${color[status]}`} />;
}

function CliNotice({ status }: { status?: Status }) {
  const t = useT();
  if (!status || (status.cli && status.cli.supported)) return null;
  const missing = !status.cli;
  return (
    <div className={`rounded-xl border px-4 py-3 text-[13px] ${missing ? "border-danger/30 bg-danger/5 text-danger" : "border-line bg-sheet text-ink"}`}>
      {missing
        ? t("tasks.cli.missing")
        : t("tasks.cli.unverified", { version: status.cli?.version ?? "" })}
    </div>
  );
}


/** First instruction of a session, read on demand from its transcript and kept only in memory. */
function usePrompt(key: string) {
  // A new task's transcript may not exist yet: keep asking every 5 s until the instruction shows up, then cache it.
  return useQuery({
    queryKey: ["prompt", key],
    queryFn: () => api.sessionPrompt(key),
    staleTime: Infinity,
    gcTime: 10 * 60_000,
    refetchInterval: (q) => (q.state.data ? false : 5_000),
  });
}

const CARD_POSE: Record<AgentSession["status"], Pose> = { working: "working", waitingInput: "waiting", failed: "failed", done: "done", idle: "idle", stopped: "idle", unknown: "idle" };

/**
 * A board card (palette-refresh 2.3): 1 duck + title + report icon, 2 role · persona, 3 brief (two lines), 4 folder +
 * status, then warning lines (report format, usage limit) and one action row under a divider. The card face is the
 * <li>; the select button and the action buttons sit side by side inside it, never nested (2.2).
 */
function Card({ s, selected, onSelect, tag, onDraft, label, expanded, children }: { s: AgentSession; selected: boolean; onSelect: () => void; tag?: TaskTag; onDraft: (d: Draft) => void; label?: string; expanded?: boolean; children?: ReactNode }) {
  const types = useTeamConfig().data?.config.types;
  const typeLabel = taskTypeLabel(tag?.taskType, types);
  const needsYou = s.status === "waitingInput";
  const prompt = usePrompt(s.key);
  const chip = reportChip(tag?.report, s.status);
  const limit = useLimitCard(s);
  // "리뷰했어요" is already the card's status line once marked, so only the button counts here (5.2).
  const follow = canFollowUp(s, tag?.taskType) || (canMarkReviewed(s.status, tag?.taskType, types) && !tag?.reviewed);
  const id = useId();
  const role = roleLine(typeLabel, tag?.agent);
  return (
    <li data-task-card className={`rounded-[10px] bg-sheet pb-2.5 transition-shadow ${needsYou ? YELLOW.waitingCard : "shadow-e1 hover:shadow-e2"} ${selected ? "outline-2 -outline-offset-2 outline-ink" : ""}`}>
      <button
        onClick={onSelect}
        aria-labelledby={`${id}-title`}
        aria-describedby={`${id}-status`}
        aria-current={selected ? "true" : undefined}
        aria-expanded={expanded}
        className="w-full rounded-[10px] px-3 pt-3 text-left"
      >
        <div className="flex items-center gap-2">
          <CardDuck taskType={tag?.taskType} pose={CARD_POSE[s.status]} />
          <span id={`${id}-title`} className="min-w-0 flex-1 truncate text-[14px] font-semibold" title={label ? s.name : undefined}>
            {label || s.name || s.key.slice(0, 8)}
          </span>
          {chip && chip.tone !== "danger" && (
            <span role="img" aria-label={chip.text} title={chip.text} className={`shrink-0 ${chip.tone === "muted" ? "text-ink-3/80" : "text-ink-3"}`}>
              <FileText size={14} aria-hidden />
            </span>
          )}
        </div>
        {role && (
          <p className="mt-0.5 truncate pl-[30px] text-[12px] text-ink-3" title={role}>
            {role}
          </p>
        )}
        {prompt.data && (
          <p className="mt-1.5 line-clamp-2 text-[13px] text-ink-2" title={prompt.data}>
            {prompt.data}
          </p>
        )}
        <div className="mt-1.5 flex items-center gap-2 text-[12px] text-ink-3">
          <span className="min-w-16 flex-1 truncate font-mono" title={s.cwd}>
            {shortPath(s.cwd)}
          </span>
          <span id={`${id}-status`} className={`shrink-0 whitespace-nowrap ${needsYou ? "font-semibold text-ink" : s.status === "failed" ? "text-danger" : ""}`}>
            {cardStatusLabel(s.status, tag)}
          </span>
        </div>
        {chip?.tone === "danger" && (
          <p className="mt-1.5 flex min-w-0 items-center gap-1.5 text-[12px] text-danger">
            <AlertTriangle size={12} aria-hidden className="shrink-0" />
            <span className="min-w-0 truncate">{chip.text}</span>
          </p>
        )}
        {limit && <LimitCardLine limit={limit} />}
      </button>
      {(follow || limit?.hasActions) && (
        <div className="mx-3 mt-2.5 border-t border-line pt-2.5" onClick={(e) => e.stopPropagation()}>
          <div className="flex flex-wrap gap-1">
            {limit && <LimitCardButtons limit={limit} />}
            {follow && <FollowUps s={s} tag={tag} onDraft={onDraft} compact />}
          </div>
          {limit && <LimitCardNote limit={limit} />}
        </div>
      )}
      {children}
    </li>
  );
}

/** "지난 작업": what was started from D.Duck, even after `claude agents` forgot it (D-069). */
/** `#record=<bg id>` from the 리뷰 tab's "작업 보기" on a task the CLI no longer lists (D-196). */
function recordFromHash(): string | null {
  const m = /^#record=([0-9A-Za-z_-]{1,64})$/.exec(window.location.hash);
  return m ? m[1] : null;
}

function History({ live, onDraft }: { live: Set<string>; onDraft: (d: Draft) => void }) {
  const t = useT();
  const qc = useQueryClient();
  const [focus] = useState(recordFromHash);
  const [open, setOpen] = useState(focus !== null);
  useEffect(() => {
    if (focus) history.replaceState(null, "", window.location.pathname + window.location.search);
  }, [focus]);
  const [filter, setFilter] = useState("");
  const [shown, setShown] = useState(20);
  const records = useQuery({ queryKey: ["task-records"], queryFn: api.taskRecords, enabled: open });
  const rows = (records.data ?? []).filter((r) => !live.has(r.bgId)).filter((r) => {
    const f = filter.trim().toLowerCase();
    return !f || (r.name ?? "").toLowerCase().includes(f) || (r.cwd ?? "").toLowerCase().includes(f);
  });
  // The record asked for may sit past the first 20: show up to it.
  const upTo = Math.max(shown, rows.findIndex((r) => r.bgId === focus) + 1);
  return (
    <section aria-label={t("tasks.history.title")} className="pt-2">
      <button onClick={() => setOpen(!open)} aria-expanded={open} className="flex items-center gap-2 text-[13px] font-semibold">
        <span className={`inline-block transition-transform ${open ? "rotate-90" : ""}`}>›</span> {t("tasks.history.title")}
        <span className="text-xs font-normal text-ink-3">{t("tasks.history.description")}</span>
      </button>
      {open && (
        <div className="mt-3 space-y-2">
          <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder={t("tasks.history.filter")} className="h-8 w-64 rounded-lg border border-line bg-sheet px-3 text-[13px]" />
          {rows.length === 0 ? (
            <p className="text-xs text-ink-3">{records.isPending ? t("tasks.loading") : t("tasks.history.empty")}</p>
          ) : (
            <ul className="divide-y divide-line rounded-xl bg-sheet shadow-e1">
              {rows.slice(0, upTo).map((r) => (
                <HistoryRow key={r.bgId} r={r} focus={r.bgId === focus} onDraft={onDraft} onDeleted={() => qc.invalidateQueries({ queryKey: ["task-records"] })} />
              ))}
            </ul>
          )}
          {rows.length > upTo && (
            <Button onClick={() => setShown(upTo + 20)}>{t("tasks.history.more", { count: rows.length - upTo })}</Button>
          )}
        </div>
      )}
    </section>
  );
}

function HistoryRow({ r, focus = false, onDraft, onDeleted }: { r: TaskRecord; focus?: boolean; onDraft: (d: Draft) => void; onDeleted: () => void }) {
  const t = useT();
  const [expanded, setExpanded] = useState(focus);
  const rowRef = useRef<HTMLLIElement>(null);
  useEffect(() => {
    if (!focus) return;
    // After the board above has laid out, so the row really ends up in view; focus marks which row it is.
    const id = window.setTimeout(() => {
      rowRef.current?.scrollIntoView?.({ block: "center" });
      rowRef.current?.querySelector<HTMLButtonElement>("button[aria-expanded]")?.focus();
    }, 150);
    return () => window.clearTimeout(id);
  }, [focus]);
  const [confirm, setConfirm] = useState(false);
  const [report, setReport] = useState(false);
  const detail = useQuery({ queryKey: ["task-record", r.bgId], queryFn: () => api.taskRecord(r.bgId), enabled: expanded, gcTime: 0 });
  const del = useMutation({ mutationFn: () => api.deleteTaskRecord(r.bgId), onSuccess: onDeleted });
  const typeLabel = taskTypeLabel(r.taskType, useTeamConfig().data?.config.types);
  return (
    <li ref={rowRef} className="px-4 py-3" data-record-row={r.bgId}>
      <div className="flex items-center gap-3">
        <button onClick={() => setExpanded(!expanded)} aria-expanded={expanded} className="min-w-0 flex-1 text-left">
          <p className="truncate text-[13px] font-medium">
            {r.name || r.bgId}
            {r.wrapped && <span className="ml-2 rounded-full border border-ok/40 px-2 py-0.5 text-[12px] font-medium text-ok">{t("tasks.history.wrapped")}</span>}
          </p>
          <p className="truncate text-[12px] text-ink-3">
            {typeLabel && `${typeLabel} · `}
            {dateTime(r.createdAt)} · {r.cwd ? shortPath(r.cwd) : "-"}
            {r.agent && ` · ${r.agent}`} · {r.promptLen ? t("tasks.history.promptLen", { count: r.promptLen }) : t("tasks.history.promptNone")}
          </p>
        </button>
        {confirm ? (
          <>
            <Button onClick={() => setConfirm(false)}>{t("common.actions.cancel")}</Button>
            <Button variant="danger" disabled={del.isPending} onClick={() => del.mutate()}>
              {t("tasks.history.delete")}
            </Button>
          </>
        ) : (
          <Button variant="danger" onClick={() => setConfirm(true)} aria-label={t("tasks.history.deleteLabel", { name: r.name ?? r.bgId })}>
            {t("tasks.history.delete")}
          </Button>
        )}
      </div>
      {expanded && (
        <div className="mt-2 space-y-2">
          {detail.data?.prompt ? (
            <p className="max-h-48 overflow-auto rounded-lg bg-paper px-3 py-2 text-[13px] whitespace-pre-wrap select-text">{detail.data.prompt}</p>
          ) : (
            <p className="text-xs text-ink-3">{detail.isPending ? t("tasks.loading") : t("tasks.history.noPrompt")}</p>
          )}
          {detail.data?.prompt && r.cwd && (
            <Button
              onClick={() =>
                onDraft({
                  cwd: r.cwd as string,
                  taskType: (r.taskType as TaskTypeId | null) ?? null,
                  brief: detail.data?.brief ?? parseBrief(detail.data?.prompt ?? ""),
                  name: r.name ?? undefined,
                  // Keeps the plan-only rule of a draft from Claude's advice (the backend also checks the brief).
                  origin: detail.data?.origin ?? undefined,
                })
              }
            >
              {t("tasks.history.retry")}
            </Button>
          )}
          {r.hasReport && <Button onClick={() => setReport(true)}>{t("tasks.history.openReport")}</Button>}
        </div>
      )}
      {report && (
        <TaskReportDialog
          id={r.bgId}
          title={r.name || r.bgId}
          session={null}
          onClose={() => setReport(false)}
          onRetry={
            r.cwd
              ? (brief) =>
                  onDraft({
                    cwd: r.cwd as string,
                    taskType: (r.taskType as TaskTypeId | null) ?? null,
                    brief: brief ?? parseBrief(detail.data?.prompt ?? ""),
                    name: r.name ?? undefined,
                    origin: detail.data?.origin ?? undefined,
                  })
              : undefined
          }
        />
      )}
    </li>
  );
}

function FollowUps({ s, tag, onDraft, compact }: { s: AgentSession; tag?: TaskTag; onDraft: (d: Draft) => void; compact?: boolean }) {
  const t = useT();
  const qc = useQueryClient();
  const reviewed = useMutation({
    mutationFn: () => api.markReviewed(s.id as string),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["agents"] }),
  });
  const follow = canFollowUp(s, tag?.taskType);
  const review = canMarkReviewed(s.status, tag?.taskType, useTeamConfig().data?.config.types) && !!s.id;
  if (!follow && !review) return null;
  const size = compact ? CARD_BUTTON : "md";
  // New sessions, not forks: the author is the worst reviewer of its own code (AG-06, AG-14).
  // Compact (on a card): the buttons join the card's one action row, after the limit buttons.
  return (
    <div className={compact ? "contents" : "flex flex-wrap gap-1.5"} onClick={(e) => e.stopPropagation()}>
      {follow && (
        <>
          <Button size={size} onClick={() => onDraft(followUpDraft(s, "qa"))}>
            {t("tasks.followUp.qa")}
          </Button>
          <Button size={size} onClick={() => onDraft(followUpDraft(s, "review"))}>
            {t("tasks.followUp.review")}
          </Button>
        </>
      )}
      {review && !tag?.reviewed && (
        <Button size={size} disabled={reviewed.isPending} onClick={() => reviewed.mutate()}>
          {t("tasks.followUp.markReviewed")}
        </Button>
      )}
      {/* On a card the status line already says 리뷰했어요; the drawer keeps this line (5.2). */}
      {review && tag?.reviewed && !compact && <span className="self-center text-[12px] text-ok">{t("tasks.followUp.reviewed")}</span>}
    </div>
  );
}

function Detail({ s, onClose, tag, onDraft, label }: { s: AgentSession; onClose: () => void; tag?: TaskTag; onDraft: (d: Draft) => void; label?: string }) {
  const t = useT();
  const typeName = taskTypeLabel(tag?.taskType, useTeamConfig().data?.config.types);
  const [confirm, setConfirm] = useState<"stop" | "kill" | "finish" | null>(null);
  const [attach, setAttach] = useState<AttachResult | null>(null);
  const [copied, setCopied] = useState(false);
  const closeRef = useRef<HTMLButtonElement>(null);
  const stop = useMutation({ mutationFn: () => api.stopTask(s.id as string), onSettled: () => setConfirm(null) });
  const kill = useMutation({ mutationFn: () => api.forceStop(s.key), onSettled: () => setConfirm(null) });
  const qc = useQueryClient();
  // The card moves lanes right away (리뷰 대기 or off the board), not on the next poll.
  const finish = useMutation({
    mutationFn: () => api.finishTask(s.id as string),
    onSettled: () => setConfirm(null),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["agents"] });
      void qc.invalidateQueries({ queryKey: ["board-wrapup"] });
    },
  });
  const open = useMutation({ mutationFn: () => api.openTerminal(s.key), onSuccess: setAttach });
  const control = canControl(s);
  const live = s.status === "working" || s.status === "waitingInput" || s.status === "idle";
  const prompt = usePrompt(s.key);
  const [report, setReport] = useState(false);
  const chip = reportChip(tag?.report, s.status);

  useEffect(() => {
    closeRef.current?.focus();
    // The report dialog handles its own Escape first (capture) and stops it there.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside role="dialog" aria-modal="false" aria-labelledby="task-detail-title" className="absolute inset-y-0 right-0 z-10 flex w-[min(540px,60%)] flex-col border-l border-line bg-sheet shadow-e3">
      <div className="flex items-start gap-3 border-b border-line px-5 py-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <StatusDot status={s.status} />
            <h2 id="task-detail-title" className="truncate text-[14px] font-semibold" title={label ? s.name : undefined}>
              {label || s.name || s.key.slice(0, 8)}
            </h2>
            <span className="shrink-0 text-xs text-ink-3">{statusLabel(s.status, tag)}</span>
          </div>
          <p className="mt-1 truncate font-mono text-xs text-ink-3" title={s.cwd}>
            {s.cwd}
          </p>
          <p className="text-xs text-ink-3">
            {t("tasks.detail.meta", { kind: KIND_LABEL[s.kind], time: dateTime(s.startedAt) })}
            {typeName && ` · ${typeName}`}
            {tag?.agent && ` · ${t("tasks.card.persona", { name: tag.agent })}`}
          </p>
        </div>
        <button ref={closeRef} onClick={onClose} aria-label={t("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
          <X size={18} />
        </button>
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-auto p-5">
        <FollowUps s={s} tag={tag} onDraft={onDraft} />
        {!!tag?.loadoutDropped && (
          <p role="note" className="rounded-lg border border-line px-3 py-2 text-xs text-ink-2">
            {t("tasks.detail.loadoutDropped", { count: tag.loadoutDropped })}
          </p>
        )}
        <section aria-label={t("tasks.detail.firstPrompt")}>
          <h3 className="mb-1 text-xs font-medium text-ink-2">{t("tasks.detail.firstPrompt")}</h3>
          {prompt.isLoading ? (
            <p className="text-xs text-ink-3">{t("tasks.detail.loadingPrompt")}</p>
          ) : prompt.data ? (
            <>
              <p className="max-h-48 overflow-auto rounded-lg bg-paper px-3 py-2 text-[13px] whitespace-pre-wrap text-ink select-text">{prompt.data}</p>
              {/* "Rewind and fix the brief" instead of arguing with the agent: a new task with the same brief. */}
              <button
                className="mt-1.5 text-xs text-ink-2 underline underline-offset-2 hover:text-ink"
                onClick={() => onDraft({ cwd: s.cwd, taskType: (tag?.taskType as TaskTypeId | null) ?? null, brief: parseBrief(prompt.data as string), origin: tag?.origin })}
              >
                {t("tasks.history.retry")}
              </button>
            </>
          ) : (
            <p className="text-xs text-ink-3">{t("tasks.detail.promptMissing")}</p>
          )}
        </section>
        {tag?.report && s.id && (
          <section aria-label={t("tasks.detail.reportTitle")} className="flex items-center gap-3 rounded-xl border border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <h3 className="text-xs font-medium text-ink-2">{t("tasks.detail.reportTitle")}</h3>
              <p className={`text-xs ${chip?.tone === "danger" ? "text-danger" : "text-ink-3"}`}>
                {t(tag.report === "ready" ? "tasks.detail.reportReady" : tag.report === "invalid" ? "tasks.detail.reportInvalid" : live ? "tasks.detail.reportFilling" : "tasks.detail.reportNone")}
              </p>
            </div>
            <Button onClick={() => setReport(true)}>{t("tasks.history.openReport")}</Button>
          </section>
        )}
        {report && s.id && (
          <TaskReportDialog
            id={s.id}
            title={label || s.name || s.key.slice(0, 8)}
            session={s.status}
            onClose={() => setReport(false)}
            onRetry={(brief) => onDraft({ cwd: s.cwd, taskType: (tag?.taskType as TaskTypeId | null) ?? null, brief: brief ?? parseBrief(prompt.data ?? ""), origin: tag?.origin })}
          />
        )}
        {s.status === "waitingInput" && <div className="rounded-xl border border-warn/30 bg-warn-soft px-4 py-3 text-[13px]">{t("tasks.detail.waiting")}{control && t("tasks.detail.waitingFinish")}</div>}
        {s.status === "failed" && <div className="rounded-xl border border-danger/30 bg-danger/5 px-4 py-3 text-[13px] text-danger">{t("tasks.detail.failed")}</div>}
        <div className="flex flex-wrap gap-2">
          <Button variant={s.status === "waitingInput" ? "primary" : "quiet"} onClick={() => open.mutate()}>
            {control ? t("tasks.detail.openTerminal") : t("tasks.detail.openFolder")}
          </Button>
          {confirm ? (
            <>
              <Button variant={confirm === "finish" ? "primary" : "danger"} onClick={() => (confirm === "stop" ? stop.mutate() : confirm === "finish" ? finish.mutate() : kill.mutate())}>
                {t(confirm === "stop" ? "tasks.detail.confirmStop" : confirm === "finish" ? "tasks.detail.finish" : "tasks.detail.confirmKill")}
              </Button>
              <Button onClick={() => setConfirm(null)}>{t("common.actions.cancel")}</Button>
            </>
          ) : (
            <>
              {control && (
                <Button disabled={!live || finish.isPending} onClick={() => setConfirm("finish")} title={t("tasks.detail.finishTitle")}>
                  {t("tasks.detail.finish")}
                </Button>
              )}
              {control && (
                <Button disabled={!live} onClick={() => setConfirm("stop")}>
                  {t("tasks.detail.stop")}
                </Button>
              )}
              <Button variant="danger" disabled={!live || !s.pid} onClick={() => setConfirm("kill")}>
                {t("tasks.detail.kill")}
              </Button>
            </>
          )}
        </div>
        <p className="text-xs text-ink-3">
          {control
            ? confirm === "finish"
              ? t("tasks.detail.finishHelp")
              : t("tasks.detail.controlHelp")
            : t("tasks.detail.externalHelp")}
        </p>
        {[stop, kill, open, finish].map((m, i) => m.isError && <p key={i} className="text-xs text-danger">{cliErrorMessage(m.error)}</p>)}
        {finish.isSuccess && <p className="text-xs text-ok">{t(finish.data === "reviewWait" ? "tasks.detail.finishedReview" : "tasks.detail.finishedDone")}</p>}
        {kill.isSuccess && <p className="text-xs text-ok">{t("tasks.detail.killed")}</p>}
        {attach && !attach.opened && (
          <div className="space-y-2 rounded-xl border border-line p-3 text-xs">
            <p className="text-ink-2">{t("tasks.detail.attachFailed")}</p>
            <code className="block rounded-lg bg-paper p-2 font-mono break-all">{attach.command}</code>
            <Button
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(attach.command);
                  setCopied(true);
                } catch {
                  setCopied(false);
                }
              }}
            >
              {copied ? t("tasks.detail.copied") : t("tasks.detail.copyCommand")}
            </Button>
          </div>
        )}
        {control && <LogView id={s.id as string} live={s.status === "working" || s.status === "waitingInput"} />}
      </div>
    </aside>
  );
}

/** One card in a column: a live session, or a D.Duck record the CLI no longer lists (리뷰 대기·정리 only). */
type Entry = { kind: "live"; s: AgentSession } | { kind: "record"; c: BoardCard };


export function TasksPage({
  status,
  agents,
  selected,
  onSelect,
  draft,
  onDraft,
  onOpenReview,
}: {
  status?: Status;
  agents?: AgentsPayload;
  selected: string | null;
  onSelect: (key: string | null) => void;
  /** Prefilled composer (follow-ups, "다시 맡기기"); `undefined` = closed. */
  draft: Draft | null | undefined;
  onDraft: (d: Draft | null | undefined) => void;
  /** "리뷰 탭에서 모두 보기" past 40 cards. */
  onOpenReview?: () => void;
}) {
  const t = useT();
  const qc = useQueryClient();
  const composing = draft !== undefined;
  const setComposing = (open: boolean) => onDraft(open ? null : undefined);
  const reviewRef = useRef<HTMLElement>(null);
  useEffect(() => {
    // "막힌 작업 N개 먼저 풀기" lands here with the review column in view.
    if (window.location.hash === "#review") {
      reviewRef.current?.scrollIntoView({ block: "start" });
      history.replaceState(null, "", window.location.pathname + window.location.search);
    }
  }, []);
  const sessions = agents?.sessions ?? [];
  const tags = agents?.tags;
  const current = sessions.find((s) => s.key === selected) ?? null;
  const columns = groupSessions(sessions, tags);
  // Read on entering the board and again when the review badge or the listing changes (no timer of its own).
  // 정리 shows 40 record cards, then "더 보기" asks for 40 more (5.2).
  const [wrapCards, setWrapCards] = useState(40);
  const board = useQuery({
    queryKey: ["board-wrapup", agents?.reviewPending ?? null, sessions.map((x) => `${x.key}:${x.status}`).join(","), wrapCards],
    queryFn: () => api.boardWrapup(wrapCards),
    placeholderData: (prev) => prev,
  });
  const [expanded, setExpanded] = useState<string | null>(null);
  const [report, setReport] = useState<{ id: string; title: string; session: AgentSession["status"] | null } | null>(null);
  const [undo, setUndo] = useState<string | null>(null);
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["agents"] });
    void qc.invalidateQueries({ queryKey: ["board-wrapup"] });
  };
  const wrap = useMutation({
    mutationFn: (id: string) => api.wrapTask(id),
    onSuccess: (_d, id) => {
      setExpanded(null);
      setUndo(id);
      refresh();
    },
  });
  const unwrap = useMutation({ mutationFn: (id: string) => api.unwrapTask(id), onSettled: () => (setUndo(null), refresh()) });
  const reviewed = useMutation({
    mutationFn: (id: string) => api.inboxMarkReviewed(id),
    onSuccess: () => (setReport(null), refresh()),
  });
  const endUndo = useCallback(() => setUndo(null), []);

  // 리뷰 대기: the backend's rows (total = the review badge); a listed one is drawn as its live card.
  const liveOf = (g: Group) => columns.find((c) => c.group === g)?.sessions ?? [];
  const entriesOf = (g: Group): { list: Entry[]; total: number; more: boolean } => {
    const live = liveOf(g);
    if (g === "reviewWait") {
      const byId = new Map(live.filter((x) => x.id).map((x) => [x.id as string, x]));
      const items = board.data?.reviewWait.items ?? [];
      const list: Entry[] = items.map((c) => {
        const s = c.listed ? byId.get(c.bgId) : undefined;
        return s ? { kind: "live", s } : { kind: "record", c };
      });
      const shown = new Set(items.map((c) => c.bgId));
      for (const s of live) if (!s.id || !shown.has(s.id)) list.push({ kind: "live", s });
      const total = board.data ? Math.max(board.data.reviewWait.total, list.length) : list.length;
      return { list, total, more: total > list.length };
    }
    if (g === "wrapup") {
      const records = board.data?.wrapup.items ?? [];
      const list: Entry[] = [...live.map((s): Entry => ({ kind: "live", s })), ...records.map((c): Entry => ({ kind: "record", c }))];
      return { list, total: live.length + (board.data?.wrapup.total ?? 0), more: (board.data?.wrapup.total ?? 0) > records.length };
    }
    return { list: live.map((s) => ({ kind: "live", s })), total: live.length, more: false };
  };
  const boardRows = (board.data?.reviewWait.total ?? 0) + (board.data?.wrapup.total ?? 0);
  // Nothing live and nothing waiting for review or wrap-up: the empty box (the board's queries finished first).
  const empty = sessions.length === 0 && boardRows === 0 && !board.isPending && !composing;

  const entryKey = (e: Entry) => (e.kind === "live" ? e.s.key : e.c.bgId);
  const renderEntry = (g: Group, e: Entry) => {
    const key = entryKey(e);
    const open = expanded === key;
    const panel = g === "wrapup" && open && (
      <WrapupPanel
        bgId={e.kind === "live" ? (e.s.id ?? null) : e.c.bgId}
        busy={wrap.isPending}
        onWrap={() => wrap.mutate(key)}
        onDetail={e.kind === "live" ? () => onSelect(e.s.key) : undefined}
      />
    );
    const onOpen = () => {
      if (g === "wrapup") return setExpanded(open ? null : key);
      if (g === "reviewWait") {
        const id = e.kind === "live" ? e.s.id : e.c.bgId;
        const title = e.kind === "live" ? agents?.labels?.[e.s.key] || e.s.name || e.s.key.slice(0, 8) : e.c.name || e.c.bgId;
        if (id) return setReport({ id, title, session: e.kind === "live" ? e.s.status : null });
      }
      if (e.kind === "live") onSelect(e.s.key);
    };
    if (e.kind === "record") {
      return (
        <RecordCard key={key} c={e.c} line={recordStatusLabel(e.c, g === "wrapup" ? "wrapup" : "reviewWait")} onOpen={onOpen} expanded={g === "wrapup" ? open : undefined}>
          {panel}
        </RecordCard>
      );
    }
    return (
      <Card key={key} s={e.s} tag={tags?.[e.s.key]} label={agents?.labels?.[e.s.key]} onDraft={onDraft} selected={current?.key === e.s.key} onSelect={onOpen} expanded={g === "wrapup" ? open : undefined}>
        {panel}
      </Card>
    );
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("tasks.page.title")} summary={boardSummary({ waiting: sessions.filter((x) => x.status === "waitingInput").length, review: agents?.reviewPending ?? 0 })}>
        <Button variant="primary" disabled={!status?.cli} onClick={() => setComposing(true)}>
          <Plus size={14} />
          {t("tasks.page.newTask")}
        </Button>
      </PageHeader>
      <div className="relative min-h-0 flex-1">
        {/* With the detail open the board makes room for it instead of being covered. */}
        {/* No bottom padding on the scroller: sticky bottom-0 is measured inside it, so padding left a strip under the
            composer footer where the form showed through (D-159). The spacer at the end keeps the 24px. */}
        {/* The right padding is the drawer's own width (min(540px, 60%) of this box) plus the 24px gutter, so
            at 900px the board keeps ~210px cards instead of being pushed under a fixed 564px (2.3). */}
        <div className={`h-full space-y-4 overflow-auto px-6 pt-6 ${current ? "pr-[calc(min(540px,60%)+24px)]" : ""}`}>
          <CliNotice status={status} />
          <LimitBanner />
          {composing && <Composer key={JSON.stringify(draft ?? {})} status={status} agents={agents} draft={draft ?? null} onClose={() => setComposing(false)} />}
          {!composing && <StartingCards sessions={agents?.sessions} />}
          {agents?.error && <p className="text-xs text-danger">{cliErrorMessage(agents.error)}</p>}
          {wrap.isError && <p className="text-xs text-danger">{messageOf(wrap.error)}</p>}
          {empty ? (
            <Sheet>
              <Empty title={t("tasks.page.emptyTitle")}>{t("tasks.page.emptyBody")}</Empty>
            </Sheet>
          ) : (
            <div className={`grid items-start gap-x-3 gap-y-4 ${current ? "grid-cols-1 2xl:grid-cols-2" : "grid-cols-2 lg:grid-cols-3 xl:grid-cols-5"}`}>
              {columns.map(({ group }) => {
                const { list, total, more } = entriesOf(group);
                const waiting = list.some((e) => e.kind === "live" && e.s.status === "waitingInput");
                return (
                  <section key={group} ref={group === "review" ? reviewRef : undefined} id={`column-${group}`} aria-label={GROUP_LABEL[group]} className="min-w-0 scroll-mt-4 rounded-xl bg-well p-2" data-board-column>
                    <header className="mb-2 px-1.5 pt-1" data-waiting={waiting || undefined}>
                      <div className="flex items-baseline gap-2">
                        <h3 className="text-[13px] font-semibold">{GROUP_LABEL[group]}</h3>
                        {/* ink-2 on the well: ink-3 there is 4.33:1 in light mode (5.2 brand). */}
                        <span className="text-xs text-ink-2" data-column-total>
                          {total}
                        </span>
                      </div>
                      <p className="truncate text-[12px] text-ink-2" title={COLUMN_HINT[group]}>
                        {COLUMN_HINT[group]}
                      </p>
                    </header>
                    {list.length === 0 ? (
                      <p className="px-1 py-3 text-[12px] text-ink-2">{t("tasks.page.none")}</p>
                    ) : (
                      <ul className="space-y-2">{list.map((e) => renderEntry(group, e))}</ul>
                    )}
                    {more && (
                      <button
                        onClick={group === "wrapup" ? () => setWrapCards((n) => n + 40) : onOpenReview}
                        className="mt-2 px-1 text-[12px] text-ink-2 underline underline-offset-2 hover:text-ink"
                      >
                        {t(group === "wrapup" ? "tasks.page.showMore" : "tasks.page.showAll", { count: total - list.length })}
                      </button>
                    )}
                  </section>
                );
              })}
            </div>
          )}
          <History live={new Set(sessions.map((x) => x.id ?? "").filter(Boolean))} onDraft={onDraft} />
          <div aria-hidden className="h-2" />
        </div>
        {/* key resets confirm/attach state per session, so a pending "강제 종료하기" can never apply to another one. */}
        {current && <Detail key={current.key} s={current} tag={tags?.[current.key]} label={agents?.labels?.[current.key]} onDraft={onDraft} onClose={() => onSelect(null)} />}
      </div>
      {report && (
        <TaskReportDialog
          key={report.id}
          id={report.id}
          title={report.title}
          session={report.session}
          stage="reviewWait"
          onClose={() => setReport(null)}
          footer={
            <>
              {reviewed.isError && <span className="text-xs text-danger">{String(reviewed.error)}</span>}
              <Button variant="primary" disabled={reviewed.isPending} onClick={() => reviewed.mutate(report.id)}>
                {t("tasks.followUp.markReviewed")}
              </Button>
              <Button onClick={() => setReport(null)}>{t("common.actions.close")}</Button>
            </>
          }
        />
      )}
      {/* key: a second 정리 끝 restarts the 5 seconds instead of inheriting the first one's timer (5.2). */}
      {undo && <UndoBar key={undo} onUndo={() => unwrap.mutate(undo)} onDone={endUndo} />}
    </div>
  );
}
