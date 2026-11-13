// Mock-mode data for the rules segment. Design review only, fake rules: the real listing, frontmatter edits,
// claudeMdExcludes writes, confirmations and the Trash live in Rust (rules.rs, memory_cmds.rs).
import rawTemplates from "../../crates/dduck-core/data/authoring_templates.json";
import { pickLang } from "../lib/bundled";
import { MOCK_IDS } from "./projects";
import { backendLang } from "./lang";

// Bundled text fields are {ko, en} pairs (app-language), picked in the backend's language on every call like Rust.
const templates = () => pickLang(rawTemplates, backendLang());

type Mock = {
  id: string;
  scope: "global" | "project";
  rel: string;
  paths: string[];
  fm?: "none" | "ok" | "complex" | "broken";
  enabled?: boolean;
  offBy?: "app" | "other" | null;
  gitTracked?: boolean | null;
  link?: boolean;
  dirLink?: boolean;
  linkOutside?: boolean;
  body: string;
  hasSecrets?: boolean;
};

const rules: Mock[] = [
  { id: "r-g1", scope: "global", rel: "korean.md", paths: [], body: "# 답변 언어\n\n- 한국어로 답한다.\n" },
  { id: "r-g2", scope: "global", rel: "commits.md", paths: [], enabled: false, offBy: "app", body: "# 커밋\n\n- 커밋은 요청할 때만 한다.\n" },
  { id: "r-g3", scope: "global", rel: "lang/typescript.md", paths: ["**/*.{ts,tsx}"], fm: "ok", body: "# TypeScript\n\n- any를 쓰지 않는다.\n" },
  { id: "r-p1", scope: "project", rel: "testing.md", paths: [], gitTracked: true, body: "# 테스트\n\n- 새 기능에는 테스트를 함께 쓴다.\n" },
  { id: "r-p2", scope: "project", rel: "api.md", paths: ["src/api/**/*.ts"], fm: "ok", gitTracked: true, body: "# API 규칙\n\n- 모든 엔드포인트는 입력을 검증한다.\n" },
  { id: "r-p3", scope: "project", rel: "frontend/react.md", paths: ["src/components/*.tsx", "photos [2024/**"], fm: "ok", gitTracked: false, body: "# React\n\n- 컴포넌트 하나에 파일 하나.\n" },
  { id: "r-p4", scope: "project", rel: "frontend/legacy.md", paths: [], enabled: false, offBy: "other", gitTracked: true, body: "# 예전 규칙\n" },
  { id: "r-p5", scope: "project", rel: "security.md", paths: [], link: true, linkOutside: true, gitTracked: null, body: "# 보안\n\n- 비밀값은 코드에 넣지 않는다.\n" },
  { id: "r-p6", scope: "project", rel: "deploy.md", paths: [], fm: "broken", gitTracked: false, hasSecrets: true, body: "배포 토큰: FAKE-not-a-real-token\n" },
  { id: "r-p7", scope: "project", rel: "style.md", paths: ["src/**"], fm: "complex", gitTracked: true, body: "# 스타일\n" },
];

const row = (r: Mock) => {
  const fm = r.fm ?? (r.paths.length ? "ok" : "none");
  const [folder, name] = r.rel.includes("/") ? [r.rel.split("/")[0], r.rel.split("/").pop() ?? r.rel] : [null, r.rel];
  return {
    id: r.id,
    scope: r.scope,
    rel: r.rel,
    name: name.replace(/\.md$/, ""),
    folder,
    paths: r.paths,
    fm,
    enabled: r.enabled ?? true,
    offBy: r.offBy ?? null,
    gitTracked: r.scope === "project" ? (r.gitTracked ?? null) : null,
    link: !!r.link,
    dirLink: !!r.dirLink,
    linkOutside: !!r.linkOutside,
    linkTarget: r.link ? "/Users/me/shared/rules/security.md" : null,
    size: r.body.length + 40,
    token: `t-${r.id}-${r.body.length}-${r.paths.join(",")}`,
    readonlyReason: r.link ? "공유 링크 · 원본에서 편집해 주세요" : null,
    hasSecrets: !!r.hasSecrets,
    warnings: r.paths.some((p) => p.includes(" [")) ? ["photos [2024/**: `[`의 짝이 없어 아무 파일과도 맞지 않아요 (`\\[`로 쓰세요)"] : [],
  };
};

function group(scope: "global" | "project", dir: string) {
  const list = rules.filter((r) => r.scope === scope).map(row);
  const always = list.filter((r) => r.enabled && (r.fm === "broken" || (r.paths.length === 0 && r.fm !== "complex" && !r.dirLink)));
  return {
    scope,
    dir,
    exists: true,
    rules: list,
    truncated: false,
    settingsPath: scope === "global" ? "/Users/me/.claude/settings.json" : "/Users/me/Agents/dev_team/.claude/settings.local.json",
    settingsToken: "s",
    settingsReadonly: null,
    alwaysCount: always.length,
    alwaysBytes: always.reduce((a, r) => a + (r.size ?? 0), 0),
    readonlyReason: null,
  };
}

const projectDir: Record<string, string> = { [MOCK_IDS.dev]: "/Users/me/Agents/dev_team", [MOCK_IDS.side]: "/Users/me/Agents/side-project", [MOCK_IDS.home]: "/Users/me" };

export const rulesHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  rules_list: (a) => {
    const dir = projectDir[String(a.project ?? "")];
    const home = { ...group("project", `${dir}/.claude/rules`), rules: [], alwaysCount: 0, alwaysBytes: 0, exists: false, readonlyReason: "홈 폴더는 전역과 같아요" };
    const project = dir ? (dir === "/Users/me" ? home : group("project", `${dir}/.claude/rules`)) : null;
    return { global: group("global", "/Users/me/.claude/rules"), project, templates: templates().rules };
  },
  rule_read: (a) => {
    const r = rules.find((x) => x.id === a.id) ?? rules[0];
    const b = row(r);
    return {
      ...b,
      body: r.body,
      pathsEditable: b.fm === "none" || b.fm === "ok",
      secrets: r.hasSecrets ? [{ kind: "비밀 설정값", count: 1, lines: [1] }] : [],
      instructionLike: [],
    };
  },
  save_rule: (a) => {
    const s = a.rule as { id?: string; scope?: "global" | "project"; name?: string; body: string; paths?: string[] };
    if (s.id) {
      const r = rules.find((x) => x.id === s.id);
      if (r) Object.assign(r, { body: s.body, ...(s.paths ? { paths: s.paths, fm: s.paths.length ? "ok" : "none" } : {}) });
      return s.id;
    }
    const id = `r-new-${rules.length}`;
    rules.push({ id, scope: s.scope ?? "project", rel: s.name ?? "new.md", paths: s.paths ?? [], body: s.body, gitTracked: false });
    return id;
  },
  set_rule_enabled: (a) => {
    const r = rules.find((x) => x.id === a.id);
    if (r) Object.assign(r, { enabled: Boolean(a.enabled), offBy: a.enabled ? null : "app" });
    return undefined;
  },
  trash_rule: () => ({ done: false, unconfirmed: false }),
};
