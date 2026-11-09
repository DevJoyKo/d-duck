## Why
작업 보드는 "완료"에서 끝난다. 그래서 오리가 일을 끝낸 순간이 작업이 끝난 순간처럼 보인다. 사용자(2026-11-09): "작업쪽에 완료만 있는데, 리뷰 하고 마지막 정리 탭이 있게 해줘", "리뷰를 해야 진짜 끝난거잖아."

`review-rework-evidence`를 셋으로 나눈 것 중 두 번째다(D-152). report-evidence 다음, review-rework 전에 만든다.

## What Changes
- 작업 보드 칸: 대기 중 → 진행 중 → 확인 필요 → **리뷰 대기** → **정리**. "완료" 칸은 없앤다.
- **리뷰 대기** = 리뷰 탭의 리뷰 전 작업과 같은 행이다. 칸 수 = 사이드바 배지 = 트레이 툴팁. 실패·중지로 끝난 보고서 작업도 여기에 온다.
- **정리** = 리뷰했고 아직 정리 끝을 누르지 않은 작업이다. 리뷰 대상이 아닌 끝난 세션은 24시간 "리뷰 없이 끝났어요"로 둔다.
- 정리 카드 점검표(로컬만, 네트워크 없음):
  - worktree·브랜치
  - 커밋 안 된 변경 수
  - 보고서에 적힌 PR 주소
  - 전달 범위(delivery-guard)
- 정리 동작:
  - "worktree 정리": 네이티브 확인, 무시된 파일 수 표시, 변경 있으면 거부, `--force` 없음
  - "정리 끝": 5초 되돌리기
- **"리뷰해야 완료" 문구**: 리뷰하지 않은 작업은 어디서도 "완료"가 아니다. "오리가 끝냈어요 · 리뷰 대기"로 쓴다. "완료"는 정리 끝 뒤에만 쓴다. 알림·트레이 툴팁·설정·루틴 실행 상태 문구가 바뀐다.
- 살아 있는 세션이 없어도 리뷰 대기·정리 칸을 보여 준다.
- DB `task_meta.wrapped_at`.

## Capabilities
### New Capabilities
- `task-lifecycle` (리뷰 대기·정리 칸, 리뷰해야 끝나는 작업 상태와 그 문구, 정리 점검표)
### Modified Capabilities
없음(관련 스펙이 아직 동기화되지 않음).

## Impact
- core:
  - `review_inbox` `pending`·`rows` 재사용
  - 새 `board_wrapup`·`wrap_task`·`unwrap_task`·`wrapup_check`·`remove_worktree`
  - git 실행 안전 옵션
  - DB 다음 번호(`wrapped_at`)
- 셸:
  - 트레이 서명·툴팁에 리뷰 대기 수
  - 끝남 알림 문구
- UI:
  - format.ts `Group`·라벨, TasksPage 보드·카드·상세·빈 보드, brief.ts `canFollowUp`
  - SettingsPage 알림 문구, routines.ts, ReviewPage "완료" 칩, TaskReport 상태 칩
  - mock
- 의존: report-evidence(카드의 "근거 N"·근거 없음 요약). delivery-guard(`delivery_guard` 열, 없으면 줄 생략).
