// Mini demos for the tour cards (tour-v2). Each one is a cut-out of a real screen at its real size (never a scaled-down
// page): the same display-only pieces the screens use (OptionLine, CriterionLine, Mark, RoleChip, typeChipClass,
// statusLine) with sample data written here. Nothing runs, nothing is fetched (TourDemos.test.tsx mocks the api module
// to throw), and the deck wraps every demo in `inert` + `aria-hidden` with the card's `demoAlt` as its text.
// Only the board demo moves (the working and waiting ducks); the others carry `tour-still`.
import { Bell, Sparkles } from "lucide-react";
import type { ReactNode } from "react";
import rawBundle from "../../crates/dduck-core/data/team_defaults.json";
import { pickLang } from "../lib/bundled";
import { statusLine, type Pose } from "../lib/agents";
import type { CriterionRow } from "../lib/report";
import { roleColor } from "../lib/roleColors";
import { YELLOW } from "../lib/yellow";
import { currentLang, t } from "../i18n";
import { tx } from "../i18n/react";
import { ROLE_STYLE, type CardId } from "../lib/tour";
import { OptionLine, questionBox } from "./BriefRefiner";
import { typeChipClass } from "./Composer";
import { DuckAvatar } from "./DuckAvatar";
import { RoleChip, RoleDuck } from "./RoleDuck";
import { CriterionLine, Mark } from "./TaskReport";

// Bundled text fields are {ko, en} pairs (app-language), picked at render time in the current language.
type Preset = { key: string; label: string; mode: string; effort: string };
const preset = (key: string): Preset =>
  (pickLang(rawBundle.types, currentLang()) as Preset[]).find((t) => t.key === key) ?? { key, label: key, mode: "manual", effort: "medium" };
/** Role names in the current language (the same words as the preset labels in Korean). */
const roleName = (key: string): string => (isDemoRole(key) ? t(`tour.roles.${key}`) : preset(key).label);

/** The roles card's five ducks, in the order a task usually moves through them. */
export const ROLE_KEYS = ["architecture", "design", "dev", "review", "qa"] as const;
const isDemoRole = (key: string): key is (typeof ROLE_KEYS)[number] => (ROLE_KEYS as readonly string[]).includes(key);

const Panel = ({ children, still = true }: { children: ReactNode; still?: boolean }) => (
  <div className={`w-full max-w-[420px] space-y-2 rounded-xl bg-sheet shadow-e1 p-3 text-left text-ink ${still ? "tour-still" : ""}`}>{children}</div>
);

const Tag = ({ children }: { children: ReactNode }) => <span className="rounded-full bg-paper px-2 py-0.5 text-[11px] font-medium whitespace-nowrap text-ink-2">{children}</span>;

/** Turns from one long session: the work drifts and the context gets asked for again. */
const soloLog = (): [number, string][] => [
  [12, t("tour.demo.solo.turn1")],
  [57, t("tour.demo.solo.turn2")],
  [96, t("tour.demo.solo.turn3")],
  [141, t("tour.demo.solo.turn4")],
];

