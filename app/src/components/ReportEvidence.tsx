// 근거·첨부 (report-evidence, D-152·D-156). Evidence chips under a criterion open one preview in place, loaded only
// when pressed (the last three kept). Attachments are drawn from what Rust cleaned: HTML as a full srcdoc inside a
// `sandbox=""` iframe (no script, network, forms or navigation), Markdown through SafeMarkdown. The app's own header
// ("에이전트가 만든 문서 · 제목", 크게 보기, 닫기) sits outside the document, so a page that imitates app buttons is
// still told apart, and closing works while focus is inside the document (Escape does not reach the app there).
import { Check, Copy, Download, FileCode2, FileText, FlaskConical, Image as ImageIcon, Lock, Maximize2, Terminal, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api, type AttachmentOpened, type AttachmentView, type EvidencePreview, type EvidenceView, later, shown, type Shown } from "../lib/api";
import { EVIDENCE_STATUS_TEXT, KIND_LABEL, LABEL, type CriterionRow } from "../lib/report";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import { stripUnsafe } from "../lib/safeMarkdown";
import { SafeMarkdown } from "./UsageInsights";

// ---------- save and copy (report-copy-save, D-174); words in the report catalogue ----------

/** Fixed codes from `save_report_attachment`; anything else reads as `failed`. */
const SAVE_ERROR_CODES = ["busy", "refused", "missing", "failed"] as const;
type SaveError = (typeof SAVE_ERROR_CODES)[number];
export const saveErrorText = (code: string) => t(`report.saveError.${(SAVE_ERROR_CODES as readonly string[]).includes(code) ? (code as SaveError) : "failed"}`);
const COPIED_MS = 1_500;
/** "저장했어요" goes away after this; errors stay until the next press. */
const SAVED_MS = 3_000;

// ---------- Escape for overlays inside the report dialog ----------

/** The report dialog's Escape asks this first: an open overlay (image, full view) closes before the dialog. */
export const overlayEscape: { current: (() => void) | null } = { current: null };

function useOverlayEscape(onClose: () => void) {
  useEffect(() => {
    const prev = overlayEscape.current;
    overlayEscape.current = onClose;
    return () => {
      overlayEscape.current = prev;
    };
  }, [onClose]);
}

/** Inside the report dialog (not `inert`, inside its focus trap); the page body when drawn alone. */
function overlayHost(): HTMLElement {
  return document.querySelector<HTMLElement>("section[data-report-dialog]") ?? document.body;
}

// ---------- loading on demand, the last three kept ----------

const CACHE = 3;
const previews = new Map<string, EvidencePreview>();

function remember(key: string, v: EvidencePreview) {
  previews.delete(key);
  previews.set(key, v);
  while (previews.size > CACHE) previews.delete(previews.keys().next().value as string);
}

function usePreview(bgId: string | undefined, criterion: number, item: number) {
  const key = `${bgId}:${criterion}:${item}`;
  const [state, setState] = useState<{ key: string; data?: EvidencePreview; error?: boolean }>(() => ({ key, data: previews.get(key) }));
  useEffect(() => {
    if (!bgId || previews.has(key)) return;
    let alive = true;
    api
      .reportEvidence(bgId, criterion, item)
      .then((d) => {
        remember(key, d);
        if (alive) setState({ key, data: d });
      })
      .catch(() => alive && setState({ key, error: true }));
    return () => {
      alive = false;
    };
  }, [bgId, criterion, item, key]);
  return state.key === key ? state : { key, data: previews.get(key) };
}

// ---------- chips ----------

const ICON = { screenshot: ImageIcon, "test-report": FlaskConical, command: Terminal, file: FileCode2 } as const;

export function chipText(e: EvidenceView): string {
  return `${KIND_LABEL[e.kind]} ${e.label}`;
}

