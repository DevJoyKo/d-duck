// 작업 보고서 (decision-report, D-108). The app draws the report from the agent's checked JSON; "맡긴 내용" comes
// from what D.Duck stored at start. Text is rendered as text only (React escapes it; Rust already removed control
// and bidi characters). The verdict line comes first; the duck wears its role colour (D-116) and walks the flow:
// footprints for steps, forks for flagged decisions, dots for the rest, puddles for blocks.
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, ShieldAlert, X } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { alertLines, guardChip } from "../lib/delivery";
import { api, type Delegated, type Report, type ReportBlocked, type ReportDecision, type ReportReference, type SessionStatus, type TaskReportView, failure, shown, type Shown } from "../lib/api";
import { taskTypeLabel } from "../lib/agents";
import type { Brief } from "../lib/brief";
import { dateTime } from "../lib/format";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import {
  CAUTION_CHIPS,
  cautions,
  criteriaRows,
  doneLines,
  FLAG_LABEL,
  flagsOf,
  flow,
  isEnded,
  MET_LABEL,
  MARK_FLAG_LABEL,
  markFlag,
  MET_MARK,
  REPORT_ERROR_TEXT,
  type MarkFlag,
  rowId,
  verdict,
  type Caution,
  type CriterionRow,
} from "../lib/report";
import type { Pose } from "../lib/agents";
import { roleColor } from "../lib/roleColors";
import { MODE_LABEL, useTeamConfig } from "../lib/team";
import { DuckAvatar } from "./DuckAvatar";
import { YELLOW } from "../lib/yellow";
import { RoleChip } from "./RoleDuck";
import { markDone } from "../lib/onboarding";
import { Button } from "./ui";
import { AttachmentsBlock, EvidenceChips, LegacyChip, overlayEscape } from "./ReportEvidence";

// DuckAvatar's stroke (ink, 1.7) for the duck's own marks; 1.5 for paths and branches.
const inkStroke = { stroke: "var(--color-ink)", strokeWidth: 1.7, strokeLinejoin: "round" as const, strokeLinecap: "round" as const };
const st = { strokeWidth: 1.5, strokeLinejoin: "round" as const, strokeLinecap: "round" as const };

/** Report sections by key (looked up by name, never by position). */
const SEC = {
  delegated: { id: "rpt-sec-delegated", label: "report.section.delegated" },
  criteria: { id: "rpt-sec-criteria", label: "report.section.criteria" },
  caution: { id: "rpt-sec-caution", label: "report.section.caution" },
  flow: { id: "rpt-sec-flow", label: "report.section.flow" },
  attachments: { id: "rpt-sec-attachments", label: "report.section.attachments" },
  records: { id: "rpt-sec-records", label: "report.section.records" },
} as const;
const SECTIONS = Object.values(SEC);

const scrollTo = (id: string) => document.getElementById(id)?.scrollIntoView?.({ block: "start", behavior: "smooth" });

// ---------- row highlight and expand (caution chips jump to rows) ----------

interface Rows {
  hl: string | null;
  open: Set<string>;
  toggle: (id: string) => void;
  jump: (id: string) => void;
}
const RowsCtx = createContext<Rows>({ hl: null, open: new Set(), toggle: () => {}, jump: () => {} });

function useRowState(): Rows {
  const [hl, setHl] = useState<string | null>(null);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const toggle = useCallback((id: string) => setOpen((s) => (s.has(id) ? new Set([...s].filter((x) => x !== id)) : new Set([...s, id]))), []);
  const jump = useCallback((id: string) => {
    setOpen((s) => new Set([...s, id]));
    setHl(id);
    const el = document.getElementById(id);
    el?.scrollIntoView?.({ block: "center", behavior: "smooth" });
    el?.focus({ preventScroll: true });
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setHl(null), 2400);
  }, []);
  return { hl, open, toggle, jump };
}

const hlClass = (on: boolean) => (on ? "bg-ink/[0.07] outline-2 outline-ink-2/50" : "");

function Block({ id, title, hint, children, label }: { id?: string; title: string; hint?: string; children: ReactNode; label?: string }) {
  return (
    <section id={id} aria-label={label ?? title} className="scroll-mt-3 rounded-xl bg-sheet shadow-e1 p-4">
      <div className="mb-3 flex flex-wrap items-baseline gap-x-2">
        <h3 className="text-[13px] font-semibold">{title}</h3>
        {hint && <span className="text-xs text-ink-3">{hint}</span>}
      </div>
      {/* The content is text people read and copy (report-copy-save); the heading above stays app chrome. */}
      <div className="selectable">{children}</div>
    </section>
  );
}

const None = ({ children }: { children: ReactNode }) => <p className="text-xs text-ink-3">{children}</p>;

function Cell({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="min-w-0 rounded-lg bg-paper px-3 py-2">
      <p className="text-[11px] text-ink-3">{label}</p>
      <div className="mt-0.5 text-[13px] wrap-anywhere whitespace-pre-wrap select-text">{children}</div>
    </div>
  );
}

const Chip = ({ children }: { children: ReactNode }) => <span data-no-select className="rounded-full bg-paper px-2 py-0.5 text-[11px] font-medium text-ink-2">{children}</span>;

