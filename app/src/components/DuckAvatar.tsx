// D.Duck's role duck, the refined "pebble" drawing (D-177, sheet duck-A): no outline, a flat cel shade 13% darker on the
// lower right, two-tone beak and feet, cheeks, rounded items. Four states (working · waiting · failed · idle) plus the
// neutral pose for "done"; the duck's yellow outside the beak and feet is only the waiting speech bubble.
// Three detail levels: full (≥ 56px), small (≥ 28px, one shade, cropped) and tiny (< 28px, flat silhouette).
// Colours go through CSS variables (--duck-body, --duck-shade, --duck-acc, …) with the role colours as fallbacks, so a
// parent can retint a duck without new props. Motion lives in index.css (`duck-*` classes) and stops under
// prefers-reduced-motion. This file is the one place app code may hold raw colours (palette-refresh 1.2).
import { useId } from "react";
import type { Pose } from "../lib/agents";

const INK = "#24283b";
const BEAK = "#ffc83d";
const BEAK_2 = "#f29e1f";
const FOOT = "#f6a623";
const FOOT_2 = "#ffc83d";
const WHITE = "#ffffff";

export type DuckItem = "belt" | "hat" | "tie";

export interface DuckColor {
  body: string;
  /** Accent for the item, a deeper shade of the body hue. Kept as `scarf` for existing callers. */
  scarf: string;
  /** The role's item (D-128): belt for hands-on roles, hat for thinking/making, bow tie for checking/talking. */
  item?: DuckItem;
}

/** Soft body tints with a deeper scarf of the same hue. Order matters: neighbours differ in hue. */
export const DUCK_COLORS: DuckColor[] = [
  { body: "#ffb3a1", scarf: "#d9533b", item: "tie" }, // coral
  { body: "#9fd0ff", scarf: "#2f7dd1", item: "belt" }, // sky
  { body: "#cbe88a", scarf: "#5f8f17", item: "hat" }, // lime
  { body: "#c9b6ff", scarf: "#6b4fd1", item: "tie" }, // lavender
  { body: "#ffbf8a", scarf: "#d06a1f", item: "belt" }, // orange
  { body: "#7fd6d6", scarf: "#147f80", item: "hat" }, // teal
  { body: "#ffb0d0", scarf: "#cc3f7b", item: "tie" }, // pink
  { body: "#9ee6c0", scarf: "#1b8f58", item: "belt" }, // mint
  { body: "#e3a6f0", scarf: "#9a37b3", item: "hat" }, // lilac
  { body: "#b8c2d6", scarf: "#4b6a96", item: "hat" }, // slate (scarf ≥3:1 on the body and on the dark sheet, D-130)
];

/** Role colours from roles.ts that are not in DUCK_COLORS still get their item (the tutor wears a beanie). */
const EXTRA_ITEMS: Record<string, DuckItem> = { "#ffe08a": "hat" };

/** The plain white duck (기본 Claude). */
export const PLAIN_DUCK: DuckColor = { body: WHITE, scarf: "" };

function hash(s: string): number {
  let h = 2166136261;
  for (const ch of s) h = Math.imul(h ^ ch.codePointAt(0)!, 16777619);
  return h >>> 0;
}

/** Stable colours for a list of names: each starts at its own hash slot and moves on if taken, so up to
 *  DUCK_COLORS.length agents on one page never share a colour and a given agent keeps its colour. */
export function assignDuckColors(names: string[]): Map<string, DuckColor> {
  const taken = new Set<number>();
  const out = new Map<string, DuckColor>();
  for (const name of [...names].sort()) {
    let i = hash(name) % DUCK_COLORS.length;
    for (let k = 0; k < DUCK_COLORS.length && taken.has(i); k++) i = (i + 1) % DUCK_COLORS.length;
    taken.add(i);
    out.set(name, DUCK_COLORS[i]);
  }
  return out;
}

/** Linear mix of two #rrggbb colours. */
export function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return "#" + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, "0")).join("");
}

export type DuckDetail = "full" | "small" | "tiny";
export const detailFor = (size: number): DuckDetail => (size >= 56 ? "full" : size >= 28 ? "small" : "tiny");

