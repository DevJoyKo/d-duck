// Korean particles picked by the word's final sound (workflow-meta-check). English has none, so the catalogs keep a
// `{ro}` placeholder in both languages and English gets "". Mirrors `dduck_core::i18n::ro`.
import { currentLang, type Lang } from "./language";

/** "로" or "으로" after `word`: "으로" after a final consonant other than ㄹ. A Latin or digit name is read aloud —
 * digits by their Korean names (영·삼·육 → 으로), a word ending in m, n or ng → 으로 — and trailing symbols are
 * skipped. "" in English. */
export function ro(word: string, lang: Lang = currentLang()): string {
  if (lang !== "ko") return "";
  const w = word.replace(/[^\p{L}\p{N}]+$/u, "").toLowerCase();
  const last = w.slice(-1);
  if (!last) return "로";
  const code = last.charCodeAt(0);
  let euro: boolean;
  if (code >= 0xac00 && code <= 0xd7a3) {
    const jong = (code - 0xac00) % 28;
    euro = jong !== 0 && jong !== 8;
  } else if ("036".includes(last)) euro = true;
  else if (last === "m" || last === "n") euro = true;
  else euro = last === "g" && w.endsWith("ng");
  return euro ? "으로" : "로";
}
