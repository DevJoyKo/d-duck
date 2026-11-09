import { useQuery } from "@tanstack/react-query";
import { Pencil, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { api, type Dimension, type Point, type Summary } from "../lib/api";
import { compact, dateTime, num, usd, usdTick } from "../lib/format";
import { buildQuery, msUntilNextLocalMidnight, PRESETS, toLocalDateInput, type Preset } from "../lib/period";
import { DistributionList } from "./DistributionList";
import type { NavTarget as Page } from "./Sidebar"; // old ids are normalized by App (nav-ia)
import { UsageInsights } from "./UsageInsights";
import { displayName, LabelEditor, SessionsSection, SessionTimeline } from "./SessionBits";
import { PageHeader, SectionTitle, Segmented, Select } from "./ui";
import type { Draft } from "../lib/brief";
import { t } from "../i18n";
import { tx, useT } from "../i18n/react";

const BREAKDOWNS: { dim: Dimension; top: number; aside?: "usage.breakdown.responses" | "usage.breakdown.top10" }[] = [
  { dim: "model", top: 0, aside: "usage.breakdown.responses" },
  { dim: "project", top: 8, aside: "usage.breakdown.responses" },
  { dim: "tool", top: 10, aside: "usage.breakdown.top10" },
  { dim: "skill", top: 10 },
  { dim: "command", top: 10 },
  { dim: "mcpServer", top: 8 },
  { dim: "subagent", top: 8 },
];

// Single-series charts: one hue each, recessive grid (dataviz skill).
// Cost bars in the secondary ink so they never outshine the yellow "waiting" accent (brightest in dark mode).
const INK = "var(--color-ink-2)";
const OUTPUT_STEEL = "var(--color-chart-2)";

function useLocalDay(): string {
  const [day, setDay] = useState(() => toLocalDateInput(Date.now()));
  useEffect(() => {
    const t = setTimeout(() => setDay(toLocalDateInput(Date.now())), msUntilNextLocalMidnight(new Date()) + 1000);
    return () => clearTimeout(t);
  }, [day]);
  return day;
}

function periodPhrase(preset: Preset): string {
  return t(`usage.phrase.${preset}`);
}

/** One headline number (the estimate) and the rest said in a sentence; token counts follow as plain text. */
function Lead({ s, preset, empty }: { s: Summary; preset: Preset; empty: boolean }) {
  const t = useT();
  const b = (v: string) => <strong className="font-semibold text-ink">{v}</strong>;
  return (
    <section>
      <div className="text-[40px] leading-none font-semibold tracking-tight">{usd(s.estCostUsd)}</div>
      {empty ? (
        <p className="mt-2 max-w-[70ch] text-[15px] text-ink-2">{t("usage.lead.empty")}</p>
      ) : (
        <p className="mt-2 max-w-[70ch] text-[15px] text-ink-2">
          {t("usage.lead.summary", {
            period: periodPhrase(preset),
            sessions: t("usage.lead.sessions", { count: s.sessions }),
            responses: t("usage.lead.responses", { count: s.responses }),
            tools: t("usage.lead.tools", { count: s.toolCalls }),
            skills: t("usage.lead.skills", { count: s.skillCalls }),
          })}
        </p>
      )}
      <p className="mt-3 text-[13px] text-ink-2">
        {tx("usage.lead.tokens", {
          output: b(compact(s.tokens.output)),
          thinking: compact(s.tokens.thinking),
          input: b(compact(s.tokens.input)),
          cacheWrite: b(compact(s.tokens.cacheWrite)),
          cacheRead: b(compact(s.tokens.cacheRead)),
        })}
      </p>
      {s.unpricedModels.length > 0 && <p className="mt-1 text-xs text-ink-2">{t("usage.lead.unpriced", { models: s.unpricedModels.join(", ") })}</p>}
    </section>
  );
}

function ChartTooltip({ active, payload, label, format }: { active?: boolean; payload?: { value: number }[]; label?: string; format: (n: number) => string }) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg bg-sheet shadow-e3 px-3 py-2 text-xs">
      <div className="text-ink-3">{label}</div>
      <div className="font-semibold text-ink">{format(payload[0].value)}</div>
    </div>
  );
}

