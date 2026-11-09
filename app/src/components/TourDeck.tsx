// Card-by-card tour (onboarding-tour; tour-v2, D-113). Each card: the key line and one supporting line on the left,
// a still cut-out of a real screen on the right (stacked when narrow). Click anywhere on the card or press →/Space for
// the next card, ← for the previous one, Esc to close (dismissed, not finished). Buttons, dots and links never advance.
// "n/5" counts the story cards and the dots match it: one round dot per story card, then a separate arrow marker for
// the action card (label "직접 해 보기"). 건너뛰기 is on every card but the last. The card is as tall as its content
// is one fixed height (the tallest card) with the bottom bar pinned to its foot, so 다음 stays in the same place. The last card opens the composer and never starts anything.
import { ArrowRight, Check, ChevronLeft, Copy, X } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import type { Status } from "../lib/api";
import { INSTALL_COMMAND } from "../lib/install";
import { restoreFocus } from "../lib/onboarding";
import { SETUP_LINKS, tourGoalExamples, type TourCard } from "../lib/tour";
import { useT } from "../i18n/react";
import type { NavTarget as Page } from "./Sidebar"; // old ids are normalized by App (nav-ia)
import { TourDemo } from "./TourDemos";
import { Button } from "./ui";
import { COPY, startLogin, useAuth } from "../lib/auth";

const NO_ADVANCE = "button, a, input, select, textarea, [data-no-advance]";
const FOCUSABLE = 'button:not([disabled]), a[href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

