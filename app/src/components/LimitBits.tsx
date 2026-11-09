// Usage-limit banner and card chip for the tasks board (usage-limit-resume).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Hourglass } from "lucide-react";
import { api, type AgentSession, type LimitEntry } from "../lib/api";
import { bannerLines, bannerTime, entryFor, limitChip, resumeLabels } from "../lib/limits";
import { Button } from "./ui";
import { gateFor, useAuth } from "../lib/auth";
import { GateLine } from "./LoginBits";
import { useT } from "../i18n/react";

export function useLimits() {
  return useQuery({ queryKey: ["limits"], queryFn: api.limitStatus, refetchInterval: 30_000 });
}

function useLimitActions() {
  const qc = useQueryClient();
  // A cancelled or started resume may unblock 정리 끝 (D-191): the open checklist reads again too.
  const done = () => Promise.all([qc.invalidateQueries({ queryKey: ["limits"] }), qc.invalidateQueries({ queryKey: ["wrapup-check"] })]);
  const resume = useMutation({ mutationFn: (id: number) => api.limitResumeNow(id), onSettled: done });
  const skip = useMutation({ mutationFn: (id: number) => api.limitCancel(id), onSettled: done });
  return { resume, skip };
}

/** One line with a time in it: the time stays whole and the words after it give way first, then the words before it
 * (an English line is longer; "resets 1:08 A…" hid the one fact the line is for). Full text on hover. */
export function TimedLine({ text, when, className = "", wrap = false }: { text: string; when?: string; className?: string; wrap?: boolean }) {
  const at = when ? text.indexOf(when) : -1;
  if (!when || at < 0)
    return (
      <span className={`block ${wrap ? "line-clamp-2" : "truncate"} ${className}`} title={text}>
        {text}
      </span>
    );
  const before = text.slice(0, at);
  const after = text.slice(at + when.length);
  // On a card: up to two lines, the time never split across them.
  if (wrap)
    return (
      <span className={`line-clamp-2 ${className}`} title={text}>
        {before}
        <span className="whitespace-nowrap" data-limit-time>
          {when}
        </span>
        {after}
      </span>
    );
  return (
    <span className={`flex min-w-0 whitespace-pre ${className}`} title={text}>
      {before && <span className="min-w-0 overflow-hidden text-ellipsis">{before}</span>}
      <span className="shrink-0" data-limit-time>
        {when}
      </span>
      {after && <span className="min-w-0 shrink-[100] overflow-hidden text-ellipsis">{after}</span>}
    </span>
  );
}

/** One line per fact above the board; late wake-ups get "지금 이어서 / 이번엔 안 이어요" per task (named, never an id). */
export function LimitBanner() {
  const t = useT();
  const view = useLimits().data;
  const { resume, skip } = useLimitActions();
  const auth = useAuth().view;
  const lines = bannerLines(view);
  // Logged out: the app-wide login banner is the only one (claude-login); this one comes back after login.
  if (lines.length === 0 || auth?.login === "loggedOut") return null;
  const late = (view?.entries ?? []).filter((e) => e.state === "needs_choice" && e.bgId);
  const labels = resumeLabels(late);
  const time = bannerTime(view);
  return (
    <div role="status" className="flex items-start gap-2.5 rounded-xl bg-sheet shadow-e1 px-4 py-3 text-[13px] text-ink">
      <Hourglass size={15} aria-hidden className="mt-0.5 shrink-0 text-ink-2" />
      <div className="min-w-0 flex-1 space-y-1.5">
        {/* Two lines at most each, full text on hover: in the 900px board beside the drawer a wrapped line took five
            rows (3.2), and one line hid the reset time in English. */}
        {lines.map((l, i) => (
          <p key={l} className="min-w-0">
            <TimedLine text={l} when={i === 0 ? time : undefined} wrap />
          </p>
        ))}
        {late.length > 0 && (
          <div className="flex flex-wrap gap-1.5 pt-0.5">
            {late.map((e) => (
              <span key={e.id} className="inline-flex gap-1.5">
                <Button size="sm" disabled={resume.isPending} onClick={() => resume.mutate(e.id)}>
                  {labels.get(e.id)}
                </Button>
                <Button size="sm" disabled={skip.isPending} onClick={() => skip.mutate(e.id)}>
                  {t("tasks.limits.skip")}
                </Button>
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

const TONE = { wait: "text-ink-2", manual: "text-ink", done: "text-ok" };

/** Card buttons: 12px on a 28px row with 8px sides, so "지금 이어서" and "이번엔 안 이어요" share one line in a 1440
 * column (palette-refresh 2.3, 3.2 review). A Button size, not a class list. */
export const CARD_BUTTON = "compact" as const;

/** The usage-limit state of one card's session (usage-limit-resume); `null` when it did not stop at a limit. */
export function useLimitCard(s: AgentSession, entry?: LimitEntry) {
  const view = useLimits().data;
  const e = entry ?? entryFor(view, s);
  const chip = limitChip(e);
  const actions = useLimitActions();
  const blocked = gateFor(useAuth().view, "model") !== null;
  if (!e || !chip) return null;
  const canResume = chip.canResume && !blocked;
  return { e, chip, blocked, canResume, canSkip: chip.canSkip, hasActions: canResume || chip.canSkip || (chip.canResume && blocked) || !!chip.note, ...actions };
}

export type LimitCard = NonNullable<ReturnType<typeof useLimitCard>>;

/** Warning line on the card face: hourglass and up to two 12px lines (an English line is longer). */
export function LimitCardLine({ limit }: { limit: LimitCard }) {
  return (
    <p className={`mt-1.5 flex min-w-0 items-start gap-1.5 text-[12px] ${TONE[limit.chip.tone]}`} title={limit.chip.text}>
      <Hourglass size={12} aria-hidden className="mt-[3px] shrink-0" />
      <TimedLine text={limit.chip.text} when={limit.chip.when} className="min-w-0" wrap />
    </p>
  );
}

/** Limit buttons for the card's action row; "지금 이어서" is the card's one primary action. */
export function LimitCardButtons({ limit }: { limit: LimitCard }) {
  const t = useT();
  const { e, resume, skip } = limit;
  return (
    <>
      {limit.canResume && (
        <Button variant="primary" size={CARD_BUTTON} disabled={resume.isPending} onClick={() => resume.mutate(e.id)}>
          {t("tasks.limits.resume")}
        </Button>
      )}
      {limit.canSkip && (
        <Button size={CARD_BUTTON} disabled={skip.isPending} onClick={() => skip.mutate(e.id)}>
          {t("tasks.limits.skip")}
        </Button>
      )}
    </>
  );
}

/** Under the action row: the login gate, or the limit's note in up to two lines (full text on hover). */
export function LimitCardNote({ limit }: { limit: LimitCard }) {
  if (limit.chip.canResume && limit.blocked) return <GateLine />;
  if (!limit.chip.note) return null;
  return (
    <p className="mt-1.5 line-clamp-2 text-[12px] text-ink-3" title={limit.chip.note}>
      {limit.chip.note}
    </p>
  );
}