/** Chips under one criterion and the preview of the pressed one. */
export function EvidenceChips({ bgId, row }: { bgId?: string; row: CriterionRow }) {
  useT();
  const [open, setOpen] = useState<number | null>(null);
  const chipRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const items = row.items ?? [];
  if (!items.length && !row.noEvidence && !row.testsFailed) return null;
  return (
    <div className="mt-1.5">
      <ul className="flex flex-wrap gap-1.5" aria-label={t("report.evidence.list")}>
        {items.map((e, i) => {
          const Icon = ICON[e.kind];
          // A valid file of a kind this role does not accept is drawn like an invalid one (design MAJOR).
          const accepted = !row.accepts || row.accepts.includes(e.kind);
          const ok = e.status === "valid" && accepted;
          const why = e.status !== "valid" ? EVIDENCE_STATUS_TEXT[e.status] : !accepted ? notAcceptedText(row.accepts) : "";
          return (
            <li key={i} className="min-w-0 max-w-full">
              <button
                ref={(el) => {
                  chipRefs.current[i] = el;
                }}
                onClick={() => setOpen(open === i ? null : i)}
                aria-expanded={open === i}
                title={e.caption || undefined}
                className={`flex max-w-full items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px] ${ok ? "border-line bg-sheet text-ink-2 hover:border-ink-3" : "border-dashed border-line text-ink-3 opacity-70"} ${open === i ? "border-ink-3" : ""}`}
                data-evidence-chip={e.kind}
              >
                <Icon size={12} className="shrink-0" aria-hidden />
                <span className="min-w-0 truncate">{chipText(e)}</span>
                {e.kind === "command" && e.exitCode != null && <span className="shrink-0 text-ink-3">{t("report.evidence.exit", { code: String(e.exitCode) })}</span>}
              </button>
              {!ok && (
                <span className="mt-0.5 block text-[11px] text-ink-3" data-chip-why>
                  {why}
                </span>
              )}
            </li>
          );
        })}
        {row.noEvidence && (
          <li>
            <span className={`inline-flex items-center rounded-md border px-1.5 py-0.5 text-[11px] font-semibold ${row.strict ? "border-danger/40 bg-danger/5 text-danger" : "border-warn/40 bg-warn/5 text-warn"}`} data-no-evidence>
              {LABEL.noEvidence}
            </span>
          </li>
        )}
        {!!row.testsFailed && (
          <li>
            <span className="inline-flex items-center rounded-md border border-warn/40 bg-warn/5 px-1.5 py-0.5 text-[11px] font-semibold text-warn">
              {LABEL.testsFailed} {row.testsFailed}
            </span>
          </li>
        )}
      </ul>
      {open !== null && items[open] && row.index != null && (
        <PreviewPanel bgId={bgId} criterion={row.index} item={open} view={items[open]} back={() => chipRefs.current[open]?.focus()} />
      )}
    </div>
  );
}

/** Why a valid item does not count for this role. */
function notAcceptedText(accepts: CriterionRow["accepts"]): string {
  if (accepts?.length === 1 && accepts[0] === "file") return t("report.evidence.reviewFileOnly");
  if (accepts && !accepts.includes("file")) return t("report.evidence.qaNoFile");
  return t("report.evidence.notForRole");
}

export function LegacyChip() {
  useT();
  return (
    <span className="inline-flex items-center rounded-full border border-line px-2 py-0.5 text-[11px] text-ink-2" data-legacy>
      {LABEL.legacy}
    </span>
  );
}

function PreviewPanel({ bgId, criterion, item, view, back }: { bgId?: string; criterion: number; item: number; view: EvidenceView; back: () => void }) {
  const { data, error } = usePreview(bgId, criterion, item);
  return (
    <div className="mt-2 rounded-lg border border-line bg-sheet p-3 text-xs" data-evidence-preview>
      {error ? <p className="text-ink-3">{t("report.evidence.readFailed")}</p> : !data ? <p className="text-ink-3">{t("report.evidence.loading")}</p> : <PreviewBody p={data} view={view} back={back} />}
      {view.caption && <p className="mt-1.5 text-ink-3">{view.caption}</p>}
    </div>
  );
}

