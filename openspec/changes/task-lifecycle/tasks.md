## 0. 게이트
- [x] 0.1 설계 리뷰 2개(design-critic + security-privacy-reviewer) 실행, BLOCKER·HIGH 해결, 결과·조치를 decisions.md 리뷰 기록과 D-152에 기록
- [x] 0.2 report-evidence가 아카이브(또는 요약 필드 머지)됐는지, delivery-guard `delivery_guard` 열·DB 번호 확인 후 이 change의 DB 번호를 design 4에 적기

## 1. core
- [x] 1.1 DB 다음 번호 `task_meta.wrapped_at`; 마이그레이션 테스트(이전 행 NULL)
- [x] 1.2 칸 판정 공용 함수(design 2: 끝남 = `running_of` None 또는 목록 없음, `pending` 재사용, 실패+pending → 리뷰 대기, 실패+비pending → 확인 필요, 리뷰함·미정리 → 정리, 리뷰 대상 아님 24시간 → 정리), `board_wrapup()`(리뷰 대기 40+합계, 정리 40+더 보기, 카드 요약만·경로 없음); 테스트(리뷰 대기 수 = `pending_count`, 실패·중지 보고서 작업이 리뷰 대기, 첫 실행 쏟아짐 없음, 중복 없음)
- [x] 1.3 `wrap_task`·`unwrap_task`(실행 중·리뷰 대기 거부, 외부 세션은 settings `wrapped_keys` 200), `unmark_reviewed`가 `wrapped_at`도 지움; 테스트
- [x] 1.4 git 안전 실행 도우미(`-c core.fsmonitor=false -c core.hooksPath=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`, 인자 배열, 시간 제한), `wrapup_check(bgId)`(D-141 경로 검사, 브랜치, 변경 수, 보고서 PR 정규식 첫 1개, delivery_guard 열); 테스트(fsmonitor 명령 미실행, 가짜 gh 미호출, 파일 이름 미전송)
- [x] 1.5 `remove_worktree(bgId)`: 실행 중·변경·추적 안 된 파일·링크·경로 밖 거부, `--ignored` `!!` 개수, 네이티브 확인 문구(폴더·브랜치·무시된 파일 N·"브랜치와 커밋은 남겨요"), `worktree remove` `--force` 없음 10초, 고정 오류 문구; 테스트(변경 있으면 거부, 브랜치 남음, 무시된 파일 수)

## 2. 셸
- [x] 2.1 트레이: 서명에 `pending_count` 추가(lib.rs `update_tray`), 툴팁 "확인 필요 N · 리뷰 대기 M"(0 생략), 확인 필요 수를 design 2 정의로; 끝남 알림(보고서 붙음 "오리가 끝냈어요 · 리뷰 대기: 이름", 그 밖 "오리가 끝냈어요: 이름"); 명령 등록; `cargo test -p dduck-core`·src-tauri `cargo check` 통과

## 3. UI
- [x] 3.1 api.ts 타입·명령, mock(리뷰 대기 done·failed·stopped, 정리 리뷰함, 리뷰 없이 끝남, worktree 남음/변경 있음/무시된 파일, PR 적힘, 로컬만, 살아 있는 세션 0)
- [x] 3.2 format.ts `Group`(`reviewWait`·`wrapup`, done 제거)·`GROUP_ORDER`·`GROUP_LABEL`·`COLUMN_HINT`, TasksPage 보드(살아 있는 세션 + `board_wrapup` bg id 병합, 칸 머리 서버 합계, 40개+"리뷰 탭에서 모두 보기"), 빈 보드 분기(TasksPage.tsx:465: 둘 다 0일 때만 빈 상자), 리뷰 대기 카드 → 보고서·"리뷰했어요", 정리 카드 펼침 점검표·"worktree 정리"·"정리 끝"·5초 되돌리기, 사이드바 작업 배지(`blockedCount`)를 design 2 확인 필요 정의로
- [x] 3.3 brief.ts `canFollowUp`을 끝남 판정으로(실패·리뷰 대기·정리 칸의 dev 작업), 테스트
- [x] 3.4 "완료" 문구 전수: format.ts `STATUS_LABEL.done` → "오리가 끝냈어요", TasksPage 카드(102행)·상세(304행) 상태 칸(design 2·8), SettingsPage "끝남·실패 알림"+설명, routines.ts `RUN_STATUS_LABEL.done` → "끝났어요", ReviewPage 정리 끝 "완료" 칩, TaskReport 상태 칩(리뷰 대기·리뷰했어요·완료), mock·테스트 문구; 바꾸지 않는 넷(완료 기준·StartChecklist "(완료)"·"방금 끝냈어요"·tour 문구) 유지; `rg '"완료"|완료·|: "완료' src src-tauri/src`가 넷만 남음
- [x] 3.5 vitest로 스펙 시나리오 전부, `npx tsc --noEmit -p .`·`npx vitest run` 통과

## 4. 통합 검증
- [ ] 4.1 mock 서버에서 라이트 1440·다크 900 스크린샷(보드 5칸 흐름·리뷰 대기 실패 카드·정리 점검표·worktree 확인 문구·정리 끝 되돌리기·살아 있는 세션 0 보드·트레이 툴팁 값), 전체 테스트 통과

## 5. 리뷰
- [ ] 5.1 테스트 리뷰(qa-verifier + design-critic + 브랜드, 엄격, 실제 스크린샷, D-145 근거 필수) HIGH·FAIL 0, 결과·조치를 decisions.md 리뷰 기록에 남김
- [ ] 5.2 테스트 리뷰(2026-11-10) 조치: critic HIGH·MAJOR 4·MINOR 6, QA FAIL 3, 브랜드 HIGH 1·MEDIUM 3·LOW 3 (decisions.md 리뷰 기록 2026-11-10 task-lifecycle 3행). 스펙 delta를 D-167·D-186에 맞추고 코드 수정, 재리뷰
