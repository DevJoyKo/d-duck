import { useQuery } from "@tanstack/react-query";
import { Check, ChevronLeft, ChevronRight, Copy, ExternalLink, FileCode, FileText, LayoutTemplate, Search } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { api, type ArtifactPreview, type ArtifactRow, failure, later, messageOf, shown, type Shown } from "../lib/api";
import { FRAME_H, FRAME_W, LIST_LIMIT, MIN_CARD_W, clampPage, fitPageSize, limiter, pageCount, pageForAnchor, pageNumbers, pageSlice, previewReason, previewSrcdoc, reasonText } from "../lib/artifactCards";
import { dateTime } from "../lib/format";
import { Button, Empty, PageHeader, Sheet, fieldClass } from "./ui";
import { SafeMarkdown } from "./UsageInsights";
import { useT } from "../i18n/react";

/** At most three preview reads in flight (security review MEDIUM). */
const fetchLimit = limiter(3);

/** True once the element has come near the viewport (and stays true). */
function useSeen<T extends Element>() {
  const ref = useRef<T>(null);
  // Without IntersectionObserver (tests) everything counts as seen.
  const [seen, setSeen] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    const el = ref.current;
    if (!el || seen) return;
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && setSeen(true), { rootMargin: "200px" });
    io.observe(el);
    return () => io.disconnect();
  }, [seen]);
  return [ref, seen] as const;
}

/** The page laid out at FRAME_W×FRAME_H (960×600) and scaled to the card width; inert, hidden from assistive tech, not focusable. */
function HtmlFrame({ html, title }: { html: string; title: string }) {
  const t = useT();
  const box = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(0.2);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const fit = () => el.clientWidth > 0 && setScale(el.clientWidth / FRAME_W);
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <div ref={box} className="absolute inset-0 overflow-hidden bg-white">
      <iframe
        title={t("artifacts.preview.frame", { title })}
        sandbox=""
        srcDoc={previewSrcdoc(html)}
        loading="lazy"
        referrerPolicy="no-referrer"
        aria-hidden="true"
        tabIndex={-1}
        className="pointer-events-none origin-top-left border-0"
        style={{ width: FRAME_W, height: FRAME_H, transform: `scale(${scale})` }}
      />
      {/* A faint edge so the white page does not glare out of a dark card; the line token adapts to the theme. */}
      <div className="pointer-events-none absolute inset-0 ring-1 ring-line ring-inset" aria-hidden="true" />
    </div>
  );
}

function Unavailable({ kind, text }: { kind: string; text: string }) {
  const Icon = kind === "typed" ? LayoutTemplate : kind === "script" ? FileCode : FileText;
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-paper px-4 text-center">
      <Icon size={22} className="text-ink-3" aria-hidden="true" />
      <p className="text-xs text-ink-3">{text}</p>
    </div>
  );
}

function PreviewBody({ data, title }: { data: ArtifactPreview; title: string }) {
  const t = useT();
  if (data.kind === "markdown")
    return (
      <div className="pointer-events-none absolute inset-0 overflow-hidden bg-sheet p-3 select-none [&_*]:text-[11px]" aria-hidden="true" inert>
        <SafeMarkdown text={data.text} />
        <div className="absolute inset-x-0 bottom-0 h-8 bg-gradient-to-t from-sheet" />
      </div>
    );
  // Script-drawn pages render blank without scripts: show the same "unavailable" state instead of a white box.
  if (data.scriptOnly) return <Unavailable kind="script" text={t("artifacts.preview.scriptOnly")} />;
  return (
    <>
      <HtmlFrame html={data.text} title={title} />
      {data.truncated && <p className="absolute inset-x-0 bottom-0 bg-paper/90 px-2 py-1 text-[11px] text-ink-3">{t("artifacts.preview.truncated")}</p>}
    </>
  );
}

function CardPreview({ a }: { a: ArtifactRow }) {
  const t = useT();
  const [ref, seen] = useSeen<HTMLDivElement>();
  const typed = a.kind === "typed";
  const q = useQuery({
    queryKey: ["artifactPreview", a.artifactId, a.lastTs],
    queryFn: () => fetchLimit(() => api.artifactPreview(a.artifactId)),
    enabled: seen && !typed,
    staleTime: 5 * 60_000,
    retry: false,
  });
  const title = a.title?.trim() || t("artifacts.card.fallbackTitle");
  let body;
  if (typed) body = <Unavailable kind="typed" text={t("artifacts.preview.none", { reason: reasonText("typed") })} />;
  else if (q.data) body = <PreviewBody data={q.data} title={title} />;
  else if (q.isError) {
    const r = previewReason(q.error);
    body = <Unavailable kind={a.kind} text={t("artifacts.preview.none", { reason: reasonText(r) })} />;
  } else body = <div role="status" className="absolute inset-0 animate-pulse bg-line/50" aria-label={t("artifacts.preview.loading")} />;
  return (
    <div ref={ref} data-artifact-preview className="relative aspect-[16/10] overflow-hidden rounded-t-xl border-b border-line" title={t("artifacts.preview.basis")}>
      {body}
    </div>
  );
}

