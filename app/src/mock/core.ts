// Browser-only stand-in for @tauri-apps/api/core, used by `npm run dev:mock` to review the UI with fake data.
// Never bundled into the app: vite.config.ts aliases it only in `mock` mode.
import { permissionHandlers } from "./permissions";
import { teamHandlers } from "./team";
import { addItemHandlers } from "./addItems";
import { memoryHandlers } from "./memory";
import { rulesHandlers } from "./rules";
import { projectHandlers } from "./projects";
import { mcpHandlers } from "./mcp";
import { loadoutHandlers } from "./loadout";
import { modelHandlers } from "./model";
import { mockArtifactPreview, mockArtifacts } from "./artifacts";
import { mockNoCli, onboardingHandlers } from "./onboarding";
import { mockTaskReport } from "./report";
import { evidenceHandlers, mockEvidenceReport, normalize } from "./evidence";
import { deleted, mockReviewPending, mockReviewReport, pollUnknown, reviewHandlers, wrapped } from "./review";
import { limitHandlers } from "./limits";
import { deliveryHandlers } from "./delivery";
import { authHandlers, mockLoggedOut, mockNotClaudeAi } from "./auth";
import { lifecycleHandlers, mockLanes, noLive } from "./lifecycle";
import { localize, delocalize } from "./english";
import { DATA_LANG, backendLang, mockChoice, setMockChoice } from "./lang";
const day = 86_400_000;
const now = Date.now();
const cloud: { id: string; name: string; enabled: boolean; cron: string | null; runOnceAt: number | null; nextRunAt: number | null; endedReason: string | null; schedule: unknown; lastRun?: unknown }[] = (() => {
  const at = new Date(now + 4 * day);
  at.setHours(9, 0, 0, 0);
  const past = new Date(now - 4 * day);
  past.setHours(9, 0, 0, 0);
  return [
    { id: "trig_1", name: "스프린트 마감 알림", enabled: true, cron: null, runOnceAt: at.getTime(), nextRunAt: at.getTime(), endedReason: null, schedule: { kind: "once", at: at.getTime() } },
    { id: "trig_2", name: "매일 아침 뉴스 요약", enabled: true, cron: "0 0 * * *", runOnceAt: null, nextRunAt: now + 15 * 3_600_000, endedReason: null, schedule: { kind: "daily", hour: 9, minute: 0 }, lastRun: { status: "succeeded", failureReason: null, firedAt: now - 9 * 3_600_000, finishedAt: now - 9 * 3_600_000 + 48_000, sessionId: "cse_mock2a" } },
    { id: "trig_4", name: "주간 저장소 점검", enabled: true, cron: "30 23 * * 0,2,4", runOnceAt: null, nextRunAt: now + 2 * day, endedReason: null, schedule: { kind: "weekly", weekdays: 0b10101, hour: 8, minute: 30 }, lastRun: { status: "failed", failureReason: "ENVIRONMENT_NOT_FOUND", firedAt: now - day, finishedAt: now - day + 5_000, sessionId: "cse_mock4a" } },
    { id: "trig_5", name: "월초 비용 정리", enabled: true, cron: "0 8 1 * *", runOnceAt: null, nextRunAt: now + 20 * day, endedReason: null, schedule: null },
    { id: "trig_3", name: "세미나 등록 시작 알림", enabled: false, cron: null, runOnceAt: past.getTime(), nextRunAt: past.getTime(), endedReason: "run_once_fired", schedule: { kind: "once", at: past.getTime() } },
  ];
})();
const tokens = (o: number) => ({ input: Math.round(o * 0.02), output: o, thinking: Math.round(o * 0.22), cacheWrite: o * 5, cacheRead: o * 320 });
const series = Array.from({ length: 7 }, (_, i) => {
  const d = new Date(now - (6 - i) * day);
  const o = [180_000, 420_000, 90_000, 610_000, 380_000, 720_000, 260_000][i];
  return { bucket: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`, tokens: tokens(o), estCostUsd: o / 9_000 };
});
const item = (key: string, count: number, total: number, extra = {}) => ({ key, count, share: (count / total) * 100, viaTool: 0, viaSlash: 0, tokens: null, estCostUsd: null, priced: true, ...extra });
const dists: Record<string, unknown[]> = {
  model: [item("claude-opus-5-5", 2104, 2277, { estCostUsd: 251.2, tokens: tokens(2_000_000) }), item("claude-haiku-5-5", 173, 2277, { estCostUsd: 0.31, tokens: tokens(12_000) })],
  project: [item("dev_team", 1402, 2277, { estCostUsd: 160.4 }), item("side-project", 610, 2277, { estCostUsd: 71.9 }), item("notebook-projects", 265, 2277, { estCostUsd: 19.2 })],
  tool: [item("Bash", 980, 2532), item("Edit", 520, 2532), item("Write", 260, 2532), item("Read", 210, 2532), item("Agent", 96, 2532), item("Skill", 21, 2532), item("기타", 445, 2532)],
  skill: [item("opsx:propose", 6, 15, { viaTool: 2, viaSlash: 4 }), item("openspec-apply-change", 5, 15, { viaTool: 5 }), item("dataviz", 4, 15, { viaTool: 4 })],
  command: [item("upgrade", 2, 3), item("reload-plugins", 1, 3)],
  mcpServer: [item("notion", 120, 140), item("claude_ai_Gmail", 20, 140)],
  subagent: [item("general-purpose", 18, 30), item("claude-code-guide", 8, 30), item("Explore", 4, 30)],
};
const sessions = [
  ["a1", "OpenSpec 설정과 D.Duck 기획", "dev_team", 2.4], ["b2", "사이드 프로젝트 결제 화면 리팩터링", "side-project", 0.6], ["c3", null, "notebook-projects", 5.1],
].map(([id, title, project, hoursAgo], i) => ({
  sessionId: `${id}000000-0000-4000-8000-000000000000`, title, projectName: project, gitBranch: "main", firstTs: now - (hoursAgo as number) * 3_600_000 - 3_600_000, lastTs: now - (hoursAgo as number) * 3_600_000,
  responses: [812, 240, 96][i], tokens: tokens([900_000, 260_000, 70_000][i]), estCostUsd: [118.1, 31.2, 7.4][i], reportedCostUsd: [118.14, 31.0, null][i],
}));
const agents = {
  sessions: [
    { key: "k1", pid: 1, id: "f3e5ff71", sessionId: "k1", name: "flaky-test-fix", cwd: "/Users/me/Agents/dev_team", kind: "background", startedAt: now - 900_000, status: "waitingInput" },
    { key: "k2", pid: 2, id: "ab12cd34", sessionId: "k2", name: "docs-refresh", cwd: "/Users/me/Agents/side-project", kind: "background", startedAt: now - 300_000, status: "working" },
    { key: "k3", pid: 3, id: null, sessionId: "k3", name: "dev-team-d5", cwd: "/Users/me/Agents/dev_team", kind: "interactive", startedAt: now - 7_200_000, status: "idle" },
    { key: "k4", pid: 4, id: "9988aabb", sessionId: "k4", name: "lint-sweep", cwd: "/Users/me/Agents/dev_team", kind: "background", startedAt: now - 86_000_000, status: "done" },
    { key: "k5", pid: 5, id: "77aa55cc", sessionId: "k5", name: "migrate-db", cwd: "/Users/me/Agents/side-project", kind: "background", startedAt: now - 1_800_000, status: "failed" },
    { key: "k6", pid: 6, id: "66bb44dd", sessionId: "k6", name: "release-notes", cwd: "/Users/me/Agents/dev_team", kind: "background", startedAt: now - 5_400_000, status: "stopped" },
    // Started outside D.Duck (task-lifecycle: "리뷰 없이 끝났어요" in 정리).
    { key: "k7", pid: 7, id: null, sessionId: "k7", name: "settings-polish", cwd: "/Users/me/Agents/side-project", kind: "interactive", startedAt: now - 10_800_000, status: "done" },
  ],
  error: null,
  labels: { k2: "문서 최신화" },
  tags: {
    k1: { agent: "dduck-developer", taskType: "dev", reviewed: false, report: "ready" },
    k2: { agent: "dduck-developer", taskType: "dev", reviewed: false, report: "missing" },
    k4: { agent: "dduck-developer", taskType: "dev", reviewed: false, report: "ready" },
    k5: { agent: "dduck-qa", taskType: "qa", reviewed: false, report: "invalid" },
    k6: { agent: null, taskType: "comms", reviewed: false, report: "ready" },
  },
};
const liveSessions = () => (noLive() ? [] : agents.sessions);
const routines = [
  { id: 1, name: "아침 점검", cwd: "/Users/me/Agents/dev_team", promptLen: 42, permissionMode: "plan", schedule: { kind: "daily", hour: 9, minute: 0 }, scheduleText: "매일 09:00", enabled: true, nextRunAt: now + 5 * 3_600_000, lastStatus: "done", active: false },
  { id: 2, name: "의존성 업데이트 확인", cwd: "/Users/me/Agents/side-project", promptLen: 88, permissionMode: "manual", schedule: { kind: "weekly", weekdays: 0b10001, hour: 14, minute: 30 }, scheduleText: "매주 월·금 14:30", enabled: false, nextRunAt: null, lastStatus: "missed", active: false },
  { id: 3, name: "테스트 감시", cwd: "/Users/me/Agents/dev_team", promptLen: 30, permissionMode: "plan", schedule: { kind: "interval", hours: 6 }, scheduleText: "6시간마다", enabled: true, nextRunAt: now + 2 * 3_600_000, lastStatus: "running", active: true },
];
const runs = [
  { id: 9, scheduledFor: now - 600_000, startedAt: now - 600_000, endedAt: null, status: "running", bgId: "ab12cd34", error: null },
  { id: 8, scheduledFor: now - 6 * 3_600_000, startedAt: now - 6 * 3_600_000, endedAt: now - 5.5 * 3_600_000, status: "done", bgId: "9988aabb", error: null },
  { id: 7, scheduledFor: now - 12 * 3_600_000, startedAt: now - 12 * 3_600_000, endedAt: now - 12 * 3_600_000, status: "start_failed", bgId: null, error: "untrustedWorkspace" },
  { id: 6, scheduledFor: now - 30 * 3_600_000, startedAt: null, endedAt: null, status: "missed", bgId: null, error: null },
  { id: 5, scheduledFor: now - 36 * 3_600_000, startedAt: null, endedAt: null, status: "skipped", bgId: null, error: "busy" },
];
const risk = { hooks: false, mcpServers: false, dangerMode: false, unreadable: false };
const agentDef = (name: string, source: string, description: string, extra = {}) => ({
  id: `f-${name}`, name, source, scope: source === "project" ? "/Users/me/Agents/dev_team" : null, path: source === "builtin" ? null : `/Users/me/Agents/dev_team/.claude/agents/${name}.md`,
  description, model: null, tools: [] as string[], disallowedTools: [] as string[], skills: [] as string[], malformed: false, editable: source === "project" || source === "user", readonlyReason: source === "builtin" ? "Claude Code 내장 에이전트예요" : null, primary: true, usable: true, risk, ...extra,
});
const catalog = [
  agentDef("design-critic", "project", "설계·계획 단계의 비판적 리뷰어. 누락된 요구사항과 모순을 찾는다.", { tools: ["Read", "Grep", "Glob", "Bash", "mcp__figma__*"], skills: ["frontend-design"], disallowedTools: ["mcp__sentry__*", "mcp__notebooklm-mcp__*"] }),
  // Plugin agent (agent-loadout): its loadout is read-only.
  agentDef("cloudflare:workers-reviewer", "plugin", "Workers 코드를 리뷰한다.", { skills: ["cloudflare:wrangler"], readonlyReason: "플러그인 에이전트는 읽기 전용이에요", path: "/Users/me/.claude/plugins/cache/cloudflare/cloudflare/1.0.2/agents/workers-reviewer.md" }),
  agentDef("qa-verifier", "project", "테스트 단계 QA. 실제로 돌려 보고 PASS/FAIL을 판정한다.", { tools: ["Read", "Bash"], model: "sonnet" }),
  agentDef("security-privacy-reviewer", "project", "자격증명·원문 저장·IPC 경계를 리뷰하는 보안 리뷰어.", { tools: ["Read", "Grep"] }),
  agentDef("release-helper", "user", "릴리스 노트와 버전 태그를 정리한다.", { risk: { ...risk, hooks: true } }),
  agentDef("old-notes", "user", "", { malformed: true, description: null }),
  agentDef("general-purpose", "builtin", "복잡한 질문 조사, 코드 검색, 여러 단계 작업을 맡는 범용 에이전트"),
  agentDef("Explore", "builtin", "여러 파일을 훑어 코드 위치를 찾는 읽기 전용 탐색 에이전트"),
];
const activity = [
  { agent: "", state: "waiting", count: 1, total: 2, detail: "flaky-test-fix", sessionKey: "k1", subagent: false },
  { agent: "design-critic", state: "working", count: 1, total: 1, detail: "agent-profiles 설계 비판", sessionKey: "k2", subagent: true },
  { agent: "qa-verifier", state: "failed", count: 1, total: 1, detail: "migrate-db", sessionKey: "k5", subagent: false },
  { agent: "Explore", state: "done", count: 1, total: 1, detail: "아티팩트 파서 위치 찾기", sessionKey: "k2", subagent: true },
  { agent: "web-fetch", state: "working", count: 1, total: 1, detail: "Tauri 배지 문서 읽기", sessionKey: "k2", subagent: true },
];
const pItem = (name: string, description: string | null, uses = 0) => ({ name, description, uses, lastUsed: uses ? now - day : null });
const noParts = { skills: [], commands: [], agents: [], hooks: false, lsp: false, unreadable: false };
const plugins = [
  {
    id: "agent-skills@addy-agent-skills", name: "agent-skills", marketplace: "addy-agent-skills", version: "0.6.15", scope: "user", enabled: true, projectEnabled: true,
    installedAt: "2026-11-07T08:45:46.567Z", lastUpdated: "2026-11-07T08:45:46.567Z", mcp: [], locked: null,
    components: {
      ...noParts,
      skills: [pItem("agent-skills:code-review-and-quality", "변경 사항을 여러 관점에서 리뷰", 7), pItem("agent-skills:spec-driven-development", "스펙부터 쓰고 구현", 2), pItem("agent-skills:test-driven-development", "실패하는 테스트부터")],
      commands: [pItem("agent-skills:build", "빌드하고 오류 고치기", 3), pItem("agent-skills:ship", "배포 전 점검 목록")],
      agents: [pItem("agent-skills:code-reviewer", "시니어 코드 리뷰어"), pItem("agent-skills:security-auditor", "보안 감사")],
      hooks: true,
    },
  },
  {
    id: "cloudflare@cloudflare", name: "cloudflare", marketplace: "cloudflare", version: "1.0.2", scope: "user", enabled: true, projectEnabled: false,
    installedAt: "2026-10-26T08:01:30.460Z", lastUpdated: "2026-10-26T08:01:30.460Z", mcp: [{ name: "cloudflare", kind: "http" }], locked: null,
    components: { ...noParts, skills: [pItem("cloudflare:wrangler", "Wrangler CLI 실행과 문제 해결", 1), pItem("cloudflare:durable-objects", "Durable Objects 설계"), pItem("cloudflare:workers-best-practices", null)] },
  },
  {
    id: "expo@claude-plugins-official", name: "expo", marketplace: "claude-plugins-official", version: "1.13.9", scope: "user", enabled: false, projectEnabled: false,
    installedAt: "2026-10-26T08:14:48.257Z", lastUpdated: "2026-10-26T08:14:48.257Z", mcp: [{ name: "expo", kind: "http" }], locked: null,
    components: { ...noParts, skills: [pItem("expo:expo-router", "Expo Router 내비게이션")], hooks: true },
  },
  {
    id: "frontend-design@claude-plugins-official", name: "frontend-design", marketplace: "claude-plugins-official", version: "1.0.0", scope: "user", enabled: true, projectEnabled: false,
    installedAt: "2026-10-30T02:00:00.000Z", lastUpdated: "2026-10-30T02:00:00.000Z", mcp: [], locked: null,
    components: { ...noParts, skills: [pItem("frontend-design:frontend-design", "의도가 분명한 UI 디자인을 만들 때", 4)] },
  },
  {
    id: "corp-guard@internal", name: "corp-guard", marketplace: "internal", version: "2.0.0", scope: "managed", enabled: true, projectEnabled: false,
    installedAt: "2026-09-01T00:00:00.000Z", lastUpdated: "2026-09-01T00:00:00.000Z", mcp: [], locked: "관리자 설정으로 설치되어 여기서 바꿀 수 없어요",
    components: { ...noParts, hooks: true, lsp: true },
  },
  {
    id: "team-lint@acme-tools", name: "team-lint", marketplace: "acme-tools", version: "0.3.1", scope: "project", enabled: true, projectEnabled: true,
    installedAt: "2026-11-01T02:00:00.000Z", lastUpdated: "2026-11-05T02:00:00.000Z", mcp: [], locked: "프로젝트 범위 – 해당 프로젝트에서 claude plugin으로 변경",
    components: { ...noParts, commands: [pItem("team-lint:lint", "팀 규칙으로 린트", 4)] },
  },
  {
    id: "notes-kit@personal", name: "notes-kit", marketplace: "personal", version: null as string | null, scope: "user", enabled: true, projectEnabled: true,
    installedAt: "2026-10-20T02:00:00.000Z", lastUpdated: "2026-10-20T02:00:00.000Z", mcp: [], locked: null,
    components: { ...noParts, skills: [pItem("notes-kit:daily-note", "오늘 노트 만들기")] },
  },
];
// Same rule as the core (local marketplace clone): version first, else commit sha; missing → unknown.
const installedSha: Record<string, string> = { "expo@claude-plugins-official": "d4f4840", "notes-kit@personal": "aa11bb2" };
const marketplace: Record<string, { version?: string; sha?: string; updated: string }> = {
  "agent-skills@addy-agent-skills": { version: "0.6.15", updated: "2026-11-07T08:45:42.625Z" },
  "cloudflare@cloudflare": { version: "1.0.3", updated: "2026-11-06T08:01:29.687Z" },
  "expo@claude-plugins-official": { sha: "e5a9c01", updated: "2026-11-07T10:16:52.432Z" },
  "team-lint@acme-tools": { version: "0.3.1", updated: "2026-11-05T01:00:00.000Z" },
};
const cmpVersion = (a: string, b: string) => {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) if ((pb[i] ?? 0) !== (pa[i] ?? 0)) return (pb[i] ?? 0) > (pa[i] ?? 0) ? 1 : -1;
  return 0;
};
const updateOf = (p: (typeof plugins)[number]) => {
  const m = marketplace[p.id];
  const marketplaceUpdated = m?.updated ?? null;
  if (m?.version && p.version) return cmpVersion(p.version, m.version) > 0 ? { state: "available", latest: m.version, marketplaceUpdated } : { state: "latest", marketplaceUpdated };
  const sha = installedSha[p.id];
  if (m?.sha && sha) return sha === m.sha ? { state: "latest", marketplaceUpdated } : { state: "available", latest: m.sha, marketplaceUpdated };
  return { state: "unknown", marketplaceUpdated };
};

// Usage insights (D-078). `?insights=empty|stale|thin|budget` picks a review state.
function mockLanguage() {
  return { choice: mockChoice(), resolved: backendLang() };
}
const insightsMode = typeof location === "undefined" ? "" : (new URLSearchParams(location.search).get("insights") ?? "");
const adviceText = `## 비용 줄이기
- **짧은 작업은 Sonnet 5.5로**: 짧은 세션의 응답 90%가 Opus 5.5이에요. Sonnet 5.5는 입력 단가가 낮아 이런 세션 비용을 대략 40% 줄여요. [모델 비교](https://docs.claude.com/en/docs/about-claude/models/overview)
- 1시간 캐시 쓰기 비중(95%)에 비해 다시 읽는 양이 적어요. 오래 쉬지 않는 작업이면 기본 5분 캐시로 충분해요.

## 작업 성공률 높이기
1. Bash 실패율 35%: \`CLAUDE.md\`에 빌드·테스트 명령을 적어 두면 재시도가 줄어들어요.
2. 압축이 잦은 세션은 단계마다 새 세션으로 나누고 탐색은 서브에이전트에 맡기세요.

## 업데이트·새 기능
- 기록상 Claude Code 2.1.300 — 최신은 2.1.319이에요. \`claude update\`로 올리세요. [변경 기록](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md)

## 출처
- [모델 개요](https://docs.claude.com/en/docs/about-claude/models/overview)
- [가격](https://platform.claude.com/docs/en/about-claude/pricing)
- <img src=x onerror=alert(1)> 원시 HTML은 글자로만 보여요. 자동 링크 없음: https://example.com`;
let lastAdvice: unknown =
  insightsMode === "empty" ? null : { text: adviceText, atMs: insightsMode === "stale" ? now - 41 * day : now - 2 * 3_600_000, costUsd: 0.14, stale: insightsMode === "stale" };
let insightsCancel = false;
const insightsReport = () => withNames(rawInsightsReport());
const withNames = (r: { findings: Record<string, unknown>[] }) => ({ ...r, findings: r.findings.map((f) => ({ localNames: [], ...f })) });
const rawInsightsReport = () =>
  insightsMode === "thin"
    ? { enough: false, responses: 12, findings: [], notChecked: [], priceDate: "2026-11-07", cliVersion: "2.1.300" }
    : {
        enough: true,
        responses: 2277,
        priceDate: "2026-11-07",
        cliVersion: "2.1.300",
        notChecked: ["안 쓰는 스킬·플러그인"],
        findings: [
          { id: "tool_errors", level: "medium", title: "실패가 잦은 도구가 있어요", evidence: "Bash 980번 중 35% 실패, WebFetch 41번 중 22% 실패", effect: "실패한 호출마다 다시 시도하는 응답이 붙어 토큰과 시간이 늘어나요", action: "자주 거부하는 도구는 권한 규칙을 정리하세요.", detail: "권한 거부도 실패로 세요. 명령 실패가 많다면 CLAUDE.md에 빌드·테스트 명령을 적어 두면 재시도가 줄어들어요.", link: "permissions" },
          { id: "cache_ttl", level: "medium", title: "1시간 캐시를 많이 쓰지만 다시 읽히는 양이 적어요", evidence: "캐시 쓰기의 95%가 1시간 캐시, 캐시 쓰기 1토큰당 읽기 2.1토큰", effect: "5분 캐시였다면 대략 $18 덜 들어요 (기간 비용의 약 7%)", action: "오래 쉬었다 이어 하는 작업이 아니라면 기본(5분) 캐시로 충분한지 확인하세요.", detail: "1시간 캐시는 쓰기 단가가 5분 캐시의 약 1.6배예요.", link: null },
          { id: "long_sessions", level: "medium", title: "대화 압축이 잦은 긴 세션이 많아요", evidence: "세션 40개 중 6개(15%)에서 압축이 2번 이상. 서브에이전트 호출 30번", effect: "압축 뒤에는 앞의 결정·맥락이 줄어 실수가 늘고, 큰 컨텍스트를 매번 다시 읽어 비용이 커져요", action: "큰 작업은 단계마다 새 세션(/clear)으로 나누세요.", detail: "압축이 필요하면 /compact 뒤에 남길 내용을 적어(예: /compact 결정 사항과 남은 할 일만) 직접 압축하세요. 탐색·조사는 서브에이전트(Explore 등)에 맡기면 메인 컨텍스트가 덜 차요.", link: null },
          { id: "subagent_opus", level: "medium", title: "서브에이전트가 대부분 Opus급 모델로 돌아요", evidence: "서브에이전트 응답 312번 중 88%가 Opus급 모델 (많은 순: design-critic, Explore, qa-verifier)", effect: "이 응답들을 Sonnet으로 했다면 대략 $9.40 절감 (기간 비용의 약 4%)", action: "조사·리뷰용 에이전트 정의에 model: sonnet 이나 haiku 를 적으세요.", detail: "에이전트 정의에 model이 없으면 메인 세션의 모델을 그대로 써요. 어려운 설계를 맡는 에이전트만 Opus로 두면 돼요.", link: null, localNames: ["design-critic", "qa-verifier"] },
          { id: "top_model_short", level: "info", title: "짧은 작업에도 최상위 모델을 주로 써요", evidence: "짧은 세션 14개에서 응답의 91%가 Opus급 모델 (짧은 세션 = 응답 10번·도구 5번 이하)", effect: "이 세션들을 Sonnet으로 했다면 대략 $3.10 절감 (기간 비용의 1% 미만)", action: "간단한 질문·작은 수정은 /model 로 Sonnet이나 Haiku로 바꾸세요.", detail: "/effort 를 낮추는 것도 방법이에요. 어려운 설계·디버깅에만 최상위 모델을 쓰면 돼요.", link: null },
        ],
      };
const insightsSummary = JSON.stringify(
  {
    period_days: 7,
    price_table_date: "2026-11-07",
    claude_code_version: "2.1.300",
    responses: "1000-4999",
    sessions: "1-9",
    est_cost_api_equivalent: "$100-500",
    models: [
      { model: "claude-opus-5-5", responses: "1000-4999", output_tokens: "1M-10M", cost_share_pct: 100 },
      { model: "claude-haiku-5-5", responses: "100-499", output_tokens: "<100K", cost_share_pct: 0 },
    ],
    cache: { read_ratio_pct: 95, write_1h_share_pct: 95 },
    sessions_by_responses: { "1-10": "1-9", "200+": "1-9" },
    sessions_compacted_2plus_pct: 10,
    tools: [{ name: "Bash", calls: "500-999", error_rate_pct: 35 }, { name: "Edit", calls: "500-999", error_rate_pct: 5 }, { name: "mcp-1", calls: "100-499", error_rate_pct: 0 }, { name: "tool-1", calls: "1-9", error_rate_pct: 0 }],
    skills: [{ name: "skill-1", calls: "1-9" }, { name: "skill-2", calls: "1-9" }],
    subagents: [{ name: "agent-1", calls: "10-49" }, { name: "agent-2", calls: "1-9" }],
    local_findings: ["tool_errors", "cache_ttl", "long_sessions", "subagent_opus", "top_model_short"],
  },
  null,
  2,
);

// Brief refining (brief-grilling): a two-round grilling scenario, then a final rewrite. `?refine=bad` answers with
// a malformed response. Design review only — the real limits and consent live in Rust (brief_refine.rs).
const refineMode = typeof location === "undefined" ? "" : (new URLSearchParams(location.search).get("refine") ?? "");
let refineCancel = false;
let refineSpent = 0;
let refineRuns = 0;
type MockBrief = { goal: string; outOfScope: string; constraints: string; done: string };
type MockAnswer = { question: string; field: string; chosen: string[]; custom: string; skipped: boolean };
const mq = (id: string, title: string, body: string, field: string, options: [string, string][], recommended: number[], multi = false) => ({
  id,
  title,
  body,
  field,
  multi,
  options: options.map(([label, detail]) => ({ label, detail })),
  recommended,
});
const refineRounds = [
  [
    mq("q1", "실패를 재현하는 테스트를 먼저 만들고 고칠까요?", "가끔 실패하는 문제는 재현 절차가 없으면 고쳤는지 확인할 수 없어요.", "done", [["예, 재현 테스트 먼저", "실패를 일으키는 조건을 테스트로 고정"], ["로그만 추가해 원인 추적", "재현이 어려울 때"], ["바로 수정", "원인을 이미 알 때"]], [0]),
    mq("q2", "범위를 로그인 흐름(세션·토큰 갱신)으로 좁힐까요?", "범위가 넓으면 관련 없는 인증 코드까지 바뀔 수 있어요.", "outOfScope", [["로그인·토큰 갱신만", "회원가입·소셜 로그인 제외"], ["인증 전체", "권한 검사까지 포함"]], [0]),
    mq("q3", "어떤 검증을 완료 기준에 넣을까요?", "측정할 수 있어야 끝났는지 알아요.", "done", [["테스트 20회 연속 통과", "flaky 확인"], ["원인 설명 3줄", "다음에 같은 문제를 막도록"], ["스테이징 수동 확인", "사람이 확인"]], [0, 1], true),
  ],
  [
    mq("q1", "토큰 만료 직전 경합을 우선 의심할까요?", "가끔 실패는 시간에 따라 달라지는 경합인 경우가 많아요.", "constraints", [["예, 만료 경합 먼저", "시계를 고정해 재현"], ["네트워크 재시도 먼저", "타임아웃·재시도 정책"]], [0]),
    mq("q2", "고친 뒤 되돌릴 방법을 남길까요?", "인증 변경은 문제가 생기면 바로 되돌릴 수 있어야 해요.", "constraints", [["변경을 커밋 하나로", "되돌리기 쉽게"], ["기능 플래그 뒤에", "배포 후 끄기 가능"]], [0]),
  ],
];
const refineRewrite = (b: MockBrief, history: MockAnswer[][]) => {
  const said = history.flat().filter((a) => !a.skipped);
  const pick = (t: string) => said.find((a) => a.question.includes(t));
  return {
    goal: `${b.goal.replace(/가끔/g, "간헐적으로")} — 재현 테스트로 원인을 고정한 뒤 고친다`,
    outOfScope: [b.outOfScope.trim(), pick("범위") ? "회원가입·소셜 로그인 코드" : ""].filter(Boolean).join("\n") || "로그인 흐름 밖의 인증 코드",
    constraints: [b.constraints.trim(), pick("토큰 만료") ? "토큰 만료 직전 경합을 시계 고정으로 먼저 확인한다" : "", pick("되돌릴") ? "변경은 커밋 하나로 묶어 되돌리기 쉽게 한다" : ""].filter(Boolean).join("\n"),
    done: ["로그인 테스트 20회 연속 통과", "원인 설명 3줄"].join("\n"),
  };
};

/** Starts in flight in mock mode, by start id → cancel pressed (D-161). */
const mockStarts = new Map<string, boolean>();

const handlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  // `claude --bg` takes a moment; the "시작하는 중" card shows meanwhile (D-159).
  start_task: async (a) => {
    const id = typeof a.startId === "string" ? a.startId : null;
    if (id) mockStarts.set(id, false);
    await new Promise((r) => setTimeout(r, 2_500));
    const cancelled = id ? mockStarts.get(id) : false;
    if (id) mockStarts.delete(id);
    if (cancelled) throw { kind: "cancelled" };
    return "bgmock01";
  },
  start_cancel: (a) => {
    const id = String(a.startId);
    if (!mockStarts.has(id)) return false;
    mockStarts.set(id, true);
    return true;
  },
  refine_cancel: () => {
    refineCancel = true;
  },
  refine_brief: async (a) => {
    const req = a.req as { mode: string; brief: MockBrief; history: MockAnswer[][]; token: string | null };
    refineCancel = false;
    if (!req.token || refineRuns >= 4) {
      refineSpent = 0;
      refineRuns = 0;
    }
    // `?refine=slow` waits ~6 s so the progress row and 취소 can be seen (D-159).
    for (let i = 0; i < (refineMode === "slow" ? 120 : 14) && !refineCancel; i++) await new Promise((r) => setTimeout(r, 50));
    const cost = 0.018 + req.history.length * 0.006;
    refineSpent += cost;
    refineRuns += 1;
    if (refineCancel) return { kind: "failed", error: { kind: "cancelled" }, message: "취소했어요", costUsd: null, spentUsd: refineSpent };
    if (refineMode === "bad") return { kind: "failed", error: { kind: "badResponse" }, message: "Claude 응답을 읽지 못했어요. 다시 시도하세요", costUsd: cost, spentUsd: refineSpent };
    const n = req.history.length;
    const last = req.mode === "rewrite" || n >= refineRounds.length;
    return {
      kind: "done",
      token: "0f1e2d3c",
      costUsd: cost,
      spentUsd: refineSpent,
      runsLeft: 4 - refineRuns,
      result: {
        questions: last ? [] : refineRounds[n],
        rewrite: refineRewrite(req.brief, req.history),
        finished: last,
        note: last ? "질문 없이 정리했어요. 코드에서 확인할 사실은 완료 기준에 넣었어요." : n === 0 ? "결정할 것만 물었어요. 추천이 미리 골라져 있어요." : "",
      },
    };
  },
  usage_insights_local: () => insightsReport(),
  usage_insights_preview: () => insightsSummary,
  usage_insights_last: () => lastAdvice,
  usage_insights_delete: () => {
    lastAdvice = null;
  },
  usage_insights_cancel: () => {
    insightsCancel = true;
  },
  usage_insights_claude: async () => {
    insightsCancel = false;
    for (let i = 0; i < 40 && !insightsCancel; i++) await new Promise((r) => setTimeout(r, 50));
    if (insightsCancel) return { kind: "failed", error: { kind: "cancelled" }, message: "취소했어요" };
    if (insightsMode === "budget") return { kind: "failed", error: { kind: "budget", costUsd: 0.3 }, message: "예산 상한($0.30)에 도달해 중단되었어요" };
    lastAdvice = { text: adviceText, atMs: Date.now(), costUsd: 0.12, stale: false };
    return { kind: "done", advice: lastAdvice };
  },
  open_insight_link: () => true,
  ...permissionHandlers,
  ...teamHandlers,
  plugin_list: () => plugins.map((p) => ({ ...structuredClone(p), update: updateOf(p) })),
  set_plugin_enabled: (a) => {
    const p = plugins.find((x) => x.id === a.id);
    if (p) p.enabled = Boolean(a.enabled);
    return { kind: "done" };
  },
  update_plugin: (a) => {
    // expo stands in for a marketplace that wants a command approved; cloudflare updates normally.
    if (a.id === "expo@claude-plugins-official") return { kind: "needsApproval" };
    const p = plugins.find((x) => x.id === a.id);
    const u = p && updateOf(p);
    if (!p || u?.state !== "available" || !("latest" in u)) return { kind: "unchanged" };
    p.version = String(u.latest);
    p.lastUpdated = new Date().toISOString();
    return { kind: "done" };
  },
  get_status: () => ({ cli: mockNoCli ? null : { path: "/Users/me/.nvm/versions/node/v22.14.0/bin/claude", version: "2.1.319", supported: true }, configDir: "/Users/me/.claude", configDirExplicit: false, notifyWaiting: true, storeTitles: true, permissionModes: ["manual", "plan", "acceptEdits", "dontAsk", "auto", "bypassPermissions"], defaultCwd: "/Users/me/Agents/dev_team", reviewCapacity: 3, notifyFinished: true, remoteAlerts: false, storeTaskPrompts: true, taskReport: true, limitNotify: true, limitAutoResume: true, limitAutoShipped: true }),
  usage_summary: () => ({ tokens: tokens(2_110_000), estCostUsd: 251.51, unpricedModels: [], sessions: 9, responses: 2277, toolCalls: 2532, skillCalls: 15, commandCalls: 3 }),
  usage_timeseries: () => series,
  usage_distribution: (a) => dists[a.dim as string] ?? [],
  usage_sessions: () => sessions,
  usage_session_detail: (a) => ({ session: sessions.find((s) => s.sessionId === a.sessionId) ?? sessions[0], models: dists.model, tools: dists.tool.slice(0, 4), skills: dists.skill }),
  usage_filter_options: () => [["dev_team", "side-project", "notebook-projects"], ["claude-haiku-5-5", "claude-opus-5-5"]],
  list_agents: () => {
    const sessions = liveSessions();
    return { ...agents, sessions, tags: mockLanes(sessions, agents.tags), reviewPending: pollUnknown() ? undefined : mockReviewPending() };
  },
  session_prompt: (a) =>
    ({
      k1: "auth.spec.ts가 가끔 실패하는데 원인을 찾아서 고쳐 줘. 시간 의존 코드가 의심돼.",
      k2: "README와 docs/ 아래 설치 문서를 최신 CLI 옵션에 맞게 고쳐 줘",
      k5: "users 테이블에 last_seen 컬럼을 추가하는 마이그레이션을 만들어 줘",
    })[a.key as string] ?? null,
  list_artifacts: (a) => mockArtifacts.filter((x) => x.title.includes(String(a.query ?? ""))).map((x) => ({ artifactId: x.artifactId, url: x.url, title: x.title, kind: x.kind, seq: x.seq, firstSeenTs: x.firstSeenTs, lastTs: x.lastTs, projectName: x.projectName })),
  artifact_preview: (a) => mockArtifactPreview(a.artifactId),
  list_routines: () => routines,
  routine_runs: (a) => (a.id === 3 ? runs : a.id === 1 ? runs.slice(1, 2) : runs.slice(3)),
  routine_prompt: () => "어제 이후 바뀐 테스트를 돌리고 실패 원인을 정리해 줘",
  folder_trusted: (a) => !String(a.path).includes("side-project"),
  save_routine: () => 4,
  agent_catalog: (a) => catalog.map((d) => ({ ...d, usable: d.source !== "project" || String(a.cwd ?? "").startsWith("/Users/me/Agents/dev_team") })),
  agent_activity: () => activity,
  agent_definition: (a) => { const d = catalog.find((x) => x.id === a.id); return { content: `---\nname: ${d?.name}\ndescription: ${d?.description ?? ""}\ntools: ${d?.tools.join(", ")}\n---\n\n너는 ${d?.name} 역할이다.\n`, token: "t" }; },
  repo_check: (a) => (String(a.path).includes("side-project") ? { kind: "ok" } : { kind: "homeRoot" }),
  mark_reviewed: () => true,
  set_remote_alerts: (a) => a.enabled,
  set_pref: () => null,
  // app-language: `?lang=en|ko|system` picks the stored choice; without it the mock user has chosen Korean, so design
  // screenshots and app-level tests stay as they were. `system` follows the browser like the real OS rule.
  get_language: () => mockLanguage(),
  set_language: (a) => {
    setMockChoice(a.choice);
    return mockLanguage();
  },
  task_records: () => [
    { bgId: "f3e5ff71", name: "flaky-test-fix", cwd: "/Users/me/Agents/dev_team", agent: "dduck-developer", taskType: "dev", createdAt: now - 900_000, promptLen: 64, reviewed: false },
    { bgId: "aa11bb22", name: "payment-refactor", cwd: "/Users/me/Agents/side-project", agent: null, taskType: "dev", createdAt: now - 2 * day, promptLen: 120, reviewed: true, hasReport: true },
    { bgId: "ee0c0d22", name: "market-research", cwd: "/Users/me/Agents/dev_team", agent: null, taskType: "research", createdAt: now - 4 * day, promptLen: 48, reviewed: false, hasReport: false },
    { bgId: "cc33dd44", name: "release-notes-1.2", cwd: "/Users/me/Agents/side-project", agent: "dduck-release", taskType: "deploy", createdAt: now - 5 * day, promptLen: 0, reviewed: false, hasReport: true },
  ]
    .filter((r) => !deleted.has(r.bgId))
    .map((r) => ({ ...r, wrapped: wrapped.has(r.bgId) })),
  task_record: (a) =>
    a.id === "aa11bb22"
      ? { prompt: "## 목표\n결제 화면 상태 관리를 정리한다\n\n## 완료 기준\n기존 테스트 통과", brief: { goal: "결제 화면 상태 관리를 정리한다", outOfScope: "", constraints: "", done: "기존 테스트 통과" }, effort: "high", worktree: true, modelLayer: "task", modelUsed: "sonnet", modelRequested: null, modelOther: "claude-haiku-5-5" }
      : a.id === "f3e5ff71"
        ? { prompt: null, brief: null, effort: null, worktree: false, modelLayer: "type", modelUsed: "opus", modelRequested: "claude-opus-3-9", modelOther: null }
        : { prompt: null, brief: null, effort: null, worktree: false },
  delete_task_record: () => true,
  task_report: (a) => {
    const v = (mockEvidenceReport(String(a.id)) ?? mockReviewReport(String(a.id)) ?? mockTaskReport(String(a.id))) as Record<string, unknown>;
    return v.report ? { ...v, report: normalize(v.report as Record<string, unknown>) } : v;
  },
  delete_all_task_records: () => true,
  cloud_routines: () => ({ hasMore: false, costUsd: 0.0012, routines: cloud }),
  // Edit (cloud-routine-edit): a name containing "충돌" or "로그인" shows those failures in the mock.
  update_cloud_routine: (a) => {
    const e = a.edit as { enabled?: boolean; name?: string; schedule?: Record<string, number | string> };
    if (e.name?.includes("충돌")) return Promise.reject("다른 곳에서 바뀌었어요. 다시 불러오세요");
    if (e.name?.includes("로그인")) return Promise.reject({ kind: "login_required" });
    const i = cloud.findIndex((r) => r.id === a.id);
    const r = { ...cloud[i] };
    if (e.name != null) r.name = e.name;
    if (e.enabled != null) r.enabled = e.enabled;
    if (e.schedule) {
      r.schedule = e.schedule as typeof r.schedule;
      if (e.schedule.kind === "once") r.runOnceAt = r.nextRunAt = e.schedule.at as number;
      else r.nextRunAt = now + 20 * 3_600_000;
    }
    cloud[i] = r;
    return { outcome: "done", routine: r, costUsd: 0.0017 };
  },
  open_cloud_routine: () => undefined,
  finish_task: () => "done",
  // Run history (D-166): trig_4's newest run (cse_mock4a) failed.
  cloud_routine_runs: (a) => {
    return {
      hasMore: false,
      costUsd: 0.0014,
      runs: [0, 1, 2].map((i) => ({
        id: `cse_mock${String(a.id).slice(5)}${"abc"[i]}`,
        title: `⚡ ${cloud.find((r) => r.id === a.id)?.name ?? "루틴"}`,
        status: "active",
        workerStatus: "idle",
        createdAt: now - (i + 1) * day,
        lastEventAt: now - (i + 1) * day + 40_000,
        url: `https://claude.ai/code/session_mock${i}`,
      })),
    };
  },
  cloud_run_log: (a) => {
    const failed = String(a.sessionId).endsWith("4a");
    const t = now - day;
    const events = [
      { at: t, kind: "user", text: "어제 머지된 PR을 요약해 Slack에 보낸다." },
      { at: t + 600, kind: "env[info]", text: "Allocating sandbox" },
      { at: t + 2_100, kind: "init", text: "model=claude-sonnet-5-5 cwd=/home/user" },
      { at: t + 4_000, kind: "tool_use Bash", text: '{"command":"gh pr list --state merged --limit 20"}' },
      // Long unbroken tokens, as real logs carry (URLs, JSON, paths): the layout must wrap them.
      { at: t + 4_500, kind: "tool_result", text: `{"url":"https://github.com/example-org/example-repository/pull/1234/files?diff=split&w=1#diff-${"0a1b2c3d".repeat(12)}","path":"/home/user/${"very_long_directory_name/".repeat(8)}result.md"}` },
      failed
        ? { at: t + 5_000, kind: "result", text: "error_during_execution is_error=true — environment not found" }
        : { at: t + 48_000, kind: "result", text: "success is_error=false turns=4 duration=46s — 요약을 보냈어요" },
    ];
    return { sessionId: a.sessionId, events, outcome: failed ? "failed" : "succeeded", hasOlder: false, costUsd: 0.0016 };
  },
  usage_sessions_page: (a) => {
    const chips = [
      { topTools: [{ name: "Bash", count: 212 }, { name: "Edit", count: 96 }, { name: "Read", count: 71 }], skills: [{ name: "opsx:propose", count: 1 }, { name: "openspec-apply-change", count: 2 }], subagents: [{ name: "Explore", count: 2 }, { name: "design-critic", count: 1 }] },
      { topTools: [{ name: "Read", count: 40 }, { name: "Edit", count: 18 }, { name: "Bash", count: 9 }], skills: [], subagents: [] },
      { topTools: [{ name: "WebSearch", count: 6 }], skills: [{ name: "dataviz", count: 1 }], subagents: [{ name: "general-purpose", count: 3 }] },
    ];
    const all = Array.from({ length: 34 }, (_, i) => ({ ...sessions[i % 3], ...chips[i % 3], sessionId: `${String(i).padStart(8, "0")}-0000-4000-8000-000000000000`, label: i === 0 ? "D.Duck 기획 정리" : null, lastTs: now - i * 3_600_000 }));
    const off = Number(a.offset ?? 0);
    return { rows: all.slice(off, off + 10), total: all.length };
  },
  session_timeline: () => ({
    truncatedFront: false,
    skippedLines: 0,
    items: [
      { kind: "command", ts: now - 3_700_000, name: "opsx:propose", raw: [[0, 1]] },
      { kind: "prompt", ts: now - 3_600_000, text: "auth.spec.ts가 가끔 실패하는데 원인을 찾아서 고쳐 줘.", raw: [[1, 1]] },
      { kind: "reply", ts: now - 3_590_000, text: "먼저 테스트를 여러 번 돌려 실패를 재현해 볼게요.", raw: [[2, 1], [3, 1]] },
      { kind: "skill", ts: now - 3_585_000, id: "s1", name: "openspec-apply-change", ok: true, error: null, raw: [[4, 1], [5, 1]] },
      { kind: "agent", ts: now - 3_584_000, id: "a1", subagentType: "Explore", description: "인증 모듈에서 시간 의존 코드 찾기", ok: true, error: null, agentId: "a1b2c3", raw: [[6, 1], [7, 1]] },
      { kind: "tool", ts: now - 3_580_000, id: "t1", name: "Bash", summary: "npm test -- auth.spec.ts --repeat 20", ok: false, error: "1 failed: expected 200, got 401", raw: [[3, 1], [8, 1]] },
      { kind: "tool", ts: now - 3_570_000, id: "t2", name: "Read", summary: "/Users/me/Agents/dev_team/src/auth.ts", ok: true, error: null, raw: [[9, 1]] },
      { kind: "agent", ts: now - 3_565_000, id: "a2", subagentType: "design-critic", description: "수정안 리뷰", ok: null, error: null, agentId: "gone", raw: [[10, 1]] },
      { kind: "tool", ts: now - 3_560_000, id: "t3", name: "Edit", summary: "/Users/me/Agents/dev_team/src/auth.ts", ok: true, error: null, raw: [[11, 1]] },
      { kind: "compact", ts: now - 3_000_000, raw: [[12, 1]] },
      { kind: "reply", ts: now - 2_990_000, text: "시계를 고정하는 픽스처로 바꿔 20회 연속 통과를 확인했어요.", raw: [[13, 1]] },
    ],
  }),
  session_raw: (a) => {
    // Error variant: the design-critic subagent's file "changed" since the flow was read.
    if (a.agentId === "gone") throw new Error("세션 파일이 바뀌었어요. 흐름을 다시 여세요");
    const reveal = Boolean(a.reveal);
    const key = reveal ? "sk-ant-api03-mockVALUEonlyForReview" : "[키 숨김]";
    const total = a.agentId ? 5 : 20;
    const per = 7;
    const rec = (n: number) => {
      const step = n % 3;
      const type = step === 1 ? "assistant" : "user";
      const base = { parentUuid: `0000000${n}-aaaa-4aaa-8aaa-aaaaaaaaaaaa`, isSidechain: Boolean(a.agentId), type, timestamp: new Date(now - 3_700_000 + n * 5_000).toISOString(), sessionId: "00000000-0000-4000-8000-000000000000" };
      const message =
        step === 0
          ? { role: "user", content: "auth.spec.ts가 가끔 실패하는데 원인을 찾아서 고쳐 줘. 시간 의존 코드가 의심돼." }
          : step === 1
            ? { id: `msg_0${n}`, role: "assistant", model: "claude-opus-5-5", content: [{ type: "thinking", thinking: "테스트가 시간에 따라 실패한다면 Date.now()를 쓰는 곳부터 봐야 한다.", signature: "[긴 값 숨김]" }, { type: "tool_use", id: `toolu_0${n}`, name: "Bash", input: { command: `ANTHROPIC_API_KEY=${key} npm test -- auth.spec.ts --repeat 20`, description: "Run auth tests" } }], usage: { input_tokens: 3, output_tokens: 412 } }
            : { role: "user", content: [{ type: "tool_result", tool_use_id: `toolu_0${n - 1}`, content: [{ type: "text", text: "FAIL auth.spec.ts\n  ✕ refresh token (401)\nTests: 1 failed, 19 passed" }, { type: "image", source: { type: "base64", media_type: "image/png", data: "[이미지 42 KB]" } }] }] };
      if (n === total - 4) return { offset: n * 1843, kind: null as string | null, ts: null as string | null, minor: false, text: `{"type":"user","message":{"content":"${"아주 긴 붙여넣기 ".repeat(40)}`, json: false, truncated: true };
      return { offset: n * 1843, kind: step === 2 ? "도구 결과" : type, ts: base.timestamp, minor: step !== 0, text: JSON.stringify({ ...base, message }, null, 2), json: true, truncated: false };
    };
    if (Array.isArray(a.spans)) return { records: (a.spans as [number, number][]).map(([s]) => rec(s)), start: 0, next: null, fileLen: total * 1843, stopped: false };
    let first: number;
    let last: number;
    if (a.from != null) {
      first = Math.floor(Number(a.from) / 1843);
      last = Math.min(total, first + per);
    } else {
      last = a.before != null ? Math.floor(Number(a.before) / 1843) : total;
      first = Math.max(0, last - per);
    }
    const records = Array.from({ length: last - first }, (_, i) => rec(first + i));
    return { records, start: first * 1843, next: last < total ? last * 1843 : null, fileLen: total * 1843, stopped: false };
  },
  set_session_label: () => undefined,
  skill_catalog: () => [
    { name: "opsx:propose", source: "project", scope: "/Users/me/Agents/dev_team", kind: "command", description: "새 변경 제안 만들기", uses: 6, lastUsed: now - day, defined: true },
    { name: "openspec-apply-change", source: "project", scope: "/Users/me/Agents/dev_team", kind: "skill", description: "OpenSpec 변경의 태스크 구현", uses: 5, lastUsed: now - 2 * day, defined: true },
    { name: "update-config", source: "builtin", scope: null, kind: "skill", description: null, uses: 3, lastUsed: now - 3 * day, defined: false },
    { name: "anthropic-skills:docx", source: "user", scope: null, kind: "skill", description: "Word 문서 만들기", uses: 0, lastUsed: null, defined: true },
    { name: "cloudflare:wrangler", source: "plugin", scope: null, kind: "skill", description: "Wrangler CLI 실행과 문제 해결", uses: 0, lastUsed: null, defined: true },
  ],
  mcp_status: () => ({
    skipped: 1,
    connectors: [
      { name: "claude.ai Claude Docs", kind: "claudeAi", host: "docs.mcp.example", state: "connected" },
      { name: "claude.ai Gmail", kind: "claudeAi", host: "gmailmcp.googleapis.com", state: "connected" },
      { name: "claude.ai Google Calendar", kind: "claudeAi", host: "calendarmcp.googleapis.com", state: "connected" },
      { name: "claude.ai Google Drive", kind: "claudeAi", host: "drivemcp.googleapis.com", state: "connected" },
      { name: "claude.ai Atlassian", kind: "claudeAi", host: "mcp.atlassian.com", state: "connected" },
      { name: "claude.ai Figma", kind: "claudeAi", host: "mcp.figma.com", state: "needsAuth" },
      { name: "claude.ai Slack", kind: "claudeAi", host: "mcp.slack.com", state: "needsAuth" },
      { name: "claude.ai Notion", kind: "claudeAi", host: "mcp.notion.com", state: "needsAuth" },
      { name: "claude.ai Mermaid Chart", kind: "claudeAi", host: "mcp.mermaidchart.com", state: "needsAuth" },
      { name: "plugin:cloudflare:cloudflare", kind: "plugin", host: null, state: "needsAuth" },
      { name: "plugin:expo:expo", kind: "plugin", host: "mcp.expo.dev", state: "needsAuth" },
      { name: "notebooklm-mcp", kind: "local", host: null, state: "connected" },
    ],
  }),
  ingest_files: () => [{ path: "-Users-me-Agents-dev-team/a1.jsonl", skippedLines: 0, lines: 9000 }],
  task_logs: () => "\x1b[2J\x1b[H\x1b[1mClaude Code\x1b[0m v2.1.319\r\n\r\n> fix the flaky test in auth.spec.ts\r\n\r\n\x1b[33m●\x1b[0m Which approach do you prefer?\r\n  1. Retry with backoff\r\n  2. Mock the clock\r\n",
  ...addItemHandlers,
  ...memoryHandlers,
  ...rulesHandlers,
  ...projectHandlers,
  ...mcpHandlers,
  ...loadoutHandlers,
  ...modelHandlers,
  ...onboardingHandlers,
  ...reviewHandlers,
  ...evidenceHandlers,
  ...limitHandlers,
  ...deliveryHandlers,
  ...authHandlers,
  ...lifecycleHandlers(() => liveSessions()),
};

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise((r) => setTimeout(r, 30));
  // The backend gate (claude-login): one error shape for every gated command, matched by code in the UI.
  if (mockLoggedOut() && ["start_task", "start_authoring_task", "refine_brief", "usage_insights_claude", "run_routine_now", "limit_resume_now", "cloud_routines", "update_cloud_routine"].includes(cmd)) {
    throw { kind: "login_required" };
  }
  if (mockNotClaudeAi() && ["cloud_routines", "update_cloud_routine"].includes(cmd)) throw { kind: "claude_ai_required" };
  const h = handlers[cmd];
  if (!h) return undefined as T;
  // English output (english.ts): i18n.rs sentences follow the backend language, fake user data the page's `?lang`.
  try {
    return localize((await h(delocalize(args, DATA_LANG))) as T, backendLang(), DATA_LANG);
  } catch (e) {
    throw localize(e, backendLang(), DATA_LANG);
  }
}