/** ① 위임 내용(합의): the brief, type and mode the app started with. With a report below, 완료 기준 points to the
 *  comparison instead of repeating it. */
function DelegatedBlock({ d, compared }: { d: Delegated | null; compared?: number }) {
  const types = useTeamConfig().data?.config.types;
  const hint = t("report.delegated.hint");
  if (!d)
    return (
      <Block id={SEC.delegated.id} title={t("report.delegated.title")} hint={hint}>
        <None>{t("report.delegated.noRecord")}</None>
      </Block>
    );
  const type = taskTypeLabel(d.taskType, types);
  const dash = <span className="text-ink-3">—</span>;
  const lines = doneLines(d.brief?.done);
  return (
    <Block id={SEC.delegated.id} title={t("report.delegated.title")} hint={hint}>
      <div className="mb-2.5 flex flex-wrap gap-1.5">
        {d.routine && <Chip>{t("report.delegated.routine")}</Chip>}
        {type && <Chip>{type}</Chip>}
        {d.permissionMode && <Chip>{MODE_LABEL[d.permissionMode] ?? d.permissionMode}</Chip>}
        {d.folder && <Chip>{d.folder}</Chip>}
        {guardChip(d.deliveryGuard) && <Chip>{guardChip(d.deliveryGuard)}</Chip>}
      </div>
      {!d.stored ? (
        <None>{t("report.delegated.notStored")}</None>
      ) : d.brief ? (
        <div className="grid items-start gap-2 sm:grid-cols-2">
          <Cell label={t("report.delegated.goal")}>{d.brief.goal || dash}</Cell>
          <Cell label={t("report.delegated.outOfScope")}>{d.brief.outOfScope || dash}</Cell>
          <Cell label={t("report.delegated.constraints")}>{d.brief.constraints || dash}</Cell>
          <Cell label={t("report.delegated.done")}>
            {!lines.length ? (
              dash
            ) : compared != null ? (
              <button className="text-ink-2 underline underline-offset-2 hover:text-ink" onClick={() => scrollTo(SEC.criteria.id)}>
                {t("report.delegated.compared", { count: compared })}
              </button>
            ) : (
              <ul className="space-y-0.5">
                {lines.map((l, i) => (
                  <li key={i}>· {l}</li>
                ))}
              </ul>
            )}
          </Cell>
        </div>
      ) : (
        <Cell label={d.routine ? t("report.delegated.routinePrompt") : t("report.delegated.prompt")}>{d.prompt || dash}</Cell>
      )}
    </Block>
  );
}

const MET_TONE = { yes: "bg-ok/10 text-ok", no: "bg-danger/10 text-danger", partial: "bg-warn/10 text-warn" } as const;
/** A met criterion that is not backed: no evidence (△, warn) or failing tests (!, danger) — never a green ✓. */
const FLAG_MARK = { noEvidence: { mark: "△", tone: MET_TONE.partial }, testsFailed: { mark: "!", tone: MET_TONE.no } } as const;

export function Mark({ met, small, flag }: { met: CriterionRow["met"]; small?: boolean; flag?: MarkFlag | null }) {
  useT();
  const f = met && flag ? FLAG_MARK[flag] : null;
  const cls = f ? f.tone : met ? MET_TONE[met] : "border border-dashed border-ink-3/60 text-ink-3";
  const label = f ? `${MET_LABEL[met!]} · ${MARK_FLAG_LABEL[flag!]}` : met ? MET_LABEL[met] : t("report.met.none");
  return (
    <span className={`flex shrink-0 items-center justify-center rounded-full font-bold select-none ${small ? "h-4 w-4 text-[10px]" : "mt-px h-5 w-5 text-[11px]"} ${cls}`} title={label} data-mark={f ? flag : (met ?? "none")} data-no-select>
      <span aria-hidden>{f ? f.mark : met ? MET_MARK[met] : "·"}</span>
      <span className="sr-only">{label}</span>
    </span>
  );
}

/** One criterion with its verdict and evidence. Display only: shared with the tour's report card (tour-v2). */
export function CriterionLine({ c, bgId }: { c: CriterionRow; bgId?: string }) {
  useT();
  return (
    <>
      <Mark met={c.met} flag={markFlag(c)} />
      <div className="min-w-0 flex-1">
        <p className="text-[13px] wrap-anywhere select-text">
          {c.criterion}
          {!c.agreed && <span className="ml-1.5 align-middle text-[11px] text-ink-3">{t("report.criteria.agentAdded")}</span>}
        </p>
        <p className="text-xs wrap-anywhere text-ink-3 select-text">{c.met ? c.evidence || t("report.criteria.emptyEvidence") : t("report.criteria.notCompared")}</p>
        {c.met && <EvidenceChips bgId={bgId} row={c} />}
      </div>
    </>
  );
}

