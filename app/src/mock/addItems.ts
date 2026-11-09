// Mock-mode data for "추가 ▾" (install-from-link, skill-authoring). Design review only: the real parsing, GitHub
// reads, staging, confirmation dialogs and file writes live in Rust (install_link.rs, authoring.rs, add_items.rs).
import rawTemplates from "../../crates/dduck-core/data/authoring_templates.json";
import { pickLang } from "../lib/bundled";
import { mockProjectPath } from "./projects";
import { backendLang, bt } from "./lang";

// Bundled text fields are {ko, en} pairs (app-language), picked in the backend's language on every call like Rust.
const templates = () => pickLang(rawTemplates, backendLang());

const day = 86_400_000;
const now = Date.now();
const PROJ = "/Users/me/Agents/dev_team";
const SHA = "2c5958ecac7cee2b583967a2c4981d0f4a75c579";
const TOKEN = "0123456789abcdef0123456789abcdef";

type Row = { name: string; source: string; scope: string | null; kind: string; description: string | null; uses: number; lastUsed: number | null; defined: boolean; id: string | null; readonly: string | null; copiedFrom?: string | null; ccReadable?: boolean; reason?: string; registeredName?: string };
const row = (name: string, source: string, kind: string, description: string | null, extra: Partial<Row> = {}): Row => ({
  name, source, scope: source === "project" ? PROJ : null, kind, description, uses: 0, lastUsed: null, defined: true, id: `s-${name.replace(/[^a-z0-9]/gi, "")}${kind}`, readonly: null, ...extra,
});
const skills: Row[] = [
  row("opsx:propose", "project", "command", "새 변경 제안 만들기", { uses: 6, lastUsed: now - day }),
  row("openspec-apply-change", "project", "skill", "OpenSpec 변경의 태스크 구현", { uses: 5, lastUsed: now - 2 * day }),
  row("update-config", "builtin", "skill", null, { uses: 3, lastUsed: now - 3 * day, defined: false, id: null }),
  row("anthropic-skills:docx", "user", "skill", "Word 문서 만들기", { readonly: "claude.ai에서 동기화된 항목은 고칠 수 없어요" }),
  row("cloudflare:wrangler", "plugin", "skill", "Wrangler CLI 실행과 문제 해결", { readonly: "플러그인 항목은 고칠 수 없어요" }),
  row("frontend-design:frontend-design", "plugin", "skill", "의도가 분명한 UI 디자인을 만들 때", { readonly: "플러그인 항목은 고칠 수 없어요", uses: 4, lastUsed: now - 2 * day }),
  row("expo:eas-update", "plugin", "skill", "EAS Update 설정과 배포", { readonly: "플러그인 항목은 고칠 수 없어요" }),
  row("cloudflare:deploy", "plugin", "command", "Workers 배포", { readonly: "플러그인 항목은 고칠 수 없어요" }),
  row("design-review", "user", "skill", "디자인 리뷰 체크리스트로 화면을 점검할 때", { copiedFrom: "frontend-design:design-review" }),
  row("pr-writer", "user", "skill", "PR 설명을 팀 형식(배경·변경·테스트)으로 쓸 때"),
  row("standup", "user", "command", "어제·오늘·막힌 것으로 스탠드업 정리", { uses: 2, lastUsed: now - day }),
  row("triage", "user", "workflow", "새 이슈를 읽고 영역별로 분류", { ccReadable: true }),
  row("release-check", "project", "workflow", "릴리스 전 테스트·변경 로그·버전 확인", { ccReadable: true }),
  // Hand-made files (workflow-meta-check): one Claude Code skips, one it registers under its meta.name.
  row("weekly-digest", "user", "workflow", null, { ccReadable: false, reason: "notLiteral" }),
  row("notes", "user", "workflow", "회의 메모를 요약해 정리", { ccReadable: true, registeredName: "sync-notes" }),
];
const made: { kind: string; name: string; project: string | null; atMs: number }[] = [
  { kind: "skill", name: "pr-writer", project: null, atMs: now - 3_600_000 },
  { kind: "workflow", name: "triage", project: null, atMs: now - 2 * 3_600_000 },
];
let dduckNew: "off" | "ours" | "other" = "off";

