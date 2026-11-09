## Context
- **보드**: `format.ts groupOf`
  - queued = idle
  - running = working
  - review(확인 필요) = waitingInput·failed
  - done = done·stopped(24시간)
  - 칸 순서 `GROUP_ORDER`, 이름 `GROUP_LABEL`, 설명 `COLUMN_HINT`(TasksPage.tsx:28)
  - 보드는 `claude agents` 목록의 살아 있는 세션만 그린다. 세션이 0개면 보드 대신 빈 상자를 그린다(TasksPage.tsx:465).
- **리뷰 전**(review_inbox.rs `pending`): 보고서 붙음, `reviewed_at` 없음, `created_at ≥ review_since`, 끝남.
  - 끝남 = `running_of`가 None(done·failed·stopped·unknown) 또는 목록에 없음(review_inbox.rs:62-66).
  - 그래서 **실패·중지로 끝난 보고서 작업도 리뷰 전**이다. 지금 보드의 실패는 확인 필요 칸에 있어 수가 어긋난다(design-critic HIGH).
- **트레이**(lib.rs:431-460): 서명은 답 기다림 목록 + 실패 수이고, 같으면 갱신하지 않는다. 툴팁은 "확인 필요 N"이다.
- **후속 버튼**: `brief.ts canFollowUp`은 `groupOf`가 review·done일 때만 QA·리뷰 맡기기를 보인다.
- **문구**:
  - format.ts `STATUS_LABEL.done` "완료", `GROUP_LABEL.done` "완료"
  - agents.ts 포즈 "방금 끝냈어요"
  - routines.ts `RUN_STATUS_LABEL.done` "완료"
  - SettingsPage "완료·실패 알림"
  - lib.rs:369 알림 "작업이 끝났어요: 이름"
  - StartChecklist "(완료)"
- **git 실행**: delivery_guard.rs:390-392 등 기존 안전 git 실행은 인자 배열과 시간 제한이 있다. 하지만 저장소 설정의 `core.fsmonitor`(임의 명령 실행)를 끄지 않는다(security MEDIUM).

## Goals / Non-Goals
- Goals:
  - 리뷰해야 끝나는 흐름을 만든다.
  - 리뷰 대기 수 = 배지 = 툴팁.
  - 로컬만 쓰는 정리 점검표를 둔다.
- Non-Goals:
  - 브랜치 삭제·PR 조회(gh 네트워크), 일괄 정리.
  - 재작업(review-rework가 정리 칸에 "재작업 → 새 작업"을 더함).

## Decisions

### 1. 원칙과 이름
- 오리가 끝낸 것("오리가 끝냈어요")과 작업이 끝난 것("완료")을 나눈다.
- "완료"는 리뷰 + 정리 끝 뒤에만 쓴다.
- 마지막 칸 이름은 **"정리"**다. "완료 · 정리"는 정리 끝 전 카드에 "완료"를 붙이게 되어 원칙과 어긋난다.
- **바꾸지 않는 목록**(스펙과 같은 넷):
  - "완료 기준"(브리프 항목)
  - StartChecklist "(완료)"(온보딩 체크 항목)
  - agents.ts 포즈 문구 "방금 끝냈어요"
  - tour.ts 문구

### 2. 칸 판정(위에서부터 처음 맞는 것)
1. 끝나지 않음(`running_of`가 Some): 기존대로.
   - idle → 대기 중
   - working → 진행 중
   - waitingInput → 확인 필요
2. 끝남 + `pending`(리뷰 전과 같은 함수) → **리뷰 대기**. 실패·중지도 여기에 온다. 상태 칸은 각각 다음과 같다.
   - done: "오리가 끝냈어요 · 리뷰 대기"
   - failed: "실패했어요 · 리뷰 대기"
   - stopped: "멈췄어요 · 리뷰 대기"
