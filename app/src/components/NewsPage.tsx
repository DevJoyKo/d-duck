// 소식 (news-tab, D-204): Claude Code feature explainers, newest first, with a confirmed fetch and an optional daily
// fetch. Item text is Claude's checked output in the app language and is rendered as plain text only.
import { listen } from "@tauri-apps/api/event";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Bot, Cpu, ExternalLink, Loader2, Newspaper, Plug, RefreshCw, Settings2, ShieldCheck, Sparkles, SquareCode, SquareTerminal, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api, messageOf, type NewsCategory, type NewsItem, type NewsRun, type NewsView } from "../lib/api";
import { dateTime, usd } from "../lib/format";
import { categoryColor, dayMs, hostOf, splitEarlier } from "../lib/news";
import { isGateError } from "../lib/auth";
import { useLanguage, useT } from "../i18n/react";
import { currentLocale } from "../i18n/language";
import { GateLine } from "./LoginBits";
import { DuckAvatar } from "./DuckAvatar";
import { Button, PageBody, PageHeader, Switch } from "./ui";
import { useModal } from "../lib/useModal";

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
function catKey(c: NewsCategory) {
  return `news.category.${c === "other" ? "misc" : c}` as const;
}

/** "11월 9일" (the year only when it is not this year). */
function shortDate(date: string): string {
  const ms = dayMs(date);
  if (Number.isNaN(ms)) return date;
  const d = new Date(ms);
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString(currentLocale(), sameYear ? { month: "long", day: "numeric" } : { year: "numeric", month: "long", day: "numeric" });
}

function Thumb({ category, size = "row" }: { category: NewsCategory; size?: "row" | "sheet" }) {
  const t = useT();
  const c = categoryColor(category);
  const Icon = ICON[category] ?? Sparkles;
  const box = size === "row" ? "h-16 w-24" : "h-12 w-12";
  return (
    <span aria-hidden className={`grid shrink-0 place-items-center rounded-lg ${box}`} style={{ background: `color-mix(in srgb, ${c.body} var(--news-thumb-mix), transparent)` }} title={t(catKey(category))}>
      <Icon size={size === "row" ? 22 : 20} strokeWidth={1.8} style={{ color: c.scarf }} />
    </span>
  );
}

function Meta({ item }: { item: NewsItem }) {
  const t = useT();
  const date = shortDate(item.date);
  return (
    <span className="text-xs text-ink-3">
      {item.version ? t("news.list.meta", { version: item.version, date }) : date} · {t(catKey(item.category))}
    </span>
  );
}

function Row({ item, onOpen }: { item: NewsItem; onOpen: (it: NewsItem, el: HTMLButtonElement) => void }) {
  const t = useT();
  return (
    <li>
      <button
        type="button"
        data-news-row={item.id}
        onClick={(e) => onOpen(item, e.currentTarget)}
        className="flex w-full items-center gap-4 rounded-xl px-3 py-3 text-left transition-colors hover:bg-ink/4 focus-visible:outline-2 focus-visible:outline-ink"
      >
        <Thumb category={item.category} />
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2">
            {!item.seen && (
              <span className="inline-flex shrink-0 items-center" data-news-new>
                <span aria-hidden className="h-2 w-2 rounded-full bg-ink" />
                <span className="sr-only">{t("news.list.new")}</span>
              </span>
            )}
            <span className={`truncate text-[15px] leading-snug ${item.seen ? "font-medium text-ink-2" : "font-semibold text-ink"}`}>{item.title}</span>
          </span>
          <span className="mt-1 block truncate text-[13px] text-ink-2">{item.summary}</span>
          <span className="mt-1.5 block">
            <Meta item={item} />
          </span>
        </span>
      </button>
    </li>
  );
}

function Section({ title, items, onOpen }: { title: string; items: NewsItem[]; onOpen: (it: NewsItem, el: HTMLButtonElement) => void }) {
  if (items.length === 0) return null;
  return (
    <section aria-label={title} data-news-section={title}>
      <h2 className="mb-1 px-3 text-[13px] font-semibold text-ink-2">{title}</h2>
      <ul className="divide-y divide-line">
        {items.map((it) => (
          <Row key={it.id} item={it} onOpen={onOpen} />
        ))}
      </ul>
    </section>
  );
}

