// Mock 작업 보고서 (decision-report) for design review: done (lint-sweep), stopped in progress (release-notes),
// format error (migrate-db), still running with a block that needs the user (flaky-test-fix), not written yet
// (docs-refresh), many flags with an unmet ✕ criterion (payment-refactor, history), finished without a report
// (release-notes-1.2, history). Generic names only.
import { normalize } from "./evidence";

const brief = (goal: string, done: string, outOfScope = "", constraints = "") => ({ goal, outOfScope, constraints, done });

const doneReport = {
  version: 1,
  status: "done",
  updatedAt: new Date(Date.now() - 80_000_000).toISOString(),
  work: [
    { step: "린트 규칙 위반 목록 뽑기", result: "경고 142개, 오류 9개", files: [] },
    { step: "자동 수정 가능한 규칙 적용", result: "경고 118개 정리", files: ["src/lib/format.ts", "src/lib/brief.ts"] },
    { step: "남은 오류 손으로 고치기", result: "오류 9개 모두 정리", files: ["src/components/Composer.tsx"] },
    { step: "테스트·타입 검사", result: "vitest 212개 통과, tsc 오류 없음", files: [] },
  ],
  blocked: [{ what: "eslint 플러그인 버전 충돌", cause: "react-hooks 5.x가 설정 형식을 바꿈", resolution: "기존 버전을 유지하고 메모 남김", open: false, needsUser: false, afterStep: 1 }],
  decisions: [
    { title: "자동 수정 범위", options: ["모든 규칙", "안전한 규칙만"], chosen: "안전한 규칙만", reason: "동작이 바뀔 수 있는 규칙은 제외", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 2 },
    { title: "no-explicit-any 처리", options: ["규칙 끄기", "타입 채우기"], chosen: "타입 채우기", reason: "범위 밖(규칙 변경)을 피함", reversible: "easy", assumed: true, deviatesFromBrief: false, afterStep: 3 },
    { title: "테스트 스냅샷 갱신", options: ["갱신", "그대로"], chosen: "갱신", reason: "공백만 바뀜", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 4 },
    { title: "공용 유틸 이름 변경", options: ["바꾸기", "그대로"], chosen: "바꾸기", reason: "린트 규칙이 요구", reversible: "hard", assumed: false, deviatesFromBrief: true, afterStep: 3 },
    { title: "주석 처리된 코드", options: ["지우기", "남기기"], chosen: "남기기", reason: "의도를 모름", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 3 },
    { title: "import 정렬", options: ["알파벳", "그룹별"], chosen: "그룹별", reason: "기존 파일 대부분이 그룹별", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 2 },
    { title: "줄 길이 경고", options: ["무시", "줄바꿈"], chosen: "줄바꿈", reason: "읽기 쉬움", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 2 },
    { title: "console 사용", options: ["지우기", "eslint-disable"], chosen: "지우기", reason: "디버그용이었음", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 3 },
    { title: "타입 단언", options: ["유지", "가드로 바꾸기"], chosen: "가드로 바꾸기", reason: "런타임 안전", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: 3 },
  ],
  criteria: [
    { criterion: "eslint 오류 0개", met: "yes", evidence: "npx eslint . → 0 errors" },
    { criterion: "경고 절반 이하", met: "yes", evidence: "142 → 24" },
    { criterion: "기존 테스트 통과", met: "yes", evidence: "vitest 212 passed" },
    { criterion: "공개 API 그대로", met: "partial", evidence: "유틸 이름 하나가 바뀜(별칭 남김)" },
  ],
  references: [
    { kind: "command", ref: "npx eslint . --fix", note: "자동 수정" },
    { kind: "file", ref: "eslint.config.js", note: "규칙 설정" },
    { kind: "url", ref: "https://eslint.org/docs/latest/rules/", note: "규칙 설명" },
    { kind: "commit", ref: "a1b2c3d", note: "자동 수정 커밋" },
  ],
};

