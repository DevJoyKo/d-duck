// The current app language (app-language, D-171). The DB value `ui_language` is the source of truth; localStorage
// is a cache so the first paint is already in the right language. Only three values are ever accepted.

export type Choice = "system" | "ko" | "en";
export type Lang = "ko" | "en";

const KEY = "dduck.lang";
const LOCALE: Record<Lang, string> = { ko: "ko-KR", en: "en-US" };

/** Anything but the three values (missing, junk, an older format) is `system`. */
export function parseChoice(v: unknown): Choice {
  return v === "ko" || v === "en" ? v : "system";
}

export function parseLang(v: unknown): Lang | null {
  return v === "ko" || v === "en" ? v : null;
}

/** The "시스템 언어 따름" rule: a locale starting with `ko` is Korean, anything else is English. */
export function langOfLocale(locale: string | undefined | null): Lang {
  return (locale ?? "").trim().toLowerCase().startsWith("ko") ? "ko" : "en";
}

function browserLang(): Lang {
  return langOfLocale(typeof navigator === "undefined" ? "" : navigator.language);
}

export function resolve(choice: Choice, system: Lang = browserLang()): Lang {
  return choice === "system" ? system : choice;
}

type State = { choice: Choice; lang: Lang };
let state: State = { choice: "system", lang: "ko" };
const listeners = new Set<() => void>();

export function loadCachedChoice(): Choice {
  try {
    return parseChoice(localStorage.getItem(KEY));
  } catch {
    return "system";
  }
}

export function getLanguage(): State {
  return state;
}

export function currentLang(): Lang {
  return state.lang;
}

/** `ko-KR` / `en-US`, for Intl formatters. */
export function currentLocale(): string {
  return LOCALE[state.lang];
}

export function localeOf(lang: Lang): string {
  return LOCALE[lang];
}

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Sets the language now: state, `<html lang>`, cache. `resolved` is the backend's answer for `system` (sys-locale);
 * without it the browser locale decides. Values outside the allowed set are ignored. */
export function applyLanguage(choice: unknown, resolved?: unknown) {
  const c = parseChoice(choice);
  const lang = c === "system" ? (parseLang(resolved) ?? browserLang()) : c;
  if (typeof document !== "undefined") document.documentElement.lang = lang;
  try {
    localStorage.setItem(KEY, c);
  } catch {
    // Storage unavailable: the DB value still applies once it is read.
  }
  if (state.choice === c && state.lang === lang) return;
  state = { choice: c, lang };
  listeners.forEach((fn) => fn());
}

/** Before the first paint: the cached choice. */
export function initLanguage() {
  applyLanguage(loadCachedChoice());
}

/** Each language named in itself, whatever the current language (app-language: 선택지 이름은 각 언어 자체 표기). */
export const LANGUAGE_NAMES: Record<Lang, string> = { ko: "한국어", en: "English" };
