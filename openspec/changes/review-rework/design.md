## Context
- **다시 맡기기**: `Draft{cwd, taskType, brief, from, origin…}` → Composer → `start_task`. 보고서 없음의 "다시 맡기기"(D-122 (7)), "이 브리프로 다시 맡기기", `followUpDraft`가 있다. 원래 작업과의 연결은 저장하지 않는다(`from`은 배지 글뿐).
- **재개**: usage-limit-resume `ResumeSpec::from_prepared`·`args`, 고정 `RESUME_PROMPT`. 재개 cwd 검사(D-141 (2)), 범위 인자 `guard_args`(D-143 (1)).
- **선택형 UI**: BriefRefiner `OptionLine`, 대화상자 규칙 D-122 (1).
- **앞선 두 change**: report-evidence(근거 없음·`isEvidenceStrictType`), task-lifecycle(리뷰 대기·정리 칸, `board_wrapup`, `wrapped_at`).
- **model-select**(다른 change, 진행 중): 작업마다 모델을 고른다. 재작업·재개가 모델을 바꾸면 비교가 흐려진다.

## Goals / Non-Goals
- Goals: 고르기만으로 재작업, 원래 작업과의 최소 연결, 같은 실행 조건(모델·유형·모드·에이전트·규칙).
- Non-Goals: 자동 실행·자동 반복, LLM 피드백 작성, `review_of` 연결 자체, 루틴 재작업.

## Decisions

### 1. 연결: `follows` + `follows_kind`
- DB 다음 번호(task-lifecycle 다음)에 `task_meta` nullable 열 3개를 더한다.
  - `follows TEXT`(`valid_bg_id`)
  - `follows_kind TEXT CHECK(follows_kind IN ('rework'))`
  - `rework_json TEXT`
- 대안 `rework_of` 단독 열은 `review_of` 때 열이 또 생겨 기각했다.
- 원래 작업이 prune되면 "원래 작업 기록이 지워졌어요"를 보인다(FK 없음).

### 2. 요청 데이터(웹뷰 → Rust, 구조만)
```
ReworkRequest { of: bgId,
  criteria: [string],              // 원래 브리프 완료 기준 또는 원래 보고서 criteria 문구, 최대 10개
  reasons: ["evidence"|"unmet"|"outOfScope"|"deviates"|"other"],  // 1~5개, 중복 없음
  note: string,                    // 0~120자, 한 줄, display_safe
  mode: "resume" | "new" }
```
- Rust가 다시 확인한다.
  - `of`가 보고서가 붙은 끝난 작업인지
  - 기준 문구가 원래 브리프·보고서에 정확히 있는지
  - 이유가 enum인지, "기타"만이면 note가 필수인지
- `rework::compose`가 피드백 글을 만든다.
  - 머리 `## 재작업 요청 (이전 작업 리뷰)`, 그 아래 고정 이유 문구, 기준 목록, 한 줄
  - 에이전트가 쓴 근거·이유 글은 넣지 않는다.
- "작업 지시 저장"(D-069)이 꺼져 있으면 `rework_json`에 `{reasons, mode}`만 남긴다.

### 3. 다시 하는 방법
- **실행 조건 이월**: 두 방법 모두 원래 작업의 유형·권한 모드·에이전트·effort와 **모델**(model-select D-154: 넘긴 `model_arg`·`role_model`을 `ModelPin`으로 재현, 없으면 생략)을 쓴다.
  - Composer에서 사용자가 바꿀 수 있다. 바꾸면 재작업 블록에 "모델을 바꿨어요" 한 줄이 남는다.
  - (model-select 교차 항목, D-154) 열은 `task_meta.model_layer`·`model_arg`·`role_model`이다. "같은 세션에서 이어서"는 `ModelPin`으로 시작 인자를 그대로 재현한다(층 재계산 없음, 파일 에이전트 잠금은 `layer = task`일 때만). "새 작업으로"는 `layer = task`일 때만 Composer 모델 칸을 `model_arg`로 채운다. 다른 층이면 기본값으로 두고 "원래 작업은 X로 돌았어요"를 보인다. 그래야 Composer가 잠기지 않는다.
  - 이어서 하기는 바꿀 수 없다(같은 세션).
