// Backend codes → text helpers (app-language design 3): CliError kinds, InvalidInput reason codes, usage-limit banner.
import { afterEach, describe, expect, it } from "vitest";
import koErrors from "../i18n/ko/errors";
import enErrors from "../i18n/en/errors";
import { applyLanguage } from "../i18n";
import { cliErrorMessage } from "./format";
import { connectionText, messageOf, pickLang, reasonIs, type CliError } from "./api";

const HANGUL = /[가-힣]/;

afterEach(() => applyLanguage("ko"));

describe("messageOf", () => {
  it("words an invalidInput code from the catalog, with Rust's sentence only for an unknown code", () => {
    const known: CliError = { kind: "invalidInput", code: "agentNotHere", params: {}, text: "이 폴더에서 쓸 수 없는 에이전트예요", reason: "이 폴더에서 쓸 수 없는 에이전트예요" };
    expect(messageOf(known, "ko")).toBe("이 폴더에서 쓸 수 없는 에이전트예요.");
    expect(messageOf(known, "en")).toBe("This agent isn't available in this folder.");
    const future: CliError = { kind: "invalidInput", code: "somethingNew", params: {}, text: "Rust fallback", reason: "Rust fallback" };
    expect(messageOf(future, "en")).toBe("Rust fallback.");
    // An older backend that only sent `reason` still reads as before.
    expect(messageOf({ kind: "invalidInput", reason: "옛 문장" }, "ko")).toBe("옛 문장.");
    expect(messageOf({ kind: "invalidInput", reason: "" }, "en")).toBe("The input isn't valid.");
  });

  it("matches cliErrorMessage in Korean for every kind and has no Korean in English", () => {
    const all: CliError[] = [
      { kind: "spawn" },
      { kind: "timeout" },
      { kind: "untrustedWorkspace", stderr_tail: "" },
      { kind: "failed", code: 2, stderr_tail: "" },
      { kind: "unsupportedOutput" },
      { kind: "invalidId" },
      { kind: "dangerNotConfirmed" },
      { kind: "cancelled" },
      { kind: "killFailed" },
      { kind: "invalidInput", code: "guardHookMissing", reason: "로컬만 작업에 필요한 D.Duck 훅을 준비하지 못했어요" },
    ];
    for (const e of all) {
      expect(messageOf(e, "ko")).toBe(cliErrorMessage(e));
      expect(messageOf(e, "en")).not.toMatch(HANGUL);
    }
    expect(messageOf({ kind: "failed", code: 2, stderr_tail: "" }, "en")).toBe("The Claude CLI failed with exit code 2.");
    expect(messageOf("plain string", "en")).toBe("plain string");
    expect(messageOf(undefined, "en")).toBe("Something went wrong.");
  });

  it("knows every reason code Rust can send", () => {
    const src = Object.values(import.meta.glob("../../crates/dduck-core/src/i18n.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];
    const table = src.slice(src.indexOf("    ReasonCode {"), src.indexOf("impl ReasonCode"));
    const codes = [...table.matchAll(/^\s{8}([A-Z]\w+) => \(/gm)].map((m) => m[1][0].toLowerCase() + m[1].slice(1));
    expect(codes.length).toBeGreaterThan(40);
    expect(Object.keys(koErrors.invalidInput).sort()).toEqual([...codes].sort());
  });

  it("has the same reason codes in both languages, all English free of Korean", () => {
    expect(Object.keys(enErrors.invalidInput)).toEqual(Object.keys(koErrors.invalidInput));
    expect(Object.keys(koErrors.invalidInput)).toHaveLength(54);
    for (const v of Object.values(enErrors.invalidInput)) expect(v).not.toMatch(HANGUL);
  });
});

describe("connection text", () => {
  it("follows the current language by default", () => {
    applyLanguage("en");
    expect(connectionText("claudeAi")).toBe("Connected with your Claude account");
    expect(connectionText("other")).toBe("Connected another way");
    expect(connectionText(null, "fallback")).toBe("fallback");
    expect(messageOf({ kind: "timeout" })).toBe("The Claude CLI took too long to respond.");
  });
});

describe("reasonIs", () => {
  it("recognises a fixed reason in either language, never by a loose word match", () => {
    expect(reasonIs("취소했어요", "cancelled")).toBe(true);
    expect(reasonIs("Cancelled", "cancelled")).toBe(true);
    expect(reasonIs("The file changed outside the app. Load it again", "changedOutside")).toBe(true);
    expect(reasonIs("작업 시작을 취소했어요", "cancelled")).toBe(false);
    expect(reasonIs({ kind: "invalidInput", code: "listChanged", reason: "x" }, "listChanged")).toBe(true);
    expect(reasonIs(null, "cancelled")).toBe(false);
  });
});

describe("pickLang", () => {
  it("reads one side of every {ko, en} pair", () => {
    const v = { a: { ko: "가", en: "A" }, list: [{ ko: "나", en: "B" }, "x"], n: 1, deep: { ko: "다" } };
    expect(pickLang(v)).toEqual({ a: "가", list: ["나", "x"], n: 1, deep: { ko: "다" } });
    expect(pickLang(v, "en").a).toBe("A");
  });
});

describe("errors kept in screen state", () => {
  it("are worded when drawn, so a language change rewords them", async () => {
    const { shown, failure, later } = await import("./api");
    const { applyLanguage: apply } = await import("../i18n/language");
    const { t: tr } = await import("../i18n/index");
    const kept = [failure({ kind: "invalidInput", code: "agentOff", reason: "x" }), failure("취소했어요"), failure(new Error("파일이 바뀌었어요. 다시 불러오세요")), later(() => tr("common.actions.close")), failure("as sent")];
    apply("ko");
    expect(kept.map(shown)).toEqual(["꺼 둔 에이전트예요. 에이전트 설정에서 켜야 이 역할로 시작할 수 있어요.", "취소했어요", "파일이 바뀌었어요. 다시 불러오세요", "닫기", "as sent"]);
    apply("en");
    expect(kept.map(shown)).toEqual(["This agent is off. Turn it on in agent settings to start with this role.", "Cancelled", "The file changed. Load it again", "Close", "as sent"]);
    apply("ko");
  });
});
