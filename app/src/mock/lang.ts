// Mock-mode language state (app-language, D-171). Two sides, like the real app:
// - backend: the language Rust renders its own text in (i18n.rs, bundled data). It follows `set_language`.
// - data: the language of the fake user data. Korean, unless the page opened with `?lang=en` (or `system` on an
//   English browser), which swaps in English example data for README and screen captures. It never changes later,
//   because the real app never translates user data.
import { langOfLocale, type Choice, type Lang } from "../i18n/language";

const param = typeof location === "undefined" ? null : new URLSearchParams(location.search).get("lang");
let choice: Choice = param === "en" || param === "system" ? param : "ko";

const browser = (): Lang => langOfLocale(typeof navigator === "undefined" ? "" : navigator.language);
const resolved = (c: Choice): Lang => (c === "system" ? browser() : c);

/** The fake data's language, fixed at page load. */
export const DATA_LANG: Lang = resolved(choice);

export function mockChoice(): Choice {
  return choice;
}

export function setMockChoice(c: unknown) {
  if (c === "system" || c === "ko" || c === "en") choice = c;
}

/** The language the mock backend answers in now. */
export function backendLang(): Lang {
  return resolved(choice);
}

/** Backend text in the current language, for sentences the mock builds with values (i18n.rs templates). */
export function bt(ko: string, en: string): string {
  return backendLang() === "en" ? en : ko;
}