function TrendChart({
  data,
  dataKey,
  color,
  format,
  tick,
  title,
}: {
  data: Record<string, unknown>[];
  dataKey: string;
  color: string;
  format: (n: number) => string;
  tick?: (n: number) => string;
  title: string;
}) {
  return (
    <div className="border-t border-line pt-5">
      <SectionTitle>{title}</SectionTitle>
      <div className="h-48" role="img" aria-label={title}>
        <ResponsiveContainer>
          <BarChart data={data} margin={{ left: 0, right: 0, top: 4, bottom: 0 }} barCategoryGap={2}>
            <CartesianGrid vertical={false} stroke="var(--color-line)" />
            <XAxis dataKey="bucket" tickLine={false} axisLine={false} tick={{ fontSize: 11, fill: "var(--color-ink-3)" }} minTickGap={24} />
            <YAxis tickFormatter={(v) => (tick ?? format)(Number(v))} tickLine={false} axisLine={false} tick={{ fontSize: 11, fill: "var(--color-ink-3)" }} width={56} />
            <Tooltip cursor={{ fill: "var(--color-line)", opacity: 0.5 }} content={<ChartTooltip format={format} />} />
            <Bar dataKey={dataKey} fill={color} radius={[4, 4, 0, 0]} maxBarSize={28} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

function shortBucket(p: Point): string {
  // "2026-11-07" → "11/7", "2026-11-07 14:00" → "14시"
  const m = p.bucket.match(/^\d{4}-(\d{2})-(\d{2})(?: (\d{2}):00)?$/);
  if (!m) return p.bucket;
  return m[3] !== undefined ? t("usage.chart.hour", { hour: String(Number(m[3])) }) : t("usage.chart.day", { month: String(Number(m[1])), day: String(Number(m[2])) });
}

function SessionDrawer({ sessionId, onClose }: { sessionId: string; onClose: () => void }) {
  const t = useT();
  const detail = useQuery({ queryKey: ["usage", "detail", sessionId], queryFn: () => api.sessionDetail(sessionId) });
  const d = detail.data;
  const [editing, setEditing] = useState(false);
  const pencilRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    // Focus moves into the dialog and returns to the row that opened it; Esc closes.
    const opener = document.activeElement as HTMLElement | null;
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        // The title editor's input handles its own Escape (any language, so not matched by its label).
        if ((e.target as HTMLElement | null)?.tagName === "INPUT") return;
        return onClose();
      }
      if (e.key !== "Tab") return;
      // Keep Tab inside the modal dialog.
      const dialog = closeRef.current?.closest('[role="dialog"]');
      const items = dialog ? Array.from(dialog.querySelectorAll<HTMLElement>("button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])")) : [];
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      opener?.focus();
    };
  }, [onClose]);
  return (
    <div className="fixed inset-0 z-20 flex justify-end bg-ink/20" onClick={onClose}>
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="session-detail-title"
        className="h-full w-[560px] max-w-full overflow-auto border-l border-line bg-sheet p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <h2 id="session-detail-title" className="flex items-center gap-1.5 text-[15px] font-semibold">
              {d && editing ? (
                <LabelEditor
                  sessionId={d.session.sessionId}
                  current={displayName(d.session)}
                  onDone={() => {
                    setEditing(false);
                    setTimeout(() => pencilRef.current?.focus(), 0);
                  }}
                />
              ) : (
                <>
                  <span className="min-w-0 truncate">{d ? displayName(d.session) : t("usage.drawer.loading")}</span>
                  {d && (
                    <button ref={pencilRef} aria-label={t("usage.drawer.editTitle")} onClick={() => setEditing(true)} className="rounded p-1 text-ink-3 hover:bg-paper hover:text-ink">
                      <Pencil size={13} />
                    </button>
                  )}
                </>
              )}
            </h2>
            {d && (
              <p className="mt-0.5 text-xs text-ink-3">
                {d.session.projectName ?? t("usage.drawer.noProject")}
                {d.session.gitBranch ? t("usage.drawer.branch", { branch: d.session.gitBranch }) : ""}
                <br />
                {t("usage.drawer.times", { start: dateTime(d.session.firstTs), last: dateTime(d.session.lastTs) })}
              </p>
            )}
          </div>
          <button ref={closeRef} onClick={onClose} aria-label={t("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={18} />
          </button>
        </div>
        {d && (
          <div className="space-y-6">
            <dl className="grid grid-cols-2 gap-4">
              <div>
                <dd className="text-[20px] font-semibold">{usd(d.session.estCostUsd)}</dd>
                <dt className="text-xs text-ink-3">{t("usage.drawer.estCost")}</dt>
              </div>
              <div>
                <dd className="text-[20px] font-semibold">{d.session.reportedCostUsd !== null ? usd(d.session.reportedCostUsd) : t("usage.drawer.notRecorded")}</dd>
                <dt className="text-xs text-ink-3">{t("usage.drawer.reportedCost")}</dt>
              </div>
              <div>
                <dd className="text-[20px] font-semibold">{compact(d.session.tokens.output)}</dd>
                <dt className="text-xs text-ink-3">{t("usage.drawer.outputTokens")}</dt>
              </div>
              <div>
                <dd className="text-[20px] font-semibold">{num(d.session.responses)}</dd>
                <dt className="text-xs text-ink-3">{t("usage.drawer.responses")}</dt>
              </div>
            </dl>
          </div>
        )}
        <SessionTimeline sessionId={sessionId} />
        {d && (
          <details className="mt-6 rounded-lg border border-line">
            <summary className="cursor-pointer px-3 py-2 text-[13px] font-semibold select-none">{t("usage.drawer.details")}</summary>
            <div className="space-y-6 border-t border-line p-3">
              <div>
                <SectionTitle>{t("usage.drawer.model")}</SectionTitle>
                <DistributionList items={d.models} />
              </div>
              <div>
                <SectionTitle>{t("usage.drawer.tool")}</SectionTitle>
                <DistributionList items={d.tools} />
              </div>
              <div>
                <SectionTitle>{t("usage.drawer.skill")}</SectionTitle>
                <DistributionList items={d.skills} />
              </div>
            </div>
          </details>
        )}
      </aside>
    </div>
  );
}

export function UsagePage({ onNavigate, onDraft }: { onNavigate?: (p: Page) => void; onDraft?: (d: Draft) => void } = {}) {
  const t = useT();
  const day = useLocalDay();
  const [preset, setPreset] = useState<Preset>("7d");
  const [custom, setCustom] = useState(() => ({ from: toLocalDateInput(Date.now() - 6 * 86_400_000), to: toLocalDateInput(Date.now()) }));
  const [project, setProject] = useState("");
  const [model, setModel] = useState("");
  const [openSession, setOpenSession] = useState<string | null>(null);

  // `day` changes at local midnight, so "today"/"7d" roll over in a window left open for days.
  const q = useMemo(
    () => buildQuery(preset, new Date(), { projects: project ? [project] : [], models: model ? [model] : [] }, custom),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [preset, project, model, custom, day],
  );

  const options = useQuery({ queryKey: ["usage", "options"], queryFn: api.filterOptions });
  const summary = useQuery({ queryKey: ["usage", "summary", q], queryFn: () => api.summary(q) });
  const series = useQuery({ queryKey: ["usage", "series", q], queryFn: () => api.timeseries(q) });
  const breakdowns = useQuery({
    queryKey: ["usage", "breakdowns", q],
    queryFn: async () => Object.fromEntries(await Promise.all(BREAKDOWNS.map(async (b) => [b.dim, await api.distribution(q, b.dim, b.top)] as const))),
  });

  const s = summary.data;
  const empty = !!s && s.responses === 0 && s.skillCalls === 0 && s.commandCalls === 0;
  const chartData = (series.data ?? []).map((p) => ({ bucket: shortBucket(p), cost: p.estCostUsd, output: p.tokens.output }));
  const closeDrawer = useCallback(() => setOpenSession(null), []);

  return (
    <div>
      <PageHeader title={t("usage.title")}>
        <Segmented label={t("usage.period")} value={preset} options={PRESETS.map((p) => ({ id: p.id, label: t(`usage.preset.${p.id}`) }))} onChange={setPreset} />
      </PageHeader>
      {/* Filters get their own row so the header never wraps into three lines. */}
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-2.5">
        {preset === "custom" && (
          <span className="flex items-center gap-1.5 text-[13px] text-ink-2">
            {tx("usage.filter.range", {
              from: <input aria-label={t("usage.filter.from")} type="date" value={custom.from} onChange={(e) => setCustom({ ...custom, from: e.target.value })} className="h-8 rounded-lg border border-line bg-sheet px-2" />,
              to: <input aria-label={t("usage.filter.to")} type="date" value={custom.to} onChange={(e) => setCustom({ ...custom, to: e.target.value })} className="h-8 rounded-lg border border-line bg-sheet px-2" />,
            })}
          </span>
        )}
        <Select label={t("usage.filter.project")} value={project} onChange={(e) => setProject(e.target.value)}>
          <option value="">{t("usage.filter.all")}</option>
          {(options.data?.[0] ?? []).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </Select>
        <Select label={t("usage.filter.model")} value={model} onChange={(e) => setModel(e.target.value)}>
          <option value="">{t("usage.filter.all")}</option>
          {(options.data?.[1] ?? []).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </Select>
      </div>

      <div className="mx-auto max-w-[1180px] space-y-10 px-8 py-8">
        {s && <Lead s={s} preset={preset} empty={empty} />}

        <UsageInsights q={q} onNavigate={onNavigate} onDraft={onDraft} />

        <div className="grid gap-x-10 gap-y-8 lg:grid-cols-2">
          <TrendChart title={t(preset === "today" ? "usage.chart.hourlyCost" : "usage.chart.dailyCost")} data={chartData} dataKey="cost" color={INK} format={usd} tick={usdTick} />
          <TrendChart title={t(preset === "today" ? "usage.chart.hourlyOutput" : "usage.chart.dailyOutput")} data={chartData} dataKey="output" color={OUTPUT_STEEL} format={compact} />
        </div>

        <section className="grid gap-x-10 gap-y-8 border-t border-line pt-8 md:grid-cols-2 xl:grid-cols-3">
          {BREAKDOWNS.map((b) => (
            <div key={b.dim}>
              <SectionTitle aside={b.aside && t(b.aside)}>{t(`usage.breakdown.${b.dim}`)}</SectionTitle>
              <DistributionList items={breakdowns.data?.[b.dim] ?? []} showVia={b.dim === "skill"} showCost={b.dim === "model" || b.dim === "project"} prefix={b.dim === "command" ? "/" : ""} />
            </div>
          ))}
        </section>

        <SessionsSection projects={project ? [project] : []} models={model ? [model] : []} onOpen={setOpenSession} />
      </div>
      {openSession && <SessionDrawer sessionId={openSession} onClose={closeDrawer} />}
    </div>
  );
}