/** ② 완료 기준 대조. */
function CriteriaBlock({ rows, bgId, legacy }: { rows: CriterionRow[]; bgId?: string; legacy?: boolean }) {
  const { hl } = useContext(RowsCtx);
  return (
    <Block id={SEC.criteria.id} title={t("report.criteria.title")} hint={t("report.criteria.hint")}>
      {legacy && (
        <div className="mb-2">
          <LegacyChip />
        </div>
      )}
      {rows.length === 0 ? (
        <None>{t("report.criteria.none")}</None>
      ) : (
        <ul className="divide-y divide-line">
          {rows.map((c, i) => (
            <li key={i} id={rowId.criterion(i)} tabIndex={-1} className={`-mx-2 flex items-start gap-2.5 rounded-md px-2 py-2 transition-colors first:pt-0 last:pb-0 ${hlClass(hl === rowId.criterion(i))}`}>
              <CriterionLine c={c} bgId={bgId} />
            </li>
          ))}
        </ul>
      )}
    </Block>
  );
}

/** ③ 먼저 확인할 것: one chip per item, at most five before "외 N개". A chip jumps to its row. */
function CautionBlock({ items, alerts = [] }: { items: Caution[]; /** delivery-guard post-checks (local git). */ alerts?: string[] }) {
  const { jump } = useContext(RowsCtx);
  const [all, setAll] = useState(false);
  const shown = all ? items : items.slice(0, CAUTION_CHIPS);
  const rest = items.length - shown.length;
  return (
    <Block id={SEC.caution.id} title={t("report.caution.title")} label={t("report.caution.label")} hint={items.length ? t("report.caution.hint") : undefined}>
      {alerts.length > 0 && (
        <ul className="mb-2 space-y-1" aria-label={t("report.caution.delivery")}>
          {alerts.map((a) => (
            <li key={a} className="flex items-center gap-1.5 text-xs text-danger">
              <ShieldAlert size={13} className="shrink-0" aria-hidden />
              {a}
            </li>
          ))}
        </ul>
      )}
      {items.length === 0 && alerts.length === 0 ? (
        <None>{t("report.caution.none")}</None>
      ) : items.length === 0 ? null : (
        <ul className="flex flex-wrap gap-1.5">
          {shown.map((a) => (
            <li key={`${a.target}:${a.labels.join("+")}`} className="min-w-0 max-w-full">
              <button
                onClick={() => jump(a.target)}
                aria-label={`${a.labels.join(", ")}: ${a.text}`}
                className={`flex max-w-full items-center gap-1.5 rounded-full border px-2.5 py-1 text-left text-xs ${a.danger ? "border-danger/30 bg-danger/5 text-danger" : "border-line bg-paper text-ink-2"} hover:border-ink-3`}
              >
                {a.met && <Mark met={a.met} small />}
                <span className="shrink-0 font-semibold">{a.labels.join(" · ")}</span>
                <span className={`min-w-0 ${all ? "wrap-anywhere" : "truncate"}`}>{a.text}</span>
              </button>
            </li>
          ))}
          {(rest > 0 || all) && items.length > CAUTION_CHIPS && (
            <li>
              <button onClick={() => setAll(!all)} aria-expanded={all} className="rounded-full px-2 py-1 text-xs text-ink-2 underline underline-offset-2 hover:text-ink">
                {all ? t("report.caution.fold") : t("report.caution.rest", { count: rest })}
              </button>
            </li>
          )}
        </ul>
      )}
    </Block>
  );
}

// ---------- ④ 흐름: footprints, forks, dots and puddles on one stem ----------

/** A duck's webbed foot (three toes, scalloped webbing), pointing the way it walks. */
const FOOT = "M12 21.2 C10.4 18.7 7.5 14.5 4.4 8.2 Q9.3 9.6 12 3.4 Q14.7 9.6 19.6 8.2 C16.5 14.5 13.6 18.7 12 21.2 Z";
const TOES = "M12 18.6 L7 9.9 M12 18.6 V7 M12 18.6 L17 9.9";

function FootMark({ here, tilt = 0 }: { here: boolean; tilt?: number }) {
  return (
    <g transform={`rotate(${tilt} 12 13)`}>
      <path d={FOOT} style={{ fill: here ? YELLOW.duckFoot : "var(--color-ink-3)" }} {...inkStroke} />
      <path d={TOES} fill="none" style={{ stroke: here ? "var(--color-ink)" : "var(--color-sheet)", opacity: here ? 0.55 : 0.8 }} strokeWidth={1} strokeLinecap="round" />
    </g>
  );
}

function Foot({ n, here }: { n: number; here: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden className="relative bg-sheet">
      <FootMark here={here} tilt={n % 2 ? -12 : 12} />
    </svg>
  );
}