function Card({ a }: { a: ArtifactRow }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<Shown | null>(null);
  const name = a.title?.trim() || a.artifactId.slice(0, 8);
  return (
    <li data-artifact-card className="flex min-w-0 flex-col rounded-xl bg-sheet shadow-e1">
      <CardPreview a={a} />
      <div className="flex min-w-0 flex-1 flex-col gap-2 p-3">
        <div className="min-w-0">
          <h3 className="truncate text-[13px] font-medium text-ink" title={name}>
            {name}
          </h3>
          <p className="mt-0.5 truncate text-xs text-ink-3">
            {a.kind === "typed" ? t("artifacts.card.typed") : t("artifacts.card.page")} · {t("artifacts.card.version", { seq: String(a.seq) })}
          </p>
          <p className="truncate text-xs text-ink-3 tabular-nums">
            {dateTime(a.lastTs)} · {a.projectName ?? t("artifacts.card.noProject")}
          </p>
          {error && (
            <p role="alert" className="mt-1 text-xs text-danger">
              {shown(error)}
            </p>
          )}
        </div>
        <div className="mt-auto flex gap-1.5">
          <Button
            aria-label={t("artifacts.card.copyLink", { name: a.title ?? t("artifacts.card.fallbackTitle") })}
            size="icon"
            className="flex-1 justify-center"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(a.url);
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              } catch {
                setError(later(() => t("artifacts.card.copyFailed")));
              }
            }}
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
            {copied ? t("artifacts.card.copied") : t("artifacts.card.link")}
          </Button>
          <Button
            aria-label={t("artifacts.card.openLabel", { name })}
            size="icon"
            className="flex-1 justify-center"
            onClick={async () => {
              setError(null);
              try {
                await api.openArtifact(a.artifactId);
              } catch (e) {
                setError(failure(e));
              }
            }}
          >
            <ExternalLink size={14} />
            {t("artifacts.card.open")}
          </Button>
        </div>
      </div>
    </li>
  );
}

export function Pager({ page, count, onPage }: { page: number; count: number; onPage: (p: number) => void }) {
  const t = useT();
  if (count <= 1) return null;
  const btn = "inline-flex h-8 min-w-8 items-center justify-center rounded-lg px-2 text-[13px] tabular-nums transition-colors";
  return (
    <nav aria-label={t("artifacts.pager.label")} className="flex flex-wrap items-center justify-center gap-1">
      <Button aria-label={t("artifacts.pager.prevLabel")} disabled={page <= 1} onClick={() => onPage(page - 1)} size="icon">
        <ChevronLeft size={14} />
        {t("artifacts.pager.prev")}
      </Button>
      {pageNumbers(page, count).map((n, i) =>
        n === "gap" ? (
          <span key={`g${i}`} className="px-1 text-xs text-ink-3" aria-hidden="true">
            …
          </span>
        ) : (
          <button
            key={n}
            aria-label={t("artifacts.pager.page", { page: String(n) })}
            aria-current={n === page ? "page" : undefined}
            onClick={() => onPage(n)}
            className={`${btn} ${n === page ? "bg-ink font-medium text-paper" : "text-ink-2 hover:bg-line/60 hover:text-ink"}`}
          >
            {n}
          </button>
        ),
      )}
      <Button aria-label={t("artifacts.pager.nextLabel")} disabled={page >= count} onClick={() => onPage(page + 1)} size="icon">
        {t("artifacts.pager.next")}
        <ChevronRight size={14} />
      </Button>
      <span className="ml-2 text-xs text-ink-3 tabular-nums">
        {page} / {count}
      </span>
    </nav>
  );
}

/** Grid before the list area is measured (first paint, tests): the old breakpoints. */
const GRID = "grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4 2xl:grid-cols-5";
/** gap-4 */
const GAP = 16;
/** Text and buttons under the preview until a card has been measured: p-3, title, two meta lines, gap-2, h-8 buttons. */
const BODY_GUESS = 124;
/** Below the grid: space-y-4 + h-8 pager + py-6 bottom padding, so the pager stays on screen. */
const BELOW = 16 + 32 + 24;

