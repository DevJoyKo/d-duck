## 0. 게이트
- [x] 0.1 설계 리뷰 2개(design-critic + security-privacy-reviewer) 실행, BLOCKER·HIGH 해결, 결과·조치를 decisions.md 리뷰 기록과 D-152에 기록
- [ ] 0.2 의존 확인: report-evidence·task-lifecycle 아카이브, usage-limit-resume(`session_id`·`ResumeSpec`·cwd 검사)·delivery-guard(`guard_args`)·model-select(모델 열) 상태; 이 change의 DB 번호를 design 1에 적기, 이어서 하기는 의존이 없으면 `notYet`

## 1. core
- [ ] 1.1 DB 다음 번호 `task_meta.follows`·`follows_kind`(CHECK rework)·`rework_json`; 마이그레이션 테스트, `valid_bg_id`·JSON 상한
- [ ] 1.2 `rework` 모듈: `ReworkRequest` 검증(of 끝남·보고서 붙음, 기준 문구 정확 일치, 이유 enum 1~5, 기타만이면 한 줄 필수, 120자·한 줄·display_safe), `compose`(고정 이유 문구, 에이전트 글 미포함), "작업 지시 저장" 꺼짐이면 이유·방법만; 테스트(조작된 기준 거부, 미리보기 = 보낼 글)
- [ ] 1.3 `rework_preview(of)`: 브리프·유형·모드·에이전트·effort·모델(열이 있으면)·실제 세션 cwd(worktree 없으면 프로젝트 폴더+안내), 피드백 미리보기, `resume: {available, reason}`; `start_task`의 `rework` 필드 검증·피드백 덧붙임·`follows` 기록·모델 변경 표시
- [ ] 1.4 `ResumeSpec.prompt`(기본 `RESUME_PROMPT`, usage-limit-resume와 공유), `rework_resume(req)`: TaskSpec → prepare_with → 새 보고서 attach → from_prepared → args(`guard_args`·모델), 위험 모드 DangerApproval, 새 task_meta 행; 테스트(인자가 시작과 같음, 모델 유지, `--worktree`·`--remote-control` 없음, guard 오류면 거부)
- [ ] 1.5 `review_inbox`·`board_wrapup`: `reworkedBy`·`follows`, `pending`에서 재작업 자식 있는 작업 제외, task-lifecycle 칸 판정에 "재작업 부모 → 정리"; 테스트(배지 = 리뷰 대기 수 유지); `cargo test -p dduck-core`·src-tauri `cargo check` 통과

- [ ] 1.6 (model-select 교차 항목, D-154) 재작업 모델 이월을 `ModelPin` 재현(이어서)과 `layer = task`일 때만 채우는 초안(새 작업으로)으로 맞춘다. model-select 2.7·2.8 뒤에 한다. (확인: 에이전트 층 원래 작업의 새 작업 초안이 잠기지 않음, 이어서 인자가 시작 때 `--model`·역할 `model`과 같음 테스트)
## 2. UI
- [ ] 2.1 api.ts 타입·명령, mock(재작업 부모/자식·이어서 가능/불가/notYet·원래 기록 지워짐·형식 오류 보고서·모델 바꿈)
- [ ] 2.2 `ReworkSheet`(OptionLine 재사용): 기준 체크·미리 체크, 이유 칩·미리 선택, 한 줄 120자, 방법 라디오·추천·비활성 이유, "이렇게 전해요", 주 버튼(노랑 하나)·취소, 포커스(D-122 (1)); vitest(미리 채움, 기타만, 이어서 불가, 근거만 부족 → 이어서 추천)
- [ ] 2.3 Composer 재작업 초안(읽기 전용 블록, "'이름'의 재작업" 배지, worktree 안내, 모델 유지·변경 표시), 취소 시 기록 없음; TaskReport footer "재작업 요청"·맡긴 내용 아래 재작업 블록·"원래 작업 보고서 보기"
- [ ] 2.4 ReviewPage·TasksPage 카드: "재작업 요청" 버튼, "재작업 → 새 작업"(새 카드로 이동·강조), "재작업" 칩·"← 원래 작업", "원래 작업 기록이 지워졌어요"; vitest로 스펙 시나리오, `npx tsc --noEmit -p .`·`npx vitest run` 통과

## 3. 통합 검증
- [ ] 3.1 mock 서버에서 라이트 1440·다크 900 스크린샷(시트·Composer 재작업 초안·재작업 블록·정리 칸 재작업 부모·리뷰 카드), 전체 테스트 통과; 실제 재작업 새 작업 1회(사용자 확인 후)

## 4. 리뷰
- [ ] 4.1 테스트 리뷰(qa-verifier + design-critic + 브랜드, 엄격, 실제 스크린샷, D-145 근거 필수) HIGH·FAIL 0, 결과·조치를 decisions.md 리뷰 기록에 남김
