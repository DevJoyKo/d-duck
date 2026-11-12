import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef, useState } from "react";
import { api, messageOf } from "../lib/api";
import { CELL_RATIO, estimateGeometry, fitFontSize, lastScreen, LOG_FONT, measureCellRatio } from "../lib/terminal";
import { isMac } from "../lib/platform";
import { useT } from "../i18n/react";
import { t as tNow } from "../i18n";

/** xterm paints on a canvas, so it gets the terminal tokens (index.css --color-term*) as values, not classes. */
function termTheme() {
  const css = getComputedStyle(document.documentElement);
  const read = (name: string) => css.getPropertyValue(name).trim() || undefined;
  const background = read("--color-term");
  return { background, foreground: read("--color-term-ink"), cursor: background };
}

/**
 * Shows the latest terminal screen of a background session. `claude logs` returns a raw TUI stream, so it is drawn
 * by a terminal emulator — never parsed as HTML — and kept in memory only (D-014, local-agent-tasks: 로그 보기).
 */
export function LogView({ id, live }: { id: string; live: boolean }) {
  const t = useT();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  // The error itself, worded at render time so a later language change applies (D-171).
  const [error, setError] = useState<{ cause: unknown } | null>(null);

  useEffect(() => {
    if (!host.current) return;
    // xterm's own screen-reader labels, in the app language (read when the terminal is created).
    Terminal.strings.promptLabel = tNow("tasks.detail.terminalInput");
    Terminal.strings.tooMuchOutput = tNow("tasks.detail.terminalTooMuch");
    // Links in the stream (OSC 8, URL detection) must never open anything (D-017).
    const xterm = new Terminal({
      convertEol: false,
      disableStdin: true,
      fontSize: 11,
      cols: 180,
      rows: 48,
      scrollback: 0,
      fontFamily: LOG_FONT,
      theme: termTheme(),
      linkHandler: { activate: () => {}, allowNonHttpProtocols: false },
      // Only for `unicode` below: Claude Code lays out emoji such as ✅ as two cells (Unicode 11+ widths).
      allowProposedApi: true,
      // Claude Code paints fixed colors (white ⏺, dim gray) for a dark terminal; keep them readable on the light theme.
      minimumContrastRatio: 4.5,
    });
    xterm.loadAddon(new Unicode11Addon());
    xterm.unicode.activeVersion = "11";
    xterm.parser.registerOscHandler(8, () => true);
    xterm.open(host.current);
    term.current = xterm;
    return () => {
      xterm.dispose();
      term.current = null;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let shown = "";
    let ratio: number | undefined;
    const tick = async () => {
      try {
        const out = lastScreen(await api.taskLogs(id));
        if (cancelled) return;
        setError(null);
        // Redraw only on change, by homing and clearing instead of reset() (no blank flash).
        if (out !== shown) {
          shown = out;
          const xterm = term.current;
          if (xterm) {
            // Emulate the PTY size the stream was drawn for; a smaller grid clamps its rows/columns onto each other.
            const { cols, rows } = estimateGeometry(out);
            if (cols !== xterm.cols || rows !== xterm.rows) xterm.resize(cols, rows);
            const width = (host.current?.clientWidth ?? 0) - 24; // p-3 on both sides
            // macOS keeps the fixed SF Mono ratio; elsewhere the installed font is measured once (windows-support 4.4).
            xterm.options.fontSize = fitFontSize(cols, width, (ratio ??= isMac() ? CELL_RATIO : measureCellRatio(LOG_FONT)));
            xterm.write("\x1b[0m\x1b[H\x1b[2J" + out);
          }
        }
      } catch (e) {
        if (!cancelled) setError({ cause: e });
      }
      if (!cancelled && live) timer = setTimeout(tick, 2000);
    };
    tick();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [id, live]);

  return (
    <div className="space-y-1">
      {error && <p className="text-xs text-danger">{messageOf(error.cause)}</p>}
      <div ref={host} className="overflow-auto rounded-xl bg-term p-3" aria-label={t("tasks.detail.screen")} />
    </div>
  );
}
