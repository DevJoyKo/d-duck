// OS words in app text (windows-support 4.2): the catalogs are written for macOS; on Windows these names and the ⌘ key
// are swapped when a message is looked up, before its parameters are filled (a value is never rewritten).
import { isWindows } from "../lib/platform";
import type { Lang } from "./language";

const WINDOWS: Record<Lang, [RegExp, string][]> = {
  ko: [
    [/Time Machine 백업/g, "PC 백업"],
    [/이 Mac/g, "이 PC"],
    [/Mac이/g, "PC가"],
    [/Mac 앞/g, "PC 앞"],
    [/macOS/g, "Windows"],
    [/Finder/g, "파일 탐색기"],
    [/⌘/g, "Ctrl+"],
  ],
  en: [
    [/Time Machine backs it up/g, "your PC backup includes it"],
    [/\b([Tt]his|[Tt]he|[Yy]our) Mac\b/g, "$1 PC"],
    [/macOS/g, "Windows"],
    [/Finder/g, "File Explorer"],
    [/\bTrash\b/g, "Recycle Bin"],
    [/⌘/g, "Ctrl+"],
  ],
};

/** `text` with the macOS names replaced on Windows; unchanged on macOS. */
export function osWords(text: string, lang: Lang): string {
  if (!isWindows()) return text;
  return WINDOWS[lang].reduce((s, [re, to]) => s.replace(re, to), text);
}
