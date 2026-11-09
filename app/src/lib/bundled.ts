// Bundled data (`crates/dduck-core/data/*.json`, app-language design 4): text fields are `{ko, en}` pairs. No Tauri
// or catalog import here, so mocks and demos can read the files without pulling in the IPC layer.
import type { Lang } from "../i18n/language";

/** The JSON type with every `{ko, en}` text pair read as one string. */
export type Picked<T> = T extends { ko: string; en: string }
  ? string
  : T extends readonly (infer U)[]
    ? Picked<U>[]
    : T extends object
      ? { [K in keyof T]: Picked<T[K]> }
      : T;

/** Bundled data in one language (mock and demo use the Korean side, like Rust's defaults before a language). */
export function pickLang<T>(v: T, lang: Lang = "ko"): Picked<T> {
  if (Array.isArray(v)) return v.map((x) => pickLang(x, lang)) as Picked<T>;
  if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    const keys = Object.keys(o);
    if (keys.length === 2 && typeof o.ko === "string" && typeof o.en === "string") return o[lang] as Picked<T>;
    return Object.fromEntries(keys.map((k) => [k, pickLang(o[k], lang)])) as Picked<T>;
  }
  return v as Picked<T>;
}
