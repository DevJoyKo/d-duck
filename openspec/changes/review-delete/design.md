# Design: review-delete

## 결정 1. 명령 하나, 확인은 셸에서

`inbox_delete(id) -> bool`(false = 취소). 웹뷰는 bg id만 보낸다. 셸이 순서대로:
1. `valid_bg_id`, 마지막 목록이 정상인지(`poll_known` + 오류 없음), 실행 중 아님(`running_map`).
2. core `review_inbox::delete_plan(conn, id)`: 지울 대상(작업 기록 / 루틴 실행 행 id)과 확인 창 재료(이름, worktree 경고 여부, 루틴 여부)를 DB에서 읽는다. 대상이 없으면 거절.
3. `confirm_lock`을 잡고 네이티브 경고 창("삭제"/"취소"). 취소면 false.
4. 창이 열린 동안 상태가 바뀌었을 수 있으니(작업이 다시 시작됨, 다른 곳에서 지움) 새 목록으로 실행 중을 다시 보고, `delete_plan`을 다시 읽어 같은 대상일 때만 `review_inbox::delete`를 부른다(D-191 `still_removable`과 같은 방식).
5. 작업 기록이면 지시 흔적을 `ingest::scrub`.

## 결정 2. 무엇이 지워지나

- 작업 기록(`task_meta`, 보고서 유무 무관): `task_meta::delete_record_and_report`. 기록·지시·리뷰/정리 상태와 그 보고서 폴더(다른 기록이나 루틴 실행이 같은 id를 가리키면 남김). 지난 작업에서도 사라진다. 사용 한도 이어서 하기가 예약된 작업(`task_lifecycle::resume_pending`)은 거절한다: 기록이 없으면 이어서 하기의 재확인이 근거를 잃는다.
- 루틴 실행: `UPDATE routine_runs SET report_id = NULL, reviewed_at = NULL, wrapped_at = NULL WHERE id = ?`(그 bg의 보고서 있는 최신 행, `review_inbox::owner`) 뒤 보고서 폴더를 같은 "아무도 안 가리키면" 규칙으로 지운다. 실행 이력 행은 남겨 루틴 탭 기록·다음 실행 계산이 그대로다. 그 행은 보고서가 없어져 `RECORDS`에서 빠진다.
- Claude 세션·transcript, git worktree·브랜치, 다른 프로젝트 파일은 건드리지 않는다.
- 폴더 삭제는 기존 `task_report::remove_folder`만 쓴다(보고서 루트 안, `valid_id`, 링크 해석 뒤 루트 아래인지 확인).

## 결정 3. worktree가 남은 작업

기록을 지우면 정리 점검표(`task_lifecycle::record`)가 그 worktree를 찾지 못한다. 막지는 않는다(worktree가 이미 없을 수도 있고 지우는 판단은 사용자 몫). 대신 `task_meta.worktree = 1`이고 정리 끝이 아니면 확인 창에 "worktree가 남아 있으면 앱에서 더 정리할 수 없어요. 먼저 작업 탭 정리에서 지우세요" 줄을 더한다.

## 결정 4. 버튼과 흐름

- 카드 오른쪽 버튼 줄 맨 끝에 "삭제"(작은 텍스트 버튼, 위험 색 글자, 노랑·주 버튼 아님). 끝난 카드에만(`running === null`, 실행 여부 확인 중이면 없음).
- 보고서 대화상자에는 넣지 않는다(카드에서만).
- 성공하면 리뷰 목록·에이전트(배지)·보드 정리 칸을 다시 읽고, 포커스는 지운 카드 다음 카드의 열기 버튼(없으면 이전, 없으면 목록 머리). 쪽의 마지막 카드였으면 앞 쪽으로.
- 취소(false)는 아무것도 하지 않는다. 거절 오류는 기존 오류 줄(`role=alert`)에 보인다.
- 되돌리기 없음: 폴더를 실제로 지우므로 되돌릴 수 없다는 점을 확인 창에 쓴다.
- 여러 개 한꺼번에 지우기는 범위 밖(후속).

## 보안·프라이버시

- 입력은 bg id 하나. 대상·경로는 DB에서만 정한다.
- 확인 창 이름은 `display_safe` 64자, 경로는 쓰지 않는다.
- transcript·자격증명을 읽지 않는다. 지시 원문은 지울 때만 다루고(삭제), scrub로 WAL에도 남기지 않는다.

## 설계 리뷰 반영 (design-critic HIGH 2·MEDIUM 4·LOW 2, security MEDIUM 3·LOW 3)

