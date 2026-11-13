// "요구사항 다듬기" in the composer (brief-grilling, D-105): Claude asks multiple-choice questions about the brief's
// gaps and proposes a rewrite; the user picks answers and applies fields one by one. Everything lives in this
// component's memory only. The backend owns consent (one native confirmation = the configured rounds, rounds ×
// $0.15 up to $0.90, 10 minutes), rounds and budget; this side only shows them.
import { AlertTriangle, Check, MessageCircleQuestion, RotateCcw, Sparkles, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type MutableRefObject, type ReactNode } from "react";
import { api, type RefineAnswer, type RefineQuestion, type RefineRun, type Refined, failure, later, shown, type Shown } from "../lib/api";
import type { Brief } from "../lib/brief";
import { usd } from "../lib/format";
import { diffBrief, initAnswer, MAX_CUSTOM, precheck, refineBudget, toAnswer, wordDiff, type AnswerState, type FieldDiff } from "../lib/refine";
import { markDone } from "../lib/onboarding";
import { DuckAvatar } from "./DuckAvatar";
import { Button, fieldClass } from "./ui";
import { isGateError } from "../lib/auth";
import { GateLine } from "./LoginBits";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import { isWindows, shortcut } from "../lib/platform";
import { MacOnlyChip } from "./MacOnly";

type Mode = "grill" | "rewrite";

const keyOf = (b: Brief, taskType: string | null) => JSON.stringify([b.goal, b.outOfScope, b.constraints, b.done, taskType]);

/** Secondary buttons in this panel (brand review). */
const CHECK = "mt-0.5 accent-ink";

export function failText(r: Extract<RefineRun, { kind: "failed" }>): string {
  const spent = r.costUsd != null ? t("tasks.refiner.spent", { cost: usd(r.costUsd) }) : "";
  switch (r.error.kind) {
    case "cancelled":
      return t("tasks.refiner.failCancelled", { spent });
    case "budget":
      return t("tasks.refiner.failBudget", { spent });
    case "timeout":
      return t("tasks.refiner.failTimeout", { spent });
    default:
      return r.message;
  }
}

/** The cost chip: this request and the total under the current confirmation. */
type Cost = { last: number | null; spent: number; cancelled?: boolean };

function costText(c: Cost): string {
  const last = c.cancelled ? t("tasks.refiner.costCancelled") : c.last != null ? usd(c.last) : t("tasks.refiner.costUnknown");
  return t("tasks.refiner.cost", { last, total: usd(c.spent) });
}

export const questionBox = (skipped = false) => `rounded-lg border p-3 ${skipped ? "border-dashed border-line bg-paper" : "border-line bg-sheet"}`;

/** One answer option. Display only, so the tour's refine card shows the same row with a static mark as `control`. */
export function OptionLine({ label, detail, recommended, checked, control, still }: { label: string; detail?: string; recommended: boolean; checked: boolean; control: ReactNode; still?: boolean }) {
  const t = useT();
  const Tag = still ? "div" : "label";
  return (
    <Tag className={`flex items-start gap-2 rounded-md border px-2.5 py-1.5 text-[13px] ${still ? "" : "cursor-pointer"} ${checked ? "border-ink bg-paper" : "border-transparent hover:bg-paper/60"}`}>
      {control}
      <span className="min-w-0">
        <span className="text-ink">{label}</span>
        {recommended && <span className="ml-1.5 rounded-md border border-line bg-sheet px-1.5 py-0.5 text-[11px] font-medium text-ink-2">{t("tasks.refiner.recommended")}</span>}
        {/* Details only on the picked option (design review). */}
        {detail && checked && <span className="block text-xs text-ink-3">{detail}</span>}
      </span>
    </Tag>
  );
}

