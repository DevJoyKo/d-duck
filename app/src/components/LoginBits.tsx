// Login banner, gate line, progress sheet and settings row (claude-login). Copy is fixed (lib/auth COPY).
import { CheckCircle2, Loader2, LogIn, RefreshCw, TerminalSquare } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { cancelLogin, closeLoginSheet, COPY, gateFor, loginInTerminal, openLoginLink, refreshAuth, startLogin, useAuth, type Need } from "../lib/auth";
import { connectionText } from "../lib/api";
import { useT } from "../i18n/react";
import { Button, MoreInfo } from "./ui";

/** The one app-wide banner while logged out. Hidden when the CLI itself is missing (that notice comes first). */
export function LoginBanner({ cliMissing }: { cliMissing: boolean }) {
  const t = useT();
  const { view } = useAuth();
  if (cliMissing || view?.login !== "loggedOut") return null;
  return (
    <div role="status" data-testid="login-banner" className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-line bg-sheet px-6 py-2.5 text-[13px] text-ink">
      <LogIn size={15} aria-hidden className="shrink-0 text-ink-2" />
      <span className="min-w-0 flex-1">{COPY.banner}</span>
      <button type="button" className="text-[12px] text-ink-2 underline-offset-2 hover:underline" onClick={() => startLogin("console")}>
        {COPY.console}
      </button>
      <Button variant="primary" size="sm" onClick={() => startLogin("claudeAi")}>
        {t("auth.actions.login")}
      </Button>
    </div>
  );
}

/** One line next to a button that needs a login. `force` shows it for a backend refusal even before the state
 *  arrives (the state may still be loading or stale). `more` adds a 자세히 with how to get unblocked. */
export function GateLine({ need = "model", force, more, className = "" }: { need?: Need; force?: "login" | "claudeAi" | null; more?: boolean; className?: string }) {
  const t = useT();
  const { view } = useAuth();
  const gate = force ?? gateFor(view, need);
  if (!gate) return null;
  return (
    <div role="status" data-testid="gate-line" className={`flex flex-wrap items-center gap-2 text-[12px] text-ink-2 ${className}`}>
      {/* Icon and text wrap together, so the icon never sits alone on a line. */}
      <span className="inline-flex items-start gap-2">
        <LogIn size={12} aria-hidden className="mt-0.5 shrink-0" />
        <span>{gate === "claudeAi" ? COPY.cloud : COPY.gate}</span>
      </span>
      {/* While the app-wide banner is up, its [로그인] is the only one. */}
      {gate === "login" && view?.login !== "loggedOut" && (
        <Button size="xs" onClick={() => startLogin("claudeAi")}>
          {t("auth.actions.login")}
        </Button>
      )}
      {more && <MoreInfo>{gate === "claudeAi" ? COPY.cloudHint : COPY.loginHint}</MoreInfo>}
    </div>
  );
}

/** "Claude 로그인이 곧 만료돼요" [다시 로그인]: soft, never blocks. */
export function ExpiryLine({ className = "" }: { className?: string }) {
  const t = useT();
  const { view } = useAuth();
  if (!view?.expiresSoon) return null;
  return (
    <p role="status" data-testid="expiry-line" className={`flex flex-wrap items-center gap-2 text-[12px] text-ink-2 ${className}`}>
      <span>{COPY.expiry}</span>
      <Button size="xs" onClick={() => startLogin("claudeAi")}>
        {t("auth.actions.relogin")}
      </Button>
    </p>
  );
}

/** Settings: connection method, unknown state with [다시 확인] and expiry. How to log out is the row's 자세히. */
export function AuthSettingsRow() {
  const t = useT();
  const { view } = useAuth();
  if (!view) return null;
  return (
    <div className="space-y-1.5" data-testid="auth-row">
      {view.login === "loggedIn" && (
        <p className="text-[13px] text-ink">
          {connectionText(view.connectionCode, view.connection)}
          {view.plan && <span className="ml-1.5 text-ink-3">· {view.plan}</span>}
        </p>
      )}
      {view.login === "loggedOut" && (
        <p className="flex flex-wrap items-center gap-2 text-[13px] text-ink">
          <span>{COPY.settingsOut}</span>
          <Button size="sm" onClick={() => startLogin("claudeAi")}>
            {t("auth.actions.login")}
          </Button>
          <Button size="sm" onClick={() => startLogin("console")}>
            {COPY.console}
          </Button>
        </p>
      )}
      {view.login === "unknown" && (
        <p className="flex flex-wrap items-center gap-2 text-[13px] text-ink-2">
          <span>{COPY.unknown}</span>
          <Button size="sm" onClick={() => void refreshAuth()}>
            <RefreshCw size={12} aria-hidden />
            {t("auth.actions.recheck")}
          </Button>
        </p>
      )}
      <ExpiryLine />
    </div>
  );
}

