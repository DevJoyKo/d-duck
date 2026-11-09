# Tasks

## 1. 기반

- [x] 1.1 `src/i18n/`: ko 기준 카탈로그, en(`Messages` 타입 강제), `t()`·`useT()`·LanguageProvider, `Intl.PluralRules` 복수형, 파라미터 치환. 테스트: 키·파라미터 완전성, 복수형, 없는 키 개발 경고
- [x] 1.2 dduck-core `settings`: `ui_language`(system|ko|en, USER_PREFS 허용 목록) 읽기·쓰기, `sys-locale`로 system 해석(ko 접두만 판정), `i18n.rs` `msg(lang, code, &Params)`(문자열 params display_safe 강제, URL host·경로 basename 생성자), 모든 코드 두 언어·userinfo 비노출 테스트
- [x] 1.3 셸 명령 `get_language`·`set_language`(set_user_pref 경유, 저장 후 트레이 다시 만들기), UI 첫 그리기 localStorage 캐시 → DB 값 맞춤(세 값 외 무시), `<html lang>` 동기화·RoutinesPage 고정 lang 정리, mock `?lang=en`
- [x] 1.4 `format.ts` 날짜·상대 시간·숫자를 locale 인자로, 24시간 고정 지점 유지, 테스트

## 2. 설정과 공통

- [x] 2.1 설정 페이지 "언어" 항목(선택지 이름은 각 언어 자체 표기) + 사이드바 테마 버튼 옆 빠른 언어 전환 메뉴(같은 `ui_language`, 접근 이름 현재 언어), 바로 적용·초안 유지 테스트
- [x] 2.2 번들 데이터 `team_defaults.json`·`permission_catalog.json` 문구 필드 {ko,en}, 사용자 수정 항목 표시(`custom`)는 번역 안 함, 기본 고정 지시 시작 언어 판, 테스트
  - 테스트 리뷰 HIGH 반영: [안전]/[필수] 표식과 EVIDENCE·PUSH·LOCAL_ONLY 고정 지시도 시작 언어 판([Safety]/[Required]), 두 언어 의미 대조 테스트. 이어하기는 task_meta.lang으로 팀 규칙을 다시 만든다.
- [x] 2.3 공통 문구 이행: 사이드바·배지 접근 이름(복수형)·ui 컴포넌트·STATUS_LABEL·THEME_LABEL·auth COPY·빈 상태

## 3. 화면 묶음 이행 (묶음마다 en 문구 작성 + 기존 테스트 ko 고정 통과)

- [x] 3.1 작업·작업 시키기·보드·시작 막힘 문구
- [x] 3.2 보고서·리뷰 탭·근거·첨부
- [x] 3.3 에이전트·팀·권한·로드아웃
- [x] 3.4 확장(스킬·MCP·플러그인·규칙·메모리)·설치 링크
- [x] 3.5 사용량·인사이트·세션 원문 보기
  - 근거: usage·tasks.raw 카탈로그, `UsageRoutinesEnglish.test.tsx`(사용량·인사이트), 하드코딩 검사 0, en 스윕 0. 테스트 리뷰 뒤 확인해 체크(2026-11-10).
- [x] 3.6 아티팩트·루틴·클라우드 루틴
- [x] 3.7 로그인·한도·push 막기·둘러보기·온보딩
  - 근거: auth·tour·tasks.limits·tasks.delivery 카탈로그, `LoginTourEnglish.test.tsx`(둘러보기·로그인·설정), 한도 배너 Rust·UI 문구 일치 테스트(`rustCopies.test.ts`). 테스트 리뷰 뒤 확인해 체크(2026-11-10).

## 4. 백엔드 문구

- [x] 4.1 기존 코드 체계 확장: `CliError::InvalidInput.reason`을 ReasonCode+타입 params+대체 text로(57곳), CliError·LimitReason·ReportErrorCode·PreviewReason UI 카탈로그 매핑, 모르는 코드 대체 문장 테스트, params에 원문·자격증명·userinfo 없음 테스트
- [x] 4.2 백엔드 표시 문장 코드화: 한도 배너 text·시각(limit_resume.rs), 로그인 상태 줄 등 → 코드+ms+숫자, 문장·시각 형식은 UI
- [x] 4.3 트레이 메뉴·OS 알림·휴대폰 원격 알림·네이티브 확인 창·파일 선택 창 제목을 `i18n.rs`로(작업 이름 display_safe 64 유지), 언어 변경 시 트레이 즉시 갱신
- [x] 4.4 brief_refine·grilling·보고서 지시·리뷰/QA 근거 지시에 응답 언어 한 줄, 시작 언어를 task_meta에 기록, 이어하기·재작업이 따름, 인자 테스트

## 5. 검사와 검증

- [x] 5.1 copyRules를 ko 카탈로그·Rust ko 열로 좁히고 en 규칙 검사 추가
- [x] 5.2 하드코딩 한글 검사(UI 리터럴 + Rust 비테스트 리터럴, 허용 목록) 실패로 강제, 남은 지점 0
- [x] 5.3 Playwright 스크린샷: en·ko × 라이트 1440·다크 900, 주요 화면 8개, 잘림 없음 확인
  - 진행(리뷰 전): 언어 스윕 완료(전 화면 + 컨트롤 1개씩 + 상태 13개). en: 사용자 데이터 외 한글 0(app·mock-backend·user-data·key 0, 언어 이름 한국어만), ko: 영어 카탈로그 누출·key 0. mock 이중 언어(`?lang=en` 영어 fixture), 가드 테스트 `src/mock/english.test.ts`. 남은 것: 라이트·다크 × 8화면 잘림 확인 스크린샷.
  - 완료: 8화면×en·ko×라이트 1440·다크 900 실측 — 문서 넘침 0, title 없는 잘림 0, 넘침·겹침 0(scratchpad i18n/53/results.md). 한도 시각 잘림·사용자 데이터 줄 title·en 정리 힌트 수정.
- [x] 5.4 docs/features catalog에 F-SHL 언어 기능·TC 추가, THIRD_PARTY.md에 sys-locale
- [x] 5.5 테스트 단계 리뷰(qa-verifier + design-critic + brand-reviewer), 지적 조치·decisions 기록