const texts: Record<string, string> = {
  "s-prwriterskill": "---\nname: pr-writer\ndescription: PR 설명을 팀 형식(배경·변경·테스트)으로 쓸 때. \"PR 설명 써 줘\"\n---\n\n# pr-writer\n\n## 언제 쓰나\n\n- PR을 열기 전에 설명을 쓸 때\n\n## 절차\n\n1. git diff로 바뀐 것을 읽는다\n2. 배경 · 변경 · 테스트 세 칸으로 쓴다\n\n## 하지 말 것\n\n- 비밀값을 설명에 넣지 않는다\n",
  "s-cloudflarewranglerskill": "---\nname: wrangler\ndescription: Wrangler CLI 실행과 문제 해결\nallowed-tools: Bash(wrangler:*)\n---\n\n# Wrangler\n\nwrangler.toml이 있는 프로젝트에서 …\n",
};

const marketplace = {
  kind: "marketplace", token: TOKEN, installed: false, source: "github.com/anthropics/skills", sha: SHA, market: "anthropic-agent-skills",
  description: "Anthropic example skills", skipped: 0, alreadyAdded: null,
  plugins: [
    { name: "document-skills", description: "Excel·Word·PowerPoint·PDF 문서 처리 스킬 모음", version: null, origin: "in_repo", declares: [] },
    { name: "example-skills", description: "스킬 만들기·MCP 빌드·디자인 등 예제 스킬 모음", version: null, origin: "in_repo", declares: [] },
    { name: "claude-api", description: "Claude API 사용 가이드", version: "1.2.0", origin: "github", declares: ["훅", "MCP"] },
  ],
};
const skillMd = "---\nname: pdf-tools\ndescription: PDF를 읽고 양식을 채울 때. \"PDF 채워 줘\"\nallowed-tools: Read, Bash(python3:*)\n---\n\n# PDF 도구\n\n현재 폴더: !`ls *.pdf`\n\n1. scripts/fill.py로 양식을 채운다\n2. 결과를 확인한다⟦U+202E⟧\n";
const skillPreview = {
  kind: "skill", token: TOKEN, installed: false, source: "github.com/acme/skills @ main / pdf-tools", sha: SHA, name: "pdf-tools",
  description: "PDF를 읽고 양식을 채울 때. \"PDF 채워 줘\"", executables: 2, totalBytes: 18_420, skipped: [".DS_Store"],
  files: [
    { path: "SKILL.md", size: 412, executable: false },
    { path: "forms.md", size: 11_854, executable: false },
    { path: "scripts/fill.py", size: 5_210, executable: true },
    { path: "scripts/setup.sh", size: 944, executable: true },
  ],
  skillMd,
  risk: { allowedTools: "Read, Bash(python3:*)", hooks: false, shellLines: 1, danger: false },
};

const jobs = [
  {
    token: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", kind: "skill", createdMs: now - 1_200_000, bgId: "9988aabb", title: "릴리스 노트를 우리 형식으로 써 주는 스킬", stray: [],
    produced: [
      {
        kind: "skill", name: "release-notes", files: ["SKILL.md", "examples.md"], problem: null, warnings: [],
        risk: { allowedTools: null, hooks: false, shellLines: 0, danger: false },
        content: "---\nname: release-notes\ndescription: 릴리스 노트를 우리 형식(새 기능·고침·주의)으로 쓸 때. \"릴리스 노트 써 줘\"\n---\n\n# release-notes\n\n## 언제 쓰나\n\n- 태그를 만들기 전에 변경 내용을 정리할 때\n\n## 절차\n\n1. 지난 태그 이후 커밋을 읽는다\n2. 새 기능 · 고침 · 주의 세 칸으로 나눈다\n3. examples.md의 문체를 따른다\n",
      },
      {
        kind: "command", name: "notes", files: ["notes.md"], problem: null, warnings: ["allowed-tools가 있어요: 이 도구들은 확인 없이 실행될 수 있어요"],
        risk: { allowedTools: "Bash(git log:*)", hooks: false, shellLines: 0, danger: false },
        content: "---\ndescription: 릴리스 노트 초안\nallowed-tools: Bash(git log:*)\n---\n\n$ARGUMENTS 이후 변경으로 릴리스 노트 초안을 써요.\n",
      },
    ],
  },
];

