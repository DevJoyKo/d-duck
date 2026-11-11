# Tasks

## 1. core

- [x] 1.1 `team_defaults.json` `preferred` 기본값, `Bundle.preferred`, `Stored.preferred: Option`, `TeamInput.preferred: Option`, `merge_in`·`diff`·`reset_item`. 테스트: 필드 없음 → 기본값, 명시한 빈 목록 유지, 초기화 → 기본값, 기본값과 같은 목록은 저장 안 함, 번들 기본값 검사 통과, 설치되어 있으면 시작 줄에 mattpocock-skills, 설치되지 않으면 줄 없음. `routines` 테스트(`wants_preferred` 기본 참, 픽스처)와 셸 주석 갱신

## 2. webview

- [x] 2.1 `PreferredSection` "기본값으로"(기본값과 다를 때만), `AgentSettings`가 기본값 전달, mock 기본값·초기화, i18n ko/en, vitest(기본값 항목 표시·설치 안 됨·기본값으로). 버튼 이름은 다른 칸의 "기본값으로"와 겹치지 않게 "목록 기본값으로"

## 3. 문서

- [x] 3.1 README.md·README.ko.md 가드레일 절에 기본값 한 마디

## 4. 검증·리뷰

- [x] 4.1 설계 리뷰(design-critic) 반영 — design.md "설계 리뷰 반영"
- [x] 4.2 전체 검사(core 테스트·clippy·tsc·vitest·lint), mock 스크린샷(라이트 1440·다크 900) — core 607(lib 563)·vitest 586·clippy·tsc·lint
- [x] 4.3 테스트 리뷰(qa-verifier) 반영 — design.md "테스트 리뷰 반영"
- [x] 4.4 결정 줄(D-202, D-198 근거 "특정 플러그인을 하드코딩하지 않음" 일부 대체) 작성 — decisions.md는 로컬 전용이라 지시자에게 전달
