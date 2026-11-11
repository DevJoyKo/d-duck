// Mock-mode news (news-tab, D-204). Design review only: Rust gets these from one guarded `claude -p` run and checks
// every field (news.rs). Items come in the backend language, like the real app stores them per language.
// `?news=` picks a case: empty, other (only the other language has items), fail (fetch hits the budget), decline
// (the native confirmation is cancelled), daily (daily fetch on).
import { backendLang } from "./lang";

/** Read on every reset, not at load: tests reuse this module across pages with different `?news=`. */
const mode = () => (typeof location === "undefined" ? null : new URLSearchParams(location.search).get("news"));
let param = mode();
const DAY = 86_400_000;

type Cat = "model" | "command" | "setting" | "agent" | "integration" | "ide" | "security" | "other";
type Text = { title: string; summary: string; what: string; when: string; best: string };
type Seed = { id: string; version: string; ago: number; category: Cat; url: string; ko: Text; en: Text };

const DOCS = "https://code.claude.com/docs/en/";

const SEEDS: Seed[] = [
  {
    id: "2.1.322-vertex-fast-mode", version: "2.1.322", ago: 1, category: "model", url: `${DOCS}changelog`,
    ko: { title: "Vertex에서도 빠른 모드", summary: "Vertex의 global·us·eu 리전에서 빠른 모드를 쓸 수 있어요.", what: "Vertex AI로 Claude를 쓸 때도 빠른 모드를 켤 수 있어요. Vertex가 거절하면 보통 속도로 돌아가요.", when: "Vertex 계정으로 짧은 질문과 작은 수정을 많이 할 때 좋아요.", best: "긴 작업은 보통 속도로 두고, 응답 속도가 중요한 대화에서만 /fast로 켜세요." },
    en: { title: "Fast mode on Vertex", summary: "Fast mode now works in Vertex's global, us and eu regions.", what: "You can turn on fast mode when you use Claude through Vertex AI. If Vertex refuses, it falls back to standard speed.", when: "Useful on a Vertex account when you send many short questions and small edits.", best: "Keep long tasks at standard speed and turn on /fast only where response time matters." },
  },
  {
    id: "2.1.321-remote-control-settings", version: "2.1.321", ago: 2, category: "command", url: `${DOCS}changelog`,
    ko: { title: "원격 제어에 설정 파일 하나", summary: "claude remote-control이 시작하는 모든 세션에 같은 설정을 적용해요.", what: "claude remote-control에 --settings <파일>을 줄 수 있어요. 파일이 바뀌면 다시 시작할 때까지 새 세션을 열지 않아요.", when: "여러 원격 세션이 같은 권한·훅 규칙을 따라야 할 때 좋아요.", best: "설정 파일을 저장소에 두고 바꾼 뒤에는 remote-control을 다시 시작하세요." },
    en: { title: "One settings file for remote control", summary: "claude remote-control applies the same settings to every session it starts.", what: "claude remote-control takes --settings <file>. If the file changes, no new session starts until you restart it.", when: "Useful when several remote sessions must follow the same permission and hook rules.", best: "Keep the file in your repository and restart remote-control after you change it." },
  },
  {
    id: "2.1.321-fable-5-2", version: "2.1.321", ago: 2, category: "model", url: `${DOCS}model-config`,
    ko: { title: "Claude Fable 5.2가 기본 Fable 모델", summary: "1M 컨텍스트와 낮아진 캐시 읽기 단가로 Fable 5.2가 기본이 됐어요.", what: "claude-fable-5-2가 기본 Fable 모델이에요. 컨텍스트는 1M 토큰이에요.", when: "큰 저장소를 한 번에 읽어야 하는 설계·리뷰 작업에 좋아요.", best: "비용이 큰 모델이니 어려운 설계·디버깅에만 /model로 고르고, 나머지는 기본 모델로 두세요." },
    en: { title: "Claude Fable 5.2 is the default Fable model", summary: "Fable 5.2 becomes the default, with a 1M context and cheaper cache reads.", what: "claude-fable-5-2 is now the default Fable model, with a 1M-token context window.", when: "Good for design and review work that has to read a large repository at once.", best: "It costs more, so pick it with /model for hard design and debugging only and keep the default elsewhere." },
  },
  {
    id: "2.1.318-skill-doctor", version: "2.1.318", ago: 4, category: "command", url: `${DOCS}skills`,
    ko: { title: "/skill-doctor로 스킬 비용 보기", summary: "스킬마다 컨텍스트를 얼마나 쓰고 얼마나 자주 쓰이는지 보여 줘요.", what: "/skill-doctor가 설치된 스킬의 컨텍스트 비용과 사용 빈도를 한 표로 보여 줘요.", when: "세션 시작이 느려졌거나 스킬이 엉뚱하게 골라질 때 좋아요.", best: "한 달 동안 안 쓴 스킬은 끄고, 설명이 긴 스킬은 description을 줄이세요." },
    en: { title: "See skill costs with /skill-doctor", summary: "Shows how much context each skill takes and how often it gets used.", what: "/skill-doctor lists the context cost and usage of every installed skill in one table.", when: "Useful when sessions start slowly or the wrong skill gets picked.", best: "Turn off skills you haven't used in a month and shorten long descriptions." },
  },
  {
    id: "2.1.315-plugin-eval", version: "2.1.315", ago: 6, category: "integration", url: `${DOCS}plugins`,
    ko: { title: "claude plugin eval로 플러그인 시험", summary: "테스트 케이스로 플러그인을 돌려 점수를 매기고 플러그인 없는 결과와 비교해요.", what: "claude plugin eval이 케이스 묶음을 돌려 채점해요. init이 케이스와 채점 기준 초안을 만들어 줘요.", when: "플러그인을 고친 뒤 정말 나아졌는지 확인할 때 좋아요.", best: "실제로 자주 맡기는 작업 5–10개로 시작하고, 바꿀 때마다 같은 묶음으로 비교하세요." },
    en: { title: "Test plugins with claude plugin eval", summary: "Runs your plugin against test cases, scores it and compares with no plugin.", what: "claude plugin eval runs a suite of cases and grades them. init drafts the cases and graders for you.", when: "Useful after changing a plugin, to check that it really got better.", best: "Start with 5–10 tasks you actually hand off and compare on the same suite after every change." },
  },
  {
    id: "2.1.312-desktop-pop-out", version: "2.1.312", ago: 9, category: "ide", url: `${DOCS}desktop`,
    ko: { title: "데스크톱 창 분리", summary: "Claude Code 데스크톱의 창 하나를 따로 띄웠다가 다시 붙일 수 있어요.", what: "데스크톱 앱의 아무 창이나 별도 창으로 빼고 나중에 다시 붙여요.", when: "모니터가 둘이고 대화와 diff를 나란히 보고 싶을 때 좋아요.", best: "diff 창을 옆 모니터로 빼 두면 편집을 따라가며 리뷰하기 쉬워요." },
    en: { title: "Pop out Desktop panes", summary: "Pop any Claude Code Desktop pane into its own window and dock it back later.", what: "Any pane of the Desktop app can move to its own window and dock back later.", when: "Useful with two monitors, to keep the conversation and the diff side by side.", best: "Move the diff pane to the other monitor so you can review edits as they happen." },
  },
  {
    id: "2.1.309-max-effort-level", version: "2.1.309", ago: 12, category: "setting", url: `${DOCS}settings`,
    ko: { title: "maxEffortLevel 설정", summary: "모든 제공자에서 effort 수준의 상한을 정해요.", what: "settings의 maxEffortLevel이 /effort로 고를 수 있는 가장 높은 수준을 막아요.", when: "팀 비용을 예측 가능하게 두고 싶을 때 좋아요.", best: "관리 설정에 high를 두고, 정말 필요한 프로젝트에서만 프로젝트 설정으로 풀어 주세요." },
    en: { title: "The maxEffortLevel setting", summary: "Caps the effort level on every provider.", what: "maxEffortLevel in settings limits the highest level /effort can pick.", when: "Useful when you want team costs to stay predictable.", best: "Set high in managed settings and lift it only in project settings where it is really needed." },
  },
  {
    id: "2.1.305-webfetch-timeout", version: "2.1.305", ago: 16, category: "other", url: `${DOCS}changelog`,
    ko: { title: "WebFetch 5분 제한", summary: "5분 안에 내려받지 못한 페이지는 기다리지 않고 실패해요.", what: "WebFetch가 5분 안에 끝나지 않으면 멈추지 않고 실패로 돌아와요.", when: "느린 문서 사이트 때문에 세션이 멈추곤 했다면 좋아요.", best: "큰 페이지는 필요한 부분만 묻는 prompt를 함께 주면 더 빨리 끝나요." },
    en: { title: "WebFetch gives up after five minutes", summary: "A page that hasn't downloaded within five minutes fails instead of hanging.", what: "WebFetch now fails instead of hanging when a page takes more than five minutes.", when: "Useful if slow doc sites used to stall your sessions.", best: "For large pages, ask only for the part you need in the prompt so it finishes sooner." },
  },
  {
    id: "2.1.298-resume-in-desktop", version: "2.1.298", ago: 34, category: "ide", url: `${DOCS}desktop`,
    ko: { title: "데스크톱에서 터미널 세션 이어 하기", summary: "/resume으로 CLI에서 시작한 세션을 대화째 이어서 해요.", what: "데스크톱 입력창에 /resume을 치면 CLI 세션을 골라 전체 맥락과 함께 이어 가요.", when: "터미널에서 시작한 일을 다른 자리에서 데스크톱으로 마무리할 때 좋아요.", best: "세션 이름을 붙여 두면 목록에서 찾기 쉬워요." },
    en: { title: "Resume terminal sessions in Desktop", summary: "Pick up any CLI session in the Desktop app with /resume.", what: "Type /resume in the Desktop prompt box to pick a CLI session and continue with its full context.", when: "Useful when you start in the terminal and finish elsewhere on Desktop.", best: "Name your sessions so they're easy to find in the list." },
  },
  {
    id: "2.1.296-restricted-flag", version: "2.1.296", ago: 40, category: "security", url: `${DOCS}cli-reference`,
    ko: { title: "--restricted로 제한된 세션", summary: "명령 실행 도구와 사용자·프로젝트 설정 없이 세션을 시작해요.", what: "--restricted는 명령을 실행하는 도구와 사용자·프로젝트 설정을 빼고 시작해요.", when: "공용 기기에서 평가용으로 돌리거나 남의 저장소를 읽기만 할 때 좋아요.", best: "평가 스크립트에는 항상 --restricted를 붙여 설정 차이로 결과가 흔들리지 않게 하세요." },
    en: { title: "Restricted sessions with --restricted", summary: "Starts a session without the command-running tools or your user and project settings.", what: "--restricted leaves out the command-running tools and your user and project settings.", when: "Useful for evaluation runs on shared machines or for only reading someone else's repository.", best: "Add --restricted to evaluation scripts so results don't drift with local settings." },
  },
  {
    id: "2.1.292-model-picker-setting", version: "2.1.292", ago: 52, category: "setting", url: `${DOCS}model-config`,
    ko: { title: "modelPicker 설정", summary: "/model 고르기 목록에 어떤 모델이 보일지 정해요.", what: "settings의 modelPicker로 /model 목록에 나오는 모델을 고를 수 있어요.", when: "팀이 쓰는 모델만 보이게 해서 실수로 비싼 모델을 고르지 않게 할 때 좋아요.", best: "availableModels와 함께 쓰면 보이는 것과 허용되는 것이 맞아요." },
    en: { title: "The modelPicker setting", summary: "Chooses which models the /model picker lists.", what: "modelPicker in settings decides which models appear in /model.", when: "Useful to show only the models your team uses, so nobody picks an expensive one by accident.", best: "Use it together with availableModels so what's shown matches what's allowed." },
  },
  {
    id: "2.1.288-agent-feedback", version: "2.1.288", ago: 63, category: "agent", url: `${DOCS}changelog`,
    ko: { title: "Claude가 쓰는 피드백 보고서", summary: "문제가 생기면 Claude가 피드백 보고서를 써 두고 /feedback에서 보내요.", what: "세션에서 문제가 생기면 Claude가 무엇이 잘못됐는지 보고서 초안을 써요. 보내기 전에 직접 확인해요.", when: "같은 문제가 반복돼 재현 정보를 정리하기 번거로울 때 좋아요.", best: "보내기 전에 경로·이름 같은 민감한 내용이 없는지 읽어 보세요." },
    en: { title: "Claude-drafted feedback reports", summary: "When something goes wrong, Claude drafts a report you send from /feedback.", what: "When a session goes wrong, Claude drafts a report of what happened. You review it before sending.", when: "Useful when the same problem keeps coming back and writing repro notes is tedious.", best: "Read it before sending and remove anything sensitive such as paths or names." },
  },
];