/** Browser-login progress. Esc while running asks before cancelling; focus stays inside. The URL never reaches
 *  this view. */
export function LoginSheet() {
  const t = useT();
  const { sheet, login, terminalCommand } = useAuth();
  const box = useRef<HTMLDivElement>(null);
  const [asking, setAsking] = useState(false);
  const succeeded = login.phase === "succeeded";
  const running = login.phase === "running";
  useEffect(() => {
    if (!sheet) return;
    box.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [sheet, login.phase, asking]);
  // Leaving "running" drops the cancel question (state adjusted during render, not in an effect).
  const [wasRunning, setWasRunning] = useState(running);
  if (wasRunning !== running) {
    setWasRunning(running);
    if (!running) setAsking(false);
  }
  useEffect(() => {
    if (!sheet || !succeeded) return;
    const timer = setTimeout(closeLoginSheet, 1500);
    return () => clearTimeout(timer);
  }, [sheet, succeeded]);
  if (!sheet) return null;

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (running) setAsking((a) => !a);
      else closeLoginSheet();
      return;
    }
    if (e.key !== "Tab") return;
    const items = Array.from(box.current?.querySelectorAll<HTMLElement>("button") ?? []);
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  const title = terminalCommand ? t("auth.terminal.title") : succeeded ? COPY.done : login.phase === "failed" ? COPY.failed : COPY.progress;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-scrim/60 p-4 backdrop-blur-[2px]">
      <div
        ref={box}
        role="dialog"
        aria-modal="true"
        aria-labelledby="login-sheet-title"
        onKeyDown={onKeyDown}
        data-testid="login-sheet"
        className="w-full max-w-[420px] rounded-2xl border border-line bg-sheet p-5 text-ink shadow-2xl ring-1 ring-ink/10"
      >
        <h2 id="login-sheet-title" className="flex items-center gap-2 text-[15px] font-semibold">
          {succeeded ? <CheckCircle2 size={16} aria-hidden className="text-ok" /> : running ? <Loader2 size={16} aria-hidden className="animate-spin text-ink-2" /> : <LogIn size={16} aria-hidden className="text-ink-2" />}
          {title}
        </h2>
        {terminalCommand ? (
          <>
            <p className="mt-2 text-[12px] text-ink-2">{t("auth.terminal.body")}</p>
            <code className="mt-2 block rounded-lg border border-line bg-paper px-3 py-2 text-[12px] break-all">{terminalCommand}</code>
            <div className="mt-4 flex justify-end gap-2">
              <Button onClick={() => void navigator.clipboard?.writeText(terminalCommand)}>{t("auth.actions.copyCommand")}</Button>
              <Button variant="primary" onClick={closeLoginSheet}>
                {t("common.actions.close")}
              </Button>
            </div>
          </>
        ) : running && asking ? (
          <>
            <p className="mt-2 text-[13px] text-ink-2">{COPY.cancelAsk}</p>
            <div className="mt-4 flex justify-end gap-2">
              <Button onClick={() => setAsking(false)}>{t("auth.actions.keepWaiting")}</Button>
              <Button variant="primary" onClick={() => void cancelLogin()}>
                {t("auth.actions.cancelLogin")}
              </Button>
            </div>
          </>
        ) : running ? (
          <>
            <p className="mt-2 text-[13px] text-ink-2">{COPY.waiting}</p>
            <div className="mt-4 flex flex-wrap items-center justify-end gap-2">
              {login.hasLink && (
                <Button className="mr-auto" onClick={() => void openLoginLink()}>
                  {COPY.link}
                </Button>
              )}
              <Button onClick={() => void cancelLogin()}>{t("common.actions.cancel")}</Button>
            </div>
          </>
        ) : login.phase === "failed" ? (
          <>
            {login.reason && <p className="mt-2 text-[13px] text-ink-2" data-testid="login-fail-reason">{t(`auth.fail.${login.reason}`)}</p>}
            <div className="mt-4 flex flex-wrap items-center justify-end gap-2">
              <Button onClick={closeLoginSheet}>{t("common.actions.close")}</Button>
              <Button variant={login.reason === "needsTerminal" ? "primary" : "quiet"} onClick={() => void loginInTerminal()}>
                <TerminalSquare size={13} aria-hidden />
                {t("auth.actions.inTerminal")}
              </Button>
              <Button variant={login.reason === "needsTerminal" ? "quiet" : "primary"} onClick={() => void startLogin(login.method ?? "claudeAi")}>
                {t("auth.actions.retry")}
              </Button>
            </div>
          </>
        ) : (
          <>
            <p className="mt-2 text-[13px] text-ink-2">{COPY.doneBody}</p>
            <div className="mt-4 flex justify-end">
              <Button variant="primary" onClick={closeLoginSheet}>
                {t("common.actions.close")}
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
