// [전역 | <project ▾>] bar under the page title (project-switcher, D-127). Same place, height and look on every tab that
// picks a project (permissions, memory, extensions, MCP); each tab keeps its own project. The project side opens the
// picker popover right there; picking a project switches to the project side, "프로젝트 없음" back to global. Where
// this screen cannot use the picked project (home, unknown folder) the side is greyed with one line of reason.
import { ChevronsUpDown, Folder } from "lucide-react";
import { useCallback, useRef, type ReactNode } from "react";
import type { ScopeNeed } from "../lib/projects";
import { ProjectPopover } from "./ProjectPicker";
import { useProject, useScope, type Scope } from "./ProjectContext";
import { useT } from "../i18n/react";

/**
 * `value`/`onChange`: a screen-local choice instead of the shared one — 확장 and MCP show everything by default with the
 * current project marked, and their project side only narrows the list ([전체 | 이름만]).
 */
export function ScopeBar({
  need,
  globalLabel,
  projectSuffix = "",
  note,
  children,
  value,
  onChange,
}: {
  need: ScopeNeed;
  globalLabel?: string;
  projectSuffix?: string;
  note?: ReactNode;
  children?: ReactNode;
  value?: Scope;
  onChange?: (s: Scope) => void;
}) {
  const t = useT();
  const { current, setScope: setShared, pickerOpen, setPickerOpen } = useProject();
  const shared = useScope(need);
  const off = shared.off;
  const scope = value !== undefined ? (value === "project" && current && !off ? "project" : "global") : shared.scope;
  const setScope = onChange ?? setShared;
  const trigger = useRef<HTMLButtonElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const onClose = useCallback(() => setPickerOpen(false), [setPickerOpen]);
  const disabled = !!current && !!off;
  return (
    <div className="flex min-h-9 flex-wrap items-center gap-x-3 gap-y-1">
      <div className="relative">
        <div
          role="radiogroup"
          aria-label={t("tasks.scope.label")}
          className="inline-flex rounded-lg border border-line bg-sheet p-0.5"
          onKeyDown={(e) => {
            if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
            e.preventDefault();
            const next = scope === "global" ? "project" : "global";
            if (next === "project" && (!current || off)) return;
            setScope(next);
            (e.currentTarget.querySelector(`[data-id="${next}"]`) as HTMLElement | null)?.focus();
          }}
        >
          <button
            data-id="global"
            role="radio"
            aria-checked={scope === "global"}
            tabIndex={scope === "global" ? 0 : -1}
            onClick={() => setScope("global")}
            className={`inline-flex h-7 items-center rounded-md px-2.5 text-[13px] transition-colors ${scope === "global" ? "bg-ink font-medium text-paper" : "text-ink-2 hover:text-ink"}`}
          >
            {globalLabel ?? t("tasks.scope.global")}
          </button>
          <button
            ref={trigger}
            data-id="project"
            role="radio"
            aria-checked={scope === "project"}
            aria-haspopup="listbox"
            aria-expanded={pickerOpen}
            aria-disabled={disabled || undefined}
            tabIndex={scope === "project" ? 0 : -1}
            title={disabled ? (off ?? undefined) : t("tasks.scope.switch")}
            onClick={() => setPickerOpen(!pickerOpen)}
            className={`inline-flex h-7 max-w-[18rem] items-center gap-1.5 rounded-md px-2.5 text-[13px] transition-colors ${scope === "project" ? "bg-ink font-medium text-paper" : disabled ? "text-ink-3" : "text-ink-2 hover:text-ink"}`}
          >
            <Folder size={13} strokeWidth={1.8} className="shrink-0" />
            <span className="truncate">{current ? `${current.name}${projectSuffix}` : t("tasks.scope.pick")}</span>
            <ChevronsUpDown size={12} className="shrink-0 opacity-70" />
          </button>
        </div>
        {pickerOpen && <ProjectPopover onClose={onClose} onPicked={(id) => setScope(id ? "project" : "global")} boxRef={box} buttonRef={trigger} />}
      </div>
      {current && off ? <span className="text-xs text-ink-3">{off}</span> : note ? <span className="text-xs text-ink-3">{note}</span> : null}
      {children}
    </div>
  );
}