function itemOf(color: DuckColor): DuckItem | undefined {
  if (!color.scarf) return undefined;
  const body = color.body.toLowerCase();
  return color.item ?? DUCK_COLORS.find((c) => c.body === body)?.item ?? EXTRA_ITEMS[body];
}

/** Fills as CSS variables with the role colours as fallbacks (sheet duck2.js `vars`). */
function palette(color: DuckColor) {
  const plain = !color.scarf;
  const body = plain ? "#fbfbfd" : color.body;
  const acc = color.scarf || "#9aa0ae";
  return {
    body: `var(--duck-body, ${body})`,
    shade: `var(--duck-shade, ${plain ? "#dfe2ea" : mix(body, INK, 0.13)})`,
    acc: `var(--duck-acc, ${acc})`,
    accDark: `var(--duck-acc-dark, ${plain ? "#7c8293" : mix(acc, INK, 0.28)})`,
    accLight: `var(--duck-acc-light, ${plain ? "#c4c8d2" : mix(acc, WHITE, 0.35)})`,
    ink: `var(--duck-ink, ${INK})`,
    light: `var(--duck-light, ${plain ? WHITE : mix(body, WHITE, 0.3)})`,
    cheek: `var(--duck-cheek, ${plain ? "#ff9aa2" : mix(acc, "#ff7a8a", 0.55)})`,
  };
}

