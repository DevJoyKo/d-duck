import { BarChart3, Bot, Brain, CalendarClock, CircleHelp, ClipboardCheck, Layers, Monitor, Moon, Newspaper, Plug, Puzzle, Settings, ShieldCheck, SquareTerminal, Sun } from "lucide-react";
import { useState } from "react";
import { applyTheme, loadTheme, NEXT_THEME, type Theme } from "../lib/theme";
import { useT } from "../i18n/react";
import { LanguageMenu } from "./LanguageMenu";
import type { ReactNode } from "react";
import { DuckAvatar } from "./DuckAvatar";
import { YELLOW } from "../lib/yellow";
import type { IngestStatus, Status } from "../lib/api";
import { openTour } from "../lib/onboarding";
import { badgeLabel as BADGE } from "../lib/review";

import type { Page } from "../lib/nav";
export type { NavTarget, Page } from "../lib/nav";

function NavItem({ active, onClick, icon, children, badge, badgeLabel, badgeTone = "duck" }: { active: boolean; onClick: () => void; icon: ReactNode; children: ReactNode; badge?: number; badgeLabel?: (n: number) => string; badgeTone?: "duck" | "neutral" }) {
  return (
    <button
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={`relative flex h-8 w-full items-center gap-2.5 rounded-lg px-3 text-[13px] transition-colors ${active ? "bg-side-active font-medium text-ink" : "text-side-ink hover:bg-side-active/50 hover:text-ink"}`}
    >
      {active && <span aria-hidden className="absolute top-2 bottom-2 left-0 w-[3px] rounded-full bg-ink-3" />}
      {icon}
      <span className="flex-1 text-left">{children}</span>
      {badge ? (
        <span aria-label={(badgeLabel ?? BADGE.tasks)(badge)} className={`min-w-5 rounded-full px-1.5 text-center text-[12px] font-semibold ${badgeTone === "duck" ? `${YELLOW.badge} text-ink-fixed` : active ? "text-ink" : "text-side-ink"}`}>
          {badge}
        </span>
      ) : null}
    </button>
  );
}