- **지운 뒤 보드에 다시 나타남(HIGH)**: 기록이 없는 끝난 세션은 "리뷰 없음"(실패면 확인 필요)이 된다. 그래서 CLI 목록에 아직 있는 세션이면 삭제와 함께 그 세션 키를 `WRAPPED_KEYS`에 넣어 정리 끝(보드에서 빠짐)으로 보낸다. 목록에 없으면 넣을 키가 없고 보드에도 나오지 않는다.
- **루틴 실행 시작 중(HIGH, security MEDIUM)**: `delete_plan`과 재확인 모두에서 `routine_live`(starting·running·waiting)면 거절한다(`mark`와 같은 규칙).
- **실행 여부를 모를 때(MEDIUM)**: `Inbox.known`을 내보내고, 버튼은 `known && running === null`일 때만 보인다. 셸도 `known`이 아니면 거절한다.
- **같은 bg의 대상 여러 개(security MEDIUM)**: 계획은 그 bg의 `task_meta` 행(보고서 유무 무관)과 보고서 있는 모든 루틴 실행을 함께 담고 모두 지운다. 그래서 카드가 다시 나타나지 않는다.
- **재확인과 삭제의 경쟁(LOW)**: 재확인(계획을 다시 읽어 `(작업 행 여부, 루틴 행 id 목록, 보고서 id 목록)` 전체 비교, 실행 중·예약 재검사)과 DELETE/UPDATE·세션 키 추가를 `BEGIN IMMEDIATE` 트랜잭션 하나로 묶는다. 폴더 삭제는 커밋 뒤에 아무도 안 가리키는 id만 한다.
- **이어서 하기 별칭(LOW)**: 실행 중 검사는 그 bg와, `origin_bg_id`로 이어진 별칭 bg들(같은 원래 작업) 모두에 한다. 이어서 하기가 예약돼 있으면(`resume_pending`) 거절한다.
- **기존 지난 작업 삭제 우회(security MEDIUM, critic LOW)**: `delete_task_record`도 같은 core 검사 `review_inbox::delete_refusal`(실행 중·별칭 실행 중·예약)을 거친다. 확인은 지금처럼 웹뷰 두 단계 버튼이다(네이티브 창은 리뷰 탭 삭제만, 지난 작업 UI는 바꾸지 않음).
- **worktree 경고 조건(MEDIUM)**: 정리 끝이어도 worktree가 남을 수 있으므로 `task_meta.worktree = 1`이면 항상 경고 줄을 넣는다. 막지는 않는다.
- **scrub(LOW)**: 작업 기록을 지웠을 때만 `ingest::scrub`.
- **review-rework와의 관계(MEDIUM)**: 이 브랜치에는 `follows` 연결이 아직 없다. 삭제는 다른 기록을 건드리지 않는다. review-rework가 들어오면 원래 작업이 없어진 재작업을 다룰 수 있어야 한다(그 change의 몫, 결정 로그에 남김). 버튼 순서는 열기 → 리뷰했어요(→ 재작업 요청) → 삭제(맨 끝).
- **선행 change(LOW)**: 이 change는 `review-routine-runs`(PR #5, 아직 아카이브 전)의 루틴·보고서 없는 카드에 기댄다. 그 change를 먼저 아카이브한다.

## 테스트 리뷰 반영 (qa-verifier PASS·MEDIUM 1, design-critic MEDIUM 3·LOW 5, brand PASS·MAJOR 3)

- 버튼 이름은 앱의 다른 지우기(지난 작업, 확인 창 버튼 `BtnDelete`)와 같은 "지우기"(브랜드 MAJOR). 평소엔 `text-ink-3`, 가리키거나 포커스일 때만 위험 색, 주 버튼과 `ml-2` 간격(브랜드 MAJOR·MINOR). 경고는 확인 창이 맡는다.
- 확인 창 제목은 "작업 기록 지우기"/"루틴 보고서 지우기"로 나눈다. 본문은 한 줄에 한 항목: 지우는 것 → "되돌릴 수 없어요" → 남는 것 → worktree 안내("…worktree를 지워 주세요"). 작업 기록과 루틴 보고서가 함께 지워지면 두 줄을 모두 쓴다.
- 목록을 아직 못 읽었거나 새 목록을 못 받으면 "작업 상태를 확인하지 못해 지울 수 없어요"(`DeleteUnknown`), 실행 중 문구와 나눈다(QA·critic MEDIUM).
- 지난 작업 삭제도 CLI 목록에 남은 세션을 정리 끝으로 보낸다(critic·QA LOW).
- 테스트: 포커스가 다음 카드의 열기 버튼으로 감, 거절 시 오류 줄, 섞인 대상.
- 남긴 것: 확인 창 기본 버튼은 기존 "지난 작업 모두 지우기"와 같이 "지우기"(일관성, 후속 검토). worktree 안내는 실제 폴더가 남았는지 보지 않고 worktree로 시작했으면 보인다(설계 리뷰 결정). 정리 끝 키 200개 상한은 기존 규칙. "지난 작업 모두 지우기"의 거절 검사는 범위 밖.
