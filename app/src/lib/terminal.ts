// Pure helpers for the log view; the xterm component itself is not unit-tested (jsdom has no canvas).

export const MAX_LOG_CHARS = 1024 * 1024;

/**
 * Keep output from the last full-screen clear so a truncated TUI stream still renders cleanly. When the kept part is
 * still too long, the cut moves forward to the next redraw start (`ESC[H`, else the next `ESC`) so it never lands
 * inside an escape sequence or a surrogate pair and prints its parameter bytes as text.
 */
export function lastScreen(out: string): string {
  const i = out.lastIndexOf("\x1b[2J");
  const s = i >= 0 ? out.slice(i) : out;
  if (s.length <= MAX_LOG_CHARS) return s;
  const cut = s.length - MAX_LOG_CHARS;
  const home = s.indexOf("\x1b[H", cut);
  const esc = home >= 0 ? home : s.indexOf("\x1b", cut);
  return esc >= 0 ? s.slice(esc) : "";
}

/** Columns a character takes under Unicode 11 widths (what xterm uses once the unicode11 addon is active). */
export function charWidth(cp: number): 0 | 1 | 2 {
  if (cp < 0x20 || (cp >= 0x7f && cp < 0xa0)) return 0;
  if ((cp >= 0x300 && cp <= 0x36f) || cp === 0x200b || cp === 0x200d || (cp >= 0xfe00 && cp <= 0xfe0f)) return 0;
  if (
    (cp >= 0x1100 && cp <= 0x115f) ||
    (cp >= 0x231a && cp <= 0x231b) ||
    (cp >= 0x23e9 && cp <= 0x23ec) ||
    cp === 0x23f0 ||
    cp === 0x23f3 ||
    (cp >= 0x25fd && cp <= 0x25fe) ||
    (cp >= 0x2614 && cp <= 0x2615) ||
    (cp >= 0x2648 && cp <= 0x2653) ||
    [0x267f, 0x2693, 0x26a1, 0x26aa, 0x26ab, 0x26bd, 0x26be, 0x26c4, 0x26c5, 0x26ce, 0x26d4, 0x26ea, 0x26f2, 0x26f3,
      0x26f5, 0x26fa, 0x26fd, 0x2705, 0x270a, 0x270b, 0x2728, 0x274c, 0x274e, 0x2757, 0x27b0, 0x27bf, 0x2b50, 0x2b55].includes(cp) ||
    (cp >= 0x2753 && cp <= 0x2755) ||
    (cp >= 0x2795 && cp <= 0x2797) ||
    (cp >= 0x2b1b && cp <= 0x2b1c) ||
    (cp >= 0x2e80 && cp <= 0x303e) ||
    (cp >= 0x3041 && cp <= 0x33ff) ||
    (cp >= 0x3400 && cp <= 0x4dbf) ||
    (cp >= 0x4e00 && cp <= 0x9fff) ||
    (cp >= 0xa000 && cp <= 0xa4cf) ||
    (cp >= 0xa960 && cp <= 0xa97f) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe30 && cp <= 0xfe4f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1f64f) ||
    (cp >= 0x1f680 && cp <= 0x1f6ff) ||
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x1fa70 && cp <= 0x1faff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  )
    return 2;
  return 1;
}

export const MIN_GEOMETRY = { cols: 40, rows: 24 };
export const MAX_GEOMETRY = { cols: 400, rows: 200 };

/**
 * Lower bound of the PTY size a TUI stream was drawn for. `claude logs` carries no size, and Claude Code paints with
 * absolute rows (`ESC[50;1H`), absolute columns (`ESC[193G`) and relative moves from home. Drawn into a smaller
 * xterm these clamp to the last row/column and overwrite each other, so the cursor is followed on an unbounded grid
 * and the furthest cell reached is the size to emulate.
 */
