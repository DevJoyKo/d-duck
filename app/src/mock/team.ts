import { trashUnconfirmed } from "./os";
// Mock-mode data for the agent settings sheet (team rules, task types, agent on/off). Design review only: the real
// merge, validation and confirmation logic lives in Rust (team_config.rs, agent_controls.rs).
import rawBundle from "../../crates/dduck-core/data/team_defaults.json";
import { pickLang } from "../lib/bundled";
import { mockSkills } from "./loadout";
import { backendLang } from "./lang";

// Bundled text fields are {ko, en} pairs (app-language), picked in the backend's language like Rust. Saved edits
// keep the language they were made in; untouched bundled entries follow the current language.
const pick = () => pickLang(rawBundle, backendLang());

type Rule = { id: string; text: string; extra: string; locked: boolean; bundled: boolean; defaultText: string | null; changed: boolean; defaultChanged: boolean };
const FIXED = ["deploy", "security", "qa"];
// Same text as team_config.rs EVIDENCE_REVIEW / EVIDENCE_QA.
const LOCKED: Record<string, string> = {
  review: "판정마다 파일:줄 근거(file)를 남긴다. 설명만으로 충족이라 쓰지 않는다. 확인할 수 없으면 일부 충족과 이유.",
  qa: "완료 기준마다 실제로 실행한 근거 파일을 보고서 폴더 evidence/에 남긴다. 테스트는 리포트 파일, 화면은 Playwright 스크린샷, 명령은 명령·종료 코드·출력 파일. 근거 파일 없이 충족이라 쓰지 않는다. 확인하지 못했으면 일부 충족과 이유. 테스트를 돌렸으면 테스트 리포트를 꼭 남긴다.",
};
const defaults = () => {
  const bundle = pick();
  const roles = bundle.roles as Record<string, { description: string; prompt: string; tools?: string[] }>;
  return {
  header: bundle.header,
  rules: bundle.rules.map((r) => ({ ...r, extra: "", bundled: true, defaultText: r.text, changed: false, defaultChanged: false })) as Rule[],
  types: bundle.types.map((t) => ({
    ...t,
    agent: null as string | null,
    rolePrompt: roles[t.role]?.prompt ?? "",
    roleTools: roles[t.role]?.tools ?? null,
    hidden: false,
    skills: [] as { name: string; source: string; hash: string }[],
    mcpAllow: [] as string[],
    bundled: true,
    changed: false,
    defaultChanged: false,
    fixedStrict: FIXED.includes(t.key),
    lockedRule: LOCKED[t.key] ?? null,
    reportShape: (["research", "architecture", "comms", "learn"].includes(t.key) ? "free" : "criteria") as "free" | "criteria",
  })),
  refine: { ...bundle.refine },
  // The bundled default (preferred-default): the mattpocock-skills plugin.
  preferred: [{ kind: "plugin", name: "mattpocock-skills", source: "plugin:mattpocock-skills", marketplace: "claude-plugins-official" }] as { kind: string; name: string; source: string; scope?: string; marketplace?: string }[],
  errors: [] as string[],
  };
};
/** Fields still equal to the load-time bundled value take the current language's value. */
const relang = <T extends object>(v: T, was: T | undefined, now: T | undefined): T =>
  was && now ? (Object.fromEntries(Object.entries(v).map(([k, x]) => [k, JSON.stringify(x) === JSON.stringify(was[k as keyof T]) ? now[k as keyof T] : x])) as T) : v;