/** A fork with one branch per option; the chosen one in ink, the others faint. */
function Fork({ d }: { d: ReportDecision }) {
  const n = Math.min(Math.max(d.options.length, 2), 5);
  const chosen = d.options.indexOf(d.chosen);
  const branches = Array.from({ length: n }, (_, i) => {
    const a = ((-60 + (120 * i) / (n - 1)) * Math.PI) / 180;
    return { x: 12 + 9 * Math.sin(a), y: 13.5 - 9 * Math.cos(a), on: i === chosen };
  });
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden className="relative bg-sheet">
      <path d="M12 22 V13.5" fill="none" style={{ stroke: "var(--color-ink)" }} {...st} />
      {branches
        .filter((b) => !b.on)
        .map((b, i) => (
          <path key={i} d={`M12 13.5 L${b.x.toFixed(2)} ${b.y.toFixed(2)}`} fill="none" style={{ stroke: "var(--color-ink-3)", opacity: 0.55 }} {...st} />
        ))}
      {branches
        .filter((b) => b.on)
        .map((b, i) => (
          <g key={i}>
            <path d={`M12 13.5 L${b.x.toFixed(2)} ${b.y.toFixed(2)}`} fill="none" style={{ stroke: "var(--color-ink)" }} {...st} />
            <circle cx={b.x} cy={b.y} r="1.6" style={{ fill: "var(--color-ink)" }} />
          </g>
        ))}
      <circle cx="12" cy="13.5" r="1.9" style={{ fill: "var(--color-sheet)", stroke: "var(--color-ink)" }} {...st} />
    </svg>
  );
}

/** A puddle where the duck got stuck: one flat ellipse and two ripple lines; the current position stands in it with a
 *  foot at 0.7 scale (review LOW: smaller read as a stray mark). */
function Puddle({ open, here }: { open: boolean; here: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden className="relative bg-sheet">
      <ellipse cx="12" cy="17" rx="9.5" ry="4.2" style={{ fill: "var(--color-chart-2)", fillOpacity: open ? 0.4 : 0.18, stroke: "var(--color-ink)" }} {...st} />
      <path d="M7.2 17 Q12 15.3 16.8 17 M9.6 19.2 Q12 18.3 14.4 19.2" fill="none" style={{ stroke: "var(--color-ink)", opacity: 0.6 }} strokeWidth={1.1} strokeLinecap="round" />
      {here && (
        <g transform="translate(3.6 -1.6) scale(0.7)">
          <FootMark here />
        </g>
      )}
    </svg>
  );
}

/** Two small prints past the puddle: it was resolved. */
const Hop = () => (
  <svg viewBox="0 0 30 14" width="26" height="12" aria-hidden className="inline-block align-[-2px]">
    {[0, 14].map((x, i) => (
      <g key={x} transform={`translate(${x + 1} 0.5) scale(0.55)`}>
        <path d={FOOT} transform={`rotate(${i ? 12 : -12} 12 13)`} style={{ fill: "var(--color-ink-3)" }} {...inkStroke} vectorEffect="non-scaling-stroke" />
      </g>
    ))}
  </svg>
);

function FlagText({ d }: { d: ReportDecision }) {
  const f = flagsOf(d).filter((k) => k !== "hard");
  if (!f.length) return null;
  return <span className="text-[11px] font-medium text-ink-2">{f.map((k) => FLAG_LABEL[k]).join(" · ")}</span>;
}

function Flags({ d }: { d: ReportDecision }) {
  const f = flagsOf(d);
  if (!f.length) return null;
  return (
    <span className="flex flex-wrap gap-1">
      {f.map((k) => (
        <span key={k} className={`rounded-full border px-1.5 py-px text-[11px] font-medium ${k === "hard" ? "border-danger/30 bg-danger/5 text-danger" : "border-line bg-paper text-ink-2"}`}>
          {FLAG_LABEL[k]}
        </span>
      ))}
    </span>
  );
}

function DecisionNode({ d }: { d: ReportDecision }) {
  return (
    <div className="space-y-1">
      <p className="text-[13px] font-medium wrap-anywhere">
        <span className="mr-1.5 text-[11px] font-normal text-ink-3">{t("report.flow.fork")}</span>
        {d.title}
      </p>
      {d.chosen && (
        <p className="text-xs wrap-anywhere text-ink">
          <span className="text-ink-3">{t("report.flow.chosen")} </span>
          {d.chosen}
          {d.options.length > 1 && <span className="text-ink-3"> · {t("report.flow.alternatives", { count: d.options.filter((o) => o !== d.chosen).length })}</span>}
        </p>
      )}
      <Flags d={d} />
    </div>
  );
}

function BlockedNode({ b }: { b: ReportBlocked }) {
  return (
    <div className="space-y-0.5">
      <p className="text-[13px] font-medium wrap-anywhere">
        <span className="mr-1.5 text-[11px] font-normal text-ink-3">{t("report.flow.blocked")}</span>
        {b.what}
      </p>
      {b.cause && <p className="text-xs wrap-anywhere text-ink-2">{b.cause}</p>}
      {b.open ? (
        <p className={`text-xs ${b.needsUser ? "font-medium text-danger" : "text-ink-3"}`}>{b.needsUser ? t("report.flow.needsUser") : t("report.flow.stillBlocked")}</p>
      ) : (
        <p className="text-xs wrap-anywhere text-ink-3">
          <Hop /> {t("report.flow.passed")}{b.resolution ? ` · ${b.resolution}` : ""}
        </p>
      )}
    </div>
  );
}

