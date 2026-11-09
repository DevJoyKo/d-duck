// "시작하기" checklist after the tour: a small "시작하기 n/3" pill at the bottom of the sidebar, so it never covers
// page actions. Click to unfold the list above it. Items tick themselves when the user really does them (markDone)
// and the pill disappears once all three are done. Unfolding is a per-viewer convenience kept in localStorage.
import { Check, ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useState } from "react";
import type { OnboardingState } from "../lib/api";
import { CHECKLIST } from "../lib/tour";
import { useT } from "../i18n/react";
import type { NavTarget as Page } from "./Sidebar"; // old ids are normalized by App (nav-ia)

const OPEN_KEY = "dduck.checklist.open";

function loadOpen(): boolean {
  try {
    return localStorage.getItem(OPEN_KEY) === "1";
  } catch {
    return false;
  }
}

function saveOpen(open: boolean) {
  try {
    localStorage.setItem(OPEN_KEY, open ? "1" : "0");
  } catch {
    // Storage unavailable: the choice lasts until the app closes.
  }
}

export function StartChecklist({ state, onNavigate }: { state: OnboardingState; onNavigate: (p: Page) => void }) {
  const t = useT();
  const [open, setOpen] = useState(loadOpen);
  const done = CHECKLIST.filter((c) => state.checklist[c.id]).length;
  const toggle = (next: boolean) => {
    setOpen(next);
    saveOpen(next);
  };
  // The project picker opens in the same corner: fold this list when it does (project-switcher).
  useEffect(() => {
    const fold = () => setOpen(false);
    window.addEventListener("dduck-picker-open", fold);
    return () => window.removeEventListener("dduck-picker-open", fold);
  }, []);
  return (
    <section aria-label={t("tour.checklist.label")} className="relative">
      {open && (
        <ul className="absolute bottom-full left-0 z-40 mb-2 w-[232px] space-y-0.5 rounded-xl bg-sheet p-2 text-ink shadow-e3">
          {CHECKLIST.map((c) => {
            const ok = state.checklist[c.id];
            return (
              <li key={c.id}>
                <button
                  onClick={() => {
                    toggle(false);
                    onNavigate(c.page);
                  }}
                  className="flex w-full items-center gap-2 rounded-lg px-1.5 py-1.5 text-left text-[13px] hover:bg-paper"
                  aria-label={ok ? t("tour.checklist.itemDone", { label: c.label }) : c.label}
                >
                  <span className={`grid h-4 w-4 shrink-0 place-items-center rounded-full border ${ok ? "border-ok bg-ok text-paper" : "border-ink-3"}`}>
                    {ok && <Check size={10} strokeWidth={3} />}
                  </span>
                  <span className={ok ? "text-ink-3 line-through" : ""}>{c.label}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <button
        onClick={() => toggle(!open)}
        aria-expanded={open}
        className="flex h-8 w-full items-center gap-2 rounded-lg bg-sheet/70 px-3 text-left text-xs text-ink-2 shadow-e1 hover:bg-sheet hover:text-ink"
      >
        <span className="flex-1 font-medium">{t("tour.checklist.title")}</span>
        <span>
          {done}/{CHECKLIST.length}
        </span>
        {open ? <ChevronDown size={13} aria-hidden="true" /> : <ChevronUp size={13} aria-hidden="true" />}
      </button>
    </section>
  );
}