const stoppedReport = {
  version: 1,
  status: "in_progress",
  updatedAt: new Date(Date.now() - 5_000_000).toISOString(),
  work: [
    { step: "지난 릴리스 이후 커밋 모으기", result: "커밋 37개", files: [] },
    { step: "사용자에게 보이는 변경 고르기", result: "12개 골라 분류", files: ["CHANGELOG.md"] },
  ],
  blocked: [{ what: "태그가 없는 커밋 범위", cause: "지난 릴리스 태그가 원격에만 있음", resolution: "", open: true, needsUser: true, afterStep: 1 }],
  decisions: [{ title: "분류 기준", options: ["커밋 접두어", "변경 파일"], chosen: "커밋 접두어", reason: "팀 규칙이 접두어를 씀", reversible: "easy", assumed: true, deviatesFromBrief: false, afterStep: 2 }],
  criteria: [{ criterion: "사용자 변경 목록", met: "partial", evidence: "12개 중 정리 8개" }],
  references: [{ kind: "file", ref: "CHANGELOG.md", note: "초안" }],
};

const runningReport = {
  version: 1,
  status: "in_progress",
  updatedAt: new Date(Date.now() - 60_000).toISOString(),
  work: [
    { step: "실패 재현", result: "20회 중 3회 실패", files: ["src/auth.spec.ts"] },
    { step: "시간 의존 코드 찾기", result: "토큰 만료 계산이 현재 시각을 직접 씀", files: ["src/auth.ts"] },
  ],
  blocked: [{ what: "테스트 DB 접속", cause: "로컬 DB 비밀번호가 필요함", resolution: "", open: true, needsUser: true, afterStep: 2 }],
  decisions: [{ title: "시계 주입 방식", options: ["fake timer", "clock 인자"], chosen: "clock 인자", reason: "테스트가 단순해짐", reversible: "easy", assumed: true, deviatesFromBrief: false, afterStep: 2 }],
  criteria: [],
  references: [{ kind: "file", ref: "src/auth.ts", note: "만료 계산" }],
};

const dec = (title: string, chosen: string, other: string, extra: Record<string, unknown> = {}) => ({ title, options: [chosen, other], chosen, reason: "", reversible: "easy", assumed: false, deviatesFromBrief: false, afterStep: null, ...extra });

const manyReport = {
  version: 1,
  status: "done",
  updatedAt: new Date(Date.now() - 2 * 86_400_000).toISOString(),
  work: [
    { step: "결제 화면 상태 흐름 그리기", result: "상태 11개, 전이 23개", files: [] },
    { step: "리듀서로 옮기기", result: "useState 9개를 리듀서 하나로", files: ["src/payment/state.ts"] },
    { step: "화면 연결", result: "", files: ["src/payment/PaymentPage.tsx"] },
    { step: "테스트 보강", result: "상태 전이 테스트 23개 추가", files: ["src/payment/state.test.ts"] },
  ],
  blocked: [
    { what: "결제 대행 샌드박스 키", cause: "테스트 키가 만료됨", resolution: "모의 응답으로 대체", open: false, needsUser: false, afterStep: 3 },
  ],
  decisions: [
    dec("상태 관리 방식", "useReducer", "외부 상태 라이브러리", { reason: "새 의존성 금지", afterStep: 1 }),
    dec("취소 상태를 실패와 합치기", "합치기", "따로 두기", { reason: "화면이 같음", deviatesFromBrief: true, reversible: "hard", afterStep: 2 }),
    dec("결제 재시도 횟수", "3회", "무제한", { reason: "대행사 권장", assumed: true, afterStep: 2 }),
    dec("오류 문구 위치", "버튼 아래", "토스트", { reason: "", afterStep: 3 }),
    dec("영수증 메일 발송 시점을 결제 완료 직후에서 정산 확정 이후로 옮기기", "정산 후", "결제 직후", { reason: "환불 메일이 겹치지 않음", deviatesFromBrief: true, afterStep: 3 }),
    dec("로딩 표시", "스켈레톤", "스피너", { afterStep: 3 }),
    dec("금액 반올림", "원 단위 내림", "반올림", { reason: "기존 정산과 맞춤", assumed: true, afterStep: 4 }),
    dec("구 API 응답 필드 삭제", "삭제", "유지", { reason: "쓰는 곳 없음", reversible: "hard", afterStep: 4 }),
    dec("테스트 이름 규칙", "상태_이벤트_결과", "자유", { afterStep: 4 }),
  ],
  criteria: [
    { criterion: "기존 테스트 통과", met: "yes", evidence: "vitest 240 passed" },
    { criterion: "결제 실패 시 다시 시도 가능", met: "partial", evidence: "카드만 됨, 계좌이체는 아직" },
    { criterion: "화면 동작 그대로", met: "no", evidence: "취소 화면이 실패 화면과 합쳐짐" },
  ],
  references: [{ kind: "file", ref: "src/payment/state.ts", note: "새 리듀서" }],
};