function FlowBlock({ r }: { r: Report }) {
  const { items, hiddenDecisions } = flow(r);
  return (
    <Block id={SEC.flow.id} title={t("report.flow.title")} hint={t("report.flow.hint")}>
      {items.length === 0 ? (
        <None>{t("report.flow.none")}</None>
      ) : (
        <ol>
          {items.map((it, i) => {
            const dot = it.kind === "decision" && !it.flagged;
            const here = (it.kind === "step" || it.kind === "blocked") && it.here;
            return (
              <li key={i} className={`relative flex gap-3 ${dot ? "pb-2" : "pb-3"} last:pb-0`} aria-current={here ? "step" : undefined}>
                {i < items.length - 1 && <span aria-hidden className={`absolute bottom-0 left-[11px] border-l-[1.5px] border-dashed border-ink-3/50 ${dot ? "top-[14px]" : "top-[22px]"}`} />}
                <span className={`flex w-[22px] shrink-0 justify-center ${dot ? "pt-[5px]" : ""}`}>
                  {it.kind === "step" ? (
                    <Foot n={it.n} here={it.here} />
                  ) : it.kind === "decision" ? (
                    it.flagged ? (
                      <Fork d={it.decision} />
                    ) : (
                      <span aria-hidden className="relative h-1.5 w-1.5 rounded-full bg-ink" />
                    )
                  ) : (
                    <Puddle open={it.blocked.open} here={it.here} />
                  )}
                </span>
                <div className={`min-w-0 flex-1 ${dot ? "" : "pt-0.5"}`}>
                  {here && <span className="sr-only">{t("report.flow.here")}</span>}
                  {it.kind === "step" ? (
                    <div className="space-y-0.5">
                      <p className="flex items-baseline gap-1.5 text-[13px]">
                        <span className="w-3 shrink-0 text-right text-[11px] text-ink-3 tabular-nums">{it.n}</span>
                        <span className="min-w-0 wrap-anywhere">{it.work.step}</span>
                      </p>
                      {it.work.result && <p className="pl-[18px] text-xs wrap-anywhere text-ink-2">{it.work.result}</p>}
                      {it.work.files.length > 0 && (
                        <p className="truncate pl-[18px] font-mono text-[11px] text-ink-3" title={it.work.files.join("\n")}>
                          {it.work.files.join(" · ")}
                        </p>
                      )}
                    </div>
                  ) : it.kind === "decision" ? (
                    it.flagged ? (
                      <DecisionNode d={it.decision} />
                    ) : (
                      <p className="truncate text-xs text-ink-2">
                        {it.decision.title}
                        {it.decision.chosen && <span className="text-ink-3"> · {it.decision.chosen}</span>}
                      </p>
                    )
                  ) : (
                    <BlockedNode b={it.blocked} />
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      )}
      {hiddenDecisions > 0 && <p className="mt-3 text-xs text-ink-3">{t("report.flow.hidden", { count: hiddenDecisions })}</p>}
    </Block>
  );
}

// ---------- ⑤ tables ----------

const th = "py-1.5 pr-3 text-left text-xs font-medium text-ink-3";
const td = "py-1.5 pr-3 align-top wrap-anywhere";
const dash = <span className="text-ink-3">—</span>;
/** Titles longer than this get a "더 보기" control (they are clamped to two lines). */
const LONG_TITLE = 36;

function Title({ id, text }: { id: string; text: string }) {
  const { open, toggle } = useContext(RowsCtx);
  const long = text.length > LONG_TITLE;
  const isOpen = open.has(id);
  return (
    <>
      <span className={long && !isOpen ? "line-clamp-2" : undefined}>{text}</span>
      {long && (
        <button onClick={() => toggle(id)} aria-expanded={isOpen} className="mt-0.5 block text-[11px] font-normal text-ink-3 underline underline-offset-2 hover:text-ink">
          {isOpen ? t("report.records.fold") : t("report.records.expand")}
        </button>
      )}
    </>
  );
}

function DecisionTable({ list }: { list: ReportDecision[] }) {
  const { hl } = useContext(RowsCtx);
  if (!list.length) return <None>{t("report.decision.none")}</None>;
  const rank = (d: ReportDecision) => {
    const f = flagsOf(d);
    return f.length ? (["deviates", "hard", "assumed"] as const).indexOf(f[0] as "deviates") : 3;
  };
  const rows = list.map((d, i) => ({ d, i })).sort((a, b) => rank(a.d) - rank(b.d) || a.i - b.i);
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[640px] table-fixed text-[13px]">
        <thead>
          <tr className="border-b border-line">
            <th className={`${th} w-[22%] pl-2`}>{t("report.decision.title")}</th>
            <th className={`${th} w-[16%]`}>{t("report.decision.chosen")}</th>
            <th className={`${th} w-[16%]`}>{t("report.decision.alternatives")}</th>
            <th className={th}>{t("report.decision.reason")}</th>
            <th className={`${th} w-[10%]`}>{t("report.decision.reversible")}</th>
            <th className={`${th} w-[15%] pr-2`}>{t("report.decision.flags")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {rows.map(({ d, i }) => {
            const id = rowId.decision(i);
            const flagged = flagsOf(d).length > 0;
            const others = d.options.filter((o) => o !== d.chosen);
            return (
              <tr key={i} id={id} tabIndex={-1} className={`transition-colors ${flagged ? "bg-paper" : ""} ${hlClass(hl === id)}`}>
                <td className={`${td} pl-2 font-medium`}>
                  <Title id={id} text={d.title} />
                </td>
                <td className={td}>{d.chosen || dash}</td>
                <td className={`${td} text-xs text-ink-2`}>{others.length ? others.join(" · ") : dash}</td>
                <td className={`${td} text-ink-2`}>{d.reason || dash}</td>
                <td className={`${td} text-xs ${d.reversible === "hard" ? "font-medium text-danger" : "text-ink-2"}`}>{d.reversible === "hard" ? t("report.decision.hard") : d.reversible === "easy" ? t("report.decision.easy") : dash}</td>
                <td className={`${td} pr-2`}>
                  <FlagText d={d} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function BlockedTable({ list }: { list: ReportBlocked[] }) {
  const { hl } = useContext(RowsCtx);
  if (!list.length) return <None>{t("report.blockedTable.none")}</None>;
  return (
    <table className="w-full table-fixed text-[13px]">
      <thead>
        <tr className="border-b border-line">
          <th className={`${th} w-[28%] pl-2`}>{t("report.blockedTable.what")}</th>
          <th className={th}>{t("report.blockedTable.cause")}</th>
          <th className={th}>{t("report.blockedTable.resolution")}</th>
          <th className={`${th} w-[18%] pr-2`}>{t("report.blockedTable.status")}</th>
        </tr>
      </thead>
      <tbody className="divide-y divide-line">
        {list.map((b, i) => {
          const id = rowId.blocked(i);
          return (
            <tr key={i} id={id} tabIndex={-1} className={`transition-colors ${b.open && b.needsUser ? "bg-paper" : ""} ${hlClass(hl === id)}`}>
              <td className={`${td} pl-2 font-medium`}>{b.what}</td>
              <td className={`${td} text-ink-2`}>{b.cause || dash}</td>
              <td className={`${td} text-ink-2`}>{b.resolution || dash}</td>
              <td className={`${td} pr-2 text-xs font-medium ${b.open ? (b.needsUser ? "text-danger" : "text-ink-2") : "text-ok"}`}>
                {b.open ? (b.needsUser ? t("report.blockedTable.needsUser") : t("report.blockedTable.open")) : t("report.blockedTable.resolved")}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}


function References({ list }: { list: ReportReference[] }) {
  const [error, setError] = useState<Shown | null>(null);
  if (!list.length) return <None>{t("report.reference.none")}</None>;
  return (
    <>
      <ul className="divide-y divide-line">
        {list.map((r, i) => (
          <li key={i} className="flex items-start gap-2 py-1.5 first:pt-0 last:pb-0">
            <span className="mt-px w-9 shrink-0 text-[11px] text-ink-3">{t(`report.reference.kind.${r.kind}`)}</span>
            <div className="min-w-0 flex-1">
              <p className="font-mono text-xs break-all text-ink select-text">{r.ref}</p>
              {r.note && <p className="text-xs wrap-anywhere text-ink-3">{r.note}</p>}
            </div>
            {/* Links open only after the native confirmation that shows the full address. */}
            {r.kind === "url" && r.ref.startsWith("https://") && (
              <Button
                size="xs"
                aria-label={t("report.reference.openLabel", { ref: r.ref })}
                onClick={() => api.openInsightLink(r.ref).then(() => setError(null), (e) => setError(failure(e)))}
              >
                <ExternalLink size={12} /> {t("report.reference.open")}
              </Button>
            )}
          </li>
        ))}
      </ul>
      {error && <p className="mt-1 text-xs text-danger">{shown(error)}</p>}
    </>
  );
}

function TablesBlock({ r }: { r: Report }) {
  return (
    <Block id={SEC.records.id} title={t("report.records.title")} hint={t("report.records.hint")}>
      <div className="space-y-4">
        <div>
          <h4 className="mb-1 text-xs font-semibold text-ink-2">{t("report.records.decisions", { count: r.decisions.length })}</h4>
          <DecisionTable list={r.decisions} />
        </div>
        <div>
          <h4 className="mb-1 text-xs font-semibold text-ink-2">{t("report.records.blocked", { count: r.blocked.length })}</h4>
          <BlockedTable list={r.blocked} />
        </div>
        <div>
          <h4 className="mb-1 text-xs font-semibold text-ink-2">{t("report.records.references", { count: r.references.length })}</h4>
          <References list={r.references} />
        </div>
      </div>
    </Block>
  );
}

// ---------- the screen ----------

function headOf(v: TaskReportView | undefined, session: SessionStatus | null | undefined, failed: boolean): { pose: Pose; line: string; danger?: boolean } {
  const ended = isEnded(session);
  if (failed) return { pose: "failed", line: t("report.head.failed") };
  if (!v) return { pose: "idle", line: t("report.head.loading") };
  switch (v.kind) {
    case "ready":
      return verdict(v.report, v.delegated, session);
    case "invalid":
      return { pose: "failed", line: t("report.head.invalid") };
    case "missing":
      return { pose: ended ? "waiting" : "working", line: ended ? t("report.head.missingEnded") : t("report.head.missingLive") };
    case "off":
      return { pose: "idle", line: t("report.head.off") };
  }
}

function updatedText(s: string | null | undefined): string | null {
  if (!s) return null;
  const ms = Date.parse(s);
  return Number.isNaN(ms) ? s : t("report.head.updated", { time: dateTime(ms) });
}

/** "보고서 없음" with the next step: a finished task can be handed out again with the same brief. */
function MissingNotice({ ended, onRetry }: { ended: boolean; onRetry?: () => void }) {
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-xl bg-sheet shadow-e1 px-4 py-3">
      <span className="rounded-full border border-line px-2 py-0.5 text-[11px] font-medium text-ink-2">{t("report.missing.chip")}</span>
      <p className="min-w-0 flex-1 text-[13px] text-ink-2">{ended ? (onRetry ? t("report.missing.retryHint") : t("report.missing.endedNone")) : t("report.missing.live")}</p>
      {ended && onRetry && (
        <Button variant="primary" onClick={onRetry}>
          {t("report.missing.retry")}
        </Button>
      )}
    </div>
  );
}

/** delivery-guard post-checks when there is no report to put them under 먼저 확인할 것. */
function DeliveryAlerts({ lines }: { lines: string[] }) {
  if (lines.length === 0) return null;
  return (
    <ul aria-label={t("report.caution.delivery")} className="space-y-1 rounded-xl border border-danger/30 bg-danger/5 px-4 py-3">
      {lines.map((a) => (
        <li key={a} className="flex items-center gap-1.5 text-[13px] text-danger">
          <ShieldAlert size={13} className="shrink-0" aria-hidden />
          {a}
        </li>
      ))}
    </ul>
  );
}

/** The report body without the dialog frame (also used by tests). `onRetry` gets the stored brief, if any. */
export function ReportBody({ view, session, onRetry, bgId }: { view: TaskReportView; session: SessionStatus | null | undefined; onRetry?: (brief: Brief | null) => void; bgId?: string }) {
  useT();
  const rowState = useRowState();
  const d = view.delegated;
  const retry = onRetry && (() => onRetry(d?.brief ?? null));
  if (view.kind !== "ready") {
    return (
      <div className="space-y-3">
        {view.kind === "invalid" && (
          <p role="alert" className="rounded-xl border border-danger/30 bg-danger/5 px-4 py-3 text-[13px] text-danger">
            {REPORT_ERROR_TEXT[view.code]}
          </p>
        )}
        {view.kind === "missing" && <MissingNotice ended={isEnded(session)} onRetry={retry} />}
        <DeliveryAlerts lines={alertLines(d?.deliveryAlert)} />
        <DelegatedBlock d={d} />
      </div>
    );
  }
  const r = view.report;
  const rows = criteriaRows(r, d);
  return (
    <RowsCtx.Provider value={rowState}>
      <div className="space-y-3">
        <DelegatedBlock d={d} compared={rows.length} />
        <CriteriaBlock rows={rows} bgId={bgId} legacy={r.legacy} />
        <CautionBlock items={cautions(r, rows, d?.taskType)} alerts={alertLines(d?.deliveryAlert)} />
        <FlowBlock r={r} />
        {(r.attachments?.length ?? 0) > 0 && (
          <Block id={SEC.attachments.id} title={t("report.attachments.title")} hint={t("report.attachments.hint")}>
            <AttachmentsBlock bgId={bgId} list={r.attachments} />
          </Block>
        )}
        <TablesBlock r={r} />
      </div>
    </RowsCtx.Provider>
  );
}

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Modal behaviour: everything else on the page is `inert`, Tab cycles inside the dialog, Escape closes, and focus
 *  goes back where it was. The dialog is portalled to <body> so the rest of the app can be made inert as a whole. */
function useModal(root: RefObject<HTMLElement | null>, dialog: RefObject<HTMLElement | null>, first: RefObject<HTMLElement | null>, onClose: () => void) {
  // The parent re-renders on every poll: keep the latest callback without re-running the effect.
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    const host = root.current;
    const made: Element[] = [];
    for (const el of Array.from(document.body.children)) {
      if (el === host || el.hasAttribute("inert")) continue;
      el.setAttribute("inert", "");
      made.push(el);
    }
    first.current?.focus();
    const focusables = () => Array.from(dialog.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []);
    // Capture phase, stopped here: the task detail's own Escape (close the panel) does not fire as well.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        // An open image or full document view closes first (report-evidence).
        if (overlayEscape.current) overlayEscape.current();
        else close.current();
        return;
      }
      if (e.key !== "Tab") return;
      const list = focusables();
      e.preventDefault();
      if (!list.length) return;
      const i = list.indexOf(document.activeElement as HTMLElement);
      const next = i < 0 ? (e.shiftKey ? list.length - 1 : 0) : (i + (e.shiftKey ? -1 : 1) + list.length) % list.length;
      list[next].focus();
    };
    const onFocusIn = (e: FocusEvent) => {
      if (dialog.current && !dialog.current.contains(e.target as Node)) first.current?.focus();
    };
    window.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocusIn);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocusIn);
      for (const el of made) el.removeAttribute("inert");
      before?.focus?.();
    };
  }, [root, dialog, first]);
}

export type ReportStage = "reviewWait" | "reviewed" | "done";
export const stageLabel = (s: ReportStage) => t(`report.stage.${s}`);

export function TaskReportDialog({
  id,
  title,
  session,
  onClose,
  onRetry,
  footer,
  stage,
}: {
  id: string;
  title: string;
  session: SessionStatus | null;
  onClose: () => void;
  /** "다시 맡기기" for a finished task without a report; gets the stored brief, if any. */
  onRetry?: (brief: Brief | null) => void;
  /** Bar under the report (리뷰 탭: "리뷰했어요 · 다음" and "닫기", review-inbox). */
  footer?: ReactNode;
  /** Head chip (task-lifecycle): "리뷰 대기", "리뷰했어요", or "완료" only after 정리 끝. */
  stage?: ReportStage;
}) {
  useT();
  const live = !isEnded(session);
  // Read on demand, kept only while the screen is open; a running task is re-read every 5 s.
  const q = useQuery({ queryKey: ["task-report", id], queryFn: () => api.taskReport(id), gcTime: 0, refetchInterval: live ? 5_000 : false });
  // The start checklist counts a report that was really there (not the "no report" notice).
  const ready = q.data?.kind === "ready";
  useEffect(() => {
    if (ready) markDone("reportOpened");
  }, [ready]);
  const rootRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  useModal(rootRef, dialogRef, closeRef, onClose);
  const types = useTeamConfig().data?.config.types;
  const head = headOf(q.data, session, q.isError);
  const updated = q.data?.kind === "ready" ? updatedText(q.data.report.updatedAt) : null;
  const taskType = q.data?.delegated?.taskType ?? null;
  const role = taskTypeLabel(taskType, types) ?? t("report.dialog.defaultRole");
  const color = roleColor(taskType);
  return createPortal(
    <div ref={rootRef} className="fixed inset-0 z-40 flex items-start justify-center bg-scrim/45 px-4 py-6 sm:px-8" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="report-title"
        data-report-dialog
        className="flex max-h-full w-full max-w-[880px] flex-col overflow-hidden rounded-xl bg-paper shadow-e3"
      >
        <header className="shrink-0 border-b border-line bg-sheet px-5 pt-4 pb-3">
          <div className="flex items-start gap-4">
            <div className="flex shrink-0 flex-col items-center gap-1">
              <span className="rounded-full bg-duck-stage-quiet p-1">
                <DuckAvatar pose={head.pose} size={52} color={color} label={t("report.dialog.duck", { role })} />
              </span>
              <RoleChip label={role} color={color} />
            </div>
            <div className="min-w-0 flex-1 pt-0.5">
              <p className="truncate text-[11px] text-ink-3">
                {t("report.dialog.kicker")}{" "}
                <span id="report-title" className="selectable">
                  {title}
                </span>
                {stage && (
                  <span data-report-stage className={`ml-2 rounded-full border px-2 py-px text-[10px] font-medium ${stage === "reviewWait" ? "border-line text-ink-2" : "border-ok/40 text-ok"}`}>
                    {stageLabel(stage)}
                  </span>
                )}
              </p>
              <p className={`selectable mt-0.5 text-[15px] font-semibold wrap-anywhere ${head.danger ? "text-danger" : ""}`} aria-live="polite" data-verdict-danger={head.danger ? "" : undefined}>
                {head.line}
              </p>
              {updated && <p className="mt-0.5 text-xs text-ink-3">{updated}</p>}
            </div>
            <button ref={closeRef} onClick={onClose} aria-label={t("report.dialog.close")} className="shrink-0 rounded-md p-1 text-ink-3 hover:text-ink">
              <X size={18} />
            </button>
          </div>
          {q.data?.kind === "ready" && (
            <nav aria-label={t("report.dialog.toc")} className="mt-3 -mb-1 flex flex-wrap gap-x-1 gap-y-1">
              {SECTIONS.filter((s) => s.id !== SEC.attachments.id || (q.data?.kind === "ready" && (q.data.report.attachments?.length ?? 0) > 0)).map((s) => (
                <button key={s.id} onClick={() => scrollTo(s.id)} className="rounded-md px-2 py-1 text-xs text-ink-2 hover:bg-paper hover:text-ink">
                  {t(s.label)}
                </button>
              ))}
            </nav>
          )}
        </header>
        <div className="min-h-0 flex-1 overflow-auto p-4 sm:p-5">
          {q.data ? (
            <ReportBody
              view={q.data}
              bgId={id}
              session={session}
              onRetry={
                onRetry &&
                ((b) => {
                  onClose();
                  onRetry(b);
                })
              }
            />
          ) : (
            <None>{q.isError ? t("report.dialog.retryLater") : t("report.dialog.loading")}</None>
          )}
        </div>
        {footer && <footer className="flex shrink-0 items-center justify-end gap-2 border-t border-line bg-sheet px-5 py-3">{footer}</footer>}
      </section>
    </div>,
    document.body,
  );
}