- **새 작업으로**(항상 가능):
  - `rework_preview(of)` → Draft 재료를 받는다. 브리프, 실행 조건, cwd = 원래 세션의 실제 cwd(worktree면 그 worktree, worktree 끔)다.
  - worktree가 없어졌으면 프로젝트 폴더로 채우고 "원래 worktree가 없어 프로젝트 폴더에서 시작해요"를 보인다.
  - Composer에 읽기 전용 재작업 블록과 "'이름'의 재작업" 배지가 보인다.
  - 사용자가 시작을 누르면 `start_task`가 `rework` 필드를 검증하고 피드백을 덧붙인 뒤 `follows`를 기록한다.
  - delivery-guard 규칙은 평소 시작 경로 그대로다.
- **같은 세션에서 이어서**(조건부):
  - `ResumeSpec.prompt`(기본 `RESUME_PROMPT`)에 피드백 글을 넣는다.
  - 인자는 `TaskSpec` → `prepare_with` → 새 보고서 attach → `from_prepared` → `args`(`guard_args`·모델 포함)다.
  - 조건: `session_id` 있음, 끝남, 원격·루틴 아님, D-141 (2) cwd 검사 통과, 위험 모드는 DangerApproval.
  - 새 bg id·새 `report_id`·`follows`. usage-limit-resume 별칭 행(`origin_bg_id`)과 다르다.
  - 의존 change가 없으면 `resume.available=false, reason="notYet"`이고 선택지를 그리지 않는다.
- **기본 선택**: 이어서 하기가 가능하고 이유가 "근거 부족"뿐이면 이어서, 아니면 새 작업이다.
- 자동 실행은 없다. 이어서 하기는 시트의 "이어서 시작"이 시작 동작이고, 누르기 전에 "이렇게 전해요"를 보인다.

### 4. 표시
- `review_inbox`·`board_wrapup` 항목에 `reworkedBy: {bgId, running} | null`, `follows: {bgId, goal} | null`을 더한다(DB만).
- **pending 정의 변경**: 재작업 자식이 있는 작업은 리뷰 전·배지·리뷰 대기에서 빠진다. task-lifecycle 칸 판정에 "재작업 부모 → 정리"를 더한다. 상태 칸 "재작업 → 새 작업"이고, 누르면 새 작업 카드로 간다.
- 새 작업 카드: "재작업" 칩, "← 원래 작업".
- 새 보고서 "맡긴 내용" 아래 재작업 블록(앱 기록): 이유 칩, 기준, 한 줄, 방법, 모델을 바꿨으면 그 줄, "원래 작업 보고서 보기".

### 5. 시트 UI(`ReworkSheet`)
- 진입: 리뷰 카드("리뷰했어요" 왼쪽, 일반 버튼), 보고서 footer, 보드 리뷰 대기 카드의 보고서.
- 실행 중·보고서 없음이면 버튼이 없다. 형식 오류면 "새 작업으로"만 고를 수 있고 기준 목록이 없다.
- 순서(한 줄씩):
  1. 기준 체크(`OptionLine`, 미리 체크)
  2. 이유 칩(미리 선택: 근거 없음 → 근거 부족, 못 했어요 → 기준 미충족, 합의와 다름 결정 → 합의와 다름)
  3. 한 줄(120자)
  4. 방법 라디오(추천·비활성 이유)
  5. "이렇게 전해요"(접힘)
  6. 주 버튼("새 작업으로 열기"/"이어서 시작")·취소
- 노랑은 주 버튼 하나다. 포커스 규칙은 D-122 (1)이다.

## Risks / Trade-offs
- [이어서 하기가 원래 편향을 유지] → 근거 부족뿐일 때만 기본 추천.
- [재작업 자식이 있으면 원래 작업을 리뷰하지 않은 채 정리 칸으로] → 의도. 재작업 결과를 리뷰한다.
- [model-select가 아직 없음] → 모델 열이 없으면 이월하지 않고, CLI 기본 모델이 그대로다.
- [의존 change 진행 중] → 이어서 하기는 `available`로만 열린다.

## Migration Plan
- DB nullable 열 3개(마이그레이션 테스트). 되돌리기: 열이 남아도 무해.
