import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthView, LoginStatus } from "../lib/auth";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const auth = await import("../lib/auth");
const { LoginBanner, GateLine, LoginSheet, AuthSettingsRow, ExpiryLine } = await import("./LoginBits");
const { LimitBanner } = await import("./LimitBits");

const view = (login: AuthView["login"], extra: Partial<AuthView> = {}): AuthView => ({
  login,
  method: login === "loggedIn" ? "claudeAi" : login === "loggedOut" ? "none" : null,
  connection: login === "loggedIn" ? "Claude 계정으로 연결돼 있어요" : null,
  plan: null,
  claudeAi: login === "loggedIn",
  expiresSoon: false,
  ...extra,
});
const status = (phase: LoginStatus["phase"], extra: Partial<LoginStatus> = {}): LoginStatus => ({ phase, reason: null, hasLink: false, method: "claudeAi", ...extra });

beforeEach(() => {
  auth.resetAuthForTests();
  invoke.mockReset();
});
afterEach(cleanup);

describe("login banner", () => {
  it.each([
    ["loggedOut", false, 1],
    ["loggedIn", false, 0],
    ["unknown", false, 0],
    ["loggedOut", true, 0],
  ] as const)("%s (cli missing: %s) shows %i banner", (login, cliMissing, n) => {
    auth.setAuthForTests({ view: view(login) });
    render(<LoginBanner cliMissing={cliMissing} />);
    expect(screen.queryAllByTestId("login-banner")).toHaveLength(n);
  });

  it("is hidden before the state arrives", () => {
    render(<LoginBanner cliMissing={false} />);
    expect(screen.queryByTestId("login-banner")).toBeNull();
  });

  it("[로그인] starts the claude.ai login and opens the sheet; Console is the second choice", async () => {
    auth.setAuthForTests({ view: view("loggedOut") });
    invoke.mockResolvedValue(status("running"));
    render(
      <>
        <LoginBanner cliMissing={false} />
        <LoginSheet />
      </>,
    );
    fireEvent.click(screen.getByRole("button", { name: "로그인" }));
    await waitFor(() => expect(screen.getByTestId("login-sheet")).toBeTruthy());
    expect(invoke).toHaveBeenCalledWith("start_login", { method: "claudeAi" });
    fireEvent.click(screen.getByRole("button", { name: auth.COPY.console }));
    expect(invoke).toHaveBeenCalledWith("start_login", { method: "console" });
  });

  it("hides the limit banner while logged out and brings it back after login", async () => {
    invoke.mockImplementation(async (cmd) => (cmd === "limit_status" ? { banner: { text: "한도가 풀리면 이어서 해요" }, late: 0, unknown: 0, entries: [] } : undefined));
    auth.setAuthForTests({ view: view("loggedOut") });
    const qc = new QueryClient();
    render(
      <QueryClientProvider client={qc}>
        <LoginBanner cliMissing={false} />
        <LimitBanner />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(qc.getQueryData(["limits"])).toBeTruthy());
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByTestId("login-banner")).toBeTruthy();
    act(() => auth.setAuthForTests({ view: view("loggedIn") }));
    expect(screen.queryByTestId("login-banner")).toBeNull();
    expect(screen.getByText("한도가 풀리면 이어서 해요")).toBeTruthy();
  });
});

