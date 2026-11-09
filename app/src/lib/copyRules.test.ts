// copy-sweep (D-123, D-130) narrowed by app-language 5.1 (D-171): screen text lives in the catalogs, so the Korean
// rules read the ko catalogs, the Korean column of `i18n.rs` and the `ko` side of the bundled data; the English
// rules read the en side of the same three. Hardcoded text outside them is caught by hardcodedKorean.test.ts.
import { describe, expect, it } from "vitest";

const glob = (g: Record<string, unknown>) => Object.entries(g as Record<string, string>).map(([file, text]) => ({ file: file.replace(/^(\.\.\/)+|^\//, ""), text }));
const koCatalog = glob(import.meta.glob("/src/i18n/ko/*.ts", { query: "?raw", import: "default", eager: true }));
const enCatalog = glob(import.meta.glob("/src/i18n/en/*.ts", { query: "?raw", import: "default", eager: true }));
const i18nRs = Object.values(import.meta.glob("../../crates/dduck-core/src/i18n.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];
const bundled = glob(import.meta.glob(["../../crates/dduck-core/data/permission_catalog.json", "../../crates/dduck-core/data/team_defaults.json"], { query: "?raw", import: "default", eager: true }));
const authStatusRs = Object.values(import.meta.glob("../../crates/dduck-core/src/auth_status.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];
const routinesPage = Object.values(import.meta.glob("/src/components/RoutinesPage.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];

const STR = String.raw`"((?:[^"\\]|\\.)*)"`;
/** (ko, en) template pairs of every `table!` row in i18n.rs; a ko side that names a constant is skipped. */
const rustPairs: [string, string][] = [...i18nRs.matchAll(new RegExp(String.raw`=> \(\s*(?:${STR}|[\w:]+)\s*,\s*${STR}`, "g"))].map((m) => [m[1] ?? "", m[2]]);

/** Every `{"ko": …, "en": …}` pair in the bundled data. */
function bundledPairs(): [string, string][] {
  const out: [string, string][] = [];
  const walk = (v: unknown) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") {
      const o = v as Record<string, unknown>;
      if (typeof o.ko === "string" && typeof o.en === "string") out.push([o.ko, o.en]);
      else Object.values(o).forEach(walk);
    }
  };
  for (const b of bundled) walk(JSON.parse(b.text));
  return out;
}

const ko = [...koCatalog, { file: "i18n.rs (ko)", text: rustPairs.map((p) => p[0]).join("\n") }, { file: "bundled (ko)", text: bundledPairs().map((p) => p[0]).join("\n") }];
/** English strings one by one: catalog values, the Rust en column, the bundled en side. */
const enStrings: { file: string; text: string }[] = [
  ...enCatalog.flatMap((c) => [...c.text.matchAll(new RegExp(STR, "g"))].map((m) => ({ file: c.file, text: m[1] }))),
  ...rustPairs.map((p) => ({ file: "i18n.rs (en)", text: p[1] })),
  ...bundledPairs().map((p) => ({ file: "bundled (en)", text: p[1] })),
];

/** 합쇼체 endings: -습니다, any syllable with a ㅂ final before 니다 (합니다·됩니다·입니다…), -십시오, -ㅂ니까. */
export function formalEndings(text: string): string[] {
  const hits: string[] = [];
  for (const m of text.matchAll(/([가-힣])니(다|까)/g)) {
    const c = m[1].charCodeAt(0) - 0xac00;
    if (m[1] === "습" || c % 28 === 17) hits.push(m[0]);
  }
  for (const m of text.matchAll(/[가-힣]?십시오/g)) hits.push(m[0]);
  return hits;
}

/** A short label written in Title Case ("Review Capacity"): two or more words, every word capitalized. */
export function titleCase(text: string): boolean {
  if (/[.?!:·{}()'"`/@<>=_]/.test(text)) return false;
  const words = text.trim().split(/\s+/).filter((w) => /^[A-Za-z]/.test(w));
  const NAMES = new Set(["Claude", "Code", "D.Duck", "GitHub", "MCP", "CLI", "API", "Mac", "Remote", "Control", "Time", "Machine", "Opus", "Sonnet", "Haiku", "Kubernetes", "Docker", "AWS", "Terraform"]);
  const plain = words.filter((w) => !NAMES.has(w) && !/^[A-Z0-9-]+$/.test(w));
  return words.length >= 2 && words.length <= 6 && plain.length >= 2 && plain.every((w) => /^[A-Z]/.test(w));
}

describe("copy rules (Korean)", () => {
  it("reads every ko catalog, the Rust Korean column and the bundled data", () => {
    expect(koCatalog.length).toBeGreaterThanOrEqual(13);
    expect(rustPairs.length).toBeGreaterThan(400);
    expect(bundledPairs().length).toBeGreaterThan(50);
  });

  it("says 에이전트 in Korean sentences (no 'agent이에요')", () => {
    expect(ko.filter((s) => /agent(이에요|예요)/.test(s.text)).map((s) => s.file)).toEqual([]);
  });

  it("detects the formal endings it is meant to forbid", () => {
    expect(formalEndings("저장했습니다. 적용됩니다 이름입니다 하십시오 할까요 해요")).toEqual(["습니다", "됩니다", "입니다", "하십시오"]);
    expect(formalEndings("저장했어요 · 이름이에요 · 다시 해 주세요")).toEqual([]);
  });

  it("says 완료 only for 정리 끝 and the four kept phrases, never 완료로 (task-lifecycle design 8, 5.2)", () => {
    // design.md 8: rg '"완료"|완료·|: "완료|완료로'. 정리 끝 chips are exactly "완료"; the kept phrases are 완료 기준,
    // the onboarding "(완료)" and tour text. 끝내고 넘기기 (D-167) must not promise 완료 while a report waits.
    const hits = ko.flatMap((c) =>
      (c.file.endsWith("(ko)") ? c.text.split("\n") : [...c.text.matchAll(new RegExp(STR, "g"))].map((m) => m[1]))
        .filter((v) => /"완료"|완료·|완료로/.test(`"${v}"`) || (v.includes("완료") && v !== "완료" && !/완료 기준|\(완료\)/.test(v) && !c.file.endsWith("tour.ts")))
        .map((v) => `${c.file}: ${v}`),
    ).filter((h) => !h.endsWith(": 완료"));
    expect(hits).toEqual([]);
  });

  it("has no 합니다체 in the Korean text", () => {
    expect(ko.flatMap((s) => formalEndings(s.text).map((h) => `${s.file}: ${h}`))).toEqual([]);
  });

  it("says 리뷰, not 검토", () => {
    expect(ko.filter((s) => s.text.includes("검토")).map((s) => s.file)).toEqual([]);
  });

  it("says 기록 없음, not 안 씀, for usage on the extensions screens (D-147)", () => {
    const ext = koCatalog.find((s) => s.file.endsWith("ko/extensions.ts"))!;
    expect(ext.text.includes("기록 없음")).toBe(true);
    expect(ext.text.includes("안 씀")).toBe(false);
  });

  it("uses the login gate copy, not the old login hints (claude-login)", () => {
    const old = ["로그인 상태를 확인하세요", "로그인이 만료되었어요", "터미널에서 claude를 실행해 로그인하세요"];
    expect(ko.flatMap((s) => old.filter((w) => s.text.includes(w)).map((w) => `${s.file}: ${w}`))).toEqual([]);
    // One wording: the Rust constant (the fallback for the `login_required` kind) equals the catalog line.
    const gate = "Claude에 로그인해야 이 기능을 쓸 수 있어요";
    expect(authStatusRs.includes(gate)).toBe(true);
    expect(koCatalog.find((s) => s.file.endsWith("ko/auth.ts"))!.text.includes(gate)).toBe(true);
  });

  it("calls the user scope 전역 (no 사용자 범위, 내 것(모든 프로젝트), 내 플러그인)", () => {
    const words = ["사용자 범위", "내 것(모든 프로젝트)", "내 플러그인("];
    expect(ko.flatMap((s) => words.filter((w) => s.text.includes(w)).map((w) => `${s.file}: ${w}`))).toEqual([]);
  });
});

describe("copy rules (English)", () => {
  it("reads the en catalogs, the Rust English column and the bundled data", () => {
    expect(enStrings.length).toBeGreaterThan(2000);
  });

  it("has no Korean in English text", () => {
    expect(enStrings.filter((s) => /[가-힣]/.test(s.text)).map((s) => `${s.file}: ${s.text.slice(0, 60)}`)).toEqual([]);
  });

  it("detects Title Case labels", () => {
    expect(titleCase("Review Capacity")).toBe(true);
    expect(titleCase("Review capacity")).toBe(false);
    expect(titleCase("Claude Code")).toBe(false);
    expect(titleCase("Open in Terminal.")).toBe(false);
  });

  it("writes labels in sentence case", () => {
    expect(enStrings.filter((s) => titleCase(s.text)).map((s) => `${s.file}: ${s.text}`)).toEqual([]);
  });

  it("keeps the tone plain: no 'please kindly', no exclamation marks, no doubled spaces or periods", () => {
    const bad = enStrings.filter((s) => /please kindly|kindly/i.test(s.text) || /!\s*$/.test(s.text) || / {2}(?!…)/.test(s.text.replace(/^\s+/, "")) || /[^.]\.\.(?!\.)/.test(s.text));
    expect(bad.map((s) => `${s.file}: ${s.text.slice(0, 60)}`)).toEqual([]);
  });

  it("uses one word per idea: review (not inspect/examine), global (not user scope), task (not job)", () => {
    const bad = enStrings.filter((s) => /\b(inspect|examine)\w*|\buser scope\b|\bjobs?\b/i.test(s.text));
    expect(bad.map((s) => `${s.file}: ${s.text.slice(0, 60)}`)).toEqual([]);
  });

  it("matches the cloud-routine conflict text Rust sends in both languages (RoutinesPage)", () => {
    for (const w of ["다른 곳에서 바뀌었어요", "It changed somewhere else", "다시 불러오", "Load it again"]) {
      expect(routinesPage.includes(`"${w}"`), w).toBe(true);
      expect(i18nRs.includes(w), w).toBe(true);
    }
  });
});
