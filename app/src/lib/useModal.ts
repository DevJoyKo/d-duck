// Keyboard behaviour of a modal sheet or dialog (sheet-focus): focus moves in on open, Tab and Shift+Tab cycle inside,
// Escape closes through the caller's own close path, and focus goes back where it came from on unmount.
import { useEffect, useRef, useState, type RefObject } from "react";

export interface ModalOptions {
  /** Focused on open unless focus is already inside (an autoFocus field wins). Never a destructive or submit button.
   *  Default: the first focusable control. Focus that wanders outside comes back here too. */
  initial?: RefObject<HTMLElement | null>;
  /** Where focus goes on close, read at close time. Given and returning null: focus is left alone. Default: the
   *  element that had focus when the modal opened, if it is still in the page. */
  returnTo?: () => HTMLElement | null;
  /** Make every other child of <body> inert while open. `root` is the modal's own child of <body> (a portal). */
  inert?: { root: RefObject<HTMLElement | null> };
  /** "capture" handles Escape before anything else and stops it (the report dialog over the task detail).
   *  Default "bubble": an inner panel that stops or prevents its Escape keeps the modal open. */
  escapePhase?: "bubble" | "capture";
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), details > summary:first-of-type, [tabindex]';

/** Inside a closed <details> and not part of its summary: not rendered, so not a tab stop. */
function inClosedDetails(el: HTMLElement, root: HTMLElement): boolean {
  for (let p = el.parentElement; p && p !== root; p = p.parentElement) {
    if (p instanceof HTMLDetailsElement && !p.open) {
      const summary = p.querySelector(":scope > summary");
      if (!summary?.contains(el)) return true;
    }
  }
  return false;
}

/** Candidate tab stops inside `root`, in document order: attributes first (the same in jsdom), then the browser's
 *  own visibility check where it exists (display:none, e.g. a `hidden lg:block` list). A candidate can still refuse
 *  focus; the Tab handler then moves on to the next one. Of a named radio group only the checked radio (else the
 *  first) is a stop, as in the browser. */
export function focusables(root: HTMLElement): HTMLElement[] {
  const radios = new Map<string, HTMLInputElement>();
  // Sorted explicitly: jsdom's selector engine can return a selector list grouped by selector, not in page order.
  const found = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
  const all = found.filter((el) => {
    if (el.getAttribute("tabindex") === "-1") return false;
    const off = el.closest('[hidden], [inert], [aria-hidden="true"]');
    if (off && root.contains(off)) return false;
    if (inClosedDetails(el, root)) return false;
    return el.checkVisibility?.() ?? true;
  });
  for (const el of all) {
    if (!(el instanceof HTMLInputElement) || el.type !== "radio" || !el.name) continue;
    const kept = radios.get(el.name);
    if (!kept || (!kept.checked && el.checked)) radios.set(el.name, el);
  }
  return all.filter((el) => !(el instanceof HTMLInputElement && el.type === "radio" && el.name) || radios.get(el.name) === el);
}

/** Focus the first element of `list` from `start` stepping by `dir` (wrapping) that really takes focus. */
function focusFrom(list: HTMLElement[], start: number, dir: 1 | -1): boolean {
  const n = list.length;
  for (let k = 0; k < n; k++) {
    const el = list[(((start + k * dir) % n) + n) % n];
    el.focus();
    if (document.activeElement === el) return true;
  }
  return false;
}

/** Mounted modals, newest last. Only the top one reacts to keys and focus, so two sheets open together don't both
 *  close. Effects run child first: a modal nested inside another modal's component and mounted in the same commit
 *  lands below its parent. No sheet does that today (each opens from a click, in its own commit). */
const stack: object[] = [];

/** Focus return on unmount is skipped while the dialog node is still in the page (StrictMode re-running the effect).
 *  That assumes the node belongs to the component calling the hook and leaves the page with it. */
export function useModal(dialog: RefObject<HTMLElement | null>, onClose: () => void, options: ModalOptions = {}) {
  // Read during render, before an autoFocus child takes focus in the commit.
  const [opener] = useState(() => (typeof document === "undefined" ? null : (document.activeElement as HTMLElement | null)));
  // Latest callbacks without re-running the effect: typing that flips `dirty` must not move focus.
  const latest = useRef({ onClose, options });
  useEffect(() => {
    latest.current = { onClose, options };
  });
  const capture = options.escapePhase === "capture";
  const inertRoot = options.inert?.root;
  useEffect(() => {
    const me = {};
    stack.push(me);
    const top = () => stack[stack.length - 1] === me;
    const made: Element[] = [];
    if (inertRoot) {
      const host = inertRoot.current;
      for (const el of Array.from(document.body.children)) {
        if (el === host || el.hasAttribute("inert")) continue;
        el.setAttribute("inert", "");
        made.push(el);
      }
    }
    const box = () => dialog.current;
    const mounted = box();
    const toInitial = () => {
      const first = latest.current.options.initial?.current;
      if (first) {
        first.focus();
        if (document.activeElement === first) return;
      }
      const d = box();
      if (d) focusFrom(focusables(d), 0, 1);
    };
    if (!box()?.contains(document.activeElement)) toInitial();

    const onKey = (e: KeyboardEvent) => {
      if (!top()) return;
      const d = box();
      if (!d) return;
      if (e.key === "Escape") {
        // An input method finishing a syllable, or an inner control that handled the key itself.
        if (e.isComposing || e.keyCode === 229 || e.defaultPrevented) return;
        if (capture) e.stopPropagation();
        latest.current.onClose();
        return;
      }
      // Ctrl/Alt/Cmd+Tab belong to the system or the browser.
      if (e.key !== "Tab" || e.ctrlKey || e.altKey || e.metaKey) return;
      e.preventDefault();
      const list = focusables(d);
      if (!list.length) return;
      const dir = e.shiftKey ? -1 : 1;
      const active = document.activeElement as HTMLElement | null;
      const i = active ? list.indexOf(active) : -1;
      let start: number;
      if (i >= 0) start = i + dir;
      else if (active && active !== d && d.contains(active)) {
        // Focus on something inside that is not a stop (a row focused by script): continue from where it is.
        const after = list.findIndex((el) => active.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING);
        start = dir > 0 ? (after < 0 ? 0 : after) : (after < 0 ? list.length : after) - 1;
      } else start = dir > 0 ? 0 : list.length - 1;
      focusFrom(list, start, dir);
    };
    const onFocusIn = (e: FocusEvent) => {
      if (!top()) return;
      const d = box();
      if (d && !d.contains(e.target as Node)) toInitial();
    };
    window.addEventListener("keydown", onKey, capture);
    document.addEventListener("focusin", onFocusIn);
    return () => {
      stack.splice(stack.indexOf(me), 1);
      window.removeEventListener("keydown", onKey, capture);
      document.removeEventListener("focusin", onFocusIn);
      for (const el of made) el.removeAttribute("inert");
      // Still in the page: StrictMode re-running the effect, not a close. Moving focus out here would make the
      // re-run take it back to `initial` and lose an autoFocus field.
      if (mounted?.isConnected) return;
      const { returnTo } = latest.current.options;
      const back = returnTo ? returnTo() : opener;
      if (back?.isConnected) back.focus();
    };
    // Runs once per mount: the refs are stable and the callbacks are read through `latest`.
  }, [dialog, capture, inertRoot, opener]);
}
