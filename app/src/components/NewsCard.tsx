// News thumbnails (news-tab D-204, news-apply D-208): the key-fact card when an item has `keyFact`, else the category
// card; the adopt tag sits under either. Drawn by the app from text only: no remote image.
import { Bot, Cpu, Plug, Settings2, ShieldCheck, Sparkles, SquareCode, SquareTerminal } from "lucide-react";
import type { NewsAdopt, NewsCategory, NewsItem } from "../lib/api";
import { categoryColor } from "../lib/news";
import { useT } from "../i18n/react";

const ICON: Record<NewsCategory, typeof Cpu> = {
  model: Cpu,
  command: SquareTerminal,
  setting: Settings2,
  agent: Bot,
  integration: Plug,
  ide: SquareCode,
  security: ShieldCheck,
  other: Sparkles,
};

/** "other" is a reserved plural name in the catalogs, so that category's label lives under `misc`. */
export function catKey(c: NewsCategory) {
  return `news.category.${c === "other" ? "misc" : c}` as const;
}

type Size = "row" | "sheet";

function Thumb({ category, size }: { category: NewsCategory; size: Size }) {
  const t = useT();
  const c = categoryColor(category);
  const Icon = ICON[category] ?? Sparkles;
  // Rows: the same 118px as the key-fact card, so titles line up (brand review B3).
  const box = size === "row" ? "h-16 w-[118px]" : "h-12 w-12";
  return (
    <span
      aria-hidden
      data-news-category={category}
      className={`grid shrink-0 place-items-center rounded-lg ${box}`}
      style={{ background: `color-mix(in srgb, ${c.body} var(--news-thumb-mix), transparent)` }}
      title={t(catKey(category))}
    >
      <Icon size={size === "row" ? 22 : 20} strokeWidth={1.8} style={{ color: c.scarf }} />
    </span>
  );
}

/** A command the user types (`/diff`, `claude plugin eval`): shown in monospace. */
export function isCommandFact(fact: string): boolean {
  return fact.startsWith("/") || fact.startsWith("claude ");
}

// Text on every tag, never colour alone. Amber for 설정 필요: the duck yellow stays for the one call to action.
const ADOPT_CLASS: Record<NewsAdopt, string> = {
  update: "bg-ok/10 text-ok-strong",
  config: "bg-warn-soft text-warn",
  opt_in: "bg-ink/6 text-ink-2",
};

function AdoptTag({ adopt }: { adopt: NewsAdopt }) {
  const t = useT();
  return (
    <span data-news-adopt={adopt} className={`inline-block rounded-full px-1.5 py-px text-[11px] leading-4 font-medium whitespace-nowrap ${ADOPT_CLASS[adopt]}`}>
      {t(`news.adopt.${adopt === "opt_in" ? "optIn" : adopt}`)}
    </span>
  );
}

function FactCard({ fact, size }: { fact: string; size: Size }) {
  const mono = isCommandFact(fact);
  const box = size === "row" ? "h-16 w-[118px]" : "min-h-12 w-[132px]";
  // The row shows up to two lines, breaking at spaces and hyphens first, then cuts with an ellipsis (the whole text is
  // in `title` and in the sheet head, which wraps it in full).
  const text = size === "row" ? "line-clamp-2 wrap-break-word" : "block wrap-anywhere";
  return (
    <span data-news-card className={`flex shrink-0 items-center rounded-lg border border-line bg-sheet px-2.5 py-2 ${box}`}>
      <span data-news-fact title={fact} className={`min-w-0 flex-1 leading-snug font-semibold text-ink ${text} ${mono ? "font-mono text-[14px]" : "text-[15px]"}`}>
        {fact}
      </span>
    </span>
  );
}

/** The row (118px) or sheet-head (132px) thumbnail of a news item. */
export function NewsThumb({ item, size = "row" }: { item: NewsItem; size?: Size }) {
  const fact = item.keyFact?.trim();
  return (
    <span className={`flex shrink-0 flex-col items-start gap-1.5 ${size === "row" ? "w-[118px]" : ""}`}>
      {fact ? <FactCard fact={fact} size={size} /> : <Thumb category={item.category} size={size} />}
      {item.adopt && <AdoptTag adopt={item.adopt} />}
    </span>
  );
}
