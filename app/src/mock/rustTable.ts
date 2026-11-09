// The (ko, en) rows of i18n.rs `table!` lists, read from the Rust source itself (app-language review, D-171): the
// mock's English and the "Rust and UI say the same" test use these instead of keeping their own copies.
import i18nRs from "../../crates/dduck-core/src/i18n.rs?raw";
import guardRs from "../../crates/dduck-core/src/delivery_guard.rs?raw";

export interface RustRow {
  /** `Code`, `ReasonCode` or `Fail`. */
  table: string;
  /** The variant name (`AgentNameFormat`). */
  name: string;
  ko: string;
  en: string;
}

const STR = String.raw`"((?:[^"\\]|\\.)*)"`;

function unescape(s: string): string {
  return s.replace(/\\(.)/g, (_, c: string) => (c === "n" ? "\n" : c === "t" ? "\t" : c));
}

/** `pub const NAME: &str = "…";` in a Rust file. */
function consts(src: string): Map<string, string> {
  return new Map([...src.matchAll(new RegExp(String.raw`pub const (\w+): &str = ${STR};`, "g"))].map((m) => [m[1], unescape(m[2])]));
}

export function rustRows(src: string = i18nRs, guard: string = guardRs): RustRow[] {
  const known = consts(guard);
  const rows: RustRow[] = [];
  for (const block of src.split("table! {").slice(1)) {
    const body = block.slice(0, block.indexOf("\n}\n"));
    const table = /^\s*(?:\/\/\/.*\n\s*)*(\w+) \{/m.exec(body)?.[1] ?? "";
    const re = new RegExp(String.raw`(\w+) => \(\s*(?:${STR}|crate::delivery_guard::(\w+))\s*,\s*${STR}\s*,?\s*\)`, "g");
    for (const m of body.matchAll(re)) {
      const ko = m[2] !== undefined ? unescape(m[2]) : known.get(m[3] ?? "");
      if (ko === undefined) throw new Error(`i18n.rs ${m[1]}: unknown constant ${m[3]}`);
      rows.push({ table, name: m[1], ko, en: unescape(m[4]) });
    }
  }
  return rows;
}

/** The string code the webview reads for a `ReasonCode` (Rust's `ReasonCode::code`: lowerCamelCase). */
export const reasonKey = (name: string) => name[0].toLowerCase() + name.slice(1);
