// Mock artifacts for the card grid (D-102): 23 rows (three pages) with a mix of previews and "no preview" reasons.
const day = 86_400_000;
const now = Date.now();

type Mode = "html" | "report" | "markdown" | "scriptOnly" | "truncated" | "missing" | "noSource" | "refused" | "tooLarge" | "typed" | "unknown";

const titles: [string, Mode, string][] = [
  ["주간 사용량 대시보드", "html", "dev_team"],
  ["D.Duck 리서치 요약", "report", "dev_team"],
  ["릴리스 노트 초안", "markdown", "dev_team"],
  ["결제 흐름 다이어그램", "scriptOnly", "side-project"],
  ["온보딩 투어 시안", "typed", "dev_team"],
  ["API 응답 시간 보고서", "html", "side-project"],
  ["테스트 커버리지 정리", "missing", "dev_team"],
  ["분기 계획 슬라이드", "typed", "notebook-projects"],
  ["로그 분석 결과", "truncated", "side-project"],
  ["화면 흐름 탐색", "noSource", "dev_team"],
  ["의존성 점검 보고서", "report", "dev_team"],
  ["캐시 적중률 그래프", "refused", "side-project"],
  ["회의 메모 정리", "markdown", "notebook-projects"],
  ["대용량 데이터 표", "tooLarge", "notebook-projects"],
  ["에러 유형 대시보드", "html", "dev_team"],
  ["컴포넌트 목록", "missing", "side-project"],
  ["배포 체크리스트", "markdown", "dev_team"],
  ["사용자 여정 지도", "typed", "side-project"],
  ["토큰 비용 비교", "report", "dev_team"],
  ["실험 결과 요약", "unknown", "notebook-projects"],
  ["접근성 점검표", "html", "dev_team"],
  ["모델별 응답 비교", "noSource", "notebook-projects"],
  ["프로젝트 소개 페이지", "report", "side-project"],
];

export const mockArtifacts = titles.map(([title, mode, projectName], i) => ({
  artifactId: `a-${i + 1}`,
  url: `https://claude.ai/code/artifact/a-${i + 1}`,
  title,
  kind: mode === "typed" ? "typed" : "page",
  seq: (i % 4) + 1,
  firstSeenTs: now - (i + 3) * day,
  lastTs: now - i * 7 * 3_600_000 - 2 * 3_600_000,
  projectName,
  mode,
}));

const bars = (seed: number) =>
  Array.from({ length: 12 }, (_, k) => `<div style="flex:1;background:#d97757;opacity:${0.45 + ((k * seed) % 5) / 10};height:${30 + ((k * 37 + seed * 11) % 60)}%;border-radius:6px 6px 0 0"></div>`).join("");

const dashboard = (title: string, seed: number) => `<!doctype html><html><head><style>
body{margin:0;font-family:-apple-system,system-ui,sans-serif;background:#faf9f5;color:#1f1e1d;padding:48px}
h1{font-size:44px;margin:0 0 8px}p{font-size:20px;color:#6b6a65;margin:0 0 32px}
.tiles{display:flex;gap:24px;margin-bottom:32px}.tile{flex:1;background:#fff;border:1px solid #e8e6dc;border-radius:16px;padding:24px}
.tile b{display:block;font-size:40px;margin-top:8px}.chart{display:flex;align-items:flex-end;gap:12px;height:300px;background:#fff;border:1px solid #e8e6dc;border-radius:16px;padding:24px}
</style></head><body><h1>${title}</h1><p>지난 7일 · 로컬 기록 기준</p>
<div class="tiles"><div class="tile">응답<b>${1200 + seed * 37}</b></div><div class="tile">도구 호출<b>${800 + seed * 19}</b></div><div class="tile">세션<b>${9 + seed}</b></div></div>
<div class="chart">${bars(seed)}</div>
<script>document.body.innerHTML = "<h1 style='color:red;font-size:120px'>SCRIPT RAN</h1>";</script>
<img src="https://example.invalid/track.png" alt=""></body></html>`;

const report = (title: string) => `<!doctype html><html><head><style>
body{margin:0;font-family:Georgia,serif;background:#fff;color:#222;padding:64px 120px}
h1{font-size:52px;margin:0 0 16px}h2{font-size:30px;margin:40px 0 12px;color:#444}p{font-size:22px;line-height:1.6;color:#444}
</style></head><body><h1>${title}</h1><p>이 문서는 로컬에서 만든 보고서 예시예요. 핵심 결과와 다음 단계를 정리해요.</p>
<h2>요약</h2><p>세 가지 변경으로 응답 시간이 줄었고, 남은 과제는 캐시 정책 정리예요. 자세한 수치는 아래 표를 참고하세요.</p>
<h2>다음 단계</h2><p>측정 범위를 넓히고, 주간 점검을 자동화해요.</p></body></html>`;

const markdown = (title: string) => `# ${title}\n\n- 변경 사항 정리\n- 확인이 필요한 항목 **세 가지**\n- 배포 전 점검\n\n## 메모\n\n로컬 원본 Markdown을 텍스트로만 보여 줘요. [링크](https://example.invalid)는 눌리지 않아요.\n`;

export function mockArtifactPreview(artifactId: unknown) {
  const a = mockArtifacts.find((x) => x.artifactId === artifactId);
  if (!a) throw "unknown";
  switch (a.mode) {
    case "html":
      return { kind: "html", text: dashboard(a.title, Number(a.artifactId.slice(2))), truncated: false, scriptOnly: false };
    case "report":
      return { kind: "html", text: report(a.title), truncated: false, scriptOnly: false };
    case "truncated":
      return { kind: "html", text: report(a.title), truncated: true, scriptOnly: false };
    case "scriptOnly":
      return { kind: "html", text: `<div id="root"></div><script src="app.js"></script>`, truncated: false, scriptOnly: true };
    case "markdown":
      return { kind: "markdown", text: markdown(a.title), truncated: false };
    default:
      throw a.mode;
  }
}