export function TourDeck({
  cards,
  status,
  onClose,
  onNavigate,
  onStart,
}: {
  cards: TourCard[];
  status?: Status;
  onClose: (how: "done" | "dismissed") => void;
  onNavigate: (p: Page) => void;
  /** Last card: close the deck and open the composer (empty goal, example chips, refine highlighted). */
  onStart: () => void;
}) {
  const t = useT();
  const [i, setI] = useState(0);
  const [copied, setCopied] = useState(false);
  const dialog = useRef<HTMLDivElement>(null);
  const card = cards[Math.min(i, cards.length - 1)];
  const last = i >= cards.length - 1;
  const story = cards.filter((c) => c.id !== "start");
  const titleId = "tour-title";

  // Focus moves into the dialog on open and back to the opener on close. The opener is remembered by the store
  // before the shell turns inert; this cleanup runs after the inert attribute is gone.
  useEffect(() => {
    dialog.current?.focus();
    return () => restoreFocus();
  }, []);

  const next = () => (last ? undefined : setI(i + 1));
  const prev = () => setI(Math.max(0, i - 1));
  const dismiss = () => onClose("dismissed");
  const open = (p: Page) => {
    onClose("done");
    onNavigate(p);
  };

  const onCardClick = (e: MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest(NO_ADVANCE)) return;
    if (window.getSelection()?.toString()) return; // the user is selecting text, not flipping cards
    next();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const onControl = (e.target as HTMLElement).closest(NO_ADVANCE) !== null;
    if (e.key === "Escape") {
      e.preventDefault();
      dismiss();
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      next();
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      prev();
    } else if (e.key === " " && !onControl) {
      e.preventDefault();
      next();
    } else if (e.key === "Tab") {
      // Keep Tab / Shift+Tab inside the dialog.
      const items = [...(dialog.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])];
      if (!items.length) return;
      const first = items[0];
      const end = items[items.length - 1];
      const at = document.activeElement;
      if (e.shiftKey && (at === first || at === dialog.current)) {
        e.preventDefault();
        end.focus();
      } else if (!e.shiftKey && at === end) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  const copyInstall = async () => {
    try {
      await navigator.clipboard.writeText(INSTALL_COMMAND);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  const isStart = card.id === "start";
  const cliMissing = isStart && status !== undefined && !status.cli;
  // Same spot, next step: the CLI is there but logged out (claude-login).
  const loggedOut = useAuth().view?.login === "loggedOut";
  const loginMissing = isStart && status !== undefined && !!status.cli && loggedOut;
  const n = story.indexOf(card) + 1;
  const progress = isStart ? t("tour.deck.actionProgress") : `${n}/${story.length}`;

  return (
    <div className="fixed inset-0 z-50 grid justify-items-center items-start overflow-auto bg-scrim/45 p-4 pt-[12vh] backdrop-blur-[2px]">
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onKeyDown={onKeyDown}
        onClick={onCardClick}
        data-testid="tour-card"
        className="flex h-[480px] max-h-[84vh] w-full max-w-[840px] cursor-pointer flex-col overflow-hidden rounded-2xl border border-line bg-sheet text-ink shadow-2xl outline-none"
      >
        <div key={card.id} className="tour-fade grid min-h-0 flex-1 gap-5 overflow-auto p-6 md:grid-cols-[minmax(0,1fr)_420px] md:items-center">
          <div className="select-text">
            <p aria-hidden="true" className="mb-2 text-[11px] font-medium tracking-wide text-ink-3 tabular-nums">
              {progress}
            </p>
            <h2 id={titleId} className="text-[22px] leading-snug font-semibold tracking-tight break-keep text-balance">
              {card.title}
            </h2>
            <p className="mt-2 text-[14px] leading-relaxed text-ink-2 break-keep">{card.body}</p>
            <ul className="mt-5 space-y-2.5" data-testid="tour-points">
              {card.points.map((pt) => (
                <li key={pt} className="flex items-center gap-2.5 text-[13px] text-ink">
                  <span aria-hidden="true" className="grid h-5 w-5 shrink-0 place-items-center rounded-full border border-line bg-paper text-ink-2">
                    <Check size={12} strokeWidth={2.5} />
                  </span>
                  {pt}
                </li>
              ))}
            </ul>
            {i === 0 && cards.length > 1 && <p className="mt-5 text-[11px] text-ink-3">{t("tour.deck.hint")}</p>}
            {isStart && (
              <p className="mt-4 text-xs text-ink-3" data-no-advance>
                {t("tour.setup.lead")}{" "}
                {SETUP_LINKS.map((l, k) => (
                  <span key={l.page}>
                    {k > 0 && <span aria-hidden="true"> · </span>}
                    <button onClick={() => open(l.page)} className="rounded underline-offset-2 hover:text-ink hover:underline">
                      {l.label}
                    </button>
                  </span>
                ))}
              </p>
            )}
            {cliMissing && (
              <p className="mt-3 flex flex-wrap items-center gap-2 text-xs text-ink-2">
                <span className="h-1.5 w-1.5 rounded-full bg-danger" aria-hidden="true" />
                {t("tour.deck.cliMissing")}
                <Button size="sm" onClick={copyInstall}>
                  <Copy size={12} aria-hidden="true" />
                  {copied ? t("tour.deck.copied") : t("tour.deck.copyInstall")}
                </Button>
              </p>
            )}
            {loginMissing && (
              <p className="mt-3 flex flex-wrap items-center gap-2 text-xs text-ink-2" data-testid="tour-login">
                <span className="h-1.5 w-1.5 rounded-full bg-danger" aria-hidden="true" />
                {COPY.gate}
                <Button size="sm" onClick={() => void startLogin("claudeAi")}>
                  {t("auth.actions.login")}
                </Button>
              </p>
            )}
          </div>
          <div className="grid min-h-0 justify-items-center">
            <div inert aria-hidden="true" className="flex w-full justify-center" data-testid="tour-demo">
              <TourDemo id={card.id} examples={tourGoalExamples()} />
            </div>
            <p className="sr-only">{card.demoAlt}</p>
          </div>
        </div>

        <div className="flex shrink-0 flex-wrap items-center gap-2 border-t border-line px-6 py-3" data-testid="tour-bar">
          {i > 0 && (
            <Button onClick={prev} aria-label={t("tour.deck.prevCard")}>
              <ChevronLeft size={14} aria-hidden="true" />
              {t("tour.deck.prev")}
            </Button>
          )}
          {!last && (
            <button onClick={dismiss} className="h-8 px-2 text-[13px] text-ink-2 hover:text-ink">
              {t("tour.deck.skip")}
            </button>
          )}
          <span className="flex-1" />
          {cards.length > 1 && (
            <div className="flex items-center gap-1.5" data-no-advance>
              {cards.map((c, k) => {
                const action = c.id === "start";
                const on = k === i;
                return action ? (
                  <button
                    key={c.id}
                    onClick={() => setI(k)}
                    aria-label={t("tour.deck.actionDot", { title: c.title })}
                    aria-current={on ? "step" : undefined}
                    data-progress="action"
                    className={`ml-1.5 grid h-4 w-4 place-items-center rounded-full border transition-colors ${on ? "border-ink bg-ink text-sheet" : "border-ink-3/60 text-ink-3 hover:border-ink-3 hover:text-ink"}`}
                  >
                    <ArrowRight size={10} strokeWidth={2.5} aria-hidden="true" />
                  </button>
                ) : (
                  <button
                    key={c.id}
                    onClick={() => setI(k)}
                    aria-label={t("tour.deck.storyDot", { n: story.indexOf(c) + 1, total: story.length, title: c.title })}
                    aria-current={on ? "step" : undefined}
                    data-progress="story"
                    className={`h-2 rounded-full transition-all ${on ? "w-5 bg-ink" : "w-2 bg-ink-3/35 hover:bg-ink-3"}`}
                  />
                );
              })}
            </div>
          )}
          <span className="flex-1" />
          {last ? (
            isStart ? (
              <>
                <Button onClick={() => onClose("done")}>{t("tour.deck.later")}</Button>
                <Button variant="primary" onClick={onStart}>
                  {t("tour.deck.start")}
                  <ArrowRight size={14} aria-hidden="true" />
                </Button>
              </>
            ) : (
              <Button variant="primary" onClick={() => onClose("done")}>
                {t("tour.deck.ok")}
              </Button>
            )
          ) : (
            <Button variant="primary" onClick={next}>
              {t("tour.deck.next")}
              <ArrowRight size={14} aria-hidden="true" />
            </Button>
          )}
        </div>
        <p aria-live="polite" className="sr-only">
          {isStart ? t("tour.deck.liveAction", { progress }) : t("tour.deck.liveStory", { progress })}
        </p>
      </div>
    </div>
  );
}

/** One-time offer for someone who saw an older tour (tour-v2): never opens the deck by itself. */
export function TourToast({ onOpen, onClose }: { onOpen: () => void; onClose: () => void }) {
  const t = useT();
  return (
    <div role="status" className="fixed right-5 bottom-5 z-40 flex items-center gap-2 rounded-xl bg-sheet shadow-e3 py-2 pr-2 pl-4 text-[13px] text-ink">
      <span>{t("tour.toast.title")}</span>
      <Button variant="primary" size="sm" onClick={onOpen}>
        {t("tour.toast.open")}
      </Button>
      <button aria-label={t("common.actions.close")} onClick={onClose} className="rounded-md p-1 text-ink-3 hover:text-ink">
        <X size={14} />
      </button>
    </div>
  );
}
