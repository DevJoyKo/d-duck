// Project picker popover (project-switcher, D-127): opened from the project side of each tab's ScopeBar, under the
// page title. Search (auto focus), recent first, "프로젝트 없음(전역만)", worktrees folded under their parent
// (→ ← unfold and fold), an "빈 프로젝트" toggle. Combobox + listbox ARIA, ↑↓ Enter; Esc and Tab close it and focus
// goes back to where it was; ⌘P opens it on the open tab.
import {
  Check,
  ChevronRight,
  Lock,
  Search,
} from "lucide-react";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type RefObject,
} from "react";
import { messageOf } from "../lib/api";
import { pickerRows, type KnownProject } from "../lib/projects";
import { useProject } from "./ProjectContext";
import { useT } from "../i18n/react";

export const PICKER_OPEN_EVENT = "dduck-picker-open";

type Option = {
  id: string | null;
  p: KnownProject | null;
  child: boolean;
  children: number;
  section: "none" | "recent" | "all";
};

export function ProjectPopover({
  onClose,
  onPicked,
  boxRef,
  buttonRef,
}: {
  onClose: () => void;
  /** After a pick went through (the tab's scope follows: a project → project side, none → global). */
  onPicked?: (id: string | null) => void;
  boxRef: RefObject<HTMLDivElement | null>;
  buttonRef: RefObject<HTMLButtonElement | null>;
}) {
  const t = useT();
  // Where focus was before opening (⌘P can open it from anywhere); the trigger when that is gone or was the page.
  const [before] = useState(() => (typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null));
  const { list, recent, current, select } = useProject();
  const [query, setQuery] = useState("");
  const [showEmpty, setShowEmpty] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  // The error itself, worded at render time so a later language change applies (D-171).
  const [error, setError] = useState<{ cause: unknown } | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();
  const rows = pickerRows(
    list,
    recent,
    current?.id ?? null,
    query,
    showEmpty,
    expanded,
  );
  const options: Option[] = [
    ...(query.trim()
      ? []
      : [
          {
            id: null,
            p: null,
            child: false,
            children: 0,
            section: "none" as const,
          },
        ]),
    ...rows.map((r) => ({
      id: r.p.id,
      p: r.p,
      child: r.child,
      children: r.children,
      section: r.section,
    })),
  ];
  const [active, setActive] = useState(() =>
    Math.max(
      0,
      options.findIndex((o) => o.id === (current?.id ?? null)),
    ),
  );
  const hidden = list.filter((p) => p.empty).length;

  useEffect(() => {
    window.dispatchEvent(new Event(PICKER_OPEN_EVENT));
    input.current?.focus();
  }, []);
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (
        !boxRef.current?.contains(e.target as Node) &&
        !buttonRef.current?.contains(e.target as Node)
      )
        onClose();
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [onClose, boxRef, buttonRef]);

  const close = () => {
    onClose();
    const back = before && before !== document.body && before.isConnected ? before : buttonRef.current;
    back?.focus();
  };
  const choose = async (id: string | null) => {
    try {
      if (await select(id)) {
        onPicked?.(id);
        close();
      }
    } catch (e) {
      setError({ cause: e });
    }
  };
  const toggleKids = (id: string) =>
    setExpanded((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const optId = (i: number) => `${listId}-o${i}`;

  return (
    <div
      ref={boxRef}
      className="absolute top-full left-0 z-50 mt-1.5 w-80 max-w-[calc(100vw-2rem)] rounded-xl bg-sheet shadow-e3 p-2 text-ink"
    >
      <label className="relative block">
        <Search
          size={13}
          className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3"
        />
        <input
          ref={input}
          role="combobox"
          aria-expanded
          aria-controls={listId}
          aria-activedescendant={options[active] ? optId(active) : undefined}
          aria-label={t("tasks.picker.search")}
          aria-describedby={`${listId}-hint`}
          value={query}
          placeholder={t("tasks.picker.search")}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const n = options.length;
              if (n)
                setActive((a) => (a + (e.key === "ArrowDown" ? 1 : n - 1)) % n);
            } else if (e.key === "Enter") {
              e.preventDefault();
              const o = options[active];
              if (o) void choose(o.id);
            } else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
              // On a parent row with worktrees, → unfolds and ← folds; otherwise the caret moves in the text.
              const o = options[active];
              if (!o?.p || (o.children === 0 && !o.child)) return;
              const parent = o.child ? o.p.worktreeOf : o.p.id;
              if (!parent) return;
              const open = expanded.has(parent);
              if (e.key === "ArrowRight" ? open || o.child : !open) return;
              e.preventDefault();
              toggleKids(parent);
              if (o.child) setActive(Math.max(0, options.findIndex((x) => x.id === parent)));
            } else if (e.key === "Escape" || e.key === "Tab") {
              e.preventDefault();
              close();
            }
          }}
          className="h-8 w-full rounded-lg border border-line bg-paper pr-2 pl-7 text-[13px] placeholder:text-ink-3"
        />
      </label>
      <p id={`${listId}-hint`} className="sr-only">
        {t("tasks.picker.hint")}
      </p>
      <ul
        id={listId}
        role="listbox"
        aria-label={t("tasks.picker.list")}
        className="mt-2 max-h-[min(22rem,60vh)] space-y-0.5 overflow-y-auto"
      >
        {options.map((o, i) => {
          const head =
            o.section === "recent" && options[i - 1]?.section !== "recent"
              ? t("tasks.picker.recent")
              : o.section === "all" &&
                  options[i - 1]?.section !== "all" &&
                  recent.length > 0 &&
                  !query.trim()
                ? t("tasks.picker.all")
                : null;
          const selected = o.id === (current?.id ?? null);
          return (
            <li key={o.id ?? "none"} role="presentation">
              {head && (
                <p className="px-2 pt-1.5 pb-0.5 text-[10px] font-semibold tracking-wide text-ink-3 uppercase">
                  {head}
                </p>
              )}
              <div
                id={optId(i)}
                role="option"
                aria-selected={selected}
                aria-expanded={o.children > 0 && o.p ? expanded.has(o.p.id) : undefined}
                onMouseEnter={() => setActive(i)}
                onClick={() => void choose(o.id)}
                className={`flex cursor-pointer items-start gap-1.5 rounded-lg py-1.5 pr-1.5 ${o.child ? "pl-6" : "pl-2"} ${i === active ? "bg-paper" : ""}`}
              >
                <span className="mt-0.5 w-3.5 shrink-0 text-ink">
                  {selected && <Check size={13} strokeWidth={2.2} />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-1.5">
                    <span
                      className={`block truncate text-[13px] ${o.p && o.p.folder !== "ok" ? "text-ink-2" : ""}`}
                    >
                      {o.p ? o.p.name : t("tasks.picker.none")}
                    </span>
                    {o.p?.localOnly && (
                      <span
                        title={t("tasks.picker.localOnlyTitle")}
                        className="inline-flex shrink-0 items-center gap-0.5 rounded-md border border-line px-1 text-[10px] leading-4 text-ink-2"
                      >
                        <Lock size={9} strokeWidth={2.2} aria-hidden />
                        {t("tasks.delivery.chip")}
                      </span>
                    )}
                  </span>
                  {o.p && (
                    <span className="block truncate text-[11px] text-ink-3">
                      {o.p.isHome
                        ? t("tasks.picker.home")
                        : o.p.folder === "missing"
                          ? t("tasks.picker.missing")
                          : o.p.folder === "unknown"
                            ? t("tasks.picker.unknown")
                            : ""}
                      {o.p.parentDir ?? t("tasks.picker.memoryOnly")}
                    </span>
                  )}
                </span>
                {o.children > 0 && (
                  <button
                    tabIndex={-1}
                    aria-label={t(o.p && expanded.has(o.p.id) ? "tasks.picker.fold" : "tasks.picker.unfold", { count: o.children })}
                    title={t("tasks.picker.worktrees", { count: o.children })}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (o.p) toggleKids(o.p.id);
                    }}
                    className="-my-0.5 inline-flex h-6 shrink-0 items-center gap-0.5 rounded-md border border-line px-1.5 text-[11px] text-ink-2 hover:bg-line/60 hover:text-ink"
                  >
                    <ChevronRight
                      size={11}
                      className={o.p && expanded.has(o.p.id) ? "rotate-90" : ""}
                    />
                    {o.children}
                  </button>
                )}
              </div>
            </li>
          );
        })}
        {options.length === 0 && (
          <li className="px-2 py-3 text-center text-xs text-ink-3">
            {t("tasks.picker.noMatch")}
          </li>
        )}
      </ul>
      {error && <p className="px-2 pt-1 text-xs text-danger">{typeof error.cause === "string" ? messageOf(error.cause) : t("tasks.picker.changeFailed")}</p>}
      {hidden > 0 && (
        <label className="mt-1 flex items-center gap-1.5 border-t border-line px-2 pt-2 text-[11px] text-ink-3">
          <input
            type="checkbox"
            checked={showEmpty}
            onChange={(e) => setShowEmpty(e.target.checked)}
          />
          {t("tasks.picker.showEmpty", { count: hidden })}
        </label>
      )}
    </div>
  );
}

