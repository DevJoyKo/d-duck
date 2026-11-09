// Artifact cards (D-102): client-side pagination, the sandboxed preview document and reason texts.
import { t } from "../i18n";

export const PAGE_SIZE = 10;
/** The shell returns at most this many rows. */
export const LIST_LIMIT = 500;
/** Width and height the preview page is laid out at before it is scaled down to the card (≥ ~0.23 at 4 columns, 1440px). */
export const FRAME_W = 960;
export const FRAME_H = 600;

export function pageCount(total: number, size = PAGE_SIZE): number {
  return Math.max(1, Math.ceil(total / size));
}

/** Keeps a page number inside 1..pageCount (e.g. after a search shrinks the list). */
export function clampPage(page: number, total: number, size = PAGE_SIZE): number {
  return Math.min(Math.max(1, Math.floor(page) || 1), pageCount(total, size));
}

export function pageSlice<T>(rows: T[], page: number, size = PAGE_SIZE): T[] {
  const p = clampPage(page, rows.length, size);
  return rows.slice((p - 1) * size, p * size);
}

/** Narrowest card: keeps an HTML preview at ≥ 0.25 scale of FRAME_W (D-184). */
export const MIN_CARD_W = 240;
/** Preview height / width (aspect-[16/10]). */
export const PREVIEW_RATIO = 10 / 16;

/**
 * Cards per page for a measured list area (D-184): as many columns as fit at `minCard` width, times the rows whose
 * whole card fits in `height` (a card is its 16:10 preview plus `body` px of text and buttons). At least 1×1.
 * Before the area has a size (first paint, tests without layout) the old fixed PAGE_SIZE is used.
 */
export function fitPageSize({ width, height, minCard, gap, body }: { width: number; height: number; minCard: number; gap: number; body: number }): { cols: number; rows: number; size: number } {
  if (width <= 0 || height <= 0) return { cols: 0, rows: 0, size: PAGE_SIZE };
  const cols = Math.max(1, Math.floor((width + gap) / (minCard + gap)));
  const cardH = ((width - gap * (cols - 1)) / cols) * PREVIEW_RATIO + body;
  const rows = Math.max(1, Math.floor((height + gap) / (cardH + gap)));
  return { cols, rows, size: cols * rows };
}

/** The page that holds card `index` (0-based) at `size` cards per page: where to land after the page size changes. */
export function pageForAnchor(index: number, size: number): number {
  return Math.floor(Math.max(0, index) / Math.max(1, size)) + 1;
}

/** Page buttons to show: all of them up to 7, otherwise first, last and a window around the current one. */
export function pageNumbers(current: number, count: number): (number | "gap")[] {
  if (count <= 7) return Array.from({ length: count }, (_, i) => i + 1);
  const keep = new Set([1, count, current - 1, current, current + 1]);
  const out: (number | "gap")[] = [];
  for (let i = 1; i <= count; i++) {
    if (keep.has(i)) out.push(i);
    else if (out[out.length - 1] !== "gap") out.push("gap");
  }
  return out;
}

/** No script, network, base or form in the preview document; it is also `sandbox=""`. */
export const PREVIEW_CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'";

/** The srcdoc for an HTML preview: the CSP and DNS-prefetch metas come first, before anything from the file. */
export function previewSrcdoc(html: string): string {
  return `<meta http-equiv="Content-Security-Policy" content="${PREVIEW_CSP}"><meta http-equiv="x-dns-prefetch-control" content="off">${html}`;
}

export type PreviewReason = "noSource" | "missing" | "refused" | "tooLarge" | "typed" | "unknown";

const REASONS: Record<PreviewReason, true> = { noSource: true, missing: true, refused: true, tooLarge: true, typed: true, unknown: true };

/** The command rejects with a bare reason code; anything else (incl. inherited keys like "toString") is unknown. */
export function previewReason(e: unknown): PreviewReason {
  return typeof e === "string" && Object.prototype.hasOwnProperty.call(REASONS, e) ? (e as PreviewReason) : "unknown";
}

/** Read at call time, so it follows the app language. */
export function reasonText(r: PreviewReason): string {
  return t(`artifacts.reason.${r}`);
}

/** Runs at most `n` tasks at once, in call order. */
export function limiter(n: number) {
  let active = 0;
  const queue: (() => void)[] = [];
  const next = () => {
    if (active >= n) return;
    const run = queue.shift();
    if (run) {
      active++;
      run();
    }
  };
  return <T>(task: () => Promise<T>): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      queue.push(() => {
        task()
          .then(resolve, reject)
          .finally(() => {
            active--;
            next();
          });
      });
      next();
    });
}
