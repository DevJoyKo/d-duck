// Mock 근거·첨부 (report-evidence) for design review: a dev report with screenshot + JUnit evidence (one criterion
// without evidence, one test failure, an invalid item), a QA report without a test report (and a file-only
// criterion), a review report with file:line evidence, attachments-only research (HTML + Markdown table) and design
// (criteria type → warning) reports, invalid/empty attachments. Rust shapes: v1 reports are normalised the way
// task_report.rs does (summary + legacy). Generic names only; the screenshot is synthetic.
import { MOCK_SHOT } from "./evidenceAssets";

type Any = Record<string, unknown>;
const brief = (goal: string, done: string) => ({ goal, outOfScope: "", constraints: "", done });
const now = Date.now();

/** A version 1 report as Rust sends it: the evidence sentence is the summary, no items, `legacy`. */
export function normalize<T extends Any>(rep: T): T {
  if (rep.version === 2) return rep;
  const criteria = ((rep.criteria as Any[]) ?? []).map((c) => (Array.isArray(c.evidence) ? c : { criterion: c.criterion, met: c.met, summary: (c.evidence as string) ?? "", evidence: [], legacy: true }));
  return { ...rep, criteria, attachments: (rep.attachments as unknown[]) ?? [], legacy: true };
}

const v = (kind: string, label: string, extra: Any = {}) => ({ kind, label, caption: "", status: "valid", ...extra });

const devReport = {
  version: 2,
  status: "done",
  updatedAt: new Date(now - 3_000_000).toISOString(),
  work: [
    { step: "목록 화면 다크 모드 색 정리", result: "토큰 4개 교체", files: ["src/components/List.tsx"] },
    { step: "테스트·스크린샷", result: "vitest 51개 중 49 통과", files: [] },
  ],
  blocked: [],
  decisions: [],
  criteria: [
    {
      criterion: "목록 화면이 다크 모드에서 읽힌다",
      met: "yes",
      summary: "본문 대비 7.1:1, 배지 대비 4.8:1",
      evidence: [v("screenshot", "list-dark.png", { caption: "다크 모드 900px 목록" }), v("test-report", "vitest.xml", { failed: 0 })],
      legacy: false,
    },
    {
      criterion: "기존 테스트 통과",
      met: "yes",
      summary: "vitest 실행",
      evidence: [v("test-report", "vitest-all.xml", { failed: 2 }), v("command", "npx vitest run --reporter=junit", { exitCode: 1 }), { kind: "screenshot", label: "old.png", caption: "", status: "badType" }],
      legacy: false,
    },
    { criterion: "라이트 모드 그대로", met: "yes", summary: "눈으로 확인했어요", evidence: [], legacy: false },
    {
      criterion: "로그 정리",
      met: "partial",
      summary: "근거 파일이 모두 무효",
      evidence: [
        { kind: "screenshot", label: "gone.png", caption: "", status: "missing" },
        { kind: "screenshot", label: "link.png", caption: "", status: "refused" },
        { kind: "test-report", label: "huge.xml", caption: "", status: "tooLarge" },
        { kind: "command", label: "ls -la", caption: "", status: "tooMany", exitCode: 0 },
      ],
      legacy: false,
    },
  ],
  references: [{ kind: "file", ref: "src/components/List.tsx", note: "색 토큰" }],
  attachments: [],
  legacy: false,
};

const qaReport = {
  version: 2,
  status: "done",
  updatedAt: new Date(now - 6_000_000).toISOString(),
  work: [{ step: "설정 화면 실행 검증", result: "Playwright로 2개 화면 확인", files: [] }],
  blocked: [],
  decisions: [],
  criteria: [
    { criterion: "설정 화면이 한 화면에 들어온다", met: "yes", summary: "900px에서 스크롤 없음", evidence: [v("screenshot", "settings-900.png")], legacy: false },
    { criterion: "저장 버튼이 동작한다", met: "yes", summary: "코드로 확인", evidence: [v("file", "src/components/SettingsPage.tsx:120–148")], legacy: false },
  ],
  references: [],
  attachments: [],
  legacy: false,
};

