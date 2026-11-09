// React side of the catalog: `useT()` re-renders on a language change without remounting anything, so drafts and
// open sheets keep their state. `LanguageProvider` matches the cached language to the DB value once.
import { useQueryClient } from "@tanstack/react-query";
import { Fragment, useEffect, useSyncExternalStore, type ReactNode } from "react";
import { api } from "../lib/api";
import { t, translate, type MessageKey } from "./index";
import { applyLanguage, getLanguage, parseChoice, subscribe, type Choice } from "./language";

/** The current choice and language; re-renders the caller when either changes. */
export function useLanguage() {
  return useSyncExternalStore(subscribe, getLanguage, getLanguage);
}

/** `t` bound to the current language, re-rendering the component on a change. */
export function useT() {
  useLanguage();
  return t;
}

function isChoice(v: unknown): v is Choice {
  return v === "system" || v === "ko" || v === "en";
}

/** Reads the stored language and applies it. An answer outside the three values is ignored. */
export async function syncLanguage() {
  try {
    const r = await api.getLanguage();
    if (r && isChoice(r.choice)) applyLanguage(r.choice, r.resolved);
  } catch {
    // Backend unavailable: keep the cached language.
  }
}

/** Applies at once (the screen changes before the save returns), then saves; the backend rebuilds the tray menu. */
export async function changeLanguage(choice: Choice) {
  const c = parseChoice(choice);
  applyLanguage(c);
  try {
    const r = await api.setLanguage(c);
    if (r && isChoice(r.choice)) applyLanguage(r.choice, r.resolved);
  } catch {
    await syncLanguage();
  }
  // Rust now words its sentences (reasons, notes, fallback errors) in the new language: refetch what is on screen.
  window.dispatchEvent(new Event(BACKEND_LANGUAGE_EVENT));
}

export const BACKEND_LANGUAGE_EVENT = "dduck:backend-language";

export function LanguageProvider({ children }: { children: ReactNode }) {
  useLanguage();
  const qc = useQueryClient();
  useEffect(() => {
    void syncLanguage();
  }, []);
  useEffect(() => {
    const refetch = () => void qc.invalidateQueries();
    window.addEventListener(BACKEND_LANGUAGE_EVENT, refetch);
    return () => window.removeEventListener(BACKEND_LANGUAGE_EVENT, refetch);
  }, [qc]);
  return <>{children}</>;
}

/** A message with React nodes in its `{name}` slots (links, bold). Text around the slots stays plain text. */
export function tx(key: MessageKey, slots: Record<string, ReactNode>): ReactNode {
  // Each slot becomes a NUL-delimited marker (never present in catalog text), then the string is split around them.
  const marker = "\u0000";
  const text = translate(getLanguage().lang, key, Object.fromEntries(Object.keys(slots).map((k) => [k, `${marker}${k}${marker}`])));
  const parts = text.split(marker);
  return parts.map((part, i) => {
    if (i % 2 === 1 && part in slots) {
      return <Fragment key={`${part}-${i}`}>{slots[part]}</Fragment>;
    }
    return <Fragment key={i}>{part}</Fragment>;
  });
}