function NavGroup({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={label}>
      <p className="mb-1 px-3 text-[11px] font-semibold tracking-wide text-side-ink">{label}</p>
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

function ThemeButton() {
  const t = useT();
  const [theme, setTheme] = useState<Theme>(loadTheme);
  const Icon = theme === "light" ? Sun : theme === "dark" ? Moon : Monitor;
  const next = NEXT_THEME[theme];
  return (
    <button
      onClick={() => {
        applyTheme(next);
        setTheme(next);
      }}
      aria-label={t("nav.theme.label", { current: t(`common.theme.${theme}`), next: t(`common.theme.${next}`) })}
      title={t("nav.theme.title", { current: t(`common.theme.${theme}`), next: t(`common.theme.${next}`) })}
      className="grid h-7 w-7 place-items-center rounded-md text-side-ink hover:bg-side-active hover:text-ink"
    >
      <Icon size={15} strokeWidth={1.8} />
    </button>
  );
}

export function Sidebar({
  page,
  onNavigate,
  waiting,
  reviewPending,
  status,
  ingest,
  checklist,
}: {
  page: Page;
  onNavigate: (p: Page) => void;
  waiting: number;
  /** "리뷰 전" count (review-inbox); undefined while unknown. */
  reviewPending?: number;
  status?: Status;
  ingest: IngestStatus | null;
  /** The "시작하기" pill after the tour, if any. */
  checklist?: ReactNode;
}) {
  const t = useT();
  return (
    <nav className="relative flex w-[216px] shrink-0 flex-col border-r border-line bg-side px-3 pt-11 pb-4 text-side-ink backdrop-blur-[30px]">
      {/* Top strip sits under the macOS traffic lights and drags the window. */}
      <div data-tauri-drag-region className="absolute top-0 left-0 h-10 w-[216px]" />
      <div className="mb-6 flex items-center gap-2.5 px-2">
        <span aria-hidden className="grid h-8 w-8 place-items-center rounded-[10px] bg-sheet shadow-e1">
          <DuckAvatar pose="done" size={26} />
        </span>
        <span className="flex-1 text-[15px] font-bold tracking-tight whitespace-nowrap text-ink">D.Duck</span>
        <LanguageMenu />
        <ThemeButton />
      </div>
      {/* nav-ia: "작업 공간" and "Claude 구성", then usage and settings, then status. Projects are picked per tab (D-127). */}
      <div className="space-y-4">
        <NavGroup label={t("nav.groups.workspace")}>
          <NavItem active={page === "agents"} onClick={() => onNavigate("agents")} icon={<Bot size={16} strokeWidth={1.8} />}>
            {t("nav.pages.agents")}
          </NavItem>
          <NavItem active={page === "tasks"} onClick={() => onNavigate("tasks")} icon={<SquareTerminal size={16} strokeWidth={1.8} />} badge={waiting} badgeLabel={BADGE.tasks} badgeTone="neutral">
            {t("nav.pages.tasks")}
          </NavItem>
          <NavItem active={page === "review"} onClick={() => onNavigate("review")} icon={<ClipboardCheck size={16} strokeWidth={1.8} />} badge={reviewPending} badgeLabel={BADGE.review} badgeTone="neutral">
            {t("nav.pages.review")}
          </NavItem>
          <NavItem active={page === "routines"} onClick={() => onNavigate("routines")} icon={<CalendarClock size={16} strokeWidth={1.8} />}>
            {t("nav.pages.routines")}
          </NavItem>
          <NavItem active={page === "artifacts"} onClick={() => onNavigate("artifacts")} icon={<Layers size={16} strokeWidth={1.8} />}>
            {t("nav.pages.artifacts")}
          </NavItem>
        </NavGroup>
        <NavGroup label={t("nav.groups.claude")}>
          <NavItem active={page === "extensions"} onClick={() => onNavigate("extensions")} icon={<Puzzle size={16} strokeWidth={1.8} />}>
            {t("nav.pages.extensions")}
          </NavItem>
          <NavItem active={page === "mcp"} onClick={() => onNavigate("mcp")} icon={<Plug size={16} strokeWidth={1.8} />}>
            {t("nav.pages.mcp")}
          </NavItem>
          <NavItem active={page === "permissions"} onClick={() => onNavigate("permissions")} icon={<ShieldCheck size={16} strokeWidth={1.8} />}>
            {t("nav.pages.permissions")}
          </NavItem>
          <NavItem active={page === "memory"} onClick={() => onNavigate("memory")} icon={<Brain size={16} strokeWidth={1.8} />}>
            {t("nav.pages.memory")}
          </NavItem>
        </NavGroup>
      </div>
      <div className="mt-auto space-y-3 pt-4">
        <div className="space-y-0.5">
          <NavItem active={page === "news"} onClick={() => onNavigate("news")} icon={<Newspaper size={16} strokeWidth={1.8} />}>
            {t("nav.pages.news")}
          </NavItem>
          <NavItem active={page === "usage"} onClick={() => onNavigate("usage")} icon={<BarChart3 size={16} strokeWidth={1.8} />}>
            {t("nav.pages.usage")}
          </NavItem>
          <NavItem active={page === "settings"} onClick={() => onNavigate("settings")} icon={<Settings size={16} strokeWidth={1.8} />}>
            {t("nav.pages.settings")}
          </NavItem>
        </div>
        {checklist}
        <div className="space-y-1 border-t border-line px-3 pt-3 text-xs">
          <div className="flex items-center gap-2">
            <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${status?.cli ? (status.cli.supported ? "bg-ok" : "bg-warn") : "bg-danger"}`} />
            <span className="flex-1">{status?.cli ? t("nav.cli.found", { version: status.cli.version }) : t("nav.cli.missing")}</span>
            <button
              onClick={openTour}
              aria-label={t("nav.tourAgain")}
              title={t("nav.tourAgain")}
              className="-my-1 grid h-6 w-6 place-items-center rounded-md text-side-ink hover:bg-side-active hover:text-ink"
            >
              <CircleHelp size={14} strokeWidth={1.8} />
            </button>
          </div>
          <div className="text-side-ink">
            {ingest?.phase === "backfill"
                ? t("nav.ingest.backfill", { done: ingest.filesDone, total: ingest.filesTotal })
              : ingest?.phase === "error"
                ? t("nav.ingest.error")
                : t("nav.ingest.fresh")}
          </div>
        </div>
      </div>
    </nav>
  );
}