/** ① One duck with no role doing every kind of work in one long session. */
function SoloDemo() {
  return (
    <Panel>
      <div className="flex items-center gap-4 py-2">
        <RoleDuck taskType={null} label={t("tour.demo.solo.noRole")} pose="working" size={84} />
        <div className="min-w-0 flex-1 space-y-2">
          <p className="text-xs text-ink-3">{t("tour.demo.solo.meta")}</p>
          <div className="flex flex-wrap gap-1.5">
            {ROLE_KEYS.map((k) => (
              <Tag key={k}>{roleName(k)}</Tag>
            ))}
          </div>
        </div>
      </div>
      <ul className="space-y-1 border-t border-line pt-2" data-solo-log>
        {soloLog().map(([turn, text]) => (
          <li key={turn} className="flex items-baseline gap-2 text-[12px]">
            <span className="w-9 shrink-0 text-right text-[11px] text-ink-3 tabular-nums">#{turn}</span>
            <span className="min-w-0 truncate rounded-lg bg-paper px-2 py-1 text-ink-2">{text}</span>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-1.5 text-[11px]">
        <Tag>{t("tour.demo.solo.allowAll")}</Tag>
        <Tag>{t("tour.demo.solo.cost")}</Tag>
        <Tag>{t("tour.demo.solo.noDone")}</Tag>
      </div>
    </Panel>
  );
}

/** ② The differentiator: awake ducks with roles (working pose, eyes open, 56px so the role item reads), each with
 *  how that role works (ROLE_STYLE). The permission presets themselves are not shown here: dev keeps plan mode. */
function RolesDemo() {
  return (
    <Panel>
      <ul className="divide-y divide-line">
        {ROLE_KEYS.map((k) => {
          const label = roleName(k);
          return (
            <li key={k} className="flex items-center gap-3 py-1 first:pt-0 last:pb-0" data-role={k}>
              <span className="rounded-full bg-duck-stage-quiet p-0.5">
                <DuckAvatar pose="working" size={56} color={roleColor(k)} label={t("tour.demo.duck", { role: label })} />
              </span>
              <span className="w-12 shrink-0">
                <RoleChip label={label} color={roleColor(k)} />
              </span>
              <Tag>{ROLE_STYLE[k]}</Tag>
            </li>
          );
        })}
      </ul>
    </Panel>
  );
}


/** A still radio mark for OptionLine (the real one is an input). */
const Dot = ({ on }: { on: boolean }) => (
  <span className={`mt-1 grid h-3.5 w-3.5 shrink-0 place-items-center rounded-full border ${on ? "border-ink" : "border-ink-3"}`}>{on && <span className="h-2 w-2 rounded-full bg-ink" />}</span>
);

/** ③ Pick a type (it sets the role), then one refine question; the picked answer fills one line of the brief. */
function BriefDemo() {
  /** The recommended answer, and the brief line it fills. */
  const filled = t("tour.demo.brief.filledDone");
  return (
    <Panel>
      <div className="flex flex-wrap gap-1.5">
        {ROLE_KEYS.map((k) => (
          <span key={k} className={typeChipClass(k === "dev")}>
            <span className="inline-flex h-full items-center">{roleName(k)}</span>
          </span>
        ))}
      </div>
      <p className="text-xs text-ink-3">
        {t("tour.demo.brief.roleLine", { role: roleName("dev"), style: ROLE_STYLE.dev })}
      </p>
      <div className={questionBox()}>
        <p className="text-[13px] font-medium">{t("tour.demo.brief.question")}</p>
        <div className="mt-2 space-y-1">
          <OptionLine still label={filled} detail={t("tour.demo.brief.filledDetail")} recommended checked control={<Dot on />} />
          <OptionLine still label={t("tour.demo.brief.ciOnce")} recommended={false} checked={false} control={<Dot on={false} />} />
        </div>
      </div>
      <div className="space-y-1 rounded-lg border border-line bg-paper px-3 py-2 text-[13px]" data-brief-line>
        <p className="text-[11px] font-medium text-ink-3">{t("tour.demo.brief.title")}</p>
        <p className="min-w-0 truncate text-ink-2">{t("tour.demo.brief.goal")}</p>
        <p className="min-w-0 truncate text-ink-2">{t("tour.demo.brief.scope")}</p>
        <p className="min-w-0 truncate">
          {tx("tour.demo.brief.doneLine", { value: <span className="rounded bg-ink/5 px-1 font-medium">{filled}</span> })}
        </p>
      </div>
    </Panel>
  );
}

const BOARD: { key: "dev" | "review" | "qa"; pose: Pose }[] = [
  { key: "dev", pose: "working" },
  { key: "review", pose: "waiting" },
  { key: "qa", pose: "done" },
];
const STATE_OF: Record<string, "working" | "waiting" | "done"> = { working: "working", waiting: "waiting", done: "done" };

/** ④ Who is doing what: one line per duck, the same status line the agents page writes. Waiting is the highlight. */
function BoardDemo() {
  return (
    <Panel still={false}>
      <div className="flex items-center justify-between text-[11px] text-ink-3">
        <span>{t("tour.demo.board.summary")}</span>
        <span className="flex gap-1.5">
          <Tag>web-app</Tag>
          <Tag>api</Tag>
        </span>
      </div>
      <ul className="space-y-1.5">
        {BOARD.map((b) => {
          const label = roleName(b.key);
          const line = statusLine({ agent: b.key, state: STATE_OF[b.pose], count: 1, total: 1, detail: t(`tour.demo.board.${b.key}`), sessionKey: null, subagent: false });
          return (
            <li key={b.key} className={`flex items-center gap-2.5 rounded-lg border px-2 py-1.5 ${b.pose === "waiting" ? `border-transparent ${YELLOW.waitingCard}` : "border-line"}`}>
              <span className="rounded-full bg-duck-stage-quiet p-0.5">
                <DuckAvatar pose={line.pose} size={40} color={roleColor(b.key)} label={t("tour.demo.duckStatus", { role: label, head: line.head })} />
              </span>
              <span className="w-12 shrink-0">
                <RoleChip label={label} color={roleColor(b.key)} />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] font-semibold">{line.head}</span>
                <span className="block truncate text-[11px] text-ink-2">{line.detail}</span>
              </span>
            </li>
          );
        })}
      </ul>
      <div className="flex items-center gap-2 rounded-lg border border-line bg-paper px-3 py-2 text-[12px] text-ink" data-board-notice>
        <Bell size={13} aria-hidden className="shrink-0" />
        <span className="min-w-0 flex-1 truncate">{t("tour.demo.board.notice")}</span>
      </div>
    </Panel>
  );
}

/** Two met and one partly met: the first impression is a finished job with one thing to look at, not a failure. */
export const REPORT_CRITERIA: CriterionRow[] = ([
  ["c1", "yes"],
  ["c2", "yes"],
  ["c3", "partial"],
] as const).map(([id, met]) => ({
  met,
  agreed: true,
  get criterion() {
    return t(`tour.demo.report.${id}`);
  },
  get evidence() {
    return t(`tour.demo.report.${id}Evidence`);
  },
}));
const CRITERIA = REPORT_CRITERIA;
const MET_COUNT = CRITERIA.filter((c) => c.met === "yes").length;

/** Report demo section label, the same words the report uses. */
const Section = ({ children }: { children: ReactNode }) => <p className="text-[11px] font-medium text-ink-3">{children}</p>;

/** ⑤ The duck's own report in the report's order: what was agreed (맡긴 내용), each done criterion checked, then the
 *  one thing it assumed without asking. */
function ReportDemo() {
  return (
    <Panel>
      <div className="flex items-center gap-2.5 border-b border-line pb-2">
        <RoleDuck taskType="dev" label={roleName("dev")} pose="done" size={40} />
        <span className="min-w-0">
          <span className="block truncate text-[11px] text-ink-3">{t("tour.demo.report.header")}</span>
          <span className="block text-[13px] font-semibold">
            {t("tour.demo.report.met", { total: CRITERIA.length, met: MET_COUNT })}
          </span>
        </span>
      </div>
      <div className="space-y-1" data-report-section="delegated">
        <Section>{t("tour.demo.report.delegated")}</Section>
        <div className="flex flex-wrap items-center gap-1.5 text-[13px]">
          <Tag>{roleName("dev")}</Tag>
          <span className="min-w-0 truncate">{t("tour.demo.report.goal")}</span>
        </div>
      </div>
      <Section>{t("tour.demo.report.criteria")}</Section>
      <ul className="divide-y divide-line" data-report-section="criteria">
        {CRITERIA.map((c) => (
          <li key={c.criterion} className="flex items-start gap-2.5 py-1.5 first:pt-0 last:pb-0">
            <CriterionLine c={c} />
          </li>
        ))}
      </ul>
      <span className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-line bg-paper px-2.5 py-1 text-xs text-ink-2" data-report-section="caution">
        <Mark met="partial" small />
        <span className="shrink-0 font-semibold">{t("tour.demo.report.assumedTag")}</span>
        <span className="truncate">{t("tour.demo.report.assumed")}</span>
      </span>
      <div className="flex items-center gap-2 border-t border-line pt-2 text-xs" data-report-section="verify">
        <RoleChip label={roleName("review")} color={roleColor("review")} />
        <span className="min-w-0 truncate text-ink-2">{t("tour.demo.report.verdict")}</span>
      </div>
    </Panel>
  );
}

/** Action card: what the button opens. A waving duck that has no role yet next to the type chips (picking one gives it
 *  the role), an empty goal with example chips and the refine button highlighted. */
function StartDemo({ examples }: { examples: string[] }) {
  return (
    <Panel>
      <div className="flex items-center gap-3">
        <RoleDuck taskType={null} label={t("tour.demo.start.noRole")} pose="waiting" size={56} />
        <div className="min-w-0 flex-1 space-y-1.5">
          <p className="text-xs text-ink-3">{t("tour.demo.start.typeHint")}</p>
          <div className="flex flex-wrap gap-1.5">
            {ROLE_KEYS.map((k) => (
              <span key={k} className={typeChipClass(false)}>
                <span className="inline-flex h-full items-center">{roleName(k)}</span>
              </span>
            ))}
          </div>
        </div>
      </div>
      <div className="space-y-1">
        <p className="text-xs font-medium text-ink-2">{t("tour.demo.start.goalLabel")}</p>
        <div className="h-9 rounded-lg border border-line bg-paper px-3 py-2 text-[13px] text-ink-3">{t("tour.demo.start.goalPlaceholder")}</div>
        <div className="flex flex-wrap gap-1.5">
          {examples.slice(0, 2).map((g) => (
            <span key={g} className="h-7 truncate rounded-full border border-line bg-sheet px-2.5 text-[12px] leading-[26px] text-ink-2">
              {g}
            </span>
          ))}
        </div>
      </div>
      <div className="flex items-center gap-2 rounded-lg border border-line bg-paper px-3 py-2 text-xs">
        <Sparkles size={13} aria-hidden className="shrink-0" />
        <span className="flex-1">{t("tour.demo.start.refine")}</span>
        <span className="text-ink-3">{t("tour.demo.start.beforeRun")}</span>
      </div>
    </Panel>
  );
}

export function TourDemo({ id, examples = [] }: { id: CardId; examples?: string[] }) {
  switch (id) {
    case "solo":
      return <SoloDemo />;
    case "roles":
      return <RolesDemo />;
    case "brief":
      return <BriefDemo />;
    case "board":
      return <BoardDemo />;
    case "report":
      return <ReportDemo />;
    case "start":
      return <StartDemo examples={examples} />;
  }
}