function fail(msg: string): never {
  throw msg;
}

export const addItemHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  skill_catalog: () => skills.map((s) => ({ ...s })),
  copy_item: (a) => {
    const src = skills.find((x) => x.id === a.itemId) ?? fail("목록에 없는 항목이에요");
    const name = String(a.name);
    const dest = a.dest === "project" ? "project" : "user";
    if (skills.some((x) => x.kind === src.kind && x.name === name && x.source === dest)) fail("이미 있는 이름이에요");
    const copy = row(name, dest, src.kind, src.description, { scope: dest === "project" ? mockProjectPath(a.project) : null, copiedFrom: src.name });
    skills.push(copy);
    made.push({ kind: src.kind, name, project: copy.scope, atMs: Date.now() });
    return { id: copy.id };
  },
  // Mock of the Rust rules (ext_trash.rs): own items only; a deleted skill or command with usage leaves a usage-only row
  // like `catalog()` does. `release-check` shows the move-failure line for design review.
  trash_extension_item: (a) => {
    const i = skills.findIndex((x) => x.id === a.id);
    const s = i < 0 ? fail("목록에 없는 항목이에요. 다시 열어 주세요") : skills[i];
    if (s.readonly || (s.source !== "user" && s.source !== "project")) fail("플러그인·동기화 항목은 지울 수 없어요");
    if (s.name === "release-check") fail("휴지통으로 옮기지 못했어요");
    skills.splice(i, 1);
    if (s.uses > 0 && s.kind !== "workflow") skills.push(row(s.name, "builtin", "skill", null, { uses: s.uses, lastUsed: s.lastUsed, defined: false, id: null, scope: null }));
    return true;
  },
  inspect_link: (a) => {
    const input = String(a.input ?? "").trim();
    if (/\s|^http:|^git@|^file:|^\/|^-/.test(input) || !input) fail("지원하지 않는 링크예요. GitHub 저장소(owner/repo, https://github.com/…), 스킬 폴더 링크, 플러그인 이름(이름@마켓플레이스)을 붙여 넣으세요");
    if (/gitlab|bitbucket/.test(input)) fail("GitHub 링크만 지원해요(github.com, raw.githubusercontent.com)");
    if (input.includes("@")) return { kind: "plugin", token: TOKEN, installed: false, id: input, plugin: { name: input.split("@")[0], description: "이미 추가된 마켓플레이스의 플러그인", version: "1.0.0", origin: "github", declares: [] } };
    if (input.includes("/tree/") || input.endsWith("SKILL.md")) return skillPreview;
    return marketplace;
  },
  discard_link: () => undefined,
  install_plugin: (a) =>
    a.plugin === "claude-api" || String(a.plugin ?? "").length === 0
      ? { outcome: { kind: "needsApproval" }, marketplaceAdded: true, approvalCommand: `claude plugin install ${a.plugin || "kit"}@anthropic-agent-skills -s user`, removeCommand: "claude plugin marketplace remove anthropic-agent-skills" }
      : { outcome: { kind: "done" }, marketplaceAdded: true, approvalCommand: null, removeCommand: null },
  add_marketplace: () => ({ kind: "done" }),
  install_skill: () => "pdf-tools",
  authoring_info: () => ({ workflowsDisabled: false, dduckNew, made, templates: templates() }),
  create_authoring_item: (a) => {
    const name = String(a.name);
    if (skills.some((s) => s.name === name && s.kind === a.kind)) fail("이미 있는 이름이에요");
    skills.push(row(name, a.project ? "project" : "user", String(a.kind), "방금 만든 항목", a.kind === "workflow" ? { ccReadable: true } : {}));
    made.push({ kind: String(a.kind), name, project: mockProjectPath(a.project), atMs: Date.now() });
    return undefined;
  },
  read_authoring_item: (a) => {
    const s = skills.find((x) => x.id === a.id) ?? fail("목록에 없는 항목이에요. 다시 열어 주세요");
    const name = s.name;
    const content =
      texts[String(a.id)] ??
      (s.kind === "workflow"
        ? `export const meta = {\n  name: '${name}',\n  description: '${s.description}',\n  phases: [{ title: '수집' }, { title: '정리' }],\n}\n\nphase('수집')\nconst issues = await agent('열린 이슈 목록을 읽어 제목·라벨을 정리한다', { label: '수집', phase: '수집' })\n\nphase('정리')\nreturn await agent('영역별로 분류해 표로 만든다: ' + issues, { label: '정리', phase: '정리' })\n`
        : `---\ndescription: ${s.description}\n---\n\n$ARGUMENTS 에 대해 …\n`);
    return {
      content, token: "t", kind: s.kind, name, location: s.source === "plugin" ? "플러그인" : s.source === "project" ? `${bt("프로젝트", "Project")} ${PROJ}` : "전역",
      readonly: s.readonly, warnings: s.readonly && content.includes("allowed-tools") ? ["allowed-tools가 있어요: 이 도구들은 확인 없이 실행될 수 있어요"] : [],
      extraFiles: s.kind === "skill" && !s.readonly ? ["examples.md"] : [],
    };
  },
  save_authoring_item: () => undefined,
  // A rough stand-in for authoring::check_workflow_meta (the real parser is Rust): enough to show each state.
  check_workflow_meta: (a) => {
    const text = String(a.text ?? "");
    const name = String(a.name ?? "");
    const body = text.replace(/^\uFEFF?(?:\s|\/\/[^\n]*\n|\/\*[\s\S]*?\*\/)*/, "");
    const lineAt = (i: number) => text.slice(0, text.length - body.length + Math.max(i, 0)).split("\n").length;
    const bad = (reason: string, at: number | null) => ({ ok: false, reason, line: at == null ? null : lineAt(at), blankDescription: false });
    if (!/^export\s+const\s+meta\s*=\s*\{/.test(body)) return bad("notFirstStatement", 0);
    const close = body.search(/\n\}/);
    const meta = close < 0 ? body : body.slice(0, close + 2);
    const expr = meta.search(/\$\{|\.\.\.|[\w$]\s*\(/);
    if (expr >= 0) return bad("notLiteral", expr);
    const field = (k: string) => new RegExp(`${k}\\s*:\\s*(?:'([^']*)'|"([^"]*)")`).exec(meta);
    const n = field("name");
    if (!n) return bad("missingName", 0);
    if (!field("description")) return bad("missingDescription", 0);
    if ((n[1] ?? n[2]) !== name) return { ...bad("nameMismatch", null), registeredName: n[1] ?? n[2] };
    return { ok: true, reason: null, line: null, blankDescription: false };
  },
  start_authoring_task: () => ({ token: "b".repeat(32), bgId: "ab12cd34" }),
  authoring_jobs: () => jobs,
  install_authoring_result: () => undefined,
  discard_authoring_job: () => undefined,
  install_authoring_command: () => {
    dduckNew = "ours";
  },
  remove_authoring_command: () => {
    dduckNew = "off";
  },
};
