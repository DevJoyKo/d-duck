// app-language review (D-171): the same sentence lives in i18n.rs (fallback text), the UI catalogs (what the screen
// shows) and the mock (read from i18n.rs). They must say the same thing in both languages.
import { describe, expect, it } from "vitest";
import { reasonKey, rustRows } from "../mock/rustTable";
import koErrors from "../i18n/ko/errors";
import enErrors from "../i18n/en/errors";
import koTasks from "../i18n/ko/tasks";
import enTasks from "../i18n/en/tasks";
import { localize } from "../mock/english";

const rows = rustRows();
const row = (table: string, name: string) => {
  const r = rows.find((x) => x.table === table && x.name === name);
  if (!r) throw new Error(`${table}::${name} missing`);
  return r;
};
const other = (v: string | { other: string }) => (typeof v === "string" ? v : v.other);

describe("Rust and UI wording", () => {
  it("reads every table of i18n.rs, guard constants included", () => {
    expect(new Set(rows.map((r) => r.table))).toEqual(new Set(["Code", "ReasonCode", "Fail"]));
    expect(row("ReasonCode", "GuardUnreadable").ko).toBe("원격 push 막기 설정을 못 읽었어요");
  });

  it("words every InvalidInput reason code exactly as the UI catalog does, in both languages", () => {
    const reasons = rows.filter((r) => r.table === "ReasonCode");
    expect(reasons.map((r) => reasonKey(r.name)).sort()).toEqual(Object.keys(koErrors.invalidInput).sort());
    for (const r of reasons) {
      const k = reasonKey(r.name) as keyof typeof koErrors.invalidInput;
      expect([r.name, koErrors.invalidInput[k]]).toEqual([r.name, r.ko]);
      expect([r.name, enErrors.invalidInput[k]]).toEqual([r.name, r.en]);
    }
  });

  it("builds the usage-limit banner fallback from the same pieces the board shows", () => {
    const pairs: [string, string | { other: string }, string | { other: string }][] = [
      ["LimitBanner", koTasks.limits.banner, enTasks.limits.banner],
      ["LimitAuto", koTasks.limits.bannerAuto, enTasks.limits.bannerAuto],
      ["LimitManual", koTasks.limits.bannerManual, enTasks.limits.bannerManual],
      ["LimitAwake", koTasks.limits.autoNote, enTasks.limits.autoNote],
    ];
    for (const [name, ko, en] of pairs) {
      const r = row("Code", name);
      expect([name, r.ko]).toEqual([name, other(ko)]);
      expect([name, r.en]).toEqual([name, other(en)]);
    }
  });

  it("gives one Korean sentence one English sentence across the Rust tables", () => {
    const seen = new Map<string, string>();
    for (const r of rows) {
      if (seen.has(r.ko)) expect([r.name, r.en]).toEqual([r.name, seen.get(r.ko)]);
      else seen.set(r.ko, r.en);
    }
  });

  it("lets the mock speak Rust's English for every row without parameters", () => {
    for (const r of rows.filter((x) => !x.ko.includes("{"))) expect(localize(r.ko, "en", "ko")).toBe(r.en);
  });
});
