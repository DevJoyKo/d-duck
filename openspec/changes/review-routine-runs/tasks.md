# Tasks

## 1. core

- [x] 1.1 DB v16: `routine_runs.reviewed_at`·`wrapped_at`·`routine_runs(bg_id)` 인덱스 (데이터 있는 v15 → v16 테스트)
- [x] 1.2 `review_inbox::RECORDS` 합친 행 원천, `rows`·`pending_count`가 사용, `Item.routine`. 회귀 테스트: 루틴 실행 보고서가 리뷰 탭·배지에 나온다, 같은 bg의 `task_meta`가 이긴다, 보고서 없는 루틴 실행은 안 나온다, 기준 시각 전 루틴은 전체에만
- [x] 1.3 `mark`·`unmark`가 루틴 실행에도 동작(실행 중·보고서 없음 거절 그대로)
- [x] 1.4 `task_lifecycle::facts`·`lanes`·`board`가 같은 원천, `wrap`·`finish_stopped`·`unwrap`이 루틴 실행에도 동작, 정리 끝 기록은 리뷰 전 아님, 세션 키 정리 끝 → `wrapped_at` 옮김. 테스트: 배지 = 리뷰 대기 칸 수(루틴 포함), 리뷰 후 정리 칸, 정리 끝, 예전 정리 끝 루틴, 알림 판단(`lanes` ReviewWait)
- [x] 1.5 `MAX_OFFSET` = `MAX_ROWS`

## 2. webview

- [x] 2.1 `InboxItem.routine`, 카드 폴더·시각 줄 앞 "루틴", `MAX_PAGES` 50, mock에 루틴 실행 카드, vitest

## 2b. 보고서 없는 작업 (D-196)

- [x] 2b.1 core: `rows`가 보고서 없는 `task_meta` 행도 읽음, `build`가 전체에만 합침(리뷰 전·배지·has_any 그대로), 실행 중·목록 모름 제외, 루틴과 중복 제거, `CardReport::None`·`ended`·`noReport`. 테스트: 전체에만 나옴·배지 그대로·표시 거절·지시 원문 없음·목록 모름 제외
- [x] 2b.2 webview: "보고서 없음" 칩·끝난 상태 줄·"작업 보기"(보드 서랍 또는 지난 작업 행), 리뷰했어요·보고서 열기 없음, 빈 화면·보고서 꺼짐 조건, mock, vitest
- [ ] 2b.3 검증(전체 검사·스크린샷), 테스트 리뷰(qa-verifier + design-critic) 반영

## 3. 검증·리뷰

- [x] 3.1 설계 리뷰(design-critic + security-privacy-reviewer) 반영 — HIGH 2·MEDIUM 4·LOW 7 문서 반영
- [x] 3.2 `cargo test -p dduck-core`·clippy·tsc·vitest·lint, mock 스크린샷(라이트 1440·다크 900) — core 581·vitest 563·clippy·tsc·lint 통과
- [x] 3.3 테스트 리뷰(qa-verifier + design-critic) 반영 — FAIL 1(되돌리기 뒤 키로 재정리) → 키를 옮김+회귀 테스트, MEDIUM 시나리오 테스트 3개 추가, LOW lane_of 키 무시 복원·문서 정정