const NEW_ON_FETCH: Seed[] = [
  {
    id: "2.1.323-diff-panel", version: "2.1.323", ago: 0, category: "ide", url: `${DOCS}changelog`,
    ko: { title: "/diff 실시간 패널", summary: "전체 화면에서 /diff가 대화 옆에 바뀌는 대로 새로 고치는 패널을 열어요.", what: "/diff가 대화 옆에 diff 패널을 열고 Claude가 고칠 때마다 새로 고쳐요.", when: "큰 리팩터를 지켜보며 바로 멈추고 싶을 때 좋아요.", best: "패널을 연 채로 작업을 맡기고, 이상한 변경이 보이면 Esc로 바로 멈추세요." },
    en: { title: "Live /diff panel", summary: "In fullscreen, /diff opens a panel beside the chat that refreshes as Claude edits.", what: "/diff opens a diff panel next to the conversation that refreshes on every edit.", when: "Useful when you watch a big refactor and want to stop it early.", best: "Keep the panel open while it works and press Esc as soon as an odd change shows up." },
  },
];

const lang = () => backendLang();
const seedLang = (): "ko" | "en" => (param === "other" ? (lang() === "ko" ? "en" : "ko") : lang());

type Row = { seed: Seed; lang: "ko" | "en"; firstSeenMs: number };
let rows: Row[] = [];
let seen = new Set<string>();
let daily: { enabled: boolean; nextAtMs: number | null } = { enabled: false, nextAtMs: null };
let status: unknown = null;
let cancel = false;