function PreviewBody({ p, view, back }: { p: EvidencePreview; view: EvidenceView; back: () => void }) {
  const [big, setBig] = useState(false);
  const bigBtn = useRef<HTMLButtonElement>(null);
  switch (p.kind) {
    case "image":
      return (
        <>
          <button ref={bigBtn} onClick={() => setBig(true)} className="block max-w-full rounded border border-line" aria-label={t("report.evidence.enlarge", { label: view.label })}>
            <img src={p.dataUrl} alt={view.caption || view.label} className="max-h-[260px] max-w-full" />
          </button>
          <p className="mt-1 text-ink-3">
            {p.width}×{p.height} · {t("report.evidence.imageHint")}
          </p>
          {big && (
            <Lightbox
              src={p.dataUrl}
              alt={view.caption || view.label}
              label={view.label}
              size={`${p.width}×${p.height}`}
              onClose={() => {
                setBig(false);
                // Focus goes back to the chip the preview came from.
                back();
              }}
            />
          )}
        </>
      );
    case "tests": {
      const s = p.summary;
      return (
        <>
          <table className="text-[12px]">
            <tbody>
              <tr>
                <td className="pr-4 text-ink-3">{t("report.evidence.tool")}</td>
                <td>{s.tool || "—"}</td>
              </tr>
              <tr>
                <td className="pr-4 text-ink-3">{t("report.evidence.result")}</td>
                <td data-test-counts>
                  {t("report.evidence.passed", { n: s.passed })} · <span className={s.failed ? "font-semibold text-danger" : ""}>{t("report.evidence.failed", { n: s.failed })}</span> · {t("report.evidence.skipped", { n: s.skipped })}
                </td>
              </tr>
            </tbody>
          </table>
          {s.failures.length > 0 && (
            <ul className="mt-1.5 space-y-0.5" aria-label={t("report.evidence.failures")}>
              {s.failures.map((f, i) => (
                <li key={i} className="break-all text-danger">
                  ✕ {f}
                </li>
              ))}
            </ul>
          )}
        </>
      );
    }
    case "command":
      return (
        <>
          <p className="font-mono break-all">
            $ {p.cmd} <span className="text-ink-3">· {t("report.evidence.exitCode", { code: String(p.exitCode) })}</span>
          </p>
          <pre className="mt-1.5 max-h-[280px] overflow-auto rounded bg-paper p-2 text-[11px] whitespace-pre-wrap">{p.excerpt}</pre>
          {p.cut && <p className="mt-1 text-ink-3">{t("report.evidence.cut")}</p>}
        </>
      );
    case "file": {
      const text = p.lines ? `${p.path}:${p.lines[0]}–${p.lines[1]}` : p.path;
      return (
        <p className="flex items-center gap-2">
          <span className="font-mono break-all">{text}</span>
          <button onClick={() => void navigator.clipboard?.writeText(text).catch(() => undefined)} className="inline-flex shrink-0 items-center gap-1 text-ink-3 hover:text-ink">
            <Copy size={12} aria-hidden />
            {t("report.evidence.copyPath")}
          </button>
        </p>
      );
    }
    case "invalid":
      return <p className="text-ink-3">{EVIDENCE_STATUS_TEXT[p.status]}</p>;
  }
}

function Lightbox({ src, alt, label, size, onClose }: { src: string; alt: string; label: string; size: string; onClose: () => void }) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useOverlayEscape(onClose);
  useEffect(() => closeRef.current?.focus(), []);
  return createPortal(
    <div className="fixed inset-0 z-50 flex flex-col bg-scrim/85 p-4" role="dialog" aria-modal="true" aria-label={t("report.evidence.lightbox")} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="flex items-center gap-3 rounded-lg bg-sheet px-3 py-2">
        <ImageIcon size={14} className="shrink-0 text-ink-3" aria-hidden />
        <span className="min-w-0 flex-1 truncate text-[13px] text-ink" data-lightbox-label>
          {label} <span className="text-xs text-ink-3">· {size}</span>
        </span>
        <span className="hidden text-[11px] text-ink-3 sm:inline">{t("report.evidence.escClose")}</span>
        <button ref={closeRef} onClick={onClose} className="inline-flex h-8 items-center gap-1 rounded-md border border-line px-3 text-[13px] text-ink hover:bg-paper">
          <X size={14} aria-hidden />
          {t("report.evidence.close")}
        </button>
      </div>
      <div className="flex min-h-0 flex-1 items-center justify-center p-2">
        <img src={src} alt={alt} className="max-h-full max-w-full rounded shadow-e3" />
      </div>
    </div>,
    overlayHost(),
  );
}

// ---------- attachments ----------


function useAttachment(bgId: string | undefined, index: number | null) {
  const [state, setState] = useState<{ index: number | null; data?: AttachmentOpened; error?: boolean }>({ index: null });
  useEffect(() => {
    if (!bgId || index === null) return;
    let alive = true;
    api
      .reportAttachment(bgId, index)
      .then((d) => alive && setState({ index, data: d }))
      .catch(() => alive && setState({ index, error: true }));
    return () => {
      alive = false;
    };
  }, [bgId, index]);
  return state.index === index ? state : { index };
}

