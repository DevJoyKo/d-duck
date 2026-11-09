// Quick language switch next to the theme button (app-language). Same value as the settings row (`ui_language`).
// The menu is portaled to <body> with fixed placement: inside the sidebar it sat under the task page's own stacking
// contexts (QA FAIL: the task list covered it at 1440 and 900).
import { Check, Languages } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { changeLanguage, useLanguage, useT } from "../i18n/react";
import { LANGUAGE_NAMES, type Choice } from "../i18n/language";

const ORDER: Choice[] = ["ko", "en", "system"];

export function LanguageMenu() {
  const t = useT();
  const { choice } = useLanguage();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    if (!open) return;
    const r = button.current?.getBoundingClientRect();
    setAt(r ? { top: r.bottom + 4, left: r.left } : { top: 0, left: 0 });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    items.current[Math.max(0, ORDER.indexOf(choice))]?.focus();
    const close = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!ref.current?.contains(t) && !menu.current?.contains(t)) setOpen(false);
    };
    // Fixed placement does not follow a resize: close instead of floating away from the button.
    const away = () => setOpen(false);
    document.addEventListener("mousedown", close);
    window.addEventListener("resize", away);
    return () => {
      document.removeEventListener("mousedown", close);
      window.removeEventListener("resize", away);
    };
    // Focus the checked item only when the menu opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const shut = () => {
    setOpen(false);
    button.current?.focus();
  };
  const onKey = (e: KeyboardEvent) => {
    const i = items.current.findIndex((el) => el === document.activeElement);
    if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      shut();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = (i + (e.key === "ArrowDown" ? 1 : ORDER.length - 1)) % ORDER.length;
      items.current[next]?.focus();
    }
  };
  const label = (c: Choice) => (c === "system" ? t("common.language.system") : LANGUAGE_NAMES[c]);

  return (
    <div ref={ref} className="relative">
      <button
        ref={button}
        aria-label={t("nav.language.label")}
        title={t("nav.language.label")}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="grid h-7 w-7 place-items-center rounded-md text-side-ink hover:bg-side-active hover:text-ink"
      >
        <Languages size={15} strokeWidth={1.8} />
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            role="menu"
            aria-label={t("nav.language.menu")}
            onKeyDown={onKey}
            data-language-menu
            style={{ top: at?.top ?? 0, left: at?.left ?? 0, visibility: at ? undefined : "hidden" }}
            className="fixed z-50 w-44 overflow-hidden rounded-xl bg-sheet py-1 text-ink shadow-e3"
          >
          {ORDER.map((c, i) => (
            <button
              key={c}
              ref={(el) => {
                items.current[i] = el;
              }}
              role="menuitemradio"
              aria-checked={choice === c}
              lang={c === "system" ? undefined : c}
              onClick={() => {
                void changeLanguage(c);
                shut();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[13px] hover:bg-paper focus:bg-paper focus:outline-none"
            >
              <span className="w-3.5 shrink-0">{choice === c && <Check size={14} />}</span>
              <span className="truncate">{label(c)}</span>
            </button>
          ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