describe("gate line", () => {
  it("shows the gate copy when logged out (the banner owns [로그인]), nothing when logged in or unknown", () => {
    auth.setAuthForTests({ view: view("loggedOut") });
    const { rerender } = render(<GateLine />);
    expect(screen.getByText(auth.COPY.gate)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "로그인" })).toBeNull();
    act(() => auth.setAuthForTests({ view: view("loggedIn") }));
    rerender(<GateLine />);
    expect(screen.queryByTestId("gate-line")).toBeNull();
    act(() => auth.setAuthForTests({ view: view("unknown") }));
    rerender(<GateLine />);
    expect(screen.queryByTestId("gate-line")).toBeNull();
  });

  it("cloud routines with a token connection: the claude.ai line, no login button", () => {
    auth.setAuthForTests({ view: view("loggedIn", { method: "oauthToken", claudeAi: false, connection: "토큰으로 연결돼 있어요" }) });
    render(
      <>
        <GateLine need="claudeAi" />
        <GateLine />
      </>,
    );
    expect(screen.getAllByTestId("gate-line")).toHaveLength(1);
    expect(screen.getByText(auth.COPY.cloud)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "로그인" })).toBeNull();
  });

  it("a backend refusal shows the same line even while the state is unknown", () => {
    expect(auth.isGateError({ kind: "login_required" })).toBe("login");
    expect(auth.isGateError({ kind: "claude_ai_required" })).toBe("claudeAi");
    expect(auth.isGateError({ kind: "spawn" })).toBeNull();
    expect(auth.isGateError(auth.COPY.gate)).toBeNull(); // codes only, never the message text
    expect(auth.isGateError("Claude CLI가 실패했어요. 다시 시도하세요")).toBeNull();
    render(<GateLine force={auth.isGateError({ kind: "login_required" })} />);
    expect(screen.getByText(auth.COPY.gate)).toBeTruthy();
    expect(screen.getByRole("button", { name: "로그인" })).toBeTruthy(); // no banner yet, so the line offers it
  });
});

describe("login sheet", () => {
  it("running: progress, [취소], and the link button only with has_link", () => {
    auth.setAuthForTests({ sheet: true, login: status("running") });
    const { rerender } = render(<LoginSheet />);
    expect(screen.getByText(auth.COPY.progress)).toBeTruthy();
    expect(screen.queryByRole("button", { name: auth.COPY.link })).toBeNull();
    act(() => auth.setAuthForTests({ login: status("running", { hasLink: true }) }));
    rerender(<LoginSheet />);
    expect(screen.getByRole("button", { name: auth.COPY.link })).toBeTruthy();
  });

  it("failed: [다시 시도] and [터미널에서 로그인]", () => {
    auth.setAuthForTests({ sheet: true, login: status("failed", { reason: "timeout" }) });
    render(<LoginSheet />);
    expect(screen.getByText(auth.COPY.failed)).toBeTruthy();
    expect(screen.getByRole("button", { name: "다시 시도" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /터미널에서 로그인/ })).toBeTruthy();
  });

  it("succeeded says 로그인했어요 with a body line", () => {
    auth.setAuthForTests({ sheet: true, login: status("succeeded") });
    render(<LoginSheet />);
    expect(screen.getByText(auth.COPY.done)).toBeTruthy();
    expect(screen.getByText(auth.COPY.doneBody)).toBeTruthy();
  });

  it("Esc asks before cancelling a running login; focus starts inside and Tab wraps", async () => {
    invoke.mockResolvedValue(status("idle"));
    auth.setAuthForTests({ sheet: true, login: status("running", { hasLink: true }) });
    render(<LoginSheet />);
    expect(screen.getByText(auth.COPY.waiting)).toBeTruthy();
    const buttons = screen.getAllByRole("button");
    expect(document.activeElement).toBe(buttons[0]);
    buttons[buttons.length - 1].focus();
    fireEvent.keyDown(screen.getByTestId("login-sheet"), { key: "Tab" });
    expect(document.activeElement).toBe(buttons[0]);
    fireEvent.keyDown(screen.getByTestId("login-sheet"), { key: "Escape" });
    expect(screen.getByText(auth.COPY.cancelAsk)).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("cancel_login", undefined);
    fireEvent.click(screen.getByRole("button", { name: "계속 기다리기" }));
    expect(screen.getByText(auth.COPY.waiting)).toBeTruthy();
    fireEvent.keyDown(screen.getByTestId("login-sheet"), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "로그인 취소" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("cancel_login", undefined));
    await waitFor(() => expect(screen.queryByTestId("login-sheet")).toBeNull());
  });

  it.each([
    ["timeout", "5분이 지나 기다림을 멈췄어요", "다시 시도"],
    ["exited", "Claude CLI가 로그인 없이 끝났어요", "다시 시도"],
    ["needsTerminal", "코드를 붙여 넣어야 해서 터미널에서 이어 가야 해요", "터미널에서 로그인"],
  ] as const)("failed (%s) says why in one line; primary is %s", (reason, text, primary) => {
    auth.setAuthForTests({ sheet: true, login: status("failed", { reason }) });
    render(<LoginSheet />);
    expect(screen.getByTestId("login-fail-reason").textContent).toBe(text);
    const btn = screen.getByRole("button", { name: new RegExp(primary) });
    expect(btn.className).toContain("bg-ink");
  });

  it("the copy command shows when Terminal could not be opened", async () => {
    invoke.mockResolvedValue({ opened: false, command: "env -u CLAUDE_CONFIG_DIR '/c/claude' auth login --claudeai" });
    auth.setAuthForTests({ sheet: true, login: status("failed", { reason: "needsTerminal" }) });
    render(<LoginSheet />);
    fireEvent.click(screen.getByRole("button", { name: /터미널에서 로그인/ }));
    await waitFor(() => expect(screen.getByText(/auth login --claudeai/)).toBeTruthy());
    expect(invoke).toHaveBeenCalledWith("login_in_terminal", { method: "claudeAi" });
  });
});

