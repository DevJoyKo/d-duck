// Light / dark / follow-the-OS. Stored per viewer (a convenience); applied as data-theme on <html>.
import { t } from "../i18n";

export type Theme = "system" | "light" | "dark";
const KEY = "dduck.theme";

export function loadTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    return t === "light" || t === "dark" ? t : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(t: Theme) {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
  try {
    localStorage.setItem(KEY, t);
  } catch {
    // Storage unavailable: the choice lasts until the app closes.
  }
}

export const NEXT_THEME: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" };
/** Read in the current language each time (common.theme). */
export const THEME_LABEL: Readonly<Record<Theme, string>> = Object.freeze({
  get system() {
    return t("common.theme.system");
  },
  get light() {
    return t("common.theme.light");
  },
  get dark() {
    return t("common.theme.dark");
  },
});
