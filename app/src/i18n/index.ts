// App text catalogs and `t()` (app-language, D-171). One file per namespace and language: `ko/<ns>.ts` is the
// reference, `en/<ns>.ts` must satisfy its shape. Add a namespace here once; screen groups only edit their own files.
import koCommon from "./ko/common";
import koNav from "./ko/nav";
import koSettings from "./ko/settings";
import koTasks from "./ko/tasks";
import koReport from "./ko/report";
import koAgents from "./ko/agents";
import koExtensions from "./ko/extensions";
import koUsage from "./ko/usage";
import koArtifacts from "./ko/artifacts";
import koRoutines from "./ko/routines";
import koAuth from "./ko/auth";
import koTour from "./ko/tour";
import koErrors from "./ko/errors";
import koNews from "./ko/news";
import enCommon from "./en/common";
import enNav from "./en/nav";
import enSettings from "./en/settings";
import enTasks from "./en/tasks";
import enReport from "./en/report";
import enAgents from "./en/agents";
import enExtensions from "./en/extensions";
import enUsage from "./en/usage";
import enArtifacts from "./en/artifacts";
import enRoutines from "./en/routines";
import enAuth from "./en/auth";
import enTour from "./en/tour";
import enErrors from "./en/errors";
import enNews from "./en/news";
import { currentLang, localeOf, type Lang } from "./language";
import { osWords } from "./osWords";
import type { ArgsFor, At, Message, Paths, Plural, Shape, Tree } from "./types";

export const ko = {
  common: koCommon,
  nav: koNav,
  settings: koSettings,
  tasks: koTasks,
  report: koReport,
  agents: koAgents,
  extensions: koExtensions,
  usage: koUsage,
  artifacts: koArtifacts,
  routines: koRoutines,
  auth: koAuth,
  tour: koTour,
  news: koNews,
  errors: koErrors,
} as const;

export type Messages = typeof ko;

export const en = {
  common: enCommon,
  nav: enNav,
  settings: enSettings,
  tasks: enTasks,
  report: enReport,
  agents: enAgents,
  extensions: enExtensions,
  usage: enUsage,
  artifacts: enArtifacts,
  routines: enRoutines,
  auth: enAuth,
  tour: enTour,
  news: enNews,
  errors: enErrors,
} satisfies Shape<Messages>;

export const catalogs: Record<Lang, Tree> = { ko, en };

export type MessageKey = Paths<Messages>;

function lookup(tree: Tree, key: string): Message | undefined {
  let node: Message | Tree | undefined = tree;
  for (const part of key.split(".")) {
    if (node === undefined || typeof node === "string") return undefined;
    node = (node as Tree)[part];
  }
  return typeof node === "string" || (node && typeof (node as Plural).other === "string") ? (node as Message) : undefined;
}

function warn(text: string) {
  if (import.meta.env.DEV) console.warn(`[i18n] ${text}`);
}

/** Fills `{name}` placeholders in one pass. A value is never scanned again, so `{x}` inside a value stays as text.
 * A placeholder without a value is left as written (and warned about in development). */
export function fill(template: string, params: Record<string, string | number> | undefined, lang: Lang): string {
  let numbers: Intl.NumberFormat | undefined;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const v = params?.[name];
    if (v === undefined) {
      warn(`missing parameter {${name}}`);
      return whole;
    }
    if (typeof v === "number") return (numbers ??= new Intl.NumberFormat(localeOf(lang))).format(v);
    return v;
  });
}

const pluralRules: Partial<Record<Lang, Intl.PluralRules>> = {};

function pick(m: Message, params: Record<string, string | number> | undefined, lang: Lang): string {
  if (typeof m === "string") return m;
  const count = Number(params?.count);
  const rule = (pluralRules[lang] ??= new Intl.PluralRules(localeOf(lang))).select(Number.isFinite(count) ? count : 0);
  return m[rule] ?? m.other;
}

/** The message for `key` in `lang`: falls back to Korean, then to the key itself (both warned in development). */
export function translate(lang: Lang, key: string, params?: Record<string, string | number>): string {
  let m = lookup(catalogs[lang], key);
  if (m === undefined && lang !== "ko") {
    warn(`missing "${key}" in ${lang}`);
    m = lookup(catalogs.ko, key);
  }
  if (m === undefined) {
    warn(`unknown key "${key}"`);
    return key;
  }
  return fill(osWords(pick(m, params, lang), lang), params, lang);
}

/** Text in the current language. Always a plain string: put links and markup around it with `tx()`, never HTML. */
export function t<K extends MessageKey>(key: K, ...args: ArgsFor<At<Messages, K>>): string {
  return translate(currentLang(), key, args[0] as Record<string, string | number> | undefined);
}

export type TFunction = typeof t;
export { applyLanguage, currentLang, currentLocale, getLanguage, initLanguage, type Choice, type Lang } from "./language";
