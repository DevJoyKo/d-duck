# Tasks

## 1. core

- [x] 1.1 `review_inbox::delete_plan`·`delete_refusal`·`delete`(그 bg의 모든 대상, 실행 중·별칭 실행 중·예약·루틴 시작 중 거절, 재확인+삭제 한 트랜잭션, 목록 세션 키 정리 끝, 폴더는 커밋 뒤 `remove_folder`만), `Inbox.known`, `task_meta` 폴더 정리 도우미 공유. 테스트: 보고서 있는 작업·없는 작업·루틴 실행 삭제, 루틴 실행 행 유지, 다른 기록이 가리키는 폴더 유지, 루트 밖·잘못된 id 안 지움, 배지·보드 수
- [x] 1.2 i18n 확인 문구(제목·본문·루틴·worktree 줄) ko/en

## 2. shell

- [x] 2.1 `inbox_delete`: 검사 → 네이티브 확인(confirm_lock) → 새 목록으로 재확인+삭제 → 작업 기록이면 scrub. `delete_task_record`도 `delete_refusal`

## 3. webview

- [x] 3.1 카드 "삭제"(끝난 카드만), api, 목록·배지·보드 다시 읽기, 포커스 다음 카드, 오류 줄, mock, vitest

## 4. 검증·리뷰

- [x] 4.1 설계 리뷰(design-critic + security-privacy-reviewer) 반영 — design.md "설계 리뷰 반영"
- [x] 4.2 전체 검사, mock 스크린샷(라이트 1440·다크 900) — core 589·vitest 577·clippy·tsc·lint
- [x] 4.3 테스트 리뷰(qa-verifier + design-critic + brand-reviewer) 반영 — design.md "테스트 리뷰 반영"
