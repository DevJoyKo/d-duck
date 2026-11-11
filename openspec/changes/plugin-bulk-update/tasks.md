# Tasks

## 1. core

- [x] 1.1 `plugins::BulkStatus`·`Skip`·`bulk_plan`(설치 안 됨·전역 아님·최신·잘못된 id·15개 초과 건너뜀, 입력 50개·중복 제거, 같은 id 여러 범위)·`recheck`(항목 직전, 스냅샷과 다르면 건너뜀)·`run_bulk`(차례 실행, 실패해도 계속, 멈춤 뒤 남은 것 stopped, 진행 알림). 테스트: 순서, 실패 계속, 멈춤, 제외 대상, 가짜 CLI로 항목별 인자·`-y` 없음·항목별 시간 제한, 직렬화에 카나리 없음
- [x] 1.2 i18n 확인 창 문구(제목·줄·외 N개·건너뜀·위험 문단) ko/en, `update_target` 공유

## 2. shell

- [x] 2.1 `update_one` 추출(단일과 공유), `update_plugins`(busy·계획·확인 창·재확인·차례 실행·이벤트), `stop_plugin_updates`, 등록

## 3. webview

- [ ] 3.1 `lib/pluginBulk.ts`(고를 수 있음·모두 대상·15개 상한·요약·모듈 저장소) + 테스트
- [ ] 3.2 체크박스·머리 버튼·진행·멈추기·줄 상태·요약, api, mock(업데이트 여러 개·실패 하나), i18n ko/en, vitest

## 4. 검증·리뷰

- [x] 4.1 설계 리뷰(design-critic + security-privacy-reviewer) 반영 — design.md "설계 리뷰 반영"
- [ ] 4.2 전체 검사(npm ci, core test, clippy, tsc, vitest, lint), mock 스크린샷(라이트 1440·다크 900)
- [ ] 4.3 테스트 리뷰(qa-verifier + design-critic + brand-reviewer) 반영 — design.md "테스트 리뷰 반영"
