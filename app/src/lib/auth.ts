// Claude login state and the browser-login sheet (claude-login, D-150, D-155).
// The backend owns the gate; this store only mirrors it so buttons can show the one-line notice up front.
// The webview never sees the sign-in URL, an email, an org or a path: only the fields below.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import { t } from "../i18n";

export type Login = "loggedIn" | "loggedOut" | "unknown";
export type AuthMethod = "none" | "claudeAi" | "oauthToken" | "apiKey" | "apiKeyHelper" | "thirdParty" | "other";
export type LoginMethod = "claudeAi" | "console";

export interface AuthView {
  login: Login;
  method: AuthMethod | null;
  /** `claudeAi` | `token` | `other` (Rust `connection_code`); worded by `connectionText`. */
  connectionCode?: string | null;
  /** Rust's fallback sentence for an unknown code. */
  connection: string | null;
  plan: string | null;
  claudeAi: boolean;
  expiresSoon: boolean;
}

export interface LoginStatus {
  phase: "idle" | "running" | "succeeded" | "failed";
  reason: "timeout" | "exited" | "needsTerminal" | "spawn" | null;
  hasLink: boolean;
  method: LoginMethod | null;
}

export interface TerminalLogin {
  opened: boolean;
  command: string;
}

/** Fixed copy (design 8), read from the `auth` catalog in the current language each time a field is read (never frozen
 *  at load). In Korean, `gate` and `cloud` equal the Rust constants `LOGIN_REQUIRED_TEXT` and `CLAUDE_AI_REQUIRED_TEXT`
 *  in crates/dduck-core/src/auth_status.rs (checked by LoginBits.test.tsx). */
const COPY_KEYS = ["banner", "gate", "cloud", "loginHint", "cloudHint", "settingsOut", "waiting", "cancelAsk", "doneBody", "connector", "progress", "link", "console", "failed", "confirm", "unknown", "expiry", "logout", "done"] as const;
type CopyKey = (typeof COPY_KEYS)[number];
export const COPY = Object.freeze(Object.defineProperties({} as Record<CopyKey, string>, Object.fromEntries(COPY_KEYS.map((k) => [k, { enumerable: true, get: () => t(`auth.${k}`) }]))));

export const authApi = {
  state: () => invoke<AuthView>("auth_state"),
  refresh: () => invoke<AuthView>("refresh_auth"),
  status: () => invoke<LoginStatus>("login_status"),
  start: (method: LoginMethod) => invoke<LoginStatus>("start_login", { method }),
  cancel: () => invoke<LoginStatus>("cancel_login"),
  openLink: () => invoke<boolean>("open_login_link"),
  inTerminal: (method: LoginMethod | null) => invoke<TerminalLogin | null>("login_in_terminal", { method }),
};

const IDLE: LoginStatus = { phase: "idle", reason: null, hasLink: false, method: null };

interface Store {
  /** null = not loaded yet (treated as unknown: nothing is blocked). */
  view: AuthView | null;
  login: LoginStatus;
  sheet: boolean;
  /** Copy command when Terminal.app could not be opened (Automation denied). */
  terminalCommand: string | null;
}

let store: Store = { view: null, login: IDLE, sheet: false, terminalCommand: null };
const listeners = new Set<() => void>();
let started = false;

function set(next: Partial<Store>) {
  store = { ...store, ...next };
  listeners.forEach((l) => l());
}

function onLoginStatus(s: LoginStatus) {
  set({ login: s });
  if (s.phase === "succeeded") void refreshAuth();
}

/** Once from the shell: listeners first, then the first state. An event that arrives before the first reply is
 *  newer than that reply, so the reply is then ignored. */
export async function loadAuth() {
  if (started) return;
  started = true;
  let viewEvent = false;
  let loginEvent = false;
  await Promise.all([
    listen<AuthView>("auth-changed", (e) => {
      viewEvent = true;
      set({ view: e.payload });
    }),
    listen<LoginStatus>("login-status", (e) => {
      loginEvent = true;
      onLoginStatus(e.payload);
    }),
  ]).catch(() => undefined);
  authApi.state().then(
    (view) => {
      if (!viewEvent) set({ view });
    },
    () => undefined,
  );
  authApi.status().then(
    (s) => {
      if (!loginEvent) onLoginStatus(s);
    },
    () => undefined,
  );
}

/** [다시 확인]. */
export async function refreshAuth() {
  try {
    set({ view: await authApi.refresh() });
  } catch {
    // The state stays as it was; unknown never blocks.
  }
}

/** [로그인] anywhere: opens the sheet and starts (or rejoins) the one login job. */
export async function startLogin(method: LoginMethod = "claudeAi") {
  set({ sheet: true, terminalCommand: null });
  try {
    onLoginStatus(await authApi.start(method));
  } catch {
    set({ login: { ...IDLE, phase: "failed", reason: "spawn", method } });
  }
}

export async function cancelLogin() {
  try {
    onLoginStatus(await authApi.cancel());
  } catch {
    set({ login: IDLE });
  }
  set({ sheet: false });
}

export function closeLoginSheet() {
  if (store.login.phase === "running") return void cancelLogin();
  set({ sheet: false, login: store.login.phase === "succeeded" ? IDLE : store.login });
}

export async function openLoginLink() {
  await authApi.openLink().catch(() => false);
}

export async function loginInTerminal() {
  try {
    const r = await authApi.inTerminal(store.login.method);
    if (r) set({ login: IDLE, terminalCommand: r.opened ? null : r.command, sheet: !r.opened });
  } catch {
    // Another dialog is open, or the CLI is gone: the sheet stays as it was.
  }
}

export function useAuth(): Store {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => store,
  );
}

export type Need = "model" | "claudeAi";

/** Which gate line a spot shows, or null. Unknown (or not loaded) never blocks. */
export function gateFor(view: AuthView | null, need: Need): "login" | "claudeAi" | null {
  if (!view || view.login === "unknown") return null;
  if (view.login === "loggedOut") return "login";
  if (need === "claudeAi" && !view.claudeAi) return "claudeAi";
  return null;
}

/** A backend refusal from the gate. Every gated command returns the same shape, `{kind: "login_required"}` or
 *  `{kind: "claude_ai_required"}` (as its error, or as the `error` of a failed run); matched by code, never by text. */
export function isGateError(e: unknown): "login" | "claudeAi" | null {
  const kind = typeof e === "object" && e !== null ? (e as { kind?: unknown }).kind : undefined;
  if (kind === "login_required") return "login";
  if (kind === "claude_ai_required") return "claudeAi";
  return null;
}

/** Tests and the mock only. */
export function setAuthForTests(next: Partial<Store>) {
  set(next);
}

export function resetAuthForTests() {
  store = { view: null, login: IDLE, sheet: false, terminalCommand: null };
  started = false;
}
