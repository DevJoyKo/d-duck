// `?os=win` (windows-support 4.1): the mock answers as the Windows preview. Paths in fake data are written for macOS
// (`/Users/me/…`) and are shown as `C:\Users\me\…`; paths the page sends back are turned around before a handler sees
// them. `&gitbash=0` simulates a machine without Git for Windows. Design review only: the real rules live in Rust.
// Read on every call (like `?news=`), so tests that change the URL see the new value.
const param = (k: string) => (typeof location === "undefined" ? null : new URLSearchParams(location.search).get(k));
export const mockWindows = () => param("os") === "win";
const gitBash = () => param("gitbash") !== "0";

export const platformHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  platform: () => ({ os: mockWindows() ? "windows" : "macos", gitBash: !mockWindows() || gitBash() }),
};

/** Backend refusals on Windows (`ReasonCode::MacOnly`), by command; `start_task` only for a local-only folder. */
const MAC_ONLY = ["news_fetch", "usage_insights_claude", "refine_brief", "cloud_routines", "update_cloud_routine", "force_stop", "set_local_only", "set_local_only_cwd"];
export function macOnlyRefusal(cmd: string): unknown {
  if (!mockWindows() || !MAC_ONLY.includes(cmd)) return null;
  const text = "Mac 전용이에요 · Windows 미리보기에서는 아직 쓸 수 없어요";
  return { kind: "invalidInput", code: "macOnly", params: {}, text, reason: text };
}

const toWin = (s: string) => `C:${s.replace(/\//g, "\\")}`;
const toMac = (s: string) => s.slice(2).replace(/\\/g, "/");

function walk(v: unknown, f: (s: string) => string): unknown {
  if (typeof v === "string") return f(v);
  if (Array.isArray(v)) return v.map((x) => walk(x, f));
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x, f)]));
  return v;
}

/** Output: whole-string macOS paths under /Users become `C:\Users\…` (text that only mentions a path is left alone). */
export const winOut = <T,>(v: T): T => (mockWindows() ? (walk(v, (s) => (/^\/Users(\/|$)/.test(s) ? toWin(s) : s)) as T) : v);
/** Input: `C:\Users\…` back to `/Users/…` so the macOS-written handlers find their rows. */
export const winIn = (a: Record<string, unknown>): Record<string, unknown> =>
  mockWindows() ? (walk(a, (s) => (/^[Cc]:\\Users(\\|$)/.test(s) ? toMac(s) : s)) as Record<string, unknown>) : a;
