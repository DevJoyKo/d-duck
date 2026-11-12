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

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]';

/** Focusable controls inside `root`, by attributes only (no layout reads, so jsdom behaves like the app). */
export function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => {
    if (el.getAttribute("tabindex") === "-1") return false;
    const off = el.closest('[hidden], [inert], [aria-hidden="true"]');
    return !off || !root.contains(off);
  });
}

/** Mounted modals, newest last. Only the top one reacts to keys and focus, so two sheets open together don't both close. */
const stack: object[] = [];

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
    const target = () => {
      const d = box();
      return latest.current.options.initial?.current ?? (d ? focusables(d)[0] : undefined) ?? null;
    };
    if (!box()?.contains(document.activeElement)) target()?.focus();

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
      if (e.key !== "Tab") return;
      e.preventDefault();
      const list = focusables(d);
      if (!list.length) return;
      const active = document.activeElement as HTMLElement | null;
      const i = active ? list.indexOf(active) : -1;
      if (i < 0 && active && active !== d && d.contains(active)) {
        // Focus on something inside that is not a stop (a row focused by script): continue from where it is.
        const after = list.findIndex((el) => active.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING);
        const next = e.shiftKey ? (after < 0 ? list.length : after) - 1 : after;
        list[next < 0 || next >= list.length ? (e.shiftKey ? list.length - 1 : 0) : next].focus();
        return;
      }
      const next = i < 0 ? (e.shiftKey ? list.length - 1 : 0) : (i + (e.shiftKey ? -1 : 1) + list.length) % list.length;
      list[next].focus();
    };
    const onFocusIn = (e: FocusEvent) => {
      if (!top()) return;
      const d = box();
      if (d && !d.contains(e.target as Node)) target()?.focus();
    };
    window.addEventListener("keydown", onKey, capture);
    document.addEventListener("focusin", onFocusIn);
    return () => {
      stack.splice(stack.indexOf(me), 1);
      window.removeEventListener("keydown", onKey, capture);
      document.removeEventListener("focusin", onFocusIn);
      for (const el of made) el.removeAttribute("inert");
      const { returnTo } = latest.current.options;
      const back = returnTo ? returnTo() : opener;
      if (back?.isConnected) back.focus();
    };
    // Runs once per mount: the refs are stable and the callbacks are read through `latest`.
  }, [dialog, capture, inertRoot, opener]);
}
