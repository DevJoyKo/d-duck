// "사용 개선 분석" on the usage tab (D-078): free local checks first, then an optional, confirmed Claude review.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRight, ExternalLink, Lightbulb, ListPlus, Loader2, ShieldAlert, Sparkles, Trash2 } from "lucide-react";
import { Fragment, useMemo, useState, type ReactNode } from "react";
import { api, type Advice, type Finding, type InsightLink, type InsightRun, type Query, messageOf } from "../lib/api";
import { dateTime, usd } from "../lib/format";
import { parseMarkdown, type Inline } from "../lib/safeMarkdown";
import type { Draft } from "../lib/brief";
import { adviceItems, draftFromAdvice, draftFromFinding } from "../lib/refine";
import type { NavTarget as Page } from "./Sidebar"; // old ids are normalized by App (nav-ia)
import { Button, SectionTitle } from "./ui";
import { isGateError } from "../lib/auth";
import { GateLine } from "./LoginBits";
import { t } from "../i18n";
import { useT } from "../i18n/react";

const LEVEL: Record<Finding["level"], string> = {
  high: "bg-danger/10 text-danger",
  medium: "bg-warn-soft text-ink",
  info: "bg-paper text-ink-2 border border-line",
};

const LINK: Record<InsightLink, Page> = {
  plugins: "plugins",
  skills: "skills",
  permissions: "permissions",
  settings: "settings",
};

function FindingCard({ f, onNavigate, onDraft }: { f: Finding; onNavigate?: (p: Page) => void; onDraft?: (d: Draft) => void }) {
  const t = useT();
  const link = f.link ? { page: LINK[f.link], label: t(`usage.insights.link.${f.link}`) } : null;
  return (
    <article className="rounded-xl bg-sheet shadow-e1 p-4">
      <div className="flex items-start gap-2">
        <span className={`mt-0.5 shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-semibold ${LEVEL[f.level]}`}>{t(`usage.insights.level.${f.level}`)}</span>
        <h3 className="text-[14px] leading-snug font-semibold">{f.title}</h3>
      </div>
      <dl className="mt-3 space-y-1.5 text-[13px]">
        <div className="flex gap-2">
          <dt className="w-16 shrink-0 text-ink-3">{t("usage.insights.evidence")}</dt>
          <dd className="min-w-0 text-ink">{f.evidence}</dd>
        </div>
        {f.effect && (
          <div className="flex gap-2">
            <dt className="w-16 shrink-0 text-ink-3">{t("usage.insights.effect")}</dt>
            <dd className="min-w-0 text-ink">{f.effect}</dd>
          </div>
        )}
        <div className="flex gap-2">
          <dt className="w-16 shrink-0 text-ink-3">{t("usage.insights.action")}</dt>
          <dd className="min-w-0 text-ink">
            {f.action}
            {f.detail && (
              <details className="mt-1 text-ink-2">
                <summary className="cursor-pointer text-xs text-ink-3 select-none">{t("usage.insights.more")}</summary>
                <p className="mt-1 text-[13px]">{f.detail}</p>
              </details>
            )}
          </dd>
        </div>
      </dl>
      {((link && onNavigate) || onDraft) && (
        <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-line pt-2.5 text-xs">
          {onDraft && (
            <Button variant="quiet" size="sm" onClick={() => onDraft(draftFromFinding(f))}>
              <ListPlus size={12} /> {t("usage.insights.toTask")}
            </Button>
          )}
          {link && onNavigate && (
            <button onClick={() => onNavigate(link.page)} className="ml-auto inline-flex items-center gap-1 font-medium text-ink hover:underline">
              {t("usage.insights.fixIn", { tab: link.label })} <ArrowRight size={12} />
            </button>
          )}
        </div>
      )}
    </article>
  );
}

function InlineView({ items }: { items: Inline[] }) {
  return (
    <>
      {items.map((i, k) => {
        switch (i.t) {
          case "text":
            return <Fragment key={k}>{i.v}</Fragment>;
          case "b":
            return <strong key={k}>{i.v}</strong>;
          case "code":
            return (
              <code key={k} className="rounded bg-paper px-1 text-[12px]">
                {i.v}
              </code>
            );
          case "link":
            return (
              <button
                key={k}
                type="button"
                title={i.url}
                onClick={() => void api.openInsightLink(i.url).catch(() => undefined)}
                className="inline text-left text-ink underline decoration-ink-3 underline-offset-2 hover:decoration-ink"
              >
                {i.label}
                <span className="ml-1 inline-flex items-center gap-0.5 text-[11px] text-ink-3 no-underline">
                  <ExternalLink size={10} aria-hidden />
                  {i.host}
                </span>
              </button>
            );
        }
      })}
    </>
  );
}