/** The document itself: the Rust srcdoc as is, or SafeMarkdown. */
function Doc({ data, title, tall }: { data: AttachmentOpened; title: string; tall?: boolean }) {
  if (data.kind === "html")
    return <iframe sandbox="" srcDoc={data.srcdoc} title={`${t("report.attachment.head")}: ${title}`} tabIndex={0} className={`w-full rounded-b-lg border-0 bg-white ${tall ? "h-full" : "h-[60vh] min-h-[60vh]"}`} data-attachment-frame />;
  if (data.kind === "markdown")
    return (
      <div className={`selectable overflow-auto rounded-b-lg bg-sheet p-4 ${tall ? "h-full" : "max-h-[70vh] min-h-[60vh]"}`} tabIndex={0} data-attachment-text>
        <SafeMarkdown text={data.text} images={data.images} />
      </div>
    );
  return <p className="rounded-b-lg bg-sheet px-4 py-6 text-[13px] text-ink-3">{data.kind === "empty" ? t("report.attachment.empty") : EVIDENCE_STATUS_TEXT[data.code]}</p>;
}

const headBtn = "inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-xs text-ink-2 hover:bg-paper hover:text-ink disabled:opacity-50";

interface DocActionState {
  saving: boolean;
  line: { text: Shown; error?: boolean } | null;
  copied: boolean;
  save: () => void;
  copy: (md: string) => void;
}

/** Save and copy state for one open attachment, shared by its inline header and the full view (one result line,
 *  one busy state). "저장했어요" clears after a few seconds. The owner is keyed by the attachment, so a new tab starts clean. */
function useDocActions(bgId: string | undefined, index: number): DocActionState {
  const [saving, setSaving] = useState(false);
  const [line, setLine] = useState<{ text: Shown; error?: boolean } | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const id = window.setTimeout(() => setCopied(false), COPIED_MS);
    return () => window.clearTimeout(id);
  }, [copied]);
  useEffect(() => {
    if (!line || line.error) return;
    const id = window.setTimeout(() => setLine(null), SAVED_MS);
    return () => window.clearTimeout(id);
  }, [line]);
  const save = () => {
    if (!bgId) return;
    setSaving(true);
    setLine(null);
    api
      .saveReportAttachment(bgId, index)
      .then((ok) => setLine(ok ? { text: later(() => t("report.save.done")) } : null))
      .catch((e) => setLine({ text: later(() => saveErrorText(typeof e === "string" ? e : "")), error: true }))
      .finally(() => setSaving(false));
  };
  const copy = (md: string) => {
    setLine(null);
    // The text as received (= the original), without bidi and invisible control characters, like the view.
    const done = navigator.clipboard?.writeText(stripUnsafe(md));
    if (!done) return setLine({ text: t("report.copy.failed"), error: true });
    done.then(() => setCopied(true)).catch(() => setLine({ text: t("report.copy.failed"), error: true }));
  };
  return { saving, line, copied, save, copy };
}

/** "저장" (the original, through the native save panel) and, for Markdown, "복사" (the text as received). */
function DocActions({ state, data }: { state: DocActionState; data?: AttachmentOpened }) {
  if (!data || (data.kind !== "html" && data.kind !== "markdown")) return null;
  const { saving, line, copied } = state;
  const md = data.kind === "markdown" ? data.text : null;
  return (
    <>
      {line && (
        <span role="status" className={`min-w-0 truncate text-[11px] ${line.error ? "text-danger" : "text-ink-2"}`} title={shown(line.text)} data-attachment-result>
          {shown(line.text)}
        </span>
      )}
      <button onClick={state.save} disabled={saving} className={headBtn} data-attachment-save>
        <Download size={12} aria-hidden />
        {saving ? t("report.save.busy") : t("report.save.button")}
      </button>
      {md !== null && (
        <button onClick={() => state.copy(md)} className={headBtn} data-attachment-copy>
          {copied ? <Check size={12} aria-hidden /> : <Copy size={12} aria-hidden />}
          <span aria-live="polite">{copied ? t("report.copy.done") : t("report.copy.button")}</span>
        </button>
      )}
    </>
  );
}

function DocHead({ title, onBig, onClose, bigRef, closeRef, full, actions }: { title: string; onBig?: () => void; onClose: () => void; bigRef?: React.Ref<HTMLButtonElement>; closeRef?: React.Ref<HTMLButtonElement>; full?: boolean; actions?: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 rounded-t-lg border-b border-line bg-sheet px-3 py-1.5" data-attachment-head data-no-select>
      <Lock size={12} className="shrink-0 text-ink-3" aria-hidden />
      <span className="min-w-0 flex-1 truncate text-xs text-ink-2">
        {t("report.attachment.head")} · <span className="text-ink">{title}</span>
      </span>
      {actions}
      {onBig && (
        <button ref={bigRef} onClick={onBig} className={headBtn}>
          <Maximize2 size={12} aria-hidden />
          {t("report.attachment.enlarge")}
        </button>
      )}
      {full && <span className="hidden shrink-0 text-[11px] text-ink-3 sm:inline">{t("report.attachment.fullHint")}</span>}
      <button
        ref={closeRef}
        onClick={onClose}
        className={full ? "inline-flex h-8 shrink-0 items-center gap-1 rounded-md border border-line px-3 text-[13px] text-ink hover:bg-paper" : "inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-xs text-ink-2 hover:bg-paper hover:text-ink"}
      >
        <X size={full ? 14 : 12} aria-hidden />
        {t("report.attachment.close")}
      </button>
    </div>
  );
}

