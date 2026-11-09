// Sessions list (own period, 10 per page), pencil-edit titles and the per-session flow timeline.
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Bot, Braces, Check, ChevronLeft, ChevronRight, MessageSquare, Pencil, Sparkles, SquareSlash, Wrench, X, Zap } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, type SessionRow, type TimelineItem } from "../lib/api";
import { RawPager, RawPanel } from "./RawView";
import { dateTime, num, sessionLabel, timeOfDay, usd } from "../lib/format";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import { buildQuery, type Preset } from "../lib/period";
import { Button, Empty, SectionTitle, Segmented, Sheet } from "./ui";
import { markDone } from "../lib/onboarding";

const LIST_PERIOD_KEY = { all: "tasks.sessions.period.all", today: "tasks.sessions.period.today", "7d": "tasks.sessions.period.d7", "30d": "tasks.sessions.period.d30" } as const;
const listPeriods = (): { id: Preset; label: string }[] => (["all", "today", "7d", "30d"] as const).map((id) => ({ id, label: t(LIST_PERIOD_KEY[id]) }));

/** Inline title editor: Enter saves, Esc cancels, empty restores Claude's title. */
export function LabelEditor({ sessionId, current, onDone }: { sessionId: string; current: string; onDone: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const [value, setValue] = useState(current);
  const save = useMutation({
    mutationFn: () => api.setSessionLabel(sessionId, value.trim() || null),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["usage"] });
      qc.invalidateQueries({ queryKey: ["agents"] });
      onDone();
    },
  });
  return (
    <span className="flex min-w-0 flex-1 items-center gap-1" onClick={(e) => e.stopPropagation()}>
      <input
        autoFocus
        aria-label={t("tasks.sessions.titleLabel")}
        value={value}
        maxLength={80}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          // Korean IME: Enter while composing finishes the syllable, it must not save yet.
          if (e.nativeEvent.isComposing || e.keyCode === 229) return;
          if (e.key === "Enter") save.mutate();
          if (e.key === "Escape") {
            e.stopPropagation();
            onDone();
          }
        }}
        placeholder={t("tasks.sessions.titlePlaceholder")}
        className="h-7 min-w-0 flex-1 rounded-md border border-line bg-sheet px-2 text-[13px]"
      />
      <button aria-label={t("tasks.sessions.saveTitle")} onClick={() => save.mutate()} className="rounded p-1 text-ink-2 hover:bg-paper">
        <Check size={14} />
      </button>
      <button aria-label={t("common.actions.cancel")} onClick={onDone} className="rounded p-1 text-ink-3 hover:bg-paper">
        <X size={14} />
      </button>
      {save.isError && <span className="text-[11px] text-danger">{String(save.error)}</span>}
    </span>
  );
}

export function displayName(r: { label?: string | null; title: string | null; sessionId: string }): string {
  return r.label?.trim() || sessionLabel(r.title, r.sessionId);
}

function Chip({ icon, children, title }: { icon?: ReactNode; children: ReactNode; title: string }) {
  return (
    <span title={title} className="inline-flex max-w-[180px] items-center gap-1 rounded-md border border-line bg-paper px-1.5 py-px text-[11px] font-normal text-ink-2">
      {icon}
      <span className="truncate">{children}</span>
    </span>
  );
}

const MAX_CHIPS = 4;

/** Most-used tools (3), skills and subagents of a session; always with a count. At most 4 chips, then "+N". */
function UsageChips({ r }: { r: SessionRow }) {
  const t = useT();
  const all = [
    ...(r.topTools ?? []).map((x) => ({ ...x, kind: t("tasks.sessions.tool"), icon: null })),
    ...(r.skills ?? []).map((x) => ({ ...x, kind: t("tasks.sessions.skill"), icon: <Zap size={10} className="shrink-0 text-ink-3" /> })),
    ...(r.subagents ?? []).map((x) => ({ ...x, kind: t("tasks.sessions.subagent"), icon: <Bot size={10} className="shrink-0 text-ink-3" /> })),
  ];
  if (all.length === 0) return null;
  const shown = all.slice(0, MAX_CHIPS);
  const rest = all.slice(MAX_CHIPS);
  return (
    <div className="mt-1 flex flex-wrap gap-1">
      {shown.map((x) => (
        <Chip key={`${x.kind}-${x.name}`} title={t("tasks.sessions.chipTitle", { kind: x.kind, name: x.name, count: x.count })} icon={x.icon}>
          {x.name} ×{x.count}
        </Chip>
      ))}
      {rest.length > 0 && <Chip title={rest.map((x) => `${x.kind} ${x.name} ×${x.count}`).join("\n")}>+{rest.length}</Chip>}
    </div>
  );
}

