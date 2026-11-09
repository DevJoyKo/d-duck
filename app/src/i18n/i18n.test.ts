// app-language (D-171): catalog completeness over every namespace, plurals, one-pass substitution, language rules.
import { afterEach, describe, expect, it, vi } from "vitest";
import { catalogs, en, fill, ko, t, translate } from "./index";
import { applyLanguage, currentLang, getLanguage, langOfLocale, loadCachedChoice, parseChoice } from "./language";
import type { Message, Tree } from "./types";
import { clock24, dateTime, num, relativeTime } from "../lib/format";

afterEach(() => {
  applyLanguage("ko");
  vi.restoreAllMocks();
});

type Leaf = { key: string; message: Message };

function leaves(tree: Tree, prefix = ""): Leaf[] {
  return Object.entries(tree).flatMap(([k, v]) => {
    const key = `${prefix}${k}`;
    if (typeof v === "string" || typeof (v as { other?: unknown }).other === "string") return [{ key, message: v as Message }];
    return leaves(v as Tree, `${key}.`);
  });
}

function placeholders(m: Message): string[] {
  const texts = typeof m === "string" ? [m] : Object.values(m).filter((x): x is string => typeof x === "string");
  const names = new Set(texts.flatMap((s) => [...s.matchAll(/\{(\w+)\}/g)].map((x) => x[1])));
  if (typeof m !== "string") names.add("count");
  return [...names].sort();
}

describe("catalog completeness (every namespace)", () => {
  // Every ko/<ns>.ts and en/<ns>.ts file on disk, so a namespace that is not registered in index.ts also fails.
  const files = Object.keys(import.meta.glob(["./ko/*.ts", "./en/*.ts"])).map((f) => f.replace(/^\.\//, "").replace(/\.ts$/, ""));

  it("registers every namespace file in both languages", () => {
    const names = (lang: string) => files.filter((f) => f.startsWith(`${lang}/`)).map((f) => f.slice(3)).sort();
    expect(names("ko")).toEqual(Object.keys(ko).sort());
    expect(names("en")).toEqual(Object.keys(en).sort());
  });

  it("has the same keys in Korean and English", () => {
    const k = leaves(ko as unknown as Tree).map((l) => l.key);
    const e = leaves(en as unknown as Tree).map((l) => l.key);
    expect(e.filter((x) => !k.includes(x)), "only in en").toEqual([]);
    expect(k.filter((x) => !e.includes(x)), "only in ko").toEqual([]);
  });

  it("uses the same parameter names and message kind for each key", () => {
    const enMap = new Map(leaves(en as unknown as Tree).map((l) => [l.key, l.message]));
    const bad = leaves(ko as unknown as Tree).flatMap(({ key, message }) => {
      const other = enMap.get(key);
      if (other === undefined) return [];
      if (typeof message !== typeof other) return [`${key}: plural in one language only`];
      const [a, b] = [placeholders(message).join(","), placeholders(other).join(",")];
      return a === b ? [] : [`${key}: ko {${a}} vs en {${b}}`];
    });
    expect(bad).toEqual([]);
  });

  it("has no empty messages", () => {
    const empty = (["ko", "en"] as const).flatMap((lang) =>
      leaves(catalogs[lang]).filter((l) => (typeof l.message === "string" ? l.message : l.message.other).trim() === "").map((l) => `${lang}: ${l.key}`),
    );
    expect(empty).toEqual([]);
  });
});

describe("t()", () => {
  it("follows the current language without remounting anything", () => {
    expect(t("settings.title")).toBe("설정");
    applyLanguage("en");
    expect(t("settings.title")).toBe("Settings");
  });

  it("picks English singular and plural with Intl.PluralRules", () => {
    expect(translate("en", "settings.history.skipped", { count: 1 })).toBe("1 line skipped");
    expect(translate("en", "settings.history.skipped", { count: 3 })).toBe("3 lines skipped");
    expect(translate("ko", "settings.history.skipped", { count: 1 })).toBe("1줄 건너뜀");
  });

  it("groups numbers in the language's style; strings stay as written", () => {
    expect(translate("en", "settings.history.files", { count: 12345 })).toBe("Reading 12,345 history files");
    expect(fill("{v}", { v: "12345" }, "en")).toBe("12345");
  });

  it("substitutes once: a value that looks like a placeholder stays text", () => {
    expect(fill("{a} {b}", { a: "{b}", b: "X" }, "ko")).toBe("{b} X");
    expect(fill("{a}", { a: "<b>x</b>" }, "ko")).toBe("<b>x</b>");
  });

  it("warns in development about an unknown key or a missing parameter", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(translate("en", "nope.missing")).toBe("nope.missing");
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('unknown key "nope.missing"'));
    expect(translate("en", "nav.cli.found")).toBe("Claude Code {version}");
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("missing parameter {version}"));
  });
});

