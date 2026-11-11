// 리뷰 탭 (review-inbox, D-140): what each role duck was given and what came back, grouped by role, so the user can go
// through results one after another. Cards carry a summary only (Rust review_inbox); opening one shows the full report
// screen with "리뷰했어요 · 다음" to walk the queue. Starts on 모든 프로젝트; the project side narrows the list
// (screen-local scope, like 확장 and MCP). The list is not polled: it is read on entry, window focus, after a mark,
// and when the sidebar count changes. Yellow is used once: the first card waiting for review.
import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Check, ShieldAlert } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, type AgentsPayload, type InboxFilter, type InboxItem, failure, messageOf, shown, type Shown } from "../lib/api";
import { taskTypeLabel } from "../lib/agents";
import { dateTime } from "../lib/format";
import { alertLines } from "../lib/delivery";
import type { NavTarget } from "../lib/nav";
import { cardView, groupByRole, type CardView } from "../lib/review";
import { markFlag } from "../lib/report";
import { roleColor } from "../lib/roleColors";
import { useTeamConfig } from "../lib/team";
import { DuckAvatar } from "./DuckAvatar";
import { useScope } from "./ProjectContext";
import { RoleChip } from "./RoleDuck";
import { ScopeBar } from "./ScopeBar";
import { Mark, TaskReportDialog } from "./TaskReport";
import { Pager } from "./ArtifactsPage";
import { clampPage, pageCount, PAGE_SIZE } from "../lib/artifactCards";
import { Button, PageBody, PageHeader, Segmented, Sheet } from "./ui";
import { t } from "../i18n";
import { useT } from "../i18n/react";

const FILTERS: InboxFilter[] = ["pending", "all"];
const UNDO_MS = 5_000;
/** The backend counts and serves 500 rows: 50 pages of 10 (review_inbox::MAX_ROWS). */
const MAX_PAGES = Math.ceil(500 / PAGE_SIZE);

// Warn text stays ink-2 (the warn token is too close to the duck's yellow in dark mode); only its icon is colored.
const TONE: Record<CardView["tone"], string> = { plain: "text-ink-2", warn: "text-ink-2", danger: "text-danger" };

/** review-delete: finished cards only, and only once a good poll says which have finished. */
const canDelete = (i: InboxItem, known: boolean) => known && i.running === null;
const canMark = (i: InboxItem) => i.report.kind !== "none" && !i.reviewed && i.running === null;

/** A click on the card body opens the report, unless it ended a text selection, landed on a control, or is the
 *  second or third click of a word or line selection (report-copy-save: the card text can be dragged and copied). */
export function opensOnClick(target: EventTarget | null, selection: string, detail = 1): boolean {
  if (detail > 1 || selection !== "") return false;
  return !(target instanceof Element && target.closest("button, a, input, [role='tab']"));
}

/** How long a single click waits before opening, so a double click (word selection) can cancel it. */
export const OPEN_DELAY_MS = 250;

/** Card body click handler: opens after [`OPEN_DELAY_MS`] unless a further click or a selection came in between. */
function useDelayedOpen(onOpen: () => void) {
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const latest = useRef(onOpen);
  useEffect(() => {
    latest.current = onOpen;
  });
  useEffect(() => () => clearTimeout(timer.current), []);
  const selection = () => window.getSelection()?.toString() ?? "";
  return {
    onClick: (e: React.MouseEvent) => {
      clearTimeout(timer.current);
      if (!opensOnClick(e.target, selection(), e.detail)) return;
      timer.current = setTimeout(() => {
        if (selection() === "") latest.current();
      }, OPEN_DELAY_MS);
    },
    onDoubleClick: () => clearTimeout(timer.current),
  };
}

/** The primary "리뷰했어요" (first card in the queue) or the Tasks tab's normal button. */
function MarkButton({ primary, onClick, disabled, label = t("report.review.mark") }: { primary: boolean; onClick: () => void; disabled?: boolean; label?: string }) {
  if (!primary)
    return (
      <Button onClick={onClick} disabled={disabled} size="sm">
        {label}
      </Button>
    );
  return (
    <Button variant="primary" onClick={onClick} disabled={disabled} size="sm">
      {label}
    </Button>
  );
}