export function estimateGeometry(out: string): { cols: number; rows: number } {
  let row = 1;
  let col = 1;
  let maxRow = 1;
  let maxCol = 1;
  let saved: [number, number] = [1, 1];
  // eslint-disable-next-line no-control-regex -- escape sequences are what this scans
  const re = /\x1b\[([?>=!]?)([0-9;:]*)[ -/]*([@-~])|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)?|\x1b([78])|\x1b[@-_ -/][0-~]?|([\s\S])/gu;
  for (const m of out.matchAll(re)) {
    const [, priv, params, final, escSave, ch] = m;
    if (final !== undefined) {
      if (priv) continue;
      const ps = params.split(";").map((p) => parseInt(p, 10));
      const n = ps[0] > 0 ? ps[0] : 1;
      switch (final) {
        case "H":
        case "f":
          row = n;
          col = ps[1] > 0 ? ps[1] : 1;
          break;
        case "G":
        case "`":
          col = n;
          break;
        case "d":
          row = n;
          break;
        case "A":
          row = Math.max(1, row - n);
          break;
        case "B":
        case "e":
          row += n;
          break;
        case "C":
        case "a":
          col += n;
          break;
        case "D":
          col = Math.max(1, col - n);
          break;
        case "E":
          row += n;
          col = 1;
          break;
        case "F":
          row = Math.max(1, row - n);
          col = 1;
          break;
        case "s":
          saved = [row, col];
          break;
        case "u":
          [row, col] = saved;
          break;
        default:
          continue;
      }
      maxRow = Math.max(maxRow, row);
      // A parked cursor sits on a cell, so it counts; it never needs a column past the last printed one plus one.
      maxCol = Math.max(maxCol, col - 1);
    } else if (escSave !== undefined) {
      if (escSave === "7") saved = [row, col];
      else [row, col] = saved;
    } else if (ch !== undefined) {
      if (ch === "\r") col = 1;
      else if (ch === "\n") maxRow = Math.max(maxRow, ++row);
      else if (ch === "\b") col = Math.max(1, col - 1);
      else {
        const w = charWidth(ch.codePointAt(0) ?? 0);
        if (w === 0) continue;
        col += w;
        maxCol = Math.max(maxCol, col - 1);
        maxRow = Math.max(maxRow, row);
      }
    }
  }
  const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
  return {
    cols: clamp(maxCol, MIN_GEOMETRY.cols, MAX_GEOMETRY.cols),
    rows: clamp(maxRow, MIN_GEOMETRY.rows, MAX_GEOMETRY.rows),
  };
}

/** Cell width of the log font is 0.6 × font size (SF Mono / Menlo). */
export const CELL_RATIO = 0.6;

/** The log font: macOS fonts first, then Windows ones (windows-support 4.4). */
export const LOG_FONT = 'ui-monospace, "SF Mono", Menlo, "Cascadia Mono", Consolas, monospace';

/** Cell width ÷ font size of `family`, measured on a canvas (Windows fonts are narrower than 0.6, e.g. Consolas 0.55).
 * Falls back to {@link CELL_RATIO} when nothing can be measured or the result is implausible. */
export function measureCellRatio(family: string, measure?: (font: string, text: string) => number | null): number {
  const probe = "M".repeat(20);
  const size = 100;
  let w: number | null = null;
  try {
    if (measure) w = measure(`${size}px ${family}`, probe);
    else {
      const ctx = document.createElement("canvas").getContext("2d");
      if (ctx) {
        ctx.font = `${size}px ${family}`;
        w = ctx.measureText(probe).width;
      }
    }
  } catch {
    w = null;
  }
  const r = w ? w / probe.length / size : NaN;
  return r >= 0.4 && r <= 0.8 ? r : CELL_RATIO;
}

/** Largest font (7–11 px) whose `cols` columns fit `widthPx`; wider screens keep 7 px and scroll sideways. */
export function fitFontSize(cols: number, widthPx: number, ratio: number = CELL_RATIO): number {
  if (!(widthPx > 0)) return 11;
  return Math.min(11, Math.max(7, Math.floor((widthPx / (cols * ratio)) * 2) / 2));
}