function scrollParent(el: HTMLElement): HTMLElement {
  for (let p = el.parentElement; p; p = p.parentElement) if (/auto|scroll/.test(getComputedStyle(p).overflowY)) return p;
  return document.documentElement;
}

/** Cards per page from the measured list area (D-184): re-measured whenever the area or its scroller resizes. */
function usePageFit(area: RefObject<HTMLDivElement | null>) {
  const [fit, setFit] = useState(() => fitPageSize({ width: 0, height: 0, minCard: MIN_CARD_W, gap: GAP, body: BODY_GUESS }));
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    const scroller = scrollParent(el);
    const measure = () => {
      const top = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
      const card = el.querySelector<HTMLElement>("[data-artifact-card]");
      const preview = card?.querySelector<HTMLElement>("[data-artifact-preview]");
      const body = card && preview ? card.offsetHeight - preview.offsetHeight : BODY_GUESS;
      const next = fitPageSize({ width: el.clientWidth, height: scroller.clientHeight - top - BELOW, minCard: MIN_CARD_W, gap: GAP, body });
      setFit((prev) => (prev.cols === next.cols && prev.rows === next.rows && prev.size === next.size ? prev : next));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    if (scroller !== document.documentElement) ro.observe(scroller);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [area]);
  return fit;
}

export function ArtifactsPage() {
  const t = useT();
  const [query, setQuery] = useState("");
  // The page is kept as the index of its first card, so a new page size lands on the page that still shows it.
  const [first, setFirst] = useState(0);
  const [announce, setAnnounce] = useState("");
  const top = useRef<HTMLDivElement>(null);
  const area = useRef<HTMLDivElement>(null);
  const fit = usePageFit(area);
  const size = fit.size;
  const list = useQuery({ queryKey: ["artifacts", query], queryFn: () => api.artifacts(query.trim()) });
  const rows = list.data ?? [];
  const count = pageCount(rows.length, size);
  // A page past the end (the list shrank) shows the last page; `current` is always in range.
  const current = clampPage(pageForAnchor(first, size), rows.length, size);
  const shown = pageSlice(rows, current, size);
  const grid = fit.cols > 0 ? { className: "grid gap-4", style: { gridTemplateColumns: `repeat(${fit.cols}, minmax(0, 1fr))` } } : { className: GRID };
  const goTo = (p: number) => {
    const next = clampPage(p, rows.length, size);
    setFirst((next - 1) * size);
    setAnnounce(t("artifacts.pager.announce", { page: String(next), count: String(count) }));
    top.current?.scrollIntoView?.({ block: "start" });
  };
  return (
    <div>
      <PageHeader title={t("artifacts.title")}>
        <label className="relative w-64">
          <span className="sr-only">{t("artifacts.search")}</span>
          <Search size={14} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-ink-3" />
          <input
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setFirst(0);
              setAnnounce("");
            }}
            placeholder={t("artifacts.search")}
            className={`${fieldClass} h-8 py-0 pl-8`}
          />
        </label>
      </PageHeader>
      <p aria-live="polite" className="sr-only">
        {announce}
      </p>
      <div className="mx-auto max-w-7xl space-y-4 px-8 py-6">
        <p className="text-xs text-ink-3">
          {t("artifacts.intro")}
          {rows.length >= LIST_LIMIT && <span className="ml-1 font-medium text-ink-2">{t("artifacts.limit", { count: LIST_LIMIT })}</span>}
        </p>
        {/* The measured list area (D-184): always mounted, so its width is known before the rows arrive. */}
        <div ref={area} className="space-y-4">
        {list.isError ? (
          <Sheet>
            <Empty title={t("artifacts.loadFailed")}>{messageOf(list.error)}</Empty>
          </Sheet>
        ) : list.isPending ? (
          <div role="status" aria-label={t("artifacts.loading")}>
            <ul {...grid} aria-hidden="true">
              {Array.from({ length: size }, (_, i) => (
                <li key={i} className="aspect-[16/14] animate-pulse rounded-xl border border-line bg-line/40" />
              ))}
            </ul>
          </div>
        ) : rows.length === 0 ? (
          <Sheet>
            <Empty title={query ? t("artifacts.noResults") : t("artifacts.empty")}>
              {query ? t("artifacts.noResultsBody") : t("artifacts.emptyBody")}
            </Empty>
          </Sheet>
        ) : (
          <>
            <div ref={top} className="scroll-mt-4">
              <ul {...grid}>
                {shown.map((a) => (
                  <Card key={a.artifactId} a={a} />
                ))}
              </ul>
            </div>
            <Pager page={current} count={count} onPage={goTo} />
          </>
        )}
        </div>
      </div>
    </div>
  );
}
