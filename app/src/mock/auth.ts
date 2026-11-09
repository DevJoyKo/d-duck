// Mock Claude login state (claude-login). `?auth=out` logged out, `?auth=token` token connection, `?auth=other`
// cloud provider, `?auth=unknown` unknown, `?auth=soon` claude.ai login expiring in 2 days. Default: claude.ai, max_5x.
// [로그인] in the mock: "running" with a link, then logged in after 4 s (`?auth=out&login=fail` ends as a timeout).
import { emit } from "./event";

const params = (() => {
  try {
    return new URLSearchParams(window.location.search);
  } catch {
    return new URLSearchParams();
  }
})();
const mode = params.get("auth") ?? "";
const loginFails = params.get("login") === "fail";

const IN = { login: "loggedIn", method: "claudeAi", connectionCode: "claudeAi", connection: "Claude 계정으로 연결돼 있어요", plan: "max_5x", claudeAi: true, expiresSoon: false };
let view: Record<string, unknown> =
  mode === "out"
    ? { login: "loggedOut", method: "none", connection: null, plan: null, claudeAi: false, expiresSoon: false }
    : mode === "token"
      ? { login: "loggedIn", method: "oauthToken", connectionCode: "token", connection: "토큰으로 연결돼 있어요", plan: null, claudeAi: false, expiresSoon: false }
      : mode === "other"
        ? { login: "loggedIn", method: "thirdParty", connectionCode: "other", connection: "다른 방식으로 연결돼 있어요", plan: null, claudeAi: false, expiresSoon: false }
        : mode === "unknown"
          ? { login: "unknown", method: null, connection: null, plan: null, claudeAi: false, expiresSoon: false }
          : { ...IN, expiresSoon: mode === "soon" };

export const mockLoggedOut = () => view.login === "loggedOut";
export const mockNotClaudeAi = () => view.login === "loggedIn" && !view.claudeAi;

let status: Record<string, unknown> = { phase: "idle", reason: null, hasLink: false, method: null };
let timer: ReturnType<typeof setTimeout> | null = null;

function setStatus(next: Record<string, unknown>) {
  status = next;
  emit("login-status", status);
}

export const authHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  auth_state: () => view,
  refresh_auth: () => view,
  login_status: () => status,
  start_login: (a) => {
    if (status.phase === "running") return status;
    setStatus({ phase: "running", reason: null, hasLink: true, method: a.method ?? "claudeAi" });
    timer = setTimeout(() => {
      if (loginFails) return setStatus({ ...status, phase: "failed", reason: "timeout", hasLink: false });
      view = IN;
      emit("auth-changed", view);
      setStatus({ ...status, phase: "succeeded", hasLink: false });
    }, 4000);
    return status;
  },
  cancel_login: () => {
    if (timer) clearTimeout(timer);
    status = { phase: "idle", reason: null, hasLink: false, method: null };
    return status;
  },
  open_login_link: () => true,
  login_in_terminal: () => ({ opened: false, command: "env -u CLAUDE_CONFIG_DIR '/Users/me/.local/bin/claude' auth login --claudeai" }),
};