function Card({
  item,
  primary,
  onOpen,
  onMark,
  onUnmark,
  onDelete,
  busy,
  openRef,
}: {
  item: InboxItem;
  primary: boolean;
  onOpen: () => void;
  onMark: () => void;
  onUnmark: () => void;
  /** `null`: no delete on this card (running, or not known yet). */
  onDelete: (() => void) | null;
  busy: boolean;
  openRef: (el: HTMLButtonElement | null) => void;
}) {
  const v = cardView(item);
  // D-196: a task without a report has no report sheet; the card opens the task instead (onOpen is that action).
  const noReport = item.report.kind === "none";
  const body = useDelayedOpen(onOpen);
  // A card without a goal leads with its start time, so several of them can be told apart.
  const title = item.goal ? v.goal : `${dateTime(item.createdAt)} · ${v.goal}`;
  // A routine run says so before its folder (review-routine-runs); its goal is the routine's name.
  const folder = `${item.routine ? `${t("report.delegated.routine")} · ` : ""}${item.folder ?? t("report.review.noFolder")}`;
  const when = item.goal ? `${folder} · ${dateTime(item.createdAt)}` : folder;
  return (
    <li className="flex items-start gap-3 px-1 py-2" data-review-card={item.bgId}>
      {/* Not a button: WebKit cannot drag-select text inside one. Keyboard and assistive tech use "보고서 열기". */}
      <article onClick={body.onClick} onDoubleClick={body.onDoubleClick} aria-label={title} className="min-w-0 flex-1 cursor-pointer rounded-md px-1 text-left hover:bg-paper">
        <span className="selectable block truncate text-[13px] font-medium text-ink" data-card-title>
          {title}
        </span>
        {alertLines(item.deliveryAlert).length > 0 && (
          <span className="selectable mt-0.5 flex flex-wrap items-center gap-x-2 text-xs text-danger" data-delivery-alert>
            <ShieldAlert size={12} strokeWidth={2.2} className="shrink-0" aria-hidden />
            {alertLines(item.deliveryAlert).join(" · ")}
          </span>
        )}
        {/* The verdict comes first and never truncates; folder · time wraps under it when space runs out. */}
        <span className="selectable mt-0.5 flex min-w-0 flex-wrap items-baseline gap-x-2">
          <span className={`inline-flex min-w-0 items-baseline gap-1 text-[13px] wrap-anywhere ${TONE[v.tone]}`} data-tone={v.tone}>
            {v.tone === "warn" && <AlertTriangle size={12} strokeWidth={2.2} className="shrink-0 translate-y-[1px] text-warn" aria-hidden />}
            {v.line}
          </span>
          {noReport && (
            <span className="shrink-0 rounded-full border border-line px-2 py-px text-[11px] text-ink-2" data-no-report data-no-select>
              {t("report.review.noReport")}
            </span>
          )}
          <span className="min-w-0 truncate text-[11px] text-ink-3">{when}</span>
        </span>
        {v.preview.length > 0 && (
          <span className="selectable mt-1 block space-y-0.5" data-preview>
            {v.preview.map((c, i) => (
              <span key={i} className="flex min-w-0 items-center gap-1.5 text-xs">
                <Mark met={c.met} flag={markFlag(c)} small />
                <span className="shrink-0 text-ink">{c.criterion}</span>
                {c.evidence && <span className="min-w-0 truncate text-ink-3">{c.evidence}</span>}
              </span>
            ))}
            {v.more > 0 && <span className="block pl-[22px] text-[11px] text-ink-3">{t("report.review.more", { count: v.more })}</span>}
          </span>
        )}
        {v.attachment && (
          <span className="selectable mt-1 block space-y-0.5 text-xs" data-attachment-line>
            {v.attachment.title && <span className="block truncate font-medium text-ink">{v.attachment.title}</span>}
            {v.attachment.line && <span className="block truncate text-ink-3">{v.attachment.line}</span>}
          </span>
        )}
        {(v.counts || v.failedRows > 0 || v.first || v.evidence) && (
          <span className="selectable mt-1 flex min-w-0 flex-wrap items-center gap-2">
            {(v.counts || v.failedRows > 0) && (
              <span className="shrink-0 text-xs text-ink-2" data-no-select>
                {v.counts}
                {v.failedRows > 0 && (
                  <span className="font-semibold text-danger" data-tests-failed>
                    {v.counts ? " · " : ""}
                    {t("report.review.testsFailed", { n: v.failedRows })}
                  </span>
                )}
              </span>
            )}
            {v.evidence && (
              <span className="shrink-0 rounded-full border border-line px-2 py-px text-[11px] text-ink-2" data-evidence-count data-no-select>
                {t("report.review.evidence", { n: v.evidence.count })}
              </span>
            )}
            {v.evidence && v.evidence.missing > 0 && (
              <span className={`shrink-0 rounded-full border px-2 py-px text-[11px] font-semibold ${v.evidence.strict ? "border-danger/40 bg-danger/5 text-danger" : "border-warn/40 bg-warn/5 text-warn"}`} data-evidence-missing={v.evidence.strict ? "strict" : "plain"} data-no-select>
                {t("report.review.noEvidence", { n: v.evidence.missing })}
              </span>
            )}
            {v.first && (
              <span className={`inline-flex min-w-0 items-center gap-1.5 rounded-full border px-2 py-px text-xs ${v.first.danger ? "border-danger/30 bg-danger/5 text-danger" : "border-line bg-paper text-ink-2"}`} data-caution data-no-select>
                {v.first.met && <Mark met={v.first.met} small />}
                <span className="shrink-0 font-semibold">{v.first.labels.join(" · ")}</span>
                <span className="min-w-0 truncate">{v.first.text}</span>
              </span>
            )}
          </span>
        )}
      </article>
      <div className="flex shrink-0 items-center gap-2 pt-0.5">
        {noReport ? (
          <button
            ref={openRef}
            onClick={onOpen}
            className="inline-flex h-7 shrink-0 items-center rounded-md px-2 text-xs whitespace-nowrap text-ink-2 hover:bg-paper hover:text-ink"
            aria-label={t("report.review.openTaskLabel", { title })}
            data-open-task
          >
            {t("report.review.openTask")}
          </button>
        ) : (
          <>
        <button
          ref={openRef}
          onClick={onOpen}
          className="inline-flex h-7 shrink-0 items-center rounded-md px-2 text-xs whitespace-nowrap text-ink-2 hover:bg-paper hover:text-ink"
          aria-label={t("report.review.openLabel", { title, line: v.line })}
          data-open-report
        >
          {t("report.review.openReport")}
        </button>
        {item.reviewed ? (
          <>
            <span className="inline-flex items-center gap-1 rounded-full border border-ok/40 px-2 py-px text-[11px] font-medium text-ok">
              <Check size={11} strokeWidth={2.4} aria-hidden />
              {item.wrapped ? t("report.review.wrapped") : t("report.review.reviewed")}
            </span>
            <button onClick={onUnmark} disabled={busy} className="text-[11px] text-ink-3 underline underline-offset-2 hover:text-ink disabled:opacity-40">
              {t("report.review.undo")}
            </button>
          </>
        ) : v.markable ? (
          <MarkButton primary={primary} onClick={onMark} disabled={busy} />
        ) : null}
          </>
        )}
        {onDelete && (
          <button
            onClick={onDelete}
            disabled={busy}
            className="inline-flex h-7 shrink-0 items-center rounded-md px-2 text-xs whitespace-nowrap text-danger hover:bg-danger/5 disabled:opacity-40"
            aria-label={t("report.review.deleteLabel", { title })}
            data-delete
          >
            {t("report.review.delete")}
          </button>
        )}
      </div>
    </li>
  );
}