3. 끝남 + 실패 + `pending` 아님(보고서 없음, 기준 시각 전, 외부 세션) → 확인 필요(지금처럼, 사람이 봐야 함).
4. 끝남 + D.Duck 기록 있음 + `reviewed_at` 있음 + `wrapped_at` 없음 → **정리**("리뷰했어요 · 정리 남음").
5. 끝남 + 리뷰 대상 아님(기준 시각 전·보고서 없음·외부 세션, done·stopped·unknown) → **정리**에 24시간("리뷰 없이 끝났어요"). 첫 실행 때 쏟아지지 않는다.
6. 정리 끝 → 보드에 없음. 리뷰 탭 전체·지난 작업에 "완료".
- 칸 하나에 한 번만: 살아 있는 세션과 `board_wrapup` 행을 bg id로 합친다.
- **수 일치**: 리뷰 대기 칸 수 = `pending_count`. 같은 함수를 쓰고, 칸 머리 수는 서버 합계(40개 넘는 것 포함)다. 테스트로 고정한다.

### 3. 데이터: `board_wrapup()`
- DB 행을 돌려준다: 리뷰 대기(review_inbox `rows` + `pending`, 40 + 합계)와 정리(4·5, 40 + 더 보기).
- 카드 요약만 담는다(review-inbox 5 범위, report-evidence 요약 필드 포함, 경로 없음).
- 쿼리는 주기 재조회 없이 다음 때 다시 읽는다: 에이전트 폴링의 `reviewPending` 변화, 보드 진입, 표시·정리 동작.
- **빈 보드**: 살아 있는 세션이 0개여도 `board_wrapup`에 행이 있으면 칸을 그린다. 둘 다 0일 때만 기존 빈 상자다(TasksPage.tsx:465 분기 수정).

### 4. 정리 상태와 되돌리기
- 같은 DB 마이그레이션(usage-limit-resume v10·delivery-guard 다음 번호)에 `task_meta.wrapped_at INTEGER`를 더한다.
- `wrap_task(bgId)`: 끝남 + (리뷰함 또는 리뷰 대상 아님)일 때만 된다. 실행 중이거나 리뷰 대기면 거부한다.
  - D.Duck 기록이 없는 외부 세션은 settings `wrapped_keys`(세션 key, 최근 200개, 일반 setter 밖)에 남긴다.
- `unwrap_task`: 5초 "정리 끝 · 되돌리기".
- `unmark_reviewed`는 `wrapped_at`도 지운다. 그래서 리뷰하지 않은 "완료"가 생기지 않는다.

### 5. 정리 점검표(`wrapup_check(bgId)`, 펼칠 때만, 네트워크 없음)
- **git 실행 안전 옵션**: 이 change의 모든 git 호출에 적용한다.
  - 인자: `-c core.fsmonitor=false -c core.hooksPath=/dev/null`
  - 환경: `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`
  - 인자 배열로 넘기고 시간 제한을 둔다(2초, remove는 10초).
  - delivery_guard.rs:390-392 적용은 delivery-guard 2단계가 맡는다(coordinator 조율).
- **worktree·브랜치**: `task_meta.worktree`이고 세션 cwd가 `<저장소>/.claude/worktrees/` 아래(D-141 (2) 검사)일 때만 본다.
  - 있으면 "worktree가 남아 있어요 · <폴더 이름> · <브랜치>"
  - 없으면 "worktree는 정리됐어요"
  - 브랜치는 `rev-parse --abbrev-ref HEAD`로 읽는다.
- **커밋 안 된 변경**: `status --porcelain=v1 -uall`의 줄 수만 센다. "커밋 안 된 변경 없어요" / "커밋 안 된 변경 N개". 파일 이름은 보내지 않는다.
- **PR**: `gh`를 부르지 않는다.
  - 보고서 `references`의 url 중 `^https://github\.com/[\w.-]+/[\w.-]+/pull/\d+$` 첫 1개를 "보고서에 적힌 PR · #N"으로 보인다.
  - 누르면 기존 레퍼런스 규칙(확인 후 브라우저)을 따른다. 없으면 이 줄을 뺀다.
- **전달 범위**: `task_meta.delivery_guard`가 `local_only`면 자물쇠 "로컬만", `draft_pr`면 "PR 초안까지". 열이 없으면 뺀다.