export function SessionsSection({ projects, models, onOpen }: { projects: string[]; models: string[]; onOpen: (id: string) => void }) {
  const t = useT();
  const [period, setPeriod] = useState<Preset>("all");
  const [page, setPage] = useState(0);
  const [editing, setEditing] = useState<string | null>(null);
  const pencils = useRef<Record<string, HTMLButtonElement | null>>({});
  const stopEditing = (id: string) => {
    setEditing(null);
    setTimeout(() => pencils.current[id]?.focus(), 0);
  };
  const q = useMemo(() => buildQuery(period, new Date(), { projects, models }), [period, projects, models]);
  // A narrower filter starts from the first page (no "41–34 / 34").
  const filterKey = JSON.stringify([projects, models]);
  const [seenKey, setSeenKey] = useState(filterKey);
  if (seenKey !== filterKey) {
    setSeenKey(filterKey);
    setPage(0);
  }
  const data = useQuery({ queryKey: ["usage", "sessions-page", q, page], queryFn: () => api.sessionsPage(q, page * 10), placeholderData: keepPreviousData });
  const total = data.data?.total ?? 0;
  const rows = data.data?.rows ?? [];
  const pages = Math.max(1, Math.ceil(total / 10));
  return (
    <section>
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <SectionTitle aside={t("tasks.sessions.aside")}>{t("tasks.sessions.title")}</SectionTitle>
        <div className="ml-auto">
          <Segmented
            label={t("tasks.sessions.periodLabel")}
            value={period}
            options={listPeriods()}
            onChange={(p) => {
              setPeriod(p);
              setPage(0);
            }}
          />
        </div>
      </div>
      <Sheet className="overflow-hidden">
        {data.isError ? (
          <Empty title={t("tasks.sessions.loadFailed")}>{String(data.error)}</Empty>
        ) : rows.length === 0 ? (
          <Empty title={data.isPending ? t("tasks.loading") : t("tasks.sessions.emptyPeriod")} />
        ) : (
          <table className="w-full text-[13px]">
            <thead className="border-b border-line text-left text-xs text-ink-3">
              <tr>
                <th className="px-4 py-2 font-medium">{t("tasks.sessions.colSession")}</th>
                <th className="px-4 py-2 font-medium">{t("tasks.sessions.colProject")}</th>
                <th className="px-4 py-2 font-medium">{t("tasks.sessions.colLast")}</th>
                <th className="px-4 py-2 text-right font-medium">{t("tasks.sessions.colResponses")}</th>
                <th className="px-4 py-2 text-right font-medium">{t("tasks.sessions.colCost")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.sessionId} onClick={() => editing !== r.sessionId && onOpen(r.sessionId)} className="cursor-pointer border-b border-line last:border-0 hover:bg-paper">
                  <td className="max-w-[380px] px-4 py-2.5 font-medium">
                    {editing === r.sessionId ? (
                      <LabelEditor sessionId={r.sessionId} current={displayName(r)} onDone={() => stopEditing(r.sessionId)} />
                    ) : (
                      <span className="group flex items-center gap-1.5">
                        {/* A real button so the row is reachable and opens with the keyboard. */}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onOpen(r.sessionId);
                          }}
                          className="min-w-0 truncate text-left"
                        >
                          {displayName(r)}
                        </button>
                        <button
                          ref={(el) => {
                            pencils.current[r.sessionId] = el;
                          }}
                          aria-label={t("tasks.sessions.editTitle", { name: displayName(r) })}
                          onClick={(e) => {
                            e.stopPropagation();
                            setEditing(r.sessionId);
                          }}
                          className="shrink-0 rounded p-1 text-ink-3 opacity-60 group-hover:opacity-100 hover:bg-sheet hover:text-ink focus:opacity-100"
                        >
                          <Pencil size={12} />
                        </button>
                      </span>
                    )}
                    <UsageChips r={r} />
                  </td>
                  <td className="px-4 py-2.5 text-ink-2">{r.projectName ?? "-"}</td>
                  <td className="px-4 py-2.5 text-ink-2">{dateTime(r.lastTs)}</td>
                  <td className="px-4 py-2.5 text-right">{num(r.responses)}</td>
                  <td className="px-4 py-2.5 text-right font-medium">{usd(r.estCostUsd)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Sheet>
      {total > 0 && (
        <div className="mt-2 flex items-center justify-end gap-2 text-xs text-ink-3">
          <span className="tabular-nums">
            {page * 10 + 1}–{Math.min(total, page * 10 + 10)} / {total}
          </span>
          <Button aria-label={t("tasks.sessions.prevPage")} disabled={page === 0} onClick={() => setPage(page - 1)} size="compact">
            <ChevronLeft size={14} />
          </Button>
          <Button aria-label={t("tasks.sessions.nextPage")} disabled={page + 1 >= pages} onClick={() => setPage(page + 1)} size="compact">
            <ChevronRight size={14} />
          </Button>
        </div>
      )}
    </section>
  );
}

type Filter = "all" | "prompt" | "tool" | "skill" | "error";
const filters = (): { id: Filter; label: string }[] => (["all", "prompt", "tool", "skill", "error"] as const).map((id) => ({ id, label: t(`tasks.flow.filter.${id}`) }));

const isSkillish = (i: TimelineItem) => i.kind === "skill" || i.kind === "agent" || i.kind === "command";
const failed = (i: TimelineItem) => (i.kind === "tool" || i.kind === "skill" || i.kind === "agent") && i.ok === false;

function matches(i: TimelineItem, f: Filter): boolean {
  switch (f) {
    case "all":
      return true;
    case "prompt":
      return i.kind === "prompt";
    case "tool":
      return i.kind === "tool";
    case "skill":
      return isSkillish(i);
    case "error":
      return failed(i);
  }
}

function time(ts: number) {
  return timeOfDay(ts);
}

function label(item: TimelineItem): string {
  switch (item.kind) {
    case "prompt":
      return t("tasks.flow.me");
    case "reply":
      return "Claude";
    case "tool":
      return item.name;
    case "skill":
      return t("tasks.flow.skill", { name: item.name });
    case "agent":
      return [t("tasks.flow.subagent"), item.subagentType || "?", item.description].filter(Boolean).join(" · ");
    case "command":
      return t("tasks.flow.command", { name: item.name });
    case "compact":
      return t("tasks.flow.compact");
  }
}

function icon(item: TimelineItem) {
  if (failed(item)) return <AlertTriangle size={13} className="text-danger" />;
  switch (item.kind) {
    case "prompt":
      return <MessageSquare size={13} />;
    case "reply":
      return <Sparkles size={13} />;
    case "skill":
      return <Zap size={13} />;
    case "agent":
      return <Bot size={13} />;
    case "command":
      return <SquareSlash size={13} />;
    default:
      return <Wrench size={13} />;
  }
}

function ToggleLink({ open, onClick, children }: { open: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button onClick={onClick} aria-expanded={open} className="inline-flex shrink-0 items-center gap-0.5 rounded px-1 text-[11px] text-ink-3 hover:bg-paper hover:text-ink">
      <Braces size={11} />
      {children}
    </button>
  );
}

function Row({ item, sessionId }: { item: TimelineItem; sessionId: string }) {
  const t = useT();
  const [rawOpen, setRawOpen] = useState(false);
  const [subOpen, setSubOpen] = useState(false);
  const raw = item.raw ?? [];
  const rawButton = raw.length > 0 && (
    <ToggleLink open={rawOpen} onClick={() => setRawOpen(!rawOpen)}>
      {rawOpen ? t("tasks.flow.rawClose") : t("tasks.flow.raw")}
    </ToggleLink>
  );
  if (item.kind === "compact") {
    return (
      <li className="py-1 text-center text-[11px] text-ink-3">
        — {t("tasks.flow.compact")} — {rawButton}
        {rawOpen && <div className="text-left"><RawPanel sessionId={sessionId} target={{ spans: raw }} /></div>}
      </li>
    );
  }
  const special = isSkillish(item);
  const ok = item.kind === "tool" || item.kind === "skill" || item.kind === "agent" ? item.ok : null;
  const error = item.kind === "tool" || item.kind === "skill" || item.kind === "agent" ? item.error : null;
  return (
    <li className="flex gap-2.5 py-2">
      <span className={`mt-0.5 shrink-0 ${special ? "text-ink-2" : "text-ink-3"}`}>{icon(item)}</span>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2 text-[11px] text-ink-3">
          <span className={`min-w-0 font-medium ${item.kind === "agent" ? "wrap-anywhere" : "truncate"} ${special ? "rounded bg-warn-soft px-1 text-ink" : "text-ink-2"}`} title={label(item)}>
            {label(item)}
          </span>
          <span className="shrink-0 tabular-nums">{time(item.ts)}</span>
          {ok === true && <span className="shrink-0 text-ok">{t("tasks.flow.success")}</span>}
          {ok === false && <span className="shrink-0 text-danger">{t("tasks.flow.error")}</span>}
          <span className="ml-auto flex shrink-0 gap-1">
            {item.kind === "agent" && item.agentId && (
              <ToggleLink open={subOpen} onClick={() => setSubOpen(!subOpen)}>
                {subOpen ? t("tasks.flow.subRawClose") : t("tasks.flow.subRaw")}
              </ToggleLink>
            )}
            {rawButton}
          </span>
        </div>
        {item.kind === "prompt" && <p className="mt-0.5 rounded-lg bg-paper px-2.5 py-1.5 text-[13px] whitespace-pre-wrap select-text">{item.text}</p>}
        {item.kind === "reply" && <p className="mt-0.5 text-[12px] text-ink-2 select-text">{item.text}</p>}
        {item.kind === "tool" && (
          <p className="mt-0.5 truncate font-mono text-[11px] text-ink-2 select-text" title={item.summary}>
            {item.summary || "-"}
          </p>
        )}
        {error && <p className="mt-0.5 text-[11px] text-danger select-text">{error}</p>}
        {rawOpen && <RawPanel sessionId={sessionId} target={{ spans: raw }} />}
        {subOpen && item.kind === "agent" && item.agentId && (
          <div className="mt-1.5 rounded-md border border-line p-2">
            <p className="mb-1.5 text-[11px] font-medium text-ink-2">{t("tasks.flow.subRecord", { type: item.subagentType })}</p>
            <RawPager sessionId={sessionId} agentId={item.agentId} />
          </div>
        )}
      </div>
    </li>
  );
}

type View = "flow" | "raw";
const views = (): { id: View; label: string }[] => [
  { id: "flow", label: t("tasks.flow.flow") },
  { id: "raw", label: t("tasks.flow.raw") },
];

/** The session's flow, loaded only while the drawer is open and dropped when it closes (never cached). */
export function SessionTimeline({ sessionId }: { sessionId: string }) {
  const t = useT();
  const [view, setView] = useState<View>("flow");
  const [filter, setFilter] = useState<Filter>("all");
  const tl = useQuery({ queryKey: ["timeline", sessionId], queryFn: () => api.sessionTimeline(sessionId), gcTime: 0, staleTime: 30_000 });
  // Checklist "세션 흐름 열어 보기": only the fact that a flow was opened, nothing about which one.
  useEffect(() => {
    markDone("timelineOpen");
  }, []);
  const all = tl.data?.items ?? [];
  // Keyed by position in the whole flow so an open "원문" stays with its item when the filter changes.
  const items = all.map((item, at) => ({ item, at })).filter(({ item }) => matches(item, filter));
  const tools = all.filter((i) => i.kind === "tool").length;
  const skills = all.filter(isSkillish).length;
  const errors = all.filter(failed).length;
  return (
    <section aria-label={t("tasks.flow.flow")} className="mt-6">
      <div className="mb-2 flex flex-wrap items-center gap-2">
        <Segmented label={t("tasks.flow.viewLabel")} value={view} options={views()} onChange={setView} />
        <span className="text-[11px] text-ink-3">{t("tasks.flow.viewHint")}</span>
      </div>
      {view === "flow" && (
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <span className="text-xs text-ink-3">
            {t("tasks.flow.counts", { tools, skills, errors })}
          </span>
          <div className="ml-auto">
            <Segmented label={t("tasks.flow.filterLabel")} value={filter} options={filters()} onChange={setFilter} />
          </div>
        </div>
      )}
      {view === "raw" ? (
        <>
          <p className="mb-2 text-[11px] text-ink-3">
            {t("tasks.flow.rawNote")}
          </p>
          <RawPager sessionId={sessionId} />
        </>
      ) : tl.isError ? (
        <Empty title={t("tasks.flow.readFailed")}>{String(tl.error)}</Empty>
      ) : tl.isPending ? (
        <Empty title={t("tasks.raw.reading")} />
      ) : !tl.data ? (
        <Empty title={t("tasks.flow.noFile")} />
      ) : (
        <>
          {tl.data.truncatedFront && <p className="mb-1 text-[11px] text-ink-3">{t("tasks.flow.truncatedFront")}</p>}
          <ul className="divide-y divide-line">
            {items.map(({ item, at }) => (
              <Row key={at} item={item} sessionId={sessionId} />
            ))}
          </ul>
          {items.length === 0 && <Empty title={t("tasks.flow.noItems")} />}
          <p className="mt-2 text-[11px] text-ink-3">
            {t("tasks.flow.note")}
          </p>
        </>
      )}
    </section>
  );
}
