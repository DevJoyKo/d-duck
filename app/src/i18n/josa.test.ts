import { describe, expect, it } from "vitest";
import { ro } from "./josa";

describe("ro", () => {
  it.each([
    ["수집", "으로"],
    ["메일", "로"],
    ["분류", "로"],
    ["plan", "으로"],
    ["team", "으로"],
    ["sync", "로"],
    ["triage-v2", "로"],
    ["v10", "으로"],
    ["step3", "으로"],
    ["v1", "로"],
    ["ping", "으로"],
    ["bug", "로"],
    ["a-b)", "로"],
    ["", "로"],
  ])("%s → %s, the same table as dduck_core::i18n::ro", (w, p) => {
    expect(ro(w, "ko")).toBe(p);
  });

  it("is empty in English", () => {
    expect(ro("plan", "en")).toBe("");
  });
});