/** Safe Markdown: text nodes only, cleaned on every render (stored results too). `images`: checked `data:` images of
 *  a report attachment's image lines, in order (Rust, attachment.rs); without them an image line shows its alt text. */
export function SafeMarkdown({ text, images }: { text: string; images?: { name: string; data: string | null }[] }) {
  const t = useT();
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  return (
    <div className="space-y-2.5 text-[13px] leading-relaxed text-ink">
      {blocks.map((b, k) => {
        switch (b.type) {
          case "h":
            return (
              <h4 key={k} className={b.level === 2 ? "pt-2 text-[14px] font-semibold" : "pt-1 text-[13px] font-semibold"}>
                <InlineView items={b.inl} />
              </h4>
            );
          case "p":
            return (
              <p key={k}>
                <InlineView items={b.inl} />
              </p>
            );
          case "ul":
          case "ol": {
            const List = b.type === "ul" ? "ul" : "ol";
            return (
              <List key={k} className={`space-y-1 pl-5 ${b.type === "ul" ? "list-disc" : "list-decimal"}`}>
                {b.items.map((it, j) => (
                  <li key={j}>
                    <InlineView items={it} />
                  </li>
                ))}
              </List>
            );
          }
          case "pre":
            return (
              <pre key={k} className="overflow-x-auto rounded-lg bg-paper p-3 text-[12px]">
                {b.text}
              </pre>
            );
          case "table":
            return (
              <div key={k} className="overflow-x-auto">
                <table className="border-collapse text-[12px]">
                  <thead>
                    <tr>
                      {b.head.map((h, j) => (
                        <th key={j} className="border border-line bg-paper px-2 py-1 font-semibold" style={{ textAlign: b.align[j] ?? "left" }}>
                          <InlineView items={h} />
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {b.rows.map((row, r) => (
                      <tr key={r}>
                        {row.map((c, j) => (
                          <td key={j} className="border border-line px-2 py-1 align-top" style={{ textAlign: b.align[j] ?? "left" }}>
                            <InlineView items={c} />
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          case "img": {
            // Matched by name and occurrence, never by position alone (security LOW).
            const src = images?.filter((i) => i.name === b.name)[b.k]?.data;
            return src && src.startsWith("data:image/") ? (
              <img key={k} src={src} alt={b.alt} className="max-w-full rounded border border-line" />
            ) : (
              <p key={k} className="text-ink-3">{t("usage.insights.image", { name: b.alt || b.name })}</p>
            );
          }
        }
      })}
    </div>
  );
}

function runMessage(r: Extract<InsightRun, { kind: "failed" }>): ReactNode {
  const e = r.error;
  const spent = "costUsd" in e && e.costUsd != null ? t("usage.insights.error.spent", { cost: usd(e.costUsd) }) : "";
  if (e.kind === "budget") return t("usage.insights.error.budget", { spent });
  if (e.kind === "turns") return t("usage.insights.error.turns", { spent });
  if (e.kind === "cancelled") return t("usage.insights.error.cancelled");
  if (e.kind === "timeout") return t("usage.insights.error.timeout");
  return r.message;
}

/** Top-level items of Claude's advice, each with "작업으로 만들기". Hidden when the answer has no `##` sections. */
function AdviceTasks({ text, onDraft }: { text: string; onDraft: (d: Draft) => void }) {
  const t = useT();
  const items = useMemo(() => adviceItems(text), [text]);
  if (items.length === 0) return null;
  return (
    <details className="mt-4 rounded-lg border border-line">
      <summary className="cursor-pointer px-3 py-2 text-[13px] font-medium select-none">{t("usage.insights.advice.tasks", { count: items.length })}</summary>
      <ul className="space-y-1.5 border-t border-line px-3 py-2.5">
        {items.map((it, k) => (
          <li key={k} className="flex items-start gap-2 text-[13px]">
            <span className="mt-0.5 shrink-0 rounded bg-paper px-1.5 py-0.5 text-[11px] text-ink-2">{it.section}</span>
            <span className="min-w-0 flex-1 text-ink-2">
              {it.text}
              {it.risky && (
                <span className="ml-1.5 inline-flex items-center gap-0.5 rounded bg-danger/10 px-1 text-[11px] font-medium text-danger">
                  <ShieldAlert size={10} /> {t("usage.insights.advice.risky")}
                </span>
              )}
              {it.filtered && <span className="ml-1.5 text-[11px] text-ink-3">{t("usage.insights.advice.filtered")}</span>}
            </span>
            <Button variant="quiet" size="sm" className="shrink-0" onClick={() => onDraft(draftFromAdvice(it))}>
              <ListPlus size={12} /> {t("usage.insights.toTask")}
            </Button>
          </li>
        ))}
      </ul>
      <p className="border-t border-line px-3 py-2 text-[11px] text-ink-3">
        {t("usage.insights.advice.tasksNote")}
      </p>
    </details>
  );
}

function AdvicePanel({ advice, onDelete, deleting, deleteError, onDraft }: { advice: Advice; onDelete: () => void; deleting: boolean; deleteError: string | null; onDraft?: (d: Draft) => void }) {
  const t = useT();
  return (
    <section aria-label={t("usage.insights.advice.label")} className="rounded-xl bg-sheet shadow-e1">
      <header className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-line px-4 py-2.5">
        <h3 className="text-[13px] font-semibold">{t("usage.insights.advice.label")}</h3>
        <span className="rounded-md border border-line px-1.5 py-0.5 text-[11px] text-ink-2">{t("usage.insights.advice.reference")}</span>
        {advice.stale && <span className="rounded-md bg-warn-soft px-1.5 py-0.5 text-[11px] font-semibold text-ink">{t("usage.insights.advice.stale")}</span>}
        <span className="text-xs text-ink-3">
          {t("usage.insights.advice.ran", { when: dateTime(advice.atMs) })}
          {advice.costUsd != null ? t("usage.insights.advice.cost", { cost: usd(advice.costUsd) }) : ""}
        </span>
        <Button className="ml-auto" variant="quiet" onClick={onDelete} disabled={deleting}>
          <Trash2 size={13} /> {t("usage.insights.advice.delete")}
        </Button>
      </header>
      {deleteError && (
        <p role="alert" className="border-b border-line px-4 py-2 text-[13px] text-danger">
          {t("usage.insights.advice.deleteFailed", { error: deleteError })}
        </p>
      )}
      <div className="px-4 py-3">
        <SafeMarkdown text={advice.text} />
        {onDraft && <AdviceTasks text={advice.text} onDraft={onDraft} />}
        <p className="mt-4 border-t border-line pt-2 text-xs text-ink-3">
          {t("usage.insights.advice.footer")}
        </p>
      </div>
    </section>
  );
}

export function UsageInsights({ q, onNavigate, onDraft }: { q: Query; onNavigate?: (p: Page) => void; onDraft?: (d: Draft) => void }) {
  const t = useT();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [showPreview, setShowPreview] = useState(true);
  const [runError, setRunError] = useState<{ run: Extract<InsightRun, { kind: "failed" }> } | { error: unknown } | null>(null);
  const [gate, setGate] = useState<"login" | "claudeAi" | null>(null);
  const local = useQuery({ queryKey: ["insights", "local", q], queryFn: () => api.insightsLocal(q), enabled: open });
  const preview = useQuery({ queryKey: ["insights", "preview", q], queryFn: () => api.insightsPreview(q), enabled: open && showPreview && !!local.data?.enough });
  const last = useQuery({ queryKey: ["insights", "last"], queryFn: api.insightsLast });
  const run = useMutation({
    mutationFn: () => api.insightsClaude(q),
    onMutate: () => {
      setRunError(null);
      setGate(null);
    },
    onSuccess: (r) => {
      if (r.kind === "done") qc.setQueryData(["insights", "last"], r.advice);
      else if (r.kind === "failed" && isGateError(r.error)) setGate(isGateError(r.error));
      else if (r.kind === "failed") setRunError({ run: r });
    },
    onError: (e) => (isGateError(e) ? setGate(isGateError(e)) : setRunError({ error: e })),
  });
  const del = useMutation({ mutationFn: api.insightsDelete, onSuccess: () => qc.setQueryData(["insights", "last"], null) });
  const r = local.data;
  const disabledReason = !r ? t("usage.insights.claude.needLocal") : !r.enough ? t("usage.insights.claude.needRecords") : null;

  return (
    <section aria-labelledby="insights-title" className="border-t border-line pt-8">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <h2 id="insights-title" className="text-[15px] font-semibold">
          {t("usage.insights.title")}
        </h2>
        <p className="text-xs text-ink-3">{t("usage.insights.subtitle")}</p>
        <Button variant="primary" className="ml-auto" onClick={() => (open ? void local.refetch() : setOpen(true))} disabled={local.isFetching}>
          <Lightbulb size={14} /> {open ? t("usage.insights.recheck") : t("usage.insights.title")}
        </Button>
      </div>

      {open && (
        <div className="space-y-6">
          <div>
            <SectionTitle aside={r ? `${r.priceDate ? t("usage.insights.local.priceBuiltIn", { date: r.priceDate }) : t("usage.insights.local.priceUser")}${t("usage.insights.local.free")}` : undefined}>
              {t("usage.insights.local.title")}
            </SectionTitle>
            {local.isError && <p className="text-[13px] text-danger">{t("usage.insights.local.failed", { error: messageOf(local.error) })}</p>}
            {r && !r.enough && (
              <div className="rounded-xl border border-dashed border-line px-4 py-6 text-center">
                <p className="text-[13px] font-medium text-ink-2">{t("usage.insights.local.notEnough")}</p>
                <p className="mt-1 text-xs text-ink-3">{t("usage.insights.local.notEnoughBody", { count: r.responses })}</p>
              </div>
            )}
            {r?.enough && r.findings.length === 0 && <p className="text-[13px] text-ink-2">{t("usage.insights.local.none")}</p>}
            {r?.enough && r.findings.length > 0 && (
              <div className="grid gap-3 xl:grid-cols-2">
                {r.findings.map((f) => (
                  <FindingCard key={f.id} f={f} onNavigate={onNavigate} onDraft={onDraft} />
                ))}
              </div>
            )}
            {r?.enough && r.notChecked.length > 0 && <p className="mt-3 text-xs text-ink-3">{t("usage.insights.local.skipped", { list: r.notChecked.join(", ") })}</p>}
            <p className="mt-2 text-xs text-ink-3">{t("usage.insights.local.latestNote")}</p>
          </div>

          <div>
            <SectionTitle aside={`${t("usage.insights.claude.aside")}${r?.cliVersion ? t("usage.insights.claude.version", { version: r.cliVersion }) : ""}`}>{t("usage.insights.claude.title")}</SectionTitle>
            <div className="rounded-xl bg-sheet shadow-e1 p-4">
              <ul className="space-y-1 text-[13px] text-ink-2">
                <li>
                  <strong className="font-semibold text-ink">{t("usage.insights.claude.sendsLabel")}</strong>
                  {t("usage.insights.claude.sends")}
                </li>
                <li>
                  <strong className="font-semibold text-ink">{t("usage.insights.claude.keepsLabel")}</strong>
                  {t("usage.insights.claude.keeps")}
                </li>
              </ul>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <Button variant="primary" onClick={() => run.mutate()} disabled={run.isPending || !!disabledReason} title={disabledReason ?? undefined}>
                  {run.isPending ? <Loader2 size={14} className="animate-spin" /> : <Sparkles size={14} />}
                  {run.isPending ? t("usage.insights.claude.running") : t("usage.insights.claude.title")}
                </Button>
                {run.isPending && (
                  <Button variant="danger" onClick={() => void api.insightsCancel()}>
                    {t("usage.insights.claude.cancel")}
                  </Button>
                )}
                <span className="text-xs text-ink-3">{t("usage.insights.claude.cost")}</span>
                <Button variant="quiet" className="ml-auto" onClick={() => setShowPreview((v) => !v)} aria-expanded={showPreview}>
                  {showPreview ? t("usage.insights.claude.previewHide") : t("usage.insights.claude.previewShow")}
                </Button>
              </div>
              <p className="mt-2 text-xs text-ink-3">
                {disabledReason ? t("usage.insights.claude.reasonEnd", { reason: disabledReason }) : t("usage.insights.claude.confirmHint")}
                {t("usage.insights.claude.apiNote")}
              </p>
              {showPreview && r?.enough && (
                <div className="mt-3">
                  <p className="mb-1 text-xs text-ink-3">{t("usage.insights.claude.previewTitle")}</p>
                  <pre aria-label={t("usage.insights.claude.previewLabel")} className="max-h-64 overflow-auto rounded-lg border border-line bg-paper p-3 text-[12px] leading-snug">
                    {preview.data ?? t("usage.insights.claude.loading")}
                  </pre>
                </div>
              )}
              <GateLine className="mt-3" force={gate} />
              {runError && (
                <p role="alert" className="mt-3 rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-[13px] text-danger">
                  {"run" in runError ? runMessage(runError.run) : messageOf(runError.error)}
                </p>
              )}
            </div>
          </div>
        </div>
      )}

      <div className={open ? "mt-6" : ""}>
        {last.data ? (
          <AdvicePanel advice={last.data} onDraft={onDraft} onDelete={() => del.mutate()} deleting={del.isPending} deleteError={del.isError ? messageOf(del.error) : null} />
        ) : (
          open && <p className="text-xs text-ink-3">{t("usage.insights.advice.none")}</p>
        )}
      </div>
    </section>
  );
}