const current = () => {
  const d = defaults();
  return {
    ...config,
    header: d.header,
    rules: config.rules.map((r) => relang(r, loaded.rules.find((x) => x.id === r.id), d.rules.find((x) => x.id === r.id))),
    types: config.types.map((t) => relang(t, loaded.types.find((x) => x.key === t.key), d.types.find((x) => x.key === t.key))),
  };
};
const loaded = defaults();
let config = defaults();
config.rules.push({ id: "u-ko", text: "커밋 메시지는 한국어로 쓴다", extra: "", locked: false, bundled: false, defaultText: null, changed: true, defaultChanged: false });
config.types = config.types.map((t) => (t.key === "learn" ? { ...t, effort: "high", changed: true } : t));
// model-select: a stored review model that is no longer supported shows its replacement line.
config.types = config.types.map((t) => (t.key === "review" ? { ...t, model: "claude-opus-3-9", changed: true } : t));
// "기본 장비" samples: design picks a skill and figma; research keeps a pin whose file changed (shows "없음").
const pin = (n: string) => {
  const s = mockSkills.find((x) => x.name === n);
  return { name: n, source: s?.source ?? "user", hash: s?.hash ?? "" };
};
config.types = config.types.map((t) =>
  t.key === "design"
    ? { ...t, skills: [pin("frontend-design"), pin("expo:expo-ui")], mcpAllow: ["figma"], changed: true }
    : t.key === "research"
      ? { ...t, skills: [{ name: "deep-research", source: "user", hash: "f".repeat(32) }], changed: true }
      : t,
);

// "먼저 살펴볼 스킬·에이전트" stays at the bundled default; mattpocock-skills is not in the options, so it shows "설치되지 않아 빠져요".
const opt = (kind: string, name: string, source: string, description: string | null, members: string[] = [], scope: string | null = null) => ({ kind, name, source, scope, description, members, marketplace: source.startsWith("plugin:") ? "mock-market" : null, enabled: source !== "plugin:cloudflare" });
const preferredOptions = [
  opt("plugin", "writing-kit", "plugin:writing-kit", null, ["grill-me", "plan", "tdd", "write-a-prd"]),
  opt("plugin", "cloudflare", "plugin:cloudflare", null, ["workers-best-practices", "wrangler"]),
  opt("skill", "frontend-design", "user", "의도가 보이는 화면 디자인"),
  opt("skill", "writing-kit:tdd", "plugin:writing-kit", "테스트를 먼저 쓰고 구현해요"),
  opt("skill", "writing-kit:grill-me", "plugin:writing-kit", "계획을 끝까지 캐물어요"),
  opt("skill", "opsx:propose", "project", "OpenSpec 변경 제안", [], "/Users/me/work/dduck"),
  opt("agent", "code-reviewer", "user", "변경을 다섯 가지 관점으로 리뷰"),
  opt("agent", "qa-verifier", "project", "스펙 시나리오대로 직접 검증", [], "/Users/me/work/dduck"),
];

const controls = { userOff: ["release-helper"], projectOff: [] as [string, string[]][], elsewhereOff: [] as string[], hidden: ["Explore"] };

export const teamHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  team_config: () => ({ config: structuredClone(current()), defaults: defaults(), usedTypes: ["dev", "qa"], autoAllowed: ["Read", "Glob", "Grep", "git-read"] }),
  save_team_config: (a) => {
    const input = a.input as { rules: { id: string; text: string; extra: string }[]; types: typeof config.types; refine: typeof config.refine; preferred?: typeof config.preferred };
    const d = defaults();
    config = {
      ...d,
      rules: input.rules.map((r) => {
        const b = d.rules.find((x) => x.id === r.id);
        return { ...(b ?? { locked: false, bundled: false, defaultText: null, defaultChanged: false }), ...r, changed: !b || b.text !== r.text || !!r.extra } as Rule;
      }),
      types: input.types.map((t) => ({ ...t, changed: JSON.stringify(t) !== JSON.stringify(d.types.find((x) => x.key === t.key)) })),
      refine: input.refine ?? d.refine,
      preferred: input.preferred ?? d.preferred,
    };
  },
  reset_team_config: (a) => {
    if (a.item === "preferred") config.preferred = defaults().preferred;
    else config = defaults();
  },
  preferred_options: () => structuredClone(preferredOptions),
  agent_controls: () => structuredClone(controls),
  set_agent_enabled: (a) => {
    const name = String(a.id).replace(/^[fb]-/, "");
    controls.userOff = a.enabled ? controls.userOff.filter((n) => n !== name) : [...controls.userOff, name];
    return null;
  },
  set_agent_hidden: (a) => {
    controls.hidden = a.hidden ? [...controls.hidden, String(a.name)] : controls.hidden.filter((n) => n !== a.name);
  },
  trash_agent: (a) => ({ name: String(a.id).replace(/^f-/, ""), project: null, off: false, hidden: false, unconfirmed: trashUnconfirmed() }),
  forget_agent: () => undefined,
};