describe("language choice", () => {
  it("accepts three values only; anything else is system", () => {
    expect(["ko", "en", "system", "ja", "EN", "", null, 3].map(parseChoice)).toEqual(["ko", "en", "system", "system", "system", "system", "system", "system"]);
  });

  it("system means Korean only for a ko locale", () => {
    expect(["ko-KR", "ko", "KO_kr", "en-US", "ja-JP", "", undefined].map(langOfLocale)).toEqual(["ko", "ko", "ko", "en", "en", "en", "en"]);
  });

  it("applies the backend's answer for system, sets <html lang> and caches the choice", () => {
    applyLanguage("system", "ko");
    expect(getLanguage()).toEqual({ choice: "system", lang: "ko" });
    expect(document.documentElement.lang).toBe("ko");
    expect(loadCachedChoice()).toBe("system");
    applyLanguage("en");
    expect(document.documentElement.lang).toBe("en");
    expect(loadCachedChoice()).toBe("en");
    // A junk resolved value falls back to the browser locale (jsdom: en-US), never throws.
    applyLanguage("system", "xx-YY");
    expect(currentLang()).toBe("en");
    localStorage.setItem("dduck.lang", "fr");
    expect(loadCachedChoice()).toBe("system");
  });
});

describe("format (locale argument)", () => {
  const at = new Date(2026, 10, 9, 20, 19).getTime();

  it("formats list times per language (Intl values measured and fixed)", () => {
    expect(dateTime(at, "ko-KR")).toBe("11. 09. 오후 08:19");
    expect(dateTime(at, "en-US")).toBe("11/09, 08:19 PM");
    expect(dateTime(0, "en-US")).toBe("-");
  });

  it("uses the current language by default", () => {
    expect(dateTime(at)).toBe("11. 09. 오후 08:19");
    applyLanguage("en");
    expect(dateTime(at)).toBe("11/09, 08:19 PM");
  });

  it("keeps 24-hour clocks fixed in every language", () => {
    applyLanguage("en");
    expect(clock24(at)).toBe("20:19");
  });

  it("formats relative times and numbers", () => {
    expect(relativeTime(at - 3 * 60_000, at, "ko-KR")).toBe("3분 전");
    expect(relativeTime(at - 3 * 60_000, at, "en-US")).toBe("3 minutes ago");
    expect(relativeTime(at - 26 * 3_600_000, at, "en-US")).toBe("1 day ago");
    expect(relativeTime(at + 2 * 3_600_000, at, "en-US")).toBe("in 2 hours");
    expect(num(1234567, "en-US")).toBe("1,234,567");
  });
});

// Compile-time checks (`tsc --noEmit`): a wrong key or wrong parameters do not type-check. Never called.
export function typeChecks() {
  // @ts-expect-error unknown key
  t("settings.nope");
  // @ts-expect-error parameters missing
  t("nav.cli.found");
  // @ts-expect-error wrong parameter name
  t("nav.cli.found", { ver: "1" });
  // @ts-expect-error a plural needs its count
  t("settings.history.skipped");
  t("nav.cli.found", { version: "2.1" });
  t("settings.history.skipped", { count: 2 });
  t("settings.title");
}

describe("sentence endings", () => {
  // Brand review: a line that ends with a period in one language ends with one in the other (a colon introducing
  // what follows counts as an ending too), so a screen does not mix "Saved." and "Saved".
  it("keeps periods consistent between ko and en", () => {
    const ends = (s: string) => /[.:]$/.test(s.trim());
    const off: string[] = [];
    const walk = (k: unknown, e: unknown, path: string) => {
      if (typeof k === "string" && typeof e === "string") {
        if (ends(k) !== ends(e)) off.push(`${path}: ${k} | ${e}`);
      } else if (k && typeof k === "object" && e && typeof e === "object") {
        for (const key of Object.keys(k)) walk((k as Record<string, unknown>)[key], (e as Record<string, unknown>)[key], `${path}.${key}`);
      }
    };
    walk(ko, en, "");
    expect(off).toEqual([]);
  });
});