function QuestionCard({
  q,
  a,
  onChange,
  disabled,
  autoFocus,
}: {
  q: RefineQuestion;
  a: AnswerState;
  onChange: (a: AnswerState) => void;
  disabled: boolean;
  autoFocus: boolean;
}) {
  const t = useT();
  const name = `refine-${q.id}`;
  const first = useRef<HTMLInputElement | null>(null);
  // The card remounts for each new round, so this runs once when the results arrive.
  useEffect(() => {
    if (autoFocus) first.current?.focus();
  }, [autoFocus]);
  const pick = (i: number) => {
    if (q.multi) onChange({ ...a, skipped: false, customOpen: false, chosen: a.chosen.includes(i) ? a.chosen.filter((x) => x !== i) : [...a.chosen, i].sort() });
    else onChange({ ...a, skipped: false, customOpen: false, chosen: [i] });
  };
  // The first selected option takes focus when results arrive (or the first option if none).
  const focusIndex = Math.max(0, q.options.findIndex((_, i) => a.chosen.includes(i)));
  return (
    <fieldset className={questionBox(a.skipped)} disabled={disabled}>
      <legend className="float-left flex w-full items-start gap-2 text-[13px] font-medium text-ink">
        <span className="min-w-0 flex-1">{q.title}</span>
        {a.skipped && <span className="shrink-0 rounded-md border border-line bg-sheet px-1.5 py-0.5 text-[11px] font-medium text-ink-2">{t("tasks.refiner.skipped")}</span>}
      </legend>
      {!a.skipped && (
        <>
          {q.body && <p className="clear-left pt-0.5 text-xs text-ink-2">{q.body}</p>}
          {q.multi && <p className="clear-left mt-1 text-[11px] text-ink-3">{t("tasks.refiner.multi")}</p>}
          <div className="clear-left mt-2 space-y-1">
            {q.options.map((o, i) => {
              const checked = !a.customOpen && a.chosen.includes(i);
              return (
                <OptionLine
                  key={i}
                  label={o.label}
                  detail={o.detail}
                  recommended={q.recommended.includes(i)}
                  checked={checked}
                  control={
                    <input
                      ref={i === focusIndex ? first : undefined}
                      type={q.multi ? "checkbox" : "radio"}
                      name={name}
                      checked={checked}
                      onChange={() => pick(i)}
                      className={CHECK}
                    />
                  }
                />
              );
            })}
          </div>
        </>
      )}
      <div className="clear-left mt-2 flex flex-wrap items-center gap-3 text-xs">
        {!a.skipped && (
          <button type="button" onClick={() => onChange({ ...a, customOpen: !a.customOpen, skipped: false })} aria-expanded={a.customOpen} className="text-ink-2 underline underline-offset-2">
            {t("tasks.refiner.custom")}
          </button>
        )}
        <button type="button" onClick={() => onChange({ ...a, skipped: !a.skipped, customOpen: false })} aria-pressed={a.skipped} className="text-ink-2 underline underline-offset-2">
          {a.skipped ? t("tasks.refiner.reanswer") : t("tasks.refiner.skip")}
        </button>
      </div>
      {a.customOpen && !a.skipped && (
        <input
          aria-label={t("tasks.refiner.customLabel", { title: q.title })}
          value={a.custom}
          maxLength={MAX_CUSTOM}
          autoFocus
          onChange={(e) => onChange({ ...a, custom: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && !e.metaKey && !e.ctrlKey && e.preventDefault()}
          placeholder={t("tasks.refiner.customPlaceholder")}
          className={`${fieldClass} mt-2`}
        />
      )}
    </fieldset>
  );
}

function Words({ before, after }: { before: string; after: string }) {
  const parts = useMemo(() => wordDiff(before, after), [before, after]);
  return (
    <p className="text-[12px] leading-relaxed whitespace-pre-wrap text-ink-2">
      {parts.map((p, i) =>
        p.t === "same" ? (
          <span key={i}>{p.v}</span>
        ) : p.t === "add" ? (
          <ins key={i} className="rounded bg-ok/15 px-0.5 text-ink no-underline">
            {p.v}
          </ins>
        ) : (
          <del key={i} className="rounded bg-danger/10 px-0.5 text-danger">
            {p.v}
          </del>
        ),
      )}
    </p>
  );
}

function DiffRow({ d, onApply, applied, value }: { d: FieldDiff; onApply: () => void; applied: boolean; value: string }) {
  const t = useT();
  return (
    <div className="rounded-lg border border-line bg-sheet p-3">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-ink-2">{d.label}</span>
        {d.shrinks && !applied && (
          <span className="inline-flex items-center gap-1 rounded bg-danger/10 px-1.5 py-0.5 text-[11px] font-medium text-danger">
            <AlertTriangle size={11} /> {t("tasks.refiner.shrinks")}
          </span>
        )}
        <span className="ml-auto" />
        {applied ? (
          <span className="inline-flex items-center gap-1 text-[11px] text-ok">
            <Check size={12} /> {t("tasks.refiner.applied")}
          </span>
        ) : d.changed ? (
          <Button type="button" size="sm" onClick={onApply}>
            {t("tasks.refiner.apply")}
          </Button>
        ) : (
          <span className="text-[11px] text-ink-3">{t("tasks.refiner.same")}</span>
        )}
      </div>
      {applied && (
        <details className="mt-1 text-[12px]">
          <summary className="cursor-pointer text-ink-3 select-none">{t("tasks.refiner.viewApplied")}</summary>
          <p className="mt-1 whitespace-pre-wrap text-ink-2">{value || t("tasks.refiner.emptied")}</p>
        </details>
      )}
      {d.changed && !applied && (
        <div className="mt-1.5 space-y-1" aria-label={t("tasks.refiner.compare", { field: d.label })}>
          <Words before={d.before} after={d.after} />
          {d.shrinks && d.removed.map((l, i) => (
            <p key={`r${i}`} className="rounded bg-danger/10 px-1.5 text-[12px] text-danger">
              <span aria-hidden>− </span>
              <span className="line-through">{l}</span>
            </p>
          ))}
        </div>
      )}
    </div>
  );
}

export function BriefRefiner({
  brief,
  taskType,
  strict,
  cliReady,
  localNames,
  maxRounds,
  onApply,
  startRef,
  submitRef,
  onFocusField,
}: {
  brief: Brief;
  taskType: string | null;
  strict: boolean;
  cliReady: boolean;
  /** Kept for callers; the section itself is never highlighted (one yellow emphasis per form). */
  hint?: boolean;
  localNames: string[];
  maxRounds: number;
  onApply: (b: Brief) => void;
  /** Lets the composer's "질문 받기로 다듬기" banner start a grilling round (a click, never automatic). */
  startRef?: MutableRefObject<(() => void) | null>;
  /** ⌘↵ anywhere in the form submits the open answers. */
  submitRef?: MutableRefObject<(() => boolean) | null>;
  /** A pre-check chip asks the composer to focus its field. */
  onFocusField?: (k: keyof Brief) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<Mode>("grill");
  const [history, setHistory] = useState<RefineAnswer[][]>([]);
  const [current, setCurrent] = useState<Refined | null>(null);
  const [answers, setAnswers] = useState<Record<string, AnswerState>>({});
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<Shown | null>(null);
  const [gate, setGate] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [cost, setCost] = useState<Cost | null>(null);
  const [stale, setStale] = useState(false);
  const [stopped, setStopped] = useState(false);
  const [redact, setRedact] = useState(true);
  const [undo, setUndo] = useState<Brief[]>([]);
  const [applied, setApplied] = useState<Set<keyof Brief>>(new Set());
  const [lastHistory, setLastHistory] = useState<RefineAnswer[][]>([]);
  const [view, setView] = useState<"questions" | "proposal">("questions");
  // Bumped when a new round arrives so the first card takes focus once.
  const [arrived, setArrived] = useState(0);
  const live = useRef({ brief, taskType });
  useEffect(() => {
    live.current = { brief, taskType };
  });
  const pendingRef = useRef(false);
  // Each request gets a number; 취소 moves it on so a late answer is dropped (D-159).
  const runRef = useRef(0);
  const before = useRef<{ open: boolean; mode: Mode }>({ open: false, mode: "grill" });
  const [startedAt, setStartedAt] = useState(0);
  const [now, setNow] = useState(0);
  useEffect(() => {
    if (!pending) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [pending]);
  const elapsed = pending ? Math.max(0, Math.floor((now - startedAt) / 1000)) : 0;
  useEffect(
    () => () => {
      // Closing the form mid-run cancels the run.
      if (pendingRef.current) void api.refineCancel().catch(() => undefined);
    },
    [],
  );

  const checks = useMemo(() => precheck(brief), [brief]);
  // Windows preview: refinement needs the app-owned hook, verified on macOS only (MacOnly in the backend too).
  const macOnly = isWindows();
  const enabled = !macOnly && cliReady && !!brief.goal.trim();
  const finished = !!current && (current.finished || stopped);
  const round = history.length + (current ? 1 : 0);
  const diffs = useMemo(() => (current ? diffBrief(brief, current.rewrite) : []), [brief, current]);
  const names = [...new Set(localNames.filter((n) => n.trim().length >= 2))];

  const send = async (m: Mode, nextHistory: RefineAnswer[][]) => {
    const run = ++runRef.current;
    before.current = { open, mode };
    const t0 = Date.now();
    setStartedAt(t0);
    setNow(t0);
    setMode(m);
    setOpen(true);
    setPending(true);
    pendingRef.current = true;
    setError(null);
    setGate(false);
    setStale(false);
    const sent = keyOf(brief, taskType);
    try {
      const r = await api.refineBrief({ mode: m, taskType, brief, history: nextHistory, localNames: names, redactNames: redact, token });
      if (run !== runRef.current) return; // cancelled: the answer is thrown away
      if (r.kind === "done") {
        markDone("refineUsed");
        setToken(r.token);
        setHistory(nextHistory);
        setLastHistory(nextHistory);
        setCurrent(r.result);
        setAnswers(Object.fromEntries(r.result.questions.map((q) => [q.id, initAnswer(q)])));
        setCost({ last: r.costUsd, spent: r.spentUsd });
        setStopped(false);
        setApplied(new Set());
        setView(r.result.finished || r.result.questions.length === 0 ? "proposal" : "questions");
        setArrived((n) => n + 1);
        setStale(keyOf(live.current.brief, live.current.taskType) !== sent);
      } else if (r.kind === "failed" && isGateError(r.error)) {
        setGate(true);
      } else if (r.kind === "failed") {
        setError(later(() => failText(r)));
        if (r.spentUsd != null) setCost({ last: r.costUsd, spent: r.spentUsd, cancelled: r.error.kind === "cancelled" });
      }
    } catch (e) {
      if (run !== runRef.current) return;
      if (isGateError(e)) setGate(true);
      else setError(failure(e));
    } finally {
      if (run === runRef.current) {
        setPending(false);
        pendingRef.current = false;
      }
    }
  };

  /** 취소: the CLI run is killed in the backend; here the wait ends at once and the panel goes back to how it was. */
  const cancel = () => {
    runRef.current += 1;
    pendingRef.current = false;
    setPending(false);
    setMode(before.current.mode);
    setOpen(before.current.open || !!current);
    void api.refineCancel().catch(() => undefined);
  };

  const submitWith = (all: Record<string, AnswerState>) => {
    if (!current || finished || pending) return;
    const answered = current.questions.map((q) => toAnswer(q, all[q.id] ?? initAnswer(q)));
    void send("grill", [...history, answered]);
  };
  const submitAnswers = () => submitWith(answers);
  const skipAll = () => {
    if (!current) return;
    const all = Object.fromEntries(current.questions.map((q) => [q.id, { ...initAnswer(q), skipped: true }]));
    setAnswers(all);
    submitWith(all);
  };
  const canSubmit = !!current && !finished && !pending && current.questions.length > 0;

  useEffect(() => {
    if (startRef) startRef.current = enabled && !pending ? () => void send("grill", []) : null;
    if (submitRef)
      submitRef.current = canSubmit
        ? () => {
            submitAnswers();
            return true;
          }
        : null;
  });

  const applyFields = (keys: (keyof Brief)[]) => {
    if (!current) return;
    const next = { ...brief };
    for (const k of keys) next[k] = current.rewrite[k];
    setUndo((u) => [...u, brief]);
    setApplied((s) => new Set([...s, ...keys]));
    onApply(next);
  };
  const applyAll = () => {
    const keys = diffs.filter((d) => d.changed && !applied.has(d.key)).map((d) => d.key);
    const shrink = diffs.filter((d) => d.shrinks && keys.includes(d.key));
    if (shrink.length && !window.confirm(t("tasks.refiner.confirmShrink", { fields: shrink.map((d) => d.label).join("·") }))) return;
    applyFields(keys);
  };
  const revert = () => {
    const prev = undo[undo.length - 1];
    if (!prev) return;
    setUndo((u) => u.slice(0, -1));
    setApplied(new Set());
    onApply(prev);
  };

  const onKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && canSubmit) {
      e.preventDefault();
      e.stopPropagation();
      submitAnswers();
    }
  };

  const reason = macOnly ? t("common.macOnly.refine") : !cliReady ? t("tasks.composer.noCli") : !brief.goal.trim() ? t("tasks.refiner.noGoal") : null;
  const tab = (v: "questions" | "proposal", label: string) => (
    <button
      type="button"
      role="tab"
      aria-selected={view === v}
      onClick={() => setView(v)}
      className={`h-7 flex-1 rounded-md text-[12px] ${view === v ? "bg-sheet font-medium text-ink shadow-sm" : "text-ink-2"}`}
    >
      {label}
    </button>
  );

  return (
    <section id="brief-refiner" aria-label={t("tasks.refiner.title")} className="scroll-mt-4 rounded-lg border border-line bg-paper/60 p-3" onKeyDown={onKey}>
      <div className="flex flex-wrap items-center gap-2">
        <MessageCircleQuestion size={15} className="text-ink-2" aria-hidden />
        <h3 className="text-[13px] font-semibold">{t("tasks.refiner.title")}</h3>
        {macOnly && <MacOnlyChip />}
        {cost && (
          <span className="rounded-full border border-line bg-sheet px-2 py-0.5 text-[11px] text-ink-2" aria-label={t("tasks.refiner.costLabel")}>
            {costText(cost)}
          </span>
        )}
        <span className="ml-auto flex flex-wrap gap-1.5">
          <Button type="button" variant="primary" size="sm" disabled={!enabled || pending} title={reason ?? undefined} onClick={() => void send("grill", [])}>
            <MessageCircleQuestion size={13} /> {t("tasks.refiner.ask")}
          </Button>
          <Button type="button" size="sm" disabled={!enabled || pending} title={reason ?? undefined} onClick={() => void send("rewrite", [])}>
            <Sparkles size={13} /> {t("tasks.refiner.rewrite")}
          </Button>
          {open && (
            <button type="button" aria-label={t("tasks.refiner.close")} onClick={() => setOpen(false)} className="rounded-md p-1 text-ink-3 hover:text-ink">
              <X size={14} />
            </button>
          )}
        </span>
      </div>
      <p className="mt-1 text-xs text-ink-3">
        {reason ?? t("tasks.refiner.intro", { rounds: maxRounds, budget: usd(refineBudget(maxRounds)) })}
      </p>
      {checks.length > 0 && (
        <ul className="mt-2 flex flex-wrap gap-1.5" aria-label={t("tasks.refiner.precheck")}>
          {checks.map((c) => (
            <li key={c.text}>
              <button
                type="button"
                onClick={() => onFocusField?.(c.field)}
                className="rounded-full border border-line bg-sheet px-2 py-0.5 text-[11px] text-ink-2 hover:border-ink-3 hover:text-ink"
              >
                {c.text}
              </button>
            </li>
          ))}
        </ul>
      )}
      {names.length > 0 && (
        <label className="mt-2 flex items-start gap-2 text-xs text-ink-2">
          <input type="checkbox" checked={redact} onChange={(e) => setRedact(e.target.checked)} className={CHECK} />
          <span>{t("tasks.refiner.redact", { count: names.length })}</span>
        </label>
      )}

      {pending && (
        <div role="status" aria-live="polite" className="mt-3 flex items-center gap-2 rounded-lg border border-line bg-sheet px-3 py-2 text-[13px] text-ink-2" data-refine-progress>
          <DuckAvatar pose="working" size={28} label={t("tasks.refiner.avatar")} />
          <span className="min-w-0 flex-1">
            {mode === "grill" ? t("tasks.refiner.grilling") : t("tasks.refiner.rewriting")} · <span className="tabular-nums">{t("tasks.seconds", { secs: elapsed })}</span>
            <span className="text-ink-3">{t("tasks.refiner.maxWait")}</span>
          </span>
          <Button type="button" variant="danger" size="sm" className="shrink-0" onClick={cancel}>
            {t("common.actions.cancel")}
          </Button>
        </div>
      )}
      {open && (
        <div className="mt-3 space-y-3">
          {gate && <GateLine force="login" />}
          {error && (
            <p role="alert" className="rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-[13px] text-danger">
              {shown(error)}
            </p>
          )}
          {stale && current && (
            <div role="status" className="flex flex-wrap items-center gap-2 rounded-lg border border-line bg-sheet px-3 py-2 text-xs text-ink">
              {t("tasks.refiner.stale")}
              <Button type="button" size="sm" className="ml-auto" disabled={pending} onClick={() => void send(mode, lastHistory)}>
                {t("tasks.refiner.resend")}
              </Button>
            </div>
          )}
          {current && (
            <>
              <div role="tablist" aria-label={t("tasks.refiner.tabs")} className="flex gap-1 rounded-lg bg-line/50 p-0.5 lg:hidden">
                {tab("questions", t("tasks.refiner.tabQuestions"))}
                {tab("proposal", t("tasks.refiner.tabProposal"))}
              </div>
              <div className="grid gap-3 lg:grid-cols-2">
                <div className={`min-w-0 space-y-2 ${view === "questions" ? "" : "hidden"} lg:block`}>
                  {/* Sticky so the answer buttons stay in reach while scrolling through the cards. */}
                  <div className="sticky top-0 z-[1] -mx-1 flex flex-wrap items-center gap-1.5 rounded-md bg-paper/95 px-1 py-1 backdrop-blur">
                    <p className="mr-auto text-xs font-medium text-ink-2">{finished ? t("tasks.refiner.finished") : t("tasks.refiner.round", { round, max: maxRounds, count: current.questions.length })}</p>
                    {!finished && (
                      <>
                        <Button type="button" variant="primary" size="sm" disabled={pending} onClick={submitAnswers}>
                          {t("tasks.refiner.submit")} <kbd className="ml-0.5 text-[11px] opacity-70">{shortcut("↵")}</kbd>
                        </Button>
                        <Button type="button" size="sm" disabled={pending} onClick={skipAll}>
                          {t("tasks.refiner.skipAll")}
                        </Button>
                        <Button type="button" size="sm" disabled={pending} onClick={() => setStopped(true)}>
                          {t("tasks.refiner.stop")}
                        </Button>
                      </>
                    )}
                  </div>
                  {current.note && <p className="text-xs text-ink-2">{current.note}</p>}
                  {finished ? (
                    <p className="rounded-lg border border-line bg-sheet px-3 py-2 text-[13px] text-ink-2">{t("tasks.refiner.noMore")}</p>
                  ) : (
                    current.questions.map((q, i) => (
                      <QuestionCard
                        key={`${arrived}-${q.id}`}
                        q={q}
                        a={answers[q.id] ?? initAnswer(q)}
                        disabled={pending}
                        autoFocus={i === 0}
                        onChange={(a) => setAnswers((s) => ({ ...s, [q.id]: a }))}
                      />
                    ))
                  )}
                </div>
                <div className={`min-w-0 space-y-2 ${view === "proposal" ? "" : "hidden"} lg:block`}>
                  <div className="flex flex-wrap items-center gap-1.5">
                    <p className="mr-auto text-xs font-medium text-ink-2">{t("tasks.refiner.proposal")}</p>
                    {undo.length > 0 && (
                      <Button type="button" size="sm" onClick={revert}>
                        <RotateCcw size={12} /> {t("tasks.refiner.undo")}
                      </Button>
                    )}
                    <Button type="button" size="sm" disabled={!diffs.some((d) => d.changed && !applied.has(d.key))} onClick={applyAll}>
                      {t("tasks.refiner.applyAll")}
                    </Button>
                  </div>
                  {diffs.some((d) => d.shrinks) && (
                    <p className={`flex items-start gap-1.5 text-xs ${strict ? "text-danger" : "text-ink-2"}`}>
                      <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                      {strict ? t("tasks.refiner.strictPrefix") : ""}
                      {t("tasks.refiner.shrinkWarn")}
                    </p>
                  )}
                  {diffs.map((d) => (
                    <DiffRow key={d.key} d={d} value={brief[d.key]} applied={applied.has(d.key) && !d.changed} onApply={() => applyFields([d.key])} />
                  ))}
                  <p className="text-[11px] text-ink-3">{t("tasks.refiner.footer")}</p>
                </div>
              </div>
            </>
          )}
        </div>
      )}
    </section>
  );
}
