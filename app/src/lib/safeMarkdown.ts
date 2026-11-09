import { t } from "../i18n";
// A deliberately small Markdown subset for Claude's usage review (D-078) and report attachments (D-156). It builds a plain data tree that React
// renders as text, so raw HTML never becomes markup. Images are dropped, only https links survive (shown with their
// full host and opened through a native confirmation), bare URLs stay plain text, and control/bidi characters are
// stripped — on every render, including results loaded from storage.

export type Inline = { t: "text"; v: string } | { t: "b"; v: string } | { t: "code"; v: string } | { t: "link"; label: string; url: string; host: string };

export type Block =
  | { type: "h"; level: 2 | 3 | 4; inl: Inline[] }
  | { type: "p"; inl: Inline[] }
  | { type: "ul" | "ol"; items: Inline[][] }
  | { type: "pre"; text: string }
  /** GFM pipe table (report attachments, D-156): header, alignment, cells with the inline rules. */
  | { type: "table"; head: Inline[][]; align: ("left" | "center" | "right" | null)[]; rows: Inline[][][] }
  /** A same-folder image line (`![alt](name.png)` alone); `n` = its order among image lines, `k` = its occurrence
   *  among lines with the same name (Rust sends (name, data) pairs, matched by name and `k`). */
  | { type: "img"; alt: string; name: string; n: number; k: number };

/** Report attachments are 64 KB (Rust refuses larger); the usage review is far smaller. */
const MAX_BYTES = 64 * 1024;
export const TABLE_COLS = 20;
export const TABLE_ROWS = 500;
/** The same rule as attachment.rs `MD_IMAGE_RE`. */
const IMAGE_LINE = /^\s*!\[([^\]\n]{0,200})\]\(([A-Za-z0-9._-]{1,80})\)\s*$/;
const TABLE_SEP = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

/** Cells of one table row (outer pipes optional, `\|` stays a pipe). */
function cells(line: string): string[] {
  let t = line.trim();
  if (t.startsWith("|")) t = t.slice(1);
  if (t.endsWith("|") && !t.endsWith("\\|")) t = t.slice(0, -1);
  return t.split(/(?<!\\)\|/).map((c) => c.replace(/\\\|/g, "|").trim());
}

// Cc (except \n and \t), bidi/format characters, zero-width, line/paragraph separators, BOM, variation selectors, tags.
// eslint-disable-next-line no-control-regex
const UNSAFE = /[\u0000-\u0008\u000B-\u001F\u007F-\u009F\u00AD\u061C\u115F\u1160\u180E\u200B-\u200F\u2028-\u202E\u2060-\u206F\u3164\uFEFF\uFFF9-\uFFFB]|\u034F|\uFE00|\uFE01|\uFE02|\uFE03|\uFE04|\uFE05|\uFE06|\uFE07|\uFE08|\uFE09|\uFE0A|\uFE0B|\uFE0C|\uFE0D|\uFE0E|\uFE0F|\uDB40[\uDC00-\uDC7F]/g;

export function stripUnsafe(s: string): string {
  let out = s.replace(UNSAFE, "");
  while (new TextEncoder().encode(out).length > MAX_BYTES) out = out.slice(0, Math.floor(out.length * 0.9));
  return out;
}

/** https with a host and no credentials; returns the normalized URL and host. */
export function safeLink(raw: string): { url: string; host: string } | null {
  try {
    const u = new URL(raw);
    if (u.protocol !== "https:" || !u.hostname || u.username || u.password) return null;
    return { url: u.href, host: u.hostname };
  } catch {
    return null;
  }
}

const INLINE = /(\*\*[^*\n]{1,500}\*\*|`[^`\n]{1,500}`|!?\[[^\]\n]{0,200}\]\([^)\s]{1,2048}\))/g;

// A label that itself looks like a URL or domain could disguise the real target: show the host instead.
const URLISH = /^(?:(?:https?|ftp|file|javascript|data|mailto):|www\.)|\/\/|^[^\s]+\.[a-z]{2,}(?:\/|$)/i;

export function parseInline(s: string): Inline[] {
  const out: Inline[] = [];
  const push = (v: string) => {
    if (!v) return;
    const last = out[out.length - 1];
    if (last?.t === "text") last.v += v;
    else out.push({ t: "text", v });
  };
  let at = 0;
  for (const m of s.matchAll(INLINE)) {
    push(s.slice(at, m.index));
    at = (m.index ?? 0) + m[0].length;
    const tok = m[0];
    if (tok.startsWith("**")) out.push({ t: "b", v: tok.slice(2, -2) });
    else if (tok.startsWith("`")) out.push({ t: "code", v: tok.slice(1, -1) });
    else {
      const mm = /^(!?)\[([^\]]{0,200})\]\(([^)\s]{1,2048})\)$/.exec(tok);
      if (!mm) {
        push(tok);
        continue;
      }
      const [, bang, label, href] = mm;
      if (bang) {
        // Images are never loaded; keep the alt text only.
        if (label) push(t("report.markdown.image", { label }));
        continue;
      }
      const link = safeLink(href);
      if (link) out.push({ t: "link", label: !label.trim() || URLISH.test(label.trim()) ? link.host : label, ...link });
      else push(label ? `${label} (${href})` : href);
    }
  }
  push(s.slice(at));
  return out;
}

export function parseMarkdown(src: string): Block[] {
  const lines = stripUnsafe(src).replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) blocks.push({ type: "p", inl: parseInline(para.join(" ")) });
    para = [];
  };
  let images = 0;
  const byName = new Map<string, number>();
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const img = IMAGE_LINE.exec(line);
    if (img && !/^\s*```/.test(line)) {
      flush();
      const k = byName.get(img[2]) ?? 0;
      byName.set(img[2], k + 1);
      blocks.push({ type: "img", alt: img[1], name: img[2], n: images++, k });
      continue;
    }
    if (line.includes("|") && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1]) && lines[i + 1].includes("-")) {
      const head = cells(line);
      const align = cells(lines[i + 1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : c.startsWith(":") ? "left" : null));
      if (head.length <= TABLE_COLS && head.length === align.length) {
        flush();
        const rows: Inline[][][] = [];
        for (i += 2; i < lines.length && lines[i].includes("|") && lines[i].trim() && rows.length < TABLE_ROWS; i++) {
          const row = cells(lines[i]).slice(0, head.length);
          while (row.length < head.length) row.push("");
          rows.push(row.map(parseInline));
        }
        i--;
        blocks.push({ type: "table", head: head.map(parseInline), align, rows });
        continue;
      }
    }
    if (/^\s*```/.test(line)) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !/^\s*```/.test(lines[i]); i++) body.push(lines[i]);
      blocks.push({ type: "pre", text: body.join("\n") });
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) {
      flush();
      blocks.push({ type: "h", level: Math.min(4, Math.max(2, h[1].length)) as 2 | 3 | 4, inl: parseInline(h[2]) });
      continue;
    }
    const li = /^\s*(?:([-*+])|(\d+)[.)])\s+(.*)$/.exec(line);
    if (li) {
      flush();
      const type = li[1] ? "ul" : "ol";
      const last = blocks[blocks.length - 1];
      const item = parseInline(li[3]);
      if (last && last.type === type) last.items.push(item);
      else blocks.push({ type, items: [item] });
      continue;
    }
    if (!line.trim()) {
      flush();
      continue;
    }
    // Table rows and quotes are kept as text lines.
    para.push(line.replace(/^>\s?/, ""));
  }
  flush();
  return blocks;
}