function Detail({ item, onClose }: { item: NewsItem; onClose: () => void }) {
  const t = useT();
  const closeRef = useRef<HTMLButtonElement>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  // Focus in, Tab inside, Escape closes (sheet-focus). The parent puts focus back on the row by its id after the
  // list re-renders, so the hook leaves focus alone on close. The dialog is found from × instead of a ref on it.
  const [dialogRef] = useState(() => ({ get current() { return closeRef.current?.closest<HTMLElement>('[role="dialog"]') ?? null; } }));
  useModal(dialogRef, onClose, { initial: closeRef, returnTo: () => null });
  const block = (label: string, text: string, key: string) => (
    <section data-news-detail={key}>
      <h3 className="text-[13px] font-semibold text-ink">{label}</h3>
      <p className="mt-1 text-[13px] leading-relaxed whitespace-pre-line text-ink-2">{text}</p>
    </section>
  );
  return (
    <div className="fixed inset-0 z-30 flex justify-end bg-scrim/45" onClick={onClose}>
      <div role="dialog" aria-modal="true" aria-label={t("news.detail.label", { title: item.title })} onClick={(e) => e.stopPropagation()} className="flex h-full w-[min(560px,92vw)] flex-col border-l border-line bg-sheet shadow-e3">
        <div className="flex items-start gap-3 border-b border-line px-6 py-4">
          <Thumb category={item.category} size="sheet" />
          <div className="min-w-0 flex-1">
            <h2 className="text-[16px] leading-snug font-semibold">{item.title}</h2>
            <p className="mt-1">
              <Meta item={item} />
            </p>
          </div>
          <button ref={closeRef} onClick={onClose} aria-label={t("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={18} />
          </button>
        </div>
        <div className="flex-1 space-y-5 overflow-y-auto px-6 py-5">
          <p className="text-[14px] leading-relaxed text-ink">{item.summary}</p>
          {block(t("news.detail.what"), item.what, "what")}
          {block(t("news.detail.when"), item.whenToUse, "when")}
          {block(t("news.detail.best"), item.bestPractice, "best")}
          <div>
            <Button
              onClick={() => {
                setOpenError(null);
                api.newsOpenSource(item.id).catch((e) => setOpenError(messageOf(e)));
              }}
            >
              <ExternalLink size={13} /> {t("news.detail.open")}
              <span className="text-xs text-ink-3">{hostOf(item.sourceUrl)}</span>
            </Button>
            {openError && (
              <p role="alert" className="mt-2 text-[13px] text-danger">
                {t("news.detail.openFailed", { error: openError })}
              </p>
            )}
          </div>
        </div>
        <p className="border-t border-line px-6 py-3 text-xs text-ink-3">{t("news.detail.note")}</p>
      </div>
    </div>
  );
}

function failText(r: Extract<NewsRun, { kind: "failed" }>, t: ReturnType<typeof useT>): string {
  const spent = r.error.costUsd != null ? t("news.error.spent", { cost: usd(r.error.costUsd) }) : "";
  if (r.error.kind === "cancelled") return "";
  return `${r.message}${spent}`;
}

/** `quiet`: the failure is already in the alert below, so the line stays neutral (one red place). */
function StatusLine({ view, running, onCancel, quiet }: { view: NewsView; running: boolean; onCancel: () => void; quiet: boolean }) {
  const t = useT();
  const s = view.status;
  let text: ReactNode;
  if (running) {
    text = (
      <span className="inline-flex items-center gap-1.5 text-ink-2">
        <Loader2 size={13} className="animate-spin" /> {t("news.fetch.running")}
      </span>
    );
  } else if (!s) {
    text = t("news.status.never");
  } else if (s.ok) {
    text = `${t("news.status.ok", { when: dateTime(s.atMs), count: s.added })}${s.costUsd != null ? t("news.status.cost", { cost: usd(s.costUsd) }) : ""}${s.dropped ? t("news.status.dropped", { count: s.dropped }) : ""}${s.daily ? t("news.status.daily") : ""}${s.dailyOff ? t("news.status.dailyOff") : ""}`;
  } else if (s.error === "cancelled") {
    text = t("news.status.cancelled", { when: dateTime(s.atMs) });
  } else {
    text = (
      <span className={quiet ? "" : "text-danger"}>
        {t("news.status.failed", { when: dateTime(s.atMs), reason: t(`news.reason.${s.error === "login_required" ? "loginRequired" : (s.error ?? "failed")}`) })}
        {s.daily ? t("news.status.daily") : ""}
        {s.dailyOff ? t("news.status.dailyOff") : ""}
      </span>
    );
  }
  return (
    <p data-news-status className="flex min-w-0 px-3 flex-wrap items-center gap-2 text-xs text-ink-3">
      {text}
      {running && (
        <Button size="xs" variant="plain" onClick={onCancel}>
          {t("news.fetch.cancel")}
        </Button>
      )}
    </p>
  );
}

export function NewsPage() {
  const t = useT();
  const { lang } = useLanguage();
  const qc = useQueryClient();
  const key = ["news", lang];
  const [open, setOpen] = useState<NewsItem | null>(null);
  const opener = useRef<HTMLButtonElement | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [gate, setGate] = useState<"login" | "claudeAi" | null>(null);
  const [dailyNote, setDailyNote] = useState<string | null>(null);
  const view = useQuery({ queryKey: key, queryFn: api.newsList, refetchInterval: (q) => (q.state.data?.running ? 3000 : false) });

  useEffect(() => {
    const sub = listen("news-updated", () => void qc.invalidateQueries({ queryKey: ["news"] }));
    return () => {
      sub.then((un) => un());
    };
  }, [qc]);

  const fetch = useMutation({
    mutationFn: api.newsFetch,
    onMutate: () => {
      setRunError(null);
      setGate(null);
    },
    onSuccess: (r) => {
      if (r.kind === "failed" && isGateError(r.error)) setGate(isGateError(r.error));
      else if (r.kind === "failed") setRunError(failText(r, t) || null);
      void qc.invalidateQueries({ queryKey: ["news"] });
    },
    onError: (e) => (isGateError(e) ? setGate(isGateError(e)) : setRunError(messageOf(e))),
  });
  const daily = useMutation({
    mutationFn: (on: boolean) => api.newsSetDaily(on),
    onMutate: () => setDailyNote(null),
    onSuccess: (on, asked) => {
      if (asked && !on) setDailyNote(t("news.daily.turnedOff"));
      void qc.invalidateQueries({ queryKey: ["news"] });
    },
    onError: (e) => setDailyNote(messageOf(e)),
  });

  const v = view.data;
  const running = fetch.isPending || !!v?.running;
  const items = v?.items ?? [];
  const { recent, earlier } = splitEarlier(items);
  const unseen = items.filter((i) => !i.seen).length;

  const openItem = (it: NewsItem, el: HTMLButtonElement) => {
    opener.current = el;
    setOpen(it);
    if (!it.seen) {
      void api.newsMarkSeen(it.id).then(() => qc.setQueryData<NewsView>(key, (old) => (old ? { ...old, items: old.items.map((x) => (x.id === it.id ? { ...x, seen: true } : x)) } : old)));
    }
  };
  const close = useCallback(() => {
    setOpen(null);
    const id = opener.current?.dataset.newsRow;
    // The row may have re-rendered: focus it by its id, not by the old element.
    if (id) requestAnimationFrame(() => Array.from(document.querySelectorAll<HTMLButtonElement>("[data-news-row]")).find((e) => e.dataset.newsRow === id)?.focus());
  }, []);

  const fetchButton = (
    <Button variant="primary" onClick={() => fetch.mutate()} disabled={running}>
      {running ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
      {running ? t("news.fetch.running") : t("news.fetch.button")}
    </Button>
  );

  return (
    <>
      <PageHeader title={t("news.title")} summary={items.length ? t("news.summary", { count: items.length }) : undefined}>
        {items.length > 0 && fetchButton}
      </PageHeader>
      <PageBody>
        {v && (
          <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
            <StatusLine view={v} running={running} quiet={!!runError} onCancel={() => void api.newsCancel()} />
            <div className="ml-auto flex items-center gap-2">
              <Switch on={v.daily.enabled} label={t("news.daily.label")} disabled={daily.isPending} onClick={() => daily.mutate(!v.daily.enabled)} />
              <span className="text-[13px] text-ink">{t("news.daily.label")}</span>
              <span className="text-xs text-ink-3">{!v.daily.enabled ? t("news.daily.note") : v.daily.capped ? t("news.daily.capped") : v.daily.nextAtMs ? t("news.daily.next", { when: dateTime(v.daily.nextAtMs) }) : t("news.daily.note")}</span>
            </div>
          </div>
        )}
        {dailyNote && <p className="px-3 text-xs text-ink-3">{dailyNote}</p>}
        <GateLine force={gate} />
        {runError && (
          <p role="alert" className="rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-[13px] text-danger">
            {runError}
          </p>
        )}
        {view.isError && <p className="text-[13px] text-danger">{messageOf(view.error)}</p>}
        {v && items.length === 0 && (
          <div className="rounded-xl bg-sheet py-12 text-center shadow-e1" data-news-empty>
            <div className="mb-3 flex justify-center">
              <DuckAvatar pose="idle" size={72} />
            </div>
            <p className="text-[14px] font-medium text-ink">{t("news.empty.title")}</p>
            <p className="mt-1 text-xs text-ink-3">{t("news.empty.body")}</p>
            {v.otherLang > 0 && <p className="mt-1 text-xs text-ink-3">{t("news.empty.otherLang", { count: v.otherLang })}</p>}
            <div className="mt-4 flex justify-center">{fetchButton}</div>
            <p className="mt-2 text-xs text-ink-3">{t("news.fetch.cost")}</p>
          </div>
        )}
        {items.length > 0 && (
          <div className="space-y-6" aria-label={t("news.list.label")} data-unseen={unseen}>
            <Section title={t("news.list.recent")} items={recent} onOpen={openItem} />
            <Section title={t("news.list.earlier")} items={earlier} onOpen={openItem} />
            <p className="flex items-center gap-1.5 px-3 text-xs text-ink-3">
              <Newspaper size={12} aria-hidden /> {t("news.fetch.cost")}
            </p>
          </div>
        )}
      </PageBody>
      {open && <Detail item={open} onClose={close} />}
    </>
  );
}