describe("copy and loading", () => {
  it("the gate copy is the Rust constant (one source of truth)", () => {
    const rust = Object.values(import.meta.glob("../../crates/dduck-core/src/auth_status.rs", { query: "?raw", import: "default", eager: true }))[0] as string;
    const constant = (name: string) => rust.match(new RegExp(`pub const ${name}: &str = "([^"]+)";`))?.[1];
    expect(constant("LOGIN_REQUIRED_TEXT")).toBe(auth.COPY.gate);
    expect(constant("CLAUDE_AI_REQUIRED_TEXT")).toBe(auth.COPY.cloud);
  });

  it("listeners come first, and an event that beats the first reply wins", async () => {
    const order: string[] = [];
    let fire: ((e: { payload: unknown }) => void) | undefined;
    const ev = await import("@tauri-apps/api/event");
    vi.mocked(ev.listen).mockImplementation(async (name, cb) => {
      order.push(`listen:${String(name)}`);
      if (name === "auth-changed") fire = cb as (e: { payload: unknown }) => void;
      return () => {};
    });
    let reply: (v: unknown) => void = () => {};
    invoke.mockImplementation((cmd) => {
      order.push(cmd);
      return cmd === "auth_state" ? new Promise((r) => (reply = r)) : Promise.resolve(status("idle"));
    });
    const loading = auth.loadAuth();
    await loading;
    expect(order.slice(0, 2).every((o) => o.startsWith("listen:"))).toBe(true);
    act(() => fire?.({ payload: view("loggedIn") }));
    await act(async () => reply(view("loggedOut")));
    let seen: AuthView | null = null;
    function Probe() {
      seen = auth.useAuth().view;
      return null;
    }
    render(<Probe />);
    expect(seen).toEqual(view("loggedIn"));
  });
});

