## Why
리뷰 탭(review-inbox)에서 결과가 마음에 들지 않을 때 할 수 있는 일이 "리뷰했어요"뿐이다. 사용자(2026-11-09): "리뷰 했을 때 마음에 안들면 재작업 요청을 할 수 있는 인터페이스 추가해주고".

`review-rework-evidence`를 셋으로 나눈 것 중 마지막이다(D-152). report-evidence(근거 없음 판정)와 task-lifecycle(리뷰 대기·정리 칸) 다음에 만든다.

## What Changes
- 리뷰 카드·보고서 화면·보드 리뷰 대기 카드의 "리뷰했어요" 옆에 "재작업 요청"을 둔다.
- 재작업 시트는 고르기만 한다.
  - 다시 볼 기준을 체크한다. 못 했어요·일부 충족·근거 없음·대조 안 함이 미리 체크된다.
  - 이유 칩: 근거 부족·기준 미충족·범위 밖 수정·합의와 다름·기타
  - 한 줄은 선택이고 120자까지다.
  - 다시 하는 방법은 "같은 세션에서 이어서" 또는 "새 작업으로"다.
- 재작업은 원래 작업에 이어진 **새 작업 기록**이다. DB `task_meta.follows`·`follows_kind`(`rework`)·`rework_json`. `review_of`는 다음 change가 `follows_kind`만 더해 쓴다.
- 원래 작업의 **모델·유형·권한 모드·에이전트를 그대로** 쓴다(model-select와 맞춤).
- 자동 실행은 없다. 새 작업이 시작되면 원래 작업은 리뷰 전·리뷰 대기에서 빠지고, 정리 칸에서 "재작업 → 새 작업"을 보인다. 새 작업 보고서의 "맡긴 내용" 아래에 재작업 요청 블록이 보인다.

## Capabilities
### New Capabilities
- `review-rework` (리뷰 결과에서 고르기만으로 재작업을 요청하고, 원래 작업과 이어진 새 작업으로 다시 시킨다)
### Modified Capabilities
없음(관련 스펙이 아직 동기화되지 않음).

## Impact
- core:
  - 새 모듈 `rework`(검증·compose)
  - `start_task`의 `rework` 필드
  - `ResumeSpec.prompt`
  - `review_inbox`·`board_wrapup`의 `reworkedBy`·`follows`·pending 제외
  - DB 다음 번호
- 셸: 명령 `rework_preview`·`rework_resume`
- UI:
  - `ReworkSheet`(BriefRefiner `OptionLine` 재사용)
  - Composer 재작업 초안
  - TaskReport 재작업 블록·footer
  - ReviewPage·TasksPage 카드
  - mock
- 의존: report-evidence, task-lifecycle, usage-limit-resume(`session_id`·`ResumeSpec`·cwd 검사), delivery-guard(`guard_args`), model-select(모델 열이 있으면 이어 씀). 이어서 하기는 usage-limit-resume·delivery-guard가 들어오기 전에는 숨긴다.