const reviewReport = {
  version: 2,
  status: "done",
  updatedAt: new Date(now - 9_000_000).toISOString(),
  work: [{ step: "변경 범위 읽기", result: "파일 6개", files: [] }],
  blocked: [],
  decisions: [],
  criteria: [
    { criterion: "오류 경로가 모두 처리된다", met: "yes", summary: "throw 3곳 모두 catch", evidence: [v("file", "src/a.ts:10–42"), v("file", "src/b.ts:7")], legacy: false },
    { criterion: "공개 API 그대로", met: "yes", summary: "export 목록 같음", evidence: [v("file", "src/index.ts:1–30")], legacy: false },
  ],
  references: [],
  attachments: [],
  legacy: false,
};

const researchReport = {
  version: 2,
  status: "done",
  updatedAt: new Date(now - 12_000_000).toISOString(),
  work: [{ step: "후보 세 개 조사", result: "문서·가격표 비교", files: [] }],
  blocked: [],
  decisions: [],
  criteria: [],
  references: [{ kind: "url", ref: "https://docs.example.com/pricing", note: "가격표" }],
  attachments: [
    { title: "DB 후보 비교", kind: "html", status: "valid" },
    { title: "운영 비용 표", kind: "markdown", status: "valid" },
    { title: "원본 메모", kind: "markdown", status: "badType" },
    { title: "빈 문서", kind: "html", status: "valid", empty: true },
  ],
  legacy: false,
};

const designReport = {
  version: 2,
  status: "done",
  updatedAt: new Date(now - 15_000_000).toISOString(),
  work: [{ step: "온보딩 화면 시안", result: "시안 2개", files: [] }],
  blocked: [],
  decisions: [],
  criteria: [],
  references: [],
  attachments: [{ title: "온보딩 시안 설명", kind: "html", status: "valid" }],
  legacy: false,
};

const ready = (name: string, taskType: string, done: string, report: Any, permissionMode = "manual") => ({
  kind: "ready",
  delegated: { name, brief: brief(name, done), prompt: null, taskType, permissionMode, folder: "dev_team", stored: true, routine: false, deliveryGuard: "draft_pr" },
  report,
});

export const EVIDENCE_RECORDS = [
  { bgId: "e1e1e1e1", taskType: "dev", ago: 3_000_000 },
  { bgId: "e3e3e3e3", taskType: "qa", ago: 6_000_000 },
  { bgId: "e4e4e4e4", taskType: "review", ago: 9_000_000 },
  { bgId: "e2e2e2e2", taskType: "research", ago: 12_000_000 },
  { bgId: "e5e5e5e5", taskType: "design", ago: 15_000_000 },
];

export function mockEvidenceReport(id: string): Any | null {
  switch (id) {
    case "e1e1e1e1":
      return ready("list-dark-mode", "dev", "목록 화면이 다크 모드에서 읽힌다\n기존 테스트 통과\n라이트 모드 그대로\n로그 정리", devReport, "acceptEdits");
    case "e3e3e3e3":
      return ready("qa-settings", "qa", "설정 화면이 한 화면에 들어온다\n저장 버튼이 동작한다", qaReport, "plan");
    case "e4e4e4e4":
      return ready("review-errors", "review", "오류 경로가 모두 처리된다\n공개 API 그대로", reviewReport, "plan");
    case "e2e2e2e2":
      return { ...ready("db-research", "research", "", researchReport), delegated: { ...ready("db-research", "research", "", researchReport).delegated, brief: brief("DB 후보를 비교한다", "") } };
    case "e5e5e5e5":
      return { ...ready("onboarding-draft", "design", "", designReport), delegated: { ...ready("onboarding-draft", "design", "", designReport).delegated, brief: brief("온보딩 화면 시안을 만든다", "") } };
    default:
      return null;
  }
}

/** Card first line for attachment-only reports (Rust attachment::first_line). */
export function mockFirstAttachment(id: string): { title: string; line: string | null } | null {
  if (id === "e2e2e2e2") return { title: "DB 후보 비교", line: "세 후보를 비용·운영으로 비교했어요" };
  if (id === "e5e5e5e5") return { title: "온보딩 시안 설명", line: "첫 화면을 두 가지로 나눠 그렸어요" };
  return null;
}

const CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'";
const BASE =
  "html{color-scheme:light}body{margin:16px 20px;font:14px/1.6 -apple-system,BlinkMacSystemFont,system-ui,sans-serif;color:#1f2328;background:#fff}table{border-collapse:collapse;margin:8px 0}th,td{border:1px solid #d0d7de;padding:4px 8px;text-align:left}th{background:#f6f8fa}img{max-width:100%;height:auto}";
const srcdoc = (css: string, body: string) =>
  `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}"><meta http-equiv="x-dns-prefetch-control" content="off"><style>${BASE}</style><style>${css}</style></head><body>${body}</body></html>`;

