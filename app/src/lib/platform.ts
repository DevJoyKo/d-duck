// The OS the app runs on (windows-support 4.1). Read once at startup from the Rust `platform` command; until then,
// and whenever it cannot be read, the app behaves as on macOS, so macOS output never changes.
import { invoke } from "@tauri-apps/api/core";

export type Os = "macos" | "windows" | "linux";
export interface PlatformInfo {
  os: Os;
  /** Git for Windows (Git Bash) found. Always true off Windows. */
  gitBash: boolean;
}

const MAC: PlatformInfo = { os: "macos", gitBash: true };
let info: PlatformInfo = MAC;
let loading: Promise<PlatformInfo> | null = null;

function parse(v: unknown): PlatformInfo {
  if (!v || typeof v !== "object") return MAC;
  const o = v as { os?: unknown; gitBash?: unknown };
  const os: Os = o.os === "windows" || o.os === "linux" ? o.os : "macos";
  return { os, gitBash: os !== "windows" || o.gitBash === true };
}

/** Reads the platform once (later calls share the first result). A failed read keeps the macOS default. */
export function loadPlatform(): Promise<PlatformInfo> {
  loading ??= invoke<unknown>("platform")
    .then((v) => (info = parse(v)))
    .catch(() => info);
  return loading;
}

export const platform = (): PlatformInfo => info;
export const isMac = () => info.os === "macos";
export const isWindows = () => info.os === "windows";

/** For tests only: set the platform (`null` back to the macOS default) and forget any load in progress. */
export function setPlatformForTest(p: Partial<PlatformInfo> | null) {
  info = p ? parse({ ...MAC, ...p }) : MAC;
  loading = null;
}

/** `C:\…`, `C:/…` and `\\server\share\…` count as absolute on Windows; `/…` everywhere else. */
export function isAbsPath(p: string): boolean {
  if (!isWindows()) return p.startsWith("/");
  return /^[A-Za-z]:[\\/]/.test(p) || /^\\\\[^\\/]+[\\/][^\\/]+/.test(p);
}

/** Path parts without empty pieces: `/` only on macOS, `/` and `\` on Windows. */
export function splitPath(p: string): string[] {
  return p.split(isWindows() ? /[\\/]/ : "/").filter(Boolean);
}

/** The last part of a path (the folder or file name), or the path itself when it has no parts. */
export const baseName = (p: string): string => splitPath(p).pop() ?? p;

/** `home` + rest → `~` + rest. On Windows the drive and folder names compare without letter case and either slash. */
export function shortPath(p: string, home: string | null | undefined): string {
  if (!home) return p;
  if (!isWindows()) {
    const h = home.replace(/\/+$/, "");
    return h && (p === h || p.startsWith(`${h}/`)) ? `~${p.slice(h.length)}` : p;
  }
  const norm = (s: string) => s.replace(/\//g, "\\").toLowerCase();
  const h = norm(home).replace(/\\+$/, "");
  const n = norm(p);
  return h && (n === h || n.startsWith(`${h}\\`)) ? `~${p.slice(h.length)}` : p;
}

/** The modifier key name in shortcut labels: ⌘ on macOS, Ctrl elsewhere. */
export const modKey = () => (isMac() ? "⌘" : "Ctrl");

/** A modifier + key label: `⌘↵` on macOS, `Ctrl+Enter` elsewhere (keys given as their Mac symbol). */
export function shortcut(key: string): string {
  if (isMac()) return `⌘${key}`;
  return `Ctrl+${key === "↵" ? "Enter" : key}`;
}