function FullView({ data, title, onClose, actions }: { data: AttachmentOpened; title: string; onClose: () => void; actions?: React.ReactNode }) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useOverlayEscape(onClose);
  useEffect(() => closeRef.current?.focus(), []);
  return createPortal(
    <div className="fixed inset-0 z-50 flex flex-col bg-paper p-3" role="dialog" aria-modal="true" aria-label={t("report.attachment.fullLabel")}>
      <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-line">
        <DocHead title={title} onClose={onClose} closeRef={closeRef} full actions={actions} />
        <div className="min-h-0 flex-1">
          <Doc data={data} title={title} tall />
        </div>
      </div>
    </div>,
    overlayHost(),
  );
}

/** "첨부" section: one tab per attachment; the opened one is drawn below its app header. */
export function AttachmentsBlock({ bgId, list }: { bgId?: string; list: AttachmentView[] }) {
  useT();
  const [sel, setSel] = useState<number | null>(list.findIndex((a) => a.status === "valid") >= 0 ? list.findIndex((a) => a.status === "valid") : null);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const titleOf = (i: number) => list[i]?.title || t("report.attachment.untitled", { n: i + 1 });
  return (
    <div>
      <div role="tablist" aria-label={t("report.attachment.tabs")} className="flex flex-wrap gap-1.5">
        {list.map((a, i) => {
          const ok = a.status === "valid";
          return (
            <button
              key={i}
              ref={(el) => {
                tabRefs.current[i] = el;
              }}
              role="tab"
              aria-selected={sel === i}
              onClick={() => setSel(sel === i ? null : i)}
              className={`inline-flex max-w-full items-center gap-1.5 rounded-md border px-2 py-1 text-xs ${sel === i ? "border-ink-3 bg-sheet text-ink" : "border-line text-ink-2 hover:border-ink-3"} ${ok ? "" : "opacity-60"}`}
              data-attachment-tab
            >
              <FileText size={12} className="shrink-0" aria-hidden />
              <span className="min-w-0 truncate">{titleOf(i)}</span>
              <span className="shrink-0 text-[11px] text-ink-3">{a.kind === "html" ? "HTML" : "Markdown"}</span>
              {!ok && <span className="shrink-0 text-[11px] text-ink-3">· {EVIDENCE_STATUS_TEXT[a.status]}</span>}
            </button>
          );
        })}
      </div>
      {sel !== null && (
        <OpenDoc
          key={sel}
          bgId={bgId}
          index={sel}
          title={titleOf(sel)}
          onClose={() => {
            const at = sel;
            setSel(null);
            tabRefs.current[at]?.focus();
          }}
        />
      )}
    </div>
  );
}

/** One opened attachment: the inline panel and its full view share one save/copy state. */
function OpenDoc({ bgId, index, title, onClose }: { bgId?: string; index: number; title: string; onClose: () => void }) {
  const [big, setBig] = useState(false);
  const bigRef = useRef<HTMLButtonElement>(null);
  const { data, error } = useAttachment(bgId, index);
  const actions = useDocActions(bgId, index);
  const bar = bgId ? <DocActions state={actions} data={data} /> : null;
  return (
    <>
      <div className="mt-2 rounded-lg border border-line" role="tabpanel">
        <DocHead title={title} actions={bar} bigRef={bigRef} onBig={data ? () => setBig(true) : undefined} onClose={onClose} />
        {error ? (
          <p className="px-4 py-6 text-[13px] text-ink-3">{t("report.attachment.readFailed")}</p>
        ) : data ? (
          <Doc data={data} title={title} />
        ) : (
          <p className="px-4 py-6 text-[13px] text-ink-3">{t("report.attachment.loading")}</p>
        )}
      </div>
      {big && data && (
        <FullView
          data={data}
          title={title}
          actions={bar}
          onClose={() => {
            setBig(false);
            bigRef.current?.focus();
          }}
        />
      )}
    </>
  );
}