// What Rust would hand over after cleaning: links became text + host, no script.
const compareHtml = srcdoc(
  ".note{color:#57606a}.pick{background:#dafbe1}",
  `<h1>DB 후보 비교</h1><p>세 후보를 비용·운영으로 비교했어요</p><table><thead><tr><th>후보</th><th>월 비용</th><th>운영</th><th>메모</th></tr></thead><tbody>
<tr class="pick"><td>PostgreSQL (관리형)</td><td>$45</td><td>쉬움</td><td>백업 자동</td></tr><tr><td>SQLite + 복제</td><td>$5</td><td>보통</td><td>쓰기 1곳</td></tr><tr><td>문서형 DB</td><td>$60</td><td>쉬움</td><td>조인 약함</td></tr></tbody></table>
<h2>추천</h2><p>관리형 PostgreSQL을 추천해요. 지금 쿼리가 대부분 조인이에요.</p><p class="note">출처: 가격표 (docs.example.com), 운영 가이드 (ops.example.org)</p><img src="${MOCK_SHOT}" alt="대시보드 예시">`,
);
const costMd = "# 운영 비용 표\n\n세 후보의 월 비용을 사용량별로 정리했어요.\n\n| 사용량 | PostgreSQL | SQLite | 문서형 |\n|:--|--:|--:|--:|\n| 작음 | $45 | $5 | $60 |\n| 중간 | $90 | $20 | $120 |\n| 큼 | $240 | — | $300 |\n| 백업 | 포함 | 직접 | 포함 |\n| 복구 | 5분 | 30분 | 10분 |\n\n![대시보드](dash.png)\n\n**결론**: 중간 규모까지는 `SQLite`도 충분해요.";
const draftHtml = srcdoc("", "<h1>온보딩 시안 설명</h1><p>첫 화면을 두 가지로 나눠 그렸어요</p><ul><li>A안: 한 화면에 세 단계</li><li>B안: 단계마다 한 화면</li></ul>");

export const evidenceHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  report_evidence: (a) => {
    const id = String(a.id);
    const c = Number(a.criterion);
    const i = Number(a.item);
    const rep = mockEvidenceReport(id)?.report as { criteria: { evidence: { kind: string; label: string; status: string; exitCode?: number }[] }[] } | undefined;
    const it = rep?.criteria[c]?.evidence[i];
    if (!it) throw "근거를 읽지 못했어요";
    if (it.status !== "valid") return { kind: "invalid", status: it.status };
    if (it.kind === "screenshot") return { kind: "image", dataUrl: MOCK_SHOT, width: 640, height: 400 };
    if (it.kind === "test-report")
      return it.label === "vitest-all.xml"
        ? { kind: "tests", summary: { tool: "vitest tests", passed: 48, failed: 2, skipped: 1, failures: ["src/lib/report.test.ts.counts evidence per kind", "src/components/List.test.tsx.keeps focus after Esc"] } }
        : { kind: "tests", summary: { tool: "vitest tests", passed: 12, failed: 0, skipped: 0, failures: [] } };
    if (it.kind === "command")
      return { kind: "command", cmd: it.label, exitCode: it.exitCode ?? 0, excerpt: " RUN  v4.0.0\n ✓ src/lib/format.test.ts (12)\n ✗ src/lib/report.test.ts (31 | 1 failed)\n ✗ src/components/List.test.tsx (8 | 1 failed)\n\n Test Files  2 failed | 37 passed (39)\n      Tests  2 failed | 312 passed (314)\n", cut: false };
    const [path, lines] = it.label.split(":");
    const [s, e] = (lines ?? "").split("–").map(Number);
    return { kind: "file", path, lines: lines ? [s, e || s] : null };
  },
  report_attachment: (a) => {
    const id = String(a.id);
    const i = Number(a.index);
    if (id === "e2e2e2e2") return i === 0 ? { kind: "html", srcdoc: compareHtml } : i === 1 ? { kind: "markdown", text: costMd, images: [{ name: "dash.png", data: MOCK_SHOT }] } : i === 2 ? { kind: "invalid", code: "badType" } : { kind: "empty" };
    if (id === "e5e5e5e5") return { kind: "html", srcdoc: draftHtml };
    return { kind: "empty" };
  },
  // No panel in the browser: a fake "saved" (report-copy-save).
  save_report_attachment: () => true,
};