describe("local routine run now", () => {
  it("is off with the reason when logged out, back on after login", async () => {
    const routine = { id: 1, name: "아침 점검", cwd: "/w", promptLen: 3, permissionMode: "plan", schedule: { kind: "daily", hour: 9, minute: 0 }, scheduleText: "매일 09:00", enabled: true, nextRunAt: null, lastStatus: null, active: false };
    invoke.mockImplementation(async (cmd) => (cmd === "list_routines" ? [routine] : cmd === "routine_runs" ? [] : undefined));
    auth.setAuthForTests({ view: view("loggedOut") });
    const { RoutinesPage } = await import("./RoutinesPage");
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RoutinesPage onOpenTask={() => undefined} />
      </QueryClientProvider>,
    );
    const run = await screen.findByRole("button", { name: /지금 실행/ });
    expect((run as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getAllByText(auth.COPY.gate).length).toBeGreaterThan(0);
    act(() => auth.setAuthForTests({ view: view("loggedIn") }));
    expect((screen.getByRole("button", { name: /지금 실행/ }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("cloud routines dead end", () => {
  it.each([
    [view("loggedOut"), auth.COPY.gate],
    [view("loggedIn", { method: "oauthToken", claudeAi: false }), auth.COPY.cloud],
  ])("the load button is off with the reason next to it (%o)", async (v, text) => {
    invoke.mockImplementation(async (cmd) => (cmd === "list_routines" ? [] : undefined));
    auth.setAuthForTests({ view: v });
    const { RoutinesPage } = await import("./RoutinesPage");
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RoutinesPage onOpenTask={() => undefined} />
      </QueryClientProvider>,
    );
    const load = await screen.findByRole("button", { name: "클라우드 루틴 불러오기" });
    expect((load as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(text)).toBeTruthy();
    expect(screen.getByText("자세히")).toBeTruthy();
    act(() => auth.setAuthForTests({ view: view("loggedIn") }));
    expect((screen.getByRole("button", { name: "클라우드 루틴 불러오기" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("a 자세히 explains how to get a claude.ai login", () => {
    auth.setAuthForTests({ view: view("loggedIn", { method: "oauthToken", claudeAi: false }) });
    render(<GateLine need="claudeAi" more />);
    expect(screen.getByText("자세히")).toBeTruthy();
    expect(screen.getByText(auth.COPY.cloudHint)).toBeTruthy();
  });
});

describe("settings row and expiry", () => {
  it.each([
    [view("loggedIn", { plan: "max_5x" }), "Claude 계정으로 연결돼 있어요"],
    [view("loggedIn", { method: "oauthToken", claudeAi: false, connection: "토큰으로 연결돼 있어요" }), "토큰으로 연결돼 있어요"],
    [view("loggedIn", { method: "thirdParty", claudeAi: false, connection: "다른 방식으로 연결돼 있어요" }), "다른 방식으로 연결돼 있어요"],
    [view("unknown"), auth.COPY.unknown],
    [view("loggedOut"), auth.COPY.settingsOut],
  ])("shows %o as %s", (v, text) => {
    auth.setAuthForTests({ view: v });
    render(<AuthSettingsRow />);
    expect(screen.getByText(text)).toBeTruthy();
    const loginButton = screen.queryByRole("button", { name: "로그인" });
    expect(loginButton !== null).toBe(v.login === "loggedOut");
  });

  it("[다시 확인] refreshes", async () => {
    invoke.mockResolvedValue(view("loggedIn"));
    auth.setAuthForTests({ view: view("unknown") });
    render(<AuthSettingsRow />);
    fireEvent.click(screen.getByRole("button", { name: /다시 확인/ }));
    await waitFor(() => expect(screen.getByText("Claude 계정으로 연결돼 있어요")).toBeTruthy());
  });

  it("expiry notice only when the backend says it is soon (2 days yes, 10 days no)", () => {
    auth.setAuthForTests({ view: view("loggedIn", { expiresSoon: true }) });
    const { rerender } = render(<ExpiryLine />);
    expect(screen.getByText(auth.COPY.expiry)).toBeTruthy();
    expect(screen.getByRole("button", { name: "다시 로그인" })).toBeTruthy();
    act(() => auth.setAuthForTests({ view: view("loggedIn", { expiresSoon: false }) }));
    rerender(<ExpiryLine />);
    expect(screen.queryByTestId("expiry-line")).toBeNull();
  });
});
