// Transcript records as JSON (D-074), read straight from the session's JSONL when opened, never stored or cached
// (gcTime 0), and dropped when the panel closes. "가리지 않고 보기" is component state only, so it resets with the panel.
import { useQuery } from "@tanstack/react-query";
import { Check, ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight, Copy } from "lucide-react";
import { useState } from "react";
import { api, type RawPage, type RawRecord, type RawTarget } from "../lib/api";
import { Button, Empty } from "./ui";
import { currentLocale } from "../i18n/language";
import { useT } from "../i18n/react";

function targetKey(t: RawTarget): string {
  if ("spans" in t) return t.spans.map((s) => s.join(":")).join(",");
  if ("from" in t) return `from:${t.from}`;
  if ("before" in t) return `before:${t.before}`;
  return "tail";
}

function useRaw(sessionId: string, target: RawTarget, reveal: boolean) {
  return useQuery({
    queryKey: ["raw", sessionId, target.agentId ?? "", targetKey(target), reveal],
    queryFn: () => api.sessionRaw(sessionId, target, reveal),
    // Never kept after the panel stops showing it; a reveal toggle replaces the copy instead of adding one.
    gcTime: 0,
    staleTime: Infinity,
    retry: false,
  });
}

function RawToolbar({ page, reveal, setReveal }: { page: RawPage | undefined; reveal: boolean; setReveal: (v: boolean) => void }) {
  const t = useT();
  const [copied, setCopied] = useState<"ok" | "error" | null>(null);
  const copy = async () => {
    if (!page) return;
    try {
      // Exactly what is on screen: masked values stay masked.
      await navigator.clipboard.writeText(page.records.map((r) => r.text).join("\n"));
      setCopied("ok");
    } catch {
      setCopied("error");
    }
    setTimeout(() => setCopied(null), 2000);
  };
  return (
    <div className="mb-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-ink-3">
      <label className="flex cursor-pointer items-center gap-1.5">
        <input type="checkbox" checked={reveal} onChange={(e) => setReveal(e.target.checked)} className="accent-current" />
        {t("tasks.raw.reveal")}
      </label>
      {copied === "error" && <span className="text-danger">{t("tasks.raw.copyFailed")}</span>}
      <Button onClick={copy} disabled={!page || page.records.length === 0} size="xs" className="ml-auto">
        {copied === "ok" ? <Check size={12} /> : <Copy size={12} />}
        {copied === "ok" ? t("tasks.raw.copied") : t("tasks.raw.copy")}
      </Button>
      {reveal && <p className="w-full text-danger">{t("tasks.raw.revealWarn")}</p>}
    </div>
  );
}

function when(ts: string | null): string {
  if (!ts) return "";
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString(currentLocale(), { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function Record({ r, folded }: { r: RawRecord; folded: boolean }) {
  const t = useT();
  return (
    <details open={!folded} className="rounded-md border border-line bg-paper">
      <summary className="cursor-pointer px-2 py-1 text-[11px] text-ink-3 select-none">
        <span className="font-medium text-ink-2">{r.kind ?? t("tasks.raw.record")}</span>
        {r.ts && <span className="ml-2 tabular-nums">{when(r.ts)}</span>}
        {r.truncated && <span className="ml-2 text-danger">{t("tasks.raw.truncated")}</span>}
        {!r.json && !r.truncated && <span className="ml-2">{t("tasks.raw.notJson")}</span>}
      </summary>
      {/* Plain text only: no markdown, no links, control/bidi characters arrive as visible \uXXXX. */}
      <pre className="border-t border-line px-2 py-1.5 font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap text-ink select-text">{r.text}</pre>
    </details>
  );
}

function Records({ page, foldMinor }: { page: RawPage; foldMinor: boolean }) {
  return (
    <ol className="space-y-1.5">
      {page.records.map((r) => (
        <li key={r.offset}>
          <Record r={r} folded={foldMinor && r.minor} />
        </li>
      ))}
    </ol>
  );
}

function errorText(e: unknown): string {
  return String(e).replace(/^Error:\s*/, "");
}

/** The records behind one flow item, all unfolded. */
export function RawPanel({ sessionId, target }: { sessionId: string; target: RawTarget }) {
  const t = useT();
  const [reveal, setReveal] = useState(false);
  const q = useRaw(sessionId, target, reveal);
  return (
    <div className="mt-1.5">
      <RawToolbar page={q.data} reveal={reveal} setReveal={setReveal} />
      {q.isError ? (
        <p className="text-[11px] text-danger">{errorText(q.error)}</p>
      ) : q.isPending ? (
        <p className="text-[11px] text-ink-3">{t("tasks.raw.reading")}</p>
      ) : q.data.records.length === 0 ? (
        <p className="text-[11px] text-ink-3">{t("tasks.raw.noRecords")}</p>
      ) : (
        <Records page={q.data} foldMinor={false} />
      )}
    </div>
  );
}

/** A whole JSONL, 100 records per page, opening on the last page. Tool results and other bookkeeping start folded. */
export function RawPager({ sessionId, agentId }: { sessionId: string; agentId?: string | null }) {
  const t = useT();
  const [reveal, setReveal] = useState(false);
  const [target, setTarget] = useState<RawTarget>({ agentId, tail: true });
  const q = useRaw(sessionId, target, reveal);
  const page = q.data;
  const atStart = page?.start === 0;
  const atEnd = page ? page.next === null : false;
  return (
    <div>
      <div className="mb-2 flex flex-wrap items-center gap-2 text-[11px] text-ink-3">
        <span>{page ? [t("tasks.raw.count", { count: page.records.length }), atStart && t("tasks.raw.fileStart"), atEnd && t("tasks.raw.fileEnd")].filter(Boolean).join(" · ") : t("tasks.raw.reading")}</span>
        <div className="ml-auto flex items-center gap-1">
          <Button aria-label={t("tasks.raw.first")} disabled={!page || atStart} onClick={() => setTarget({ agentId, from: 0 })} size="compact">
            <ChevronsLeft size={14} />
          </Button>
          <Button aria-label={t("tasks.raw.prev")} disabled={!page || atStart} onClick={() => page && setTarget({ agentId, before: page.start })} size="compact">
            <ChevronLeft size={14} />
          </Button>
          <Button aria-label={t("tasks.raw.next")} disabled={!page || atEnd} onClick={() => page?.next != null && setTarget({ agentId, from: page.next })} size="compact">
            <ChevronRight size={14} />
          </Button>
          <Button aria-label={t("tasks.raw.last")} disabled={!page || atEnd} onClick={() => setTarget({ agentId, tail: true })} size="compact">
            <ChevronsRight size={14} />
          </Button>
        </div>
      </div>
      <RawToolbar page={page} reveal={reveal} setReveal={setReveal} />
      {q.isError ? (
        <Empty title={t("tasks.raw.readFailed")}>{errorText(q.error)}</Empty>
      ) : q.isPending ? (
        <Empty title={t("tasks.raw.reading")} />
      ) : q.data.records.length === 0 ? (
        <Empty title={t("tasks.raw.emptyHere")} />
      ) : (
        <>
          <Records page={q.data} foldMinor />
          {q.data.stopped && <p className="mt-2 text-[11px] text-ink-3">{t("tasks.raw.stopped")}</p>}
        </>
      )}
    </div>
  );
}
