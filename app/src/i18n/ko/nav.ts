// Sidebar (nav-ia).
export default {
  groups: {
    workspace: "작업 공간",
    claude: "Claude 구성",
  },
  pages: {
    agents: "에이전트",
    tasks: "작업",
    review: "리뷰",
    routines: "루틴",
    artifacts: "아티팩트",
    extensions: "확장",
    mcp: "MCP",
    permissions: "권한",
    memory: "메모리",
    usage: "사용 통계",
    settings: "설정",
  },
  theme: {
    label: "테마: {current}. 누르면 {next}",
    title: "{current} (누르면 {next})",
  },
  language: {
    label: "언어 바꾸기",
    menu: "언어",
  },
  cli: {
    found: "Claude Code {version}",
    missing: "Claude CLI 없음",
  },
  tourAgain: "튜토리얼 다시 보기",
  ingest: {
    backfill: "기록 읽는 중 {done}/{total}",
    error: "기록을 읽지 못했어요",
    fresh: "기록이 최신이에요",
  },
} as const;