/** A resting duck and one line. */
function Resting({ line, children }: { line: string; children?: ReactNode }) {
  return (
    <Sheet className="flex flex-col items-center gap-2 py-10 text-center">
      <span className="rounded-full bg-duck-stage-quiet p-1">
        <DuckAvatar pose="idle" size={48} color={roleColor("review")} label={t("report.review.restingDuck")} />
      </span>
      <p className="text-[13px] font-medium text-ink-2">{line}</p>
      {children}
    </Sheet>
  );
}

export function ReviewPage({ agents, onNavigate }: { agents: AgentsPayload | undefined; onNavigate: (p: NavTarget) => void }) {
  useT();
  const qc = useQueryClient();
  const types = useTeamConfig().data?.config.types;
  const { projectId } = useScope("folder");
  const [only, setOnly] = useState<"global" | "project">("global");
  const project = only === "project" ? projectId : null;
  const [filter, setFilter] = useState<InboxFilter>("pending");
  const [open, setOpen] = useState<InboxItem | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<Shown | null>(null);
  const [undo, setUndo] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  // A new filter or scope starts on page 1.
  const pickFilter = (f: InboxFilter) => {
    setFilter(f);
    setPage(1);
  };
  const pickScope = (o: "global" | "project") => {
    setOnly(o);
    setPage(1);
  };
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const focusNext = useRef<string | null>(null);
  const filterBox = useRef<HTMLDivElement>(null);

  const q = useQuery({
    queryKey: ["review-inbox", project, filter, page],
    queryFn: () => api.reviewInbox(project, filter, (page - 1) * PAGE_SIZE, PAGE_SIZE),
    placeholderData: keepPreviousData,
    refetchOnWindowFocus: true,
    staleTime: 0,
    // Backend errors here are deterministic (unknown project, bad input): show them at once.
    retry: false,
  });
  // The sidebar count moved (a task finished, or a mark elsewhere): read the list again.
  const pending = agents?.reviewPending;
  useEffect(() => {
    if (pending !== undefined) void qc.invalidateQueries({ queryKey: ["review-inbox"] });
  }, [pending, qc]);
  // The undo offer lasts five seconds.
  useEffect(() => {
    if (!undo) return;
    const t = window.setTimeout(() => setUndo(null), UNDO_MS);
    return () => window.clearTimeout(t);
  }, [undo]);
  const inbox = q.data;
  const items = inbox?.items ?? [];
  const pages = Math.min(pageCount(inbox?.total ?? 0), MAX_PAGES);
  const top = useRef<HTMLDivElement>(null);
  const goTo = (p: number) => {
    setPage(Math.min(Math.max(1, p), pages));
    top.current?.scrollIntoView?.({ block: "start" });
  };
  const groups = groupByRole(items, types, (k) => taskTypeLabel(k, types) ?? t("report.review.defaultRole"), inbox?.groups);
  const flat = groups.flatMap((g) => g.items);
  const primaryId = flat.find(canMark)?.bgId ?? null;

  // After a mark, focus the card that took its place (or the filter when the list is empty).
  useEffect(() => {
    const id = focusNext.current;
    if (id === null || q.isFetching || open) return;
    focusNext.current = null;
    const el = id ? refs.current.get(id) : undefined;
    if (el) el.focus();
    else (filterBox.current?.querySelector('[aria-checked="true"]') as HTMLElement | null)?.focus();
  }, [q.isFetching, items.length, open]);

  const refresh = () => Promise.all([qc.invalidateQueries({ queryKey: ["review-inbox"] }), qc.invalidateQueries({ queryKey: ["agents"] })]);
  /** The next card to review after `id`, read again after the mark (never from the list on screen): later on this
   *  page, then cards that moved onto this page (in 리뷰 전 the marked card leaves and the next page's first card
   *  moves up), then the next page, then an earlier card on this page. `before` is this page's order before the mark. */
  const nextAfter = async (id: string, before: string[], at: number): Promise<{ item: InboxItem; page: number } | null> => {
    const load = async (p: number) => {
      const inb = await api.reviewInbox(project, filter, (p - 1) * PAGE_SIZE, PAGE_SIZE);
      return { inb, order: groupByRole(inb.items, types, (k) => taskTypeLabel(k, types) ?? t("report.review.defaultRole"), inb.groups).flatMap((g) => g.items) };
    };
    try {
      const here = await load(at);
      const ok = (x: InboxItem) => canMark(x) && x.bgId !== id;
      // Cards that came after it before the mark, as they are now; then cards that moved onto this page.
      const now = new Map(here.order.map((x) => [x.bgId, x]));
      const later = before.slice(before.indexOf(id) + 1).map((b) => now.get(b)).find((x): x is InboxItem => !!x && ok(x)) ?? here.order.find((x) => ok(x) && !before.includes(x.bgId));
      if (later) return { item: later, page: at };
      if (at < Math.min(pageCount(here.inb.total), MAX_PAGES)) {
        const next = (await load(at + 1)).order.find(ok);
        if (next) return { item: next, page: at + 1 };
      }
      const earlier = here.order.find(ok);
      return earlier ? { item: earlier, page: at } : null;
    } catch {
      return null;
    }
  };

  const mark = async (item: InboxItem): Promise<boolean> => {
    setBusy(item.bgId);
    setError(null);
    try {
      await api.inboxMarkReviewed(item.bgId);
      setUndo(item.bgId);
      // The only card left on a later 리뷰 전 page: show the page before it.
      if (filter === "pending" && items.length === 1 && page > 1) setPage(clampPage(page - 1, (inbox?.total ?? 1) - 1));
      await refresh();
      return true;
    } catch (e) {
      setError(failure(e));
      return false;
    } finally {
      setBusy(null);
    }
  };
  const unmark = async (id: string) => {
    setBusy(id);
    setError(null);
    try {
      await api.unmarkReviewed(id);
      setUndo(null);
      await refresh();
    } catch (e) {
      setError(failure(e));
    } finally {
      setBusy(null);
    }
  };
  /** review-delete: the shell asks first (native dialog); `false` = cancelled, nothing changes. */
  const deleteFromCard = async (item: InboxItem) => {
    const i = flat.findIndex((x) => x.bgId === item.bgId);
    setBusy(item.bgId);
    setError(null);
    try {
      if (!(await api.inboxDelete(item.bgId))) return;
      focusNext.current = flat[i + 1]?.bgId ?? flat[i - 1]?.bgId ?? "";
      // The only card on a later page: show the page before it.
      if (items.length === 1 && page > 1) setPage(clampPage(page - 1, (inbox?.total ?? 1) - 1));
      await Promise.all([refresh(), qc.invalidateQueries({ queryKey: ["board-wrapup"] }), qc.invalidateQueries({ queryKey: ["task-records"] })]);
    } catch (e) {
      setError(failure(e));
    } finally {
      setBusy(null);
    }
  };
  const markFromCard = async (item: InboxItem) => {
    const i = flat.findIndex((x) => x.bgId === item.bgId);
    focusNext.current = filter === "pending" ? (flat[i + 1]?.bgId ?? flat[i - 1]?.bgId ?? "") : item.bgId;
    if (!(await mark(item))) focusNext.current = null;
  };
  const markAndNext = async (item: InboxItem) => {
    const before = flat.map((x) => x.bgId);
    const at = page;
    if (!(await mark(item))) return;
    const next = await nextAfter(item.bgId, before, at);
    if (next && next.page !== page) setPage(next.page);
    setOpen(next?.item ?? null);
  };

  const reportOff = !!inbox && !inbox.reportSetting;
  // Tasks without a report (D-196) keep the filters, so 전체 can still list them with the setting off.
  const offAndEmpty = reportOff && !inbox?.hasAny && !inbox?.noReport;
  // "작업 보기" (D-196): the board's session drawer while the CLI lists the task, else its 지난 작업 row.
  const openTask = (item: InboxItem) => {
    const s = agents?.sessions.find((x) => x.id === item.bgId);
    if (s) {
      window.dispatchEvent(new CustomEvent("dduck-open-task", { detail: s.key }));
      return;
    }
    history.replaceState(null, "", `${window.location.pathname}${window.location.search}#record=${encodeURIComponent(item.bgId)}`);
    onNavigate("tasks");
  };
  const undoLine = (lead: string) => (
    <span role="status" className="mr-auto text-xs text-ink-2">
      {lead} ·{" "}
      <button onClick={() => undo && void unmark(undo)} className="underline underline-offset-2 hover:text-ink">
        {t("report.review.undo")}
      </button>
    </span>
  );
  return (
    <div ref={top}>
      <PageHeader title={t("report.review.title")}>
        <span className="text-xs text-ink-3">{t("report.review.subtitle")}</span>
      </PageHeader>
      <PageBody>
        {!offAndEmpty && (
          <ScopeBar need="folder" globalLabel={t("report.review.allProjects")} value={only} onChange={pickScope}>
            <div ref={filterBox} className="ml-auto">
              <Segmented label={t("report.review.view")} value={filter} options={FILTERS.map((id) => ({ id, label: t(`report.review.filter.${id}`) }))} onChange={pickFilter} />
            </div>
          </ScopeBar>
        )}
        {reportOff && (inbox?.hasAny || !!inbox?.noReport) && (
          <p className="text-xs text-ink-2">
            {t("report.review.reportOff")}{" "}
            <button onClick={() => onNavigate("settings")} className="underline underline-offset-2 hover:text-ink">
              {t("report.review.openSettings")}
            </button>
          </p>
        )}
        {project && filter === "pending" && inbox && inbox.pendingOther > 0 && <p className="text-xs text-ink-2">{t("report.review.pendingOther", { count: inbox.pendingOther })}</p>}
        {error && (
          <p role="alert" className="text-xs text-danger">
            {shown(error)}
          </p>
        )}
        {q.isError ? (
          <Sheet className="py-8 text-center">
            <p className="text-[13px] font-medium text-ink-2">{t("report.review.loadFailed")}</p>
            <p className="mt-1 text-xs text-ink-3">{messageOf(q.error)}</p>
          </Sheet>
        ) : !inbox ? (
          <Sheet className="py-8 text-center text-xs text-ink-3">{t("report.review.loading")}</Sheet>
        ) : offAndEmpty ? (
          <Resting line={t("report.review.offEmpty")}>
            <Button onClick={() => onNavigate("settings")}>{t("report.review.openSettings")}</Button>
          </Resting>
        ) : !inbox.hasAny && items.length === 0 ? (
          <Resting line={t("report.review.noReports")} />
        ) : items.length === 0 ? (
          <Resting line={inbox.running > 0 ? t("report.review.allReviewedRunning", { count: inbox.running }) : t("report.review.allReviewed")} />
        ) : (
          <div className="space-y-3">
            {groups.map((g) => (
              <Sheet key={g.key ?? "none"}>
                <section aria-label={t("report.review.group", { label: g.label, count: g.total })} data-review-group={g.key ?? ""}>
                  <header className="sticky top-14 z-[5] flex items-center gap-2 rounded-t-xl border-b border-line bg-sheet px-4 py-1.5">
                    <span className="rounded-full bg-duck-stage-quiet p-px">
                      <DuckAvatar pose={cardView(g.items[0]).pose} size={28} color={roleColor(g.key)} label={t("report.review.duck", { role: g.label })} />
                    </span>
                    <RoleChip label={g.label} color={roleColor(g.key)} />
                    <span className="text-xs text-ink-3" data-group-total>
                      {t("report.review.groupTotal", { count: g.total })}
                    </span>
                  </header>
                  <ul className="divide-y divide-line px-3 py-1">
                    {g.items.map((item) => (
                      <Card
                        key={item.bgId}
                        item={item}
                        primary={item.bgId === primaryId}
                        busy={busy === item.bgId}
                        onOpen={() => (item.report.kind === "none" ? openTask(item) : setOpen(item))}
                        onMark={() => void markFromCard(item)}
                        onUnmark={() => void unmark(item.bgId)}
                        onDelete={canDelete(item, inbox.known ?? false) ? () => void deleteFromCard(item) : null}
                        openRef={(el) => {
                          if (el) refs.current.set(item.bgId, el);
                          else refs.current.delete(item.bgId);
                        }}
                      />
                    ))}
                  </ul>
                </section>
              </Sheet>
            ))}
            <Pager page={page} count={pages} onPage={goTo} />
          </div>
        )}
      </PageBody>
      {undo && !open && (
        <div className="fixed bottom-6 left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-xl bg-sheet px-4 py-2.5 shadow-e3">{undoLine(t("report.review.mark"))}</div>
      )}
      {open && (
        <TaskReportDialog
          key={open.bgId}
          id={open.bgId}
          title={cardView(open).goal}
          session={open.running === "waiting" ? "waitingInput" : open.running === "working" ? "working" : null}
          onClose={() => setOpen(null)}
          stage={open.wrapped ? "done" : open.reviewed ? "reviewed" : open.running ? undefined : "reviewWait"}
          footer={
            <>
              {undo && undo !== open.bgId && undoLine(t("report.review.previousReviewed"))}
              {canMark(open) && <MarkButton primary onClick={() => void markAndNext(open)} disabled={busy === open.bgId} label={t("report.review.markNext")} />}
              <Button onClick={() => setOpen(null)}>{t("report.review.close")}</Button>
            </>
          }
        />
      )}
    </div>
  );
}