export function DuckAvatar({ pose, size = 96, label, color = PLAIN_DUCK }: { pose: Pose; size?: number; label?: string; color?: DuckColor }) {
  const clip = `duck-${useId().replace(/[^A-Za-z0-9_-]/g, "")}`;
  const d = detailFor(size);
  const c = palette(color);
  const item = itemOf(color);
  const plain = !color.scarf;
  const state = pose === "done" ? "neutral" : pose;
  const ry = state === "idle" ? 34 : 36;
  const cy = state === "idle" ? 68 : 66;
  const crop = d !== "full";
  const ey = cy - 8 + (state === "working" ? 2 : 0);
  const by = ey + 6;
  const top = cy - ry;
  const f = (fill: string) => ({ style: { fill } });

  const eyes = () => {
    if (state === "idle") return <path d={`M43.5 ${ey + 1} q5 4 10 0 M66.5 ${ey + 1} q5 4 10 0`} fill="none" style={{ stroke: c.ink }} strokeWidth={d === "full" ? 2.8 : 4} />;
    const rx = d === "full" ? 3.7 : d === "small" ? 4.6 : 5.6;
    const ery = (d === "full" ? 4.6 : rx * 1.15) * (state === "working" ? 0.82 : 1);
    const ex = d === "tiny" ? 4 : 0;
    return (
      <>
        <ellipse cx={48.5 - ex} cy={ey} rx={rx} ry={ery} {...f(c.ink)} />
        <ellipse cx={71.5 + ex} cy={ey} rx={rx} ry={ery} {...f(c.ink)} />
        {d === "full" && (
          <>
            <circle cx="49.8" cy={ey - 1.6} r="1.4" {...f(WHITE)} />
            <circle cx="72.8" cy={ey - 1.6} r="1.4" {...f(WHITE)} />
          </>
        )}
        {state === "waiting" && d !== "tiny" && <path d={`M44 ${ey - 9} q4.5 -3 9 0 M67 ${ey - 9} q4.5 -3 9 0`} fill="none" style={{ stroke: c.ink }} strokeWidth={2.2} />}
        {state === "failed" && d !== "tiny" && <path d={`M43 ${ey - 7} L53 ${ey - 10.5} M77 ${ey - 7} L67 ${ey - 10.5}`} fill="none" style={{ stroke: c.ink }} strokeWidth={2.4} />}
      </>
    );
  };

  const bubble = (bx: number, bby: number, br: number) => (
    <g data-duck-mark="waiting">
      <circle cx={bx} cy={bby} r={br} {...f(BEAK)} />
      {d !== "tiny" && <circle cx={bx + 9} cy={bby + 12} r="3.2" {...f(BEAK)} />}
      <g transform={`translate(${bx} ${bby}) scale(${br / 12})`}>
        <path d="M-3.6 -3 q0 -4.6 3.8 -4.6 q3.8 0 3.8 3.6 q0 2.6 -3.8 3.8 v2" fill="none" style={{ stroke: INK }} strokeWidth={d === "full" ? 2.6 : 3} />
        <circle cx="0.2" cy="6.4" r="1.8" {...f(INK)} />
      </g>
    </g>
  );

  const [wbx, wby, wbr] = d === "tiny" ? [29, 31, 16] : d === "small" ? [24, 28, 15] : [22, 26, 12];
  const [fbx, fby, fbr] = d === "tiny" ? [91, 31, 16] : d === "small" ? [95, 28, 15] : [96, 26, 11.5];

  return (
    <svg viewBox={crop ? "12 14 96 96" : "0 0 120 120"} width={size} height={size} role="img" aria-label={label} data-pose={pose} data-detail={d} className="shrink-0">
      <defs>
        <clipPath id={clip}>
          <ellipse cx="60" cy={cy} rx="38" ry={ry} />
        </clipPath>
      </defs>
      <g strokeLinejoin="round" strokeLinecap="round">
        {!crop && <ellipse cx="60" cy="106" rx="30" ry="4" style={{ fill: "var(--color-duck-ground, rgba(36, 40, 59, 0.08))" }} />}
        {d === "full" && (
          <>
            <ellipse cx="46" cy="101" rx="9" ry="4.6" {...f(FOOT)} />
            <ellipse cx="74" cy="101" rx="9" ry="4.6" {...f(FOOT)} />
            <ellipse cx="45" cy="99.6" rx="6" ry="2.4" {...f(FOOT_2)} />
            <ellipse cx="73" cy="99.6" rx="6" ry="2.4" {...f(FOOT_2)} />
          </>
        )}
        {state === "waiting" && d !== "tiny" && (
          <g className="duck-wave">
            <g transform="rotate(30 96 44)">
              <ellipse cx="96" cy="44" rx="7.5" ry="14" {...f(c.body)} />
              <ellipse cx="98" cy="49" rx="5" ry="9" {...f(c.shade)} opacity=".7" />
            </g>
          </g>
        )}
        {plain && d !== "full" ? (
          <ellipse cx="60" cy={cy} rx="38" ry={ry} style={{ fill: c.body, stroke: "#c9ccd6" }} strokeWidth={d === "tiny" ? 5 : 3} />
        ) : (
          <ellipse cx="60" cy={cy} rx="38" ry={ry} {...f(c.body)} />
        )}
        <g clipPath={`url(#${clip})`}>
          {d !== "tiny" && (
            <>
              <rect x="0" y="0" width="120" height="120" {...f(c.shade)} />
              <ellipse cx={d === "full" ? 55 : 56} cy={cy - (d === "full" ? 7 : 6)} rx="37" ry={ry} {...f(c.body)} />
            </>
          )}
          {d === "full" && <ellipse cx="45" cy={cy - 21} rx="12" ry="6" transform={`rotate(-24 45 ${cy - 21})`} {...f(c.light)} />}
          {item === "hat" && (
            <g data-duck-item="hat">
              <rect x="10" y={top - 4} width="100" height={d === "tiny" ? 22 : 16} {...f(c.acc)} />
              {d !== "tiny" && <path d={`M10 ${top + 6} Q60 ${top + 18} 110 ${top + 6} L110 ${top + 13} Q60 ${top + 25} 10 ${top + 13} Z`} {...f(c.accDark)} />}
              {d === "full" &&
                [42, 51, 60, 69, 78].map((x) => (
                  <path key={x} d={`M${x} ${top + 11.5 + (x === 60 ? 0.4 : x === 51 || x === 69 ? 0 : -1.2)} v3.4`} style={{ stroke: c.accLight }} strokeWidth={1.6} opacity=".7" />
                ))}
            </g>
          )}
          {item === "belt" && (
            <path
              data-duck-item="belt"
              d={d === "tiny" ? "M10 82 Q60 93 110 82 L110 92 Q60 103 10 92 Z" : "M10 84 Q60 95 110 84 L110 92.5 Q60 103.5 10 92.5 Z"}
              {...f(c.acc)}
            />
          )}
        </g>
        {item === "hat" && (
          <>
            <circle cx="60" cy={top - 3} r={d === "tiny" ? 7 : 6.5} {...f(c.acc)} />
            {d === "full" && <circle cx="58" cy={top - 5} r="2.2" {...f(WHITE)} opacity=".45" />}
          </>
        )}
        {item === "belt" && state !== "working" && d !== "tiny" && (
          <g data-duck-buckle>
            <rect x="53.5" y="87" width="13" height="9" rx="3.2" {...f(WHITE)} opacity=".92" />
            <rect x="57" y="89.6" width="6" height="3.8" rx="1.6" {...f(c.acc)} />
          </g>
        )}
        {item === "tie" && (
          <g data-duck-item="tie" transform={`translate(60 84) scale(${d === "tiny" ? 1.25 : 1})`}>
            <path d="M-2 0 C-7 -7.5 -15 -7.5 -15 0 C-15 7.5 -7 7.5 -2 0 Z" {...f(c.acc)} />
            <path d="M2 0 C7 -7.5 15 -7.5 15 0 C15 7.5 7 7.5 2 0 Z" {...f(c.acc)} />
            {d === "full" && <path d="M-12.5 -1.5 C-11 -4 -8 -4 -6 -2" style={{ stroke: c.accLight }} strokeWidth={1.6} fill="none" opacity=".8" />}
            <rect x="-4" y="-4" width="8" height="8" rx="3.2" {...f(c.accDark)} />
          </g>
        )}
        {d === "full" && (
          <>
            <ellipse cx="38.5" cy={cy + 3} rx="5.2" ry="3.2" {...f(c.cheek)} opacity=".38" />
            <ellipse cx="81.5" cy={cy + 3} rx="5.2" ry="3.2" {...f(c.cheek)} opacity=".38" />
          </>
        )}
        {eyes()}
        {d === "tiny" ? (
          <rect x="48" y={by} width="24" height="11" rx="5.5" {...f(BEAK)} />
        ) : (
          <>
            <rect x="49" y={by + 4} width="22" height="7" rx="3.5" {...f(BEAK_2)} />
            <rect x="49" y={by} width="22" height="8" rx="4" {...f(BEAK)} />
          </>
        )}
        {state === "working" &&
          (d === "tiny" ? (
            <rect x="30" y="92" width="60" height="12" rx="6" {...f("#3a405c")} />
          ) : (
            <g data-duck-mark="laptop">
              <path d="M38 88 H82 L86 103 H34 Z" style={{ fill: "#3d4360", stroke: "#3d4360" }} strokeWidth={6} />
              <rect x="25" y="101" width="70" height="8" rx="4" {...f("#2c3148")} />
              {d === "full" && <circle cx="60" cy="95" r="2.6" {...f(WHITE)} opacity=".75" />}
              <ellipse className="duck-tap-l" cx="38" cy="99" rx="8.5" ry="5.2" {...f(c.body)} />
              <ellipse className="duck-tap-r" cx="82" cy="99" rx="8.5" ry="5.2" {...f(c.body)} />
            </g>
          ))}
        {state === "waiting" && bubble(wbx, wby, wbr)}
        {state === "failed" && (
          <g data-duck-mark="failed" className="duck-bob">
            {d === "full" && <path d="M86 44 q-4.5 6.5 0 9.5 q4.5 -3 0 -9.5 Z" {...f("#8ec5ff")} opacity=".9" />}
            <circle cx={fbx} cy={fby} r={fbr} {...f("#e5484d")} />
            <g transform={`translate(${fbx} ${fby}) scale(${fbr / 11.5})`}>
              <path d="M0 -6 v6.6" style={{ stroke: WHITE }} strokeWidth={3.4} />
              <circle cx="0" cy="5.4" r="2" {...f(WHITE)} />
            </g>
          </g>
        )}
        {state === "idle" &&
          (d === "tiny" ? (
            <text x="84" y="42" fontSize="30" fontWeight="800" className="duck-z" style={{ fill: "var(--color-ink-3)" }}>
              z
            </text>
          ) : (
            <>
              <text x="90" y="36" fontSize="16" className="duck-z" style={{ fill: "var(--color-ink-3)" }}>
                z
              </text>
              <text x="100" y="24" fontSize="12" className="duck-z duck-z2" style={{ fill: "var(--color-ink-3)" }}>
                z
              </text>
            </>
          ))}
      </g>
    </svg>
  );
}