### 6. worktree 정리(`remove_worktree(bgId)`)
- **거부**: 실행 중이거나, 경로 검사 실패(링크, `.claude/worktrees/` 밖), 커밋 안 된 변경·추적 안 된 파일이 1개 이상.
- **무시된 파일**: `status --porcelain=v1 --ignored -uall`의 `!!` 줄 수를 센다. git은 무시된 파일을 worktree와 함께 지운다.
- **네이티브 확인 창**: "worktree를 지울까요?\n폴더: <폴더 이름>\n브랜치: <브랜치>\n무시된 파일 N개(빌드 결과 등)도 함께 지워져요\n브랜치와 커밋은 남겨요". N = 0이면 무시된 파일 줄을 뺀다.
- **실행**: `git -C <저장소> worktree remove <경로>`(`--force` 없음, 안전 옵션, 10초). 실패하면 고정 문구 "worktree를 지우지 못했어요"를 보인다.
- 브랜치는 지우지 않는다.

### 7. 트레이·알림·후속
- **트레이 서명**: 리뷰 대기 수(`pending_count`)를 서명에 더한다. 수만 바뀌어도 툴팁이 갱신된다.
  - 툴팁: "D.Duck — 확인 필요 N · 리뷰 대기 M"(0인 쪽은 생략, 둘 다 0이면 "D.Duck")
  - 메뉴 막대 숫자는 확인 필요 그대로다.
  - 확인 필요 수도 2와 같은 정의를 쓴다(실패 + pending은 리뷰 대기로 감, 사이드바 작업 배지와 일치).
- **끝남 알림**:
  - 보고서 붙은 작업: "오리가 끝냈어요 · 리뷰 대기: 이름"
  - 그 밖: "오리가 끝냈어요: 이름"
  - 실패 알림은 그대로다.
- **`canFollowUp`**(brief.ts:121): QA·리뷰 맡기기는 dev 작업이 끝났을 때 보인다. 칸으로는 확인 필요(실패)·리뷰 대기·정리다. `groupOf` 대신 끝남 판정(`isEnded`)과 실패 여부로 다시 쓴다.

### 8. 문구 전수(tasks 4.x)
- format.ts
  - `STATUS_LABEL.done` → "오리가 끝냈어요"
  - `Group`에 `reviewWait`·`wrapup`, done 칸을 없앤다.
  - `GROUP_LABEL`: 리뷰 대기 "리뷰 대기", 정리 "정리"
  - `COLUMN_HINT`: "오리가 끝냈고 리뷰를 기다려요", "리뷰한 작업의 마무리를 확인해요"
- TasksPage 카드(102행)·상세(304행): 2의 상태 칸, 정리 끝이면 "완료"
- SettingsPage: "완료·실패 알림" → "끝남·실패 알림", 설명 "오리가 작업을 끝내거나 실패하면 macOS 알림으로 알려 줘요"
- routines.ts `RUN_STATUS_LABEL.done` → "끝났어요"
- ReviewPage "✓ 리뷰 끝" → 정리 끝이면 "완료"
- TaskReport 머리 상태 칩: "리뷰 대기"/"리뷰했어요"/"완료"
- lib.rs 알림·툴팁(7)
- mock·테스트 문구
- **확인**: `rg '"완료"|완료·|: "완료' src src-tauri/src`가 1의 바꾸지 않는 목록만 남긴다.

## Risks / Trade-offs
- [보고서 없는 끝난 작업이 "리뷰 없이 끝났어요"로 정리 칸에 24시간] → 수 일치를 위한 선택. 정리 끝으로 치울 수 있다.
- [실패한 보고서 작업이 확인 필요가 아니라 리뷰 대기로 감] → 리뷰 탭과 같은 정의. 상태 칸 "실패했어요 · 리뷰 대기"와 빨강 상태로 눈에 띈다.
- [worktree 정리는 되돌릴 수 없음] → 변경 있으면 거부, 무시된 파일 수 고지, `--force` 없음, 브랜치 유지, 네이티브 확인.
- [보고서에 적힌 PR은 에이전트 값] → "적힌"이라고 쓰고, 확인 후 브라우저로 연다.

## Migration Plan
- DB: `wrapped_at` nullable 열(마이그레이션 테스트).
- 기존 "완료" 칸 작업은 2의 규칙대로 리뷰 대기·정리로 나뉜다. 기준 시각 전 작업은 리뷰 대기에 오지 않는다.