/** The board's mock reports in the shape Rust sends (v1 → summary + legacy, report-evidence). */
export function mockTaskReport(id: string) {
  const v = rawTaskReport(id) as Record<string, unknown> & ReturnType<typeof rawTaskReport>;
  return "report" in v && v.report ? { ...v, report: normalize(v.report as Record<string, unknown>) } : v;
}

function rawTaskReport(id: string) {
  switch (id) {
    case "9988aabb":
      return {
        kind: "ready",
        delegated: { name: "lint-sweep", brief: brief("린트 경고와 오류를 정리한다", "eslint 오류 0개\n경고 절반 이하\n기존 테스트 통과\n공개 API 그대로", "공개 API 변경", "새 의존성 추가 금지"), prompt: null, taskType: "dev", permissionMode: "acceptEdits", folder: "dev_team", stored: true, routine: false, deliveryGuard: "draft_pr" },
        report: doneReport,
      };
    case "66bb44dd":
      return {
        kind: "ready",
        delegated: { name: "release-notes", brief: brief("다음 릴리스 노트 초안을 쓴다", "사용자 변경 목록\n업그레이드 주의 사항"), prompt: null, taskType: "comms", permissionMode: "manual", folder: "dev_team", stored: true, routine: false },
        report: stoppedReport,
      };
    case "f3e5ff71":
      return {
        kind: "ready",
        delegated: { name: "flaky-test-fix", brief: brief("auth.spec.ts가 가끔 실패하는 원인을 찾아 고친다", "테스트 20회 연속 통과\n원인 설명 3줄", "공개 API", "새 의존성 추가 금지"), prompt: null, taskType: "dev", permissionMode: "acceptEdits", folder: "dev_team", stored: true, routine: false, deliveryGuard: "draft_pr" },
        report: runningReport,
      };
    case "77aa55cc":
      return {
        kind: "invalid",
        code: "invalid",
        delegated: { name: "migrate-db", brief: brief("users 테이블에 last_seen 컬럼을 추가한다", "마이그레이션 왕복 테스트 통과"), prompt: null, taskType: "qa", permissionMode: "plan", folder: "side-project", stored: true, routine: false, deliveryGuard: "local_only", deliveryAlert: 1 },
      };
    case "aa11bb22":
      return {
        kind: "ready",
        delegated: { name: "payment-refactor", brief: brief("결제 화면 상태 관리를 정리한다", "기존 테스트 통과\n결제 실패 시 다시 시도 가능\n화면 동작 그대로", "결제 대행사 교체", "새 의존성 추가 금지"), prompt: null, taskType: "dev", permissionMode: "acceptEdits", folder: "side-project", stored: true, routine: false, deliveryGuard: "local_only" },
        report: manyReport,
      };
    case "cc33dd44":
      return { kind: "missing", delegated: { name: "release-notes-1.2", brief: brief("1.2 릴리스 노트를 쓴다", "사용자 변경 목록\n업그레이드 주의 사항"), prompt: null, taskType: "deploy", permissionMode: "manual", folder: "side-project", stored: true, routine: false } };
    case "ab12cd34":
      return { kind: "missing", delegated: { name: "docs-refresh", brief: null, prompt: null, taskType: "dev", permissionMode: "manual", folder: "side-project", stored: false, routine: false } };
    default:
      return { kind: "off", delegated: null };
  }
}
