import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import { describe, expect, it } from "vitest";
import { charWidth, estimateGeometry, fitFontSize, lastScreen, MAX_LOG_CHARS } from "./terminal";

const ESC = "\x1b";

/**
 * Synthetic Claude Code frame (no real log bytes): 100 × 30 PTY painted Ink-style with relative moves from home,
 * absolute columns, a long line, Korean and emoji, a status line and the cursor parked on the last row.
 */
function fixtureFrame(): string {
  const long = "x".repeat(96) + "END";
  return [
    `${ESC}[2J${ESC}[H`,
    `\r${ESC}[1B${ESC}[38;2;255;255;255m⏺${ESC}[3G${ESC}[39mChecked 회사 calendar ✅${ESC}[28Gdone`,
    `\r${ESC}[2C${ESC}[1B${long}`,
    `\r${ESC}[1B${ESC}[90G${ESC}[1mright${ESC}[22m`,
    `${ESC}[H\r${ESC}[25B${ESC}[38;5;174m✻${ESC}[39m${ESC}[3GThinking…`,
    `${ESC}[28;1H❯ ${ESC}[K`,
    `${ESC}[29;1H${ESC}[2m? for shortcuts${ESC}[22m`,
    `${ESC}[30;1H${ESC}[28;3H`,
  ].join("");
}

async function render(out: string, cols: number, rows: number): Promise<string[]> {
  const t = new Terminal({ cols, rows, scrollback: 0, allowProposedApi: true });
  t.loadAddon(new Unicode11Addon());
  t.unicode.activeVersion = "11";
  await new Promise<void>((done) => t.write(`${ESC}[0m${ESC}[H${ESC}[2J` + out, done));
  const lines: string[] = [];
  for (let y = 0; y < t.rows; y++) lines.push(t.buffer.active.getLine(y)?.translateToString(true) ?? "");
  t.dispose();
  return lines;
}

describe("lastScreen", () => {
  it("keeps the tail from the final clear", () => {
    expect(lastScreen("old\x1b[2Jmid\x1b[2Jnew")).toBe("\x1b[2Jnew");
    expect(lastScreen("plain")).toBe("plain");
  });

  it("never starts a cut-down stream inside an escape sequence", () => {
    const update = `${ESC}[H\r${ESC}[43B${ESC}[38;2;153;153;153m·${ESC}[39m${ESC}[50;1H`;
    const big = update.repeat(Math.ceil(MAX_LOG_CHARS / update.length) + 3);
    // Shift by a few chars so a naive tail slice lands mid-sequence.
    const s = lastScreen("ab" + big);
    expect(s.length).toBeLessThanOrEqual(MAX_LOG_CHARS);
    expect(s.startsWith(`${ESC}[H`)).toBe(true);
  });
});

describe("estimateGeometry", () => {
  it("finds the PTY size from absolute and relative moves", () => {
    expect(estimateGeometry(fixtureFrame())).toEqual({ cols: 101, rows: 30 });
  });

  it("follows Claude's home-relative updates past 48 rows and 180 columns", () => {
    const s = `${ESC}[H\r${ESC}[61C${ESC}[20B20${ESC}[50;1H${ESC}[48;3H${ESC}[H\r${ESC}[43B${ESC}[193Gx`;
    expect(estimateGeometry(s)).toEqual({ cols: 193, rows: 50 });
  });

  it("counts wide characters twice and stays inside the bounds", () => {
    expect(estimateGeometry("회".repeat(60)).cols).toBe(120);
    expect(estimateGeometry("ab")).toEqual({ cols: 40, rows: 24 });
    expect(estimateGeometry(`${ESC}[999;999H`)).toEqual({ cols: 400, rows: 200 });
    expect(estimateGeometry(`${ESC}[?25l${ESC}]0;title${ESC}\\x`)).toEqual({ cols: 40, rows: 24 });
  });
});

describe("charWidth", () => {
  it("matches Unicode 11 widths Claude Code uses", () => {
    for (const c of ["✅", "❌", "🦆", "회", "漢"]) expect(charWidth(c.codePointAt(0)!)).toBe(2);
    for (const c of ["⏺", "✻", "❯", "─", "·", "a"]) expect(charWidth(c.codePointAt(0)!)).toBe(1);
  });
});

describe("fitFontSize", () => {
  it("shrinks to fit the drawer down to 7 px", () => {
    expect(fitFontSize(60, 476)).toBe(11);
    expect(fitFontSize(100, 476)).toBe(7.5);
    expect(fitFontSize(193, 476)).toBe(7);
    expect(fitFontSize(80, 0)).toBe(11);
  });
});

describe("xterm render snapshot", () => {
  it("draws the frame cleanly at the estimated size", async () => {
    const out = lastScreen(fixtureFrame());
    const { cols, rows } = estimateGeometry(out);
    const lines = await render(out, cols, rows);
    expect(lines[1]).toBe("⏺ Checked 회사 calendar ✅ done");
    expect(lines[2]).toBe("  " + "x".repeat(96) + "END");
    expect(lines[3].trimStart()).toBe("right");
    expect(lines[3].indexOf("right")).toBe(89);
    expect(lines[25]).toBe("✻ Thinking…");
    expect(lines[27]).toBe("❯ ");
    expect(lines[28]).toBe("? for shortcuts");
  });

  it("is what broke before: the old fixed 180 × 48 grid without Unicode 11 overlaps rows", async () => {
    const tall = fixtureFrame().replace(`${ESC}[28;1H`, `${ESC}[58;1H`).replace(`${ESC}[29;1H`, `${ESC}[59;1H`);
    const t = new Terminal({ cols: 180, rows: 48, scrollback: 0 });
    await new Promise<void>((done) => t.write(`${ESC}[H${ESC}[2J` + tall, done));
    const last = t.buffer.active.getLine(47)?.translateToString(true) ?? "";
    const emoji = t.buffer.active.getLine(1)?.translateToString(true) ?? "";
    t.dispose();
    // Unicode 6 widths leave a gap after ✅ where Claude Code jumps to an absolute column.
    expect(emoji).toBe("⏺ Checked 회사 calendar ✅  done");
    // Prompt and status land on the same clamped row.
    expect(last).toContain("for shortcuts");
    const ok = await render(tall, estimateGeometry(tall).cols, estimateGeometry(tall).rows);
    expect(ok[57]).toBe("❯ ");
    expect(ok[58]).toBe("? for shortcuts");
  });
});
