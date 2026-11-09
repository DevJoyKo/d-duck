// "시작하는 중" cards (D-159): shown the moment 작업 시작 is pressed, until the new background session shows up in
// the agents list (the backend polls every second for 30 s after a start) or 30 s pass. Read from the mutation cache,
// so the card stays even if the composer is closed. In memory only.
import { useMutationState } from "@tanstack/react-query";
import { Loader2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type AgentSession } from "../lib/api";
import { useT } from "../i18n/react";

export const START_TASK_KEY = ["start-task"] as const;
/** How long a started task waits for the agents list before its card goes away. */
export const STARTING_FOR_MS = 30_000;

export interface Starting {
  key: number;
  label: string;
  phase: "starting" | "waiting";
  since: number;
  /** Whole seconds since 작업 시작 was pressed. */
  secs: number;
  /** Lets "취소" reach the backend while the start is being prepared (D-161). */
  startId?: string;
}

/** What a start mutation carries: the card's label and the id the backend knows the start by (D-161). */
export interface StartVars {
  label: string;
  startId: string;
}

/** A fresh id per start; short ASCII so the backend accepts it. */
export function newStartId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `s-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

type Snap = { status: string; label: string; id: string | null; at: number; startId?: string };

/** Pending starts, and successful ones not yet in `sessions` (and younger than 30 s). Pure for tests. */
export function startingCards(snaps: Snap[], sessions: AgentSession[] | undefined, now: number): Starting[] {
  const listed = new Set((sessions ?? []).map((s) => s.id).filter(Boolean));
  return snaps.flatMap((m): Starting[] => {
    const secs = Math.max(0, Math.floor((now - m.at) / 1000));
    if (m.status === "pending") return [{ key: m.at, label: m.label, phase: "starting", since: m.at, secs, startId: m.startId }];
    if (m.status === "success" && m.id && !listed.has(m.id) && now - m.at < STARTING_FOR_MS) return [{ key: m.at, label: m.label, phase: "waiting", since: m.at, secs }];
    return [];
  });
}

export function useStartingCards(sessions: AgentSession[] | undefined): Starting[] {
  const snaps = useMutationState({
    filters: { mutationKey: START_TASK_KEY },
    select: (m): Snap => ({
      status: m.state.status,
      label: typeof m.state.variables === "string" ? m.state.variables : ((m.state.variables as StartVars | undefined)?.label ?? ""),
      startId: (m.state.variables as StartVars | undefined)?.startId,
      id: typeof m.state.data === "string" ? m.state.data : null,
      at: m.state.submittedAt,
    }),
  });
  const [now, setNow] = useState(() => Date.now());
  const cards = startingCards(snaps, sessions, now);
  const live = cards.length > 0;
  useEffect(() => {
    if (!live) return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [live]);
  return cards;
}

export function StartingCard({ c }: { c: Starting }) {
  const t = useT();
  const [cancelling, setCancelling] = useState(false);
  const canCancel = c.phase === "starting" && !!c.startId;
  return (
    <div role="status" className="flex min-w-0 items-center gap-2 rounded-lg border border-line bg-sheet px-3 py-2 text-[13px]">
      <Loader2 size={14} className="shrink-0 animate-spin text-ink-3" aria-hidden />
      <span className="min-w-0 flex-1 truncate font-medium">{c.label || t("tasks.page.newTask")}</span>
      <span className="shrink-0 text-xs text-ink-3">
        {cancelling ? t("tasks.starting.cancelling") : c.phase === "starting" ? t("tasks.starting.starting") : t("tasks.starting.waiting")} · {t("tasks.seconds", { secs: c.secs })}
      </span>
      {canCancel && (
        <button
          type="button"
          className="shrink-0 rounded px-1.5 py-0.5 text-xs text-ink-3 hover:bg-line hover:text-ink disabled:opacity-50"
          disabled={cancelling}
          aria-label={t("tasks.starting.cancelLabel")}
          onClick={() => {
            setCancelling(true);
            // The backend stops whatever the start already created; the card goes away when the start ends.
            api.startCancel(c.startId!).catch(() => setCancelling(false));
          }}
        >
          <X size={12} className="inline" aria-hidden /> {t("common.actions.cancel")}
        </button>
      )}
    </div>
  );
}

export function StartingCards({ sessions }: { sessions: AgentSession[] | undefined }) {
  const t = useT();
  const cards = useStartingCards(sessions);
  if (cards.length === 0) return null;
  return (
    <div className="space-y-2" aria-label={t("tasks.starting.list")}>
      {cards.map((c) => (
        <StartingCard key={c.key} c={c} />
      ))}
    </div>
  );
}