function reset() {
  param = mode();
  const now = Date.now();
  rows = param === "empty" ? [] : SEEDS.map((seed) => ({ seed, lang: seedLang(), firstSeenMs: now - 3 * DAY }));
  // A few were opened already.
  seen = new Set(param === "empty" ? [] : ["2.1.312-desktop-pop-out", "2.1.305-webfetch-timeout", "2.1.296-restricted-flag", "2.1.292-model-picker-setting", "2.1.288-agent-feedback"].flatMap((id) => [`${id}:ko`, `${id}:en`]));
  daily = { enabled: param === "daily", nextAtMs: param === "daily" ? now + 15 * 3_600_000 : null };
  status = param === "empty" ? null : { atMs: now - 3 * DAY, ok: true, added: 12, dropped: 1, costUsd: 0.21, error: null, daily: false };
}
reset();

function ymd(ago: number): string {
  const d = new Date(Date.now() - ago * DAY);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

const vkey = (v: string) => v.split(".").map(Number);
function cmpVersion(a: string, b: string): number {
  const x = vkey(a);
  const y = vkey(b);
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return (x[i] ?? 0) - (y[i] ?? 0);
  return 0;
}

function view() {
  const l = lang();
  const items = rows
    .filter((r) => r.lang === l)
    .map((r) => {
      const t = r.seed[r.lang];
      return {
        id: r.seed.id, title: t.title, version: r.seed.version, date: ymd(r.seed.ago), category: r.seed.category, summary: t.summary,
        what: t.what, whenToUse: t.when, bestPractice: t.best, sourceUrl: r.seed.url, firstSeenMs: r.firstSeenMs, seen: seen.has(`${r.seed.id}:${r.lang}`),
      };
    })
    .sort((a, b) => (a.date === b.date ? cmpVersion(b.version, a.version) || a.id.localeCompare(b.id) : b.date.localeCompare(a.date)));
  return { items, status, daily, otherLang: rows.filter((r) => r.lang !== l).length, running: false };
}

export const newsHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  news_list: () => view(),
  news_mark_seen: (a) => {
    const id = String(a.id ?? "");
    const ok = rows.some((r) => r.seed.id === id && r.lang === lang());
    if (ok) seen.add(`${id}:${lang()}`);
    return ok;
  },
  news_set_daily: (a) => {
    const on = a.enabled === true;
    daily = { enabled: on, nextAtMs: on ? Date.now() + DAY : null };
    return on;
  },
  news_cancel: () => {
    cancel = true;
  },
  news_open_source: () => undefined,
  news_fetch: async () => {
    if (param === "decline") return { kind: "declined" };
    cancel = false;
    for (let i = 0; i < 12 && !cancel; i++) await new Promise((r) => setTimeout(r, 50));
    const now = Date.now();
    if (cancel) {
      status = { atMs: now, ok: false, added: 0, dropped: 0, costUsd: null, error: "cancelled", daily: false };
      return { kind: "failed", error: { kind: "cancelled" }, message: lang() === "ko" ? "취소했어요" : "Cancelled" };
    }
    if (param === "fail") {
      status = { atMs: now, ok: false, added: 0, dropped: 0, costUsd: 0.41, error: "budget", daily: false };
      return { kind: "failed", error: { kind: "budget", costUsd: 0.41 }, message: lang() === "ko" ? "예산 상한($0.40)에 닿아 멈췄어요" : "Stopped at the budget limit ($0.40)" };
    }
    const l = lang();
    const have = new Set(rows.filter((r) => r.lang === l).map((r) => r.seed.id));
    const fresh = [...(rows.some((r) => r.lang === l) ? [] : SEEDS), ...NEW_ON_FETCH].filter((s) => !have.has(s.id));
    rows.push(...fresh.map((seed) => ({ seed, lang: l, firstSeenMs: now })));
    status = { atMs: now, ok: true, added: fresh.length, dropped: 0, costUsd: 0.17, error: null, daily: false };
    return { kind: "done", added: fresh.length, dropped: 0, costUsd: 0.17 };
  },
  mock_news_reset: () => reset(),
};
