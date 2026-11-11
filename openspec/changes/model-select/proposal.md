## Why
사용자 요청(2026-11-09, D-153): "에이전트별로 model 고를 수 있게 해줘. 그리고 static 리스트면 안되고, 동적으로 가능한 모델 불러와야 함. 그리고 작업 시킬 때도 모델 고를 수 있게 해줘".

지금 D.Duck이 시작하는 작업은 모두 Claude Code 기본 모델로 돈다. 역할마다(리뷰는 가볍게, 설계는 무겁게), 작업마다 모델을 바꾸려면 터미널로 가야 한다. Claude Code는 `--model`, 서브에이전트 `model` 필드, `--agents` JSON의 `model`을 공식으로 받는다. 하지만 오프라인으로 "쓸 수 있는 모델 목록"을 기계가 읽을 형태로 내주는 명령은 없다. 그래서 목록은 로컬 출처를 모아 그때그때 만든다(조사: design.md "조사 결과").

## What Changes
- **모델 목록(동적)**: 앱에 모델 목록을 박아 두지 않는다. 열 때마다(캐시 + 새로 고침) 아래 로컬 출처를 모은다.
  - CLI: 설치된 `claude --help`의 `--model` 설명에 적힌 별칭. 실행 시점에 앱 데이터 폴더에서 실행해 파싱한다. 모델은 부르지 않는다.
  - 설정: settings의 `model`, `availableModels`, `modelOverrides` 키, 그리고 settings `env`·앱 환경의 모델 변수(`ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU,FABLE}_MODEL`, `ANTHROPIC_DEFAULT_MODEL`, `ANTHROPIC_CUSTOM_MODEL_OPTION`). 이름이 정해진 이 키만 읽는다.
  - 최근 사용: 수집 DB `api_calls.model`에서 최근 30일 동안 쓴 모델 id.
  - 직접 입력: 엄격한 정규식을 통과한 id만 받는다.
- **에이전트별 기본 모델**:
  - D.Duck 역할(작업 유형)은 팀 설정에 `model`을 차이로 저장한다. 시작할 때 인라인 역할 JSON(`--agents`)의 `model`로 넣는다.
  - 파일 에이전트(`~/.claude/agents`, 프로젝트)는 frontmatter `model:` 한 줄만 고친다. 다른 바이트는 그대로 둔다. 플러그인·내장 에이전트는 읽기 전용이다.
- **작업별 모델**: 새 작업 폼(Composer)에 모델 고르기를 둔다. 고른 값은 `--model=<id>`로 시작 인자에 넣는다.
- **우선순위**: 작업에서 고른 값 > 에이전트(파일) 기본값 > 작업 유형 기본값 > CLI 기본값. 앱이 실제로 쓸 값을 하나로 정해서 보여 주고 넘긴다.
- **허용 목록 존중**: `availableModels`(특히 관리 설정)가 막는 값은 고를 수 없게 보이고, 시작도 거부한다. CLI는 막힌 `--model`을 경고 하나만 내고 기본 모델로 바꾸기 때문이다.
- **비용 한 줄**: 고른 모델이 지금 기본값보다 비싸면 한 줄로 알린다. 앱에 든 단가표(`pricing.json`)로만 계산하고, 원격에서 값을 가져오지 않는다.
- **이어 하기·재작업**: 시작 때 넘긴 모델 인자와 그 층을 저장해 두고, usage-limit-resume의 재개와 review-rework의 재작업에서 그대로 재현한다(D-154).
- **지원되지 않는 모델 자동 대체**(사용자 요청 2026-11-11): 정해 둔 모델이 더는 지원되지 않으면(도움말·설정·최근 7일 기록·단가표 어디에도 없음. CLI가 시작 때 거부하면 실측 1.2 전에는 안내 한 줄만 더한다) 시작을 실패시키지 않는다. 같은 계열 별칭 → 계열을 모르면 기본값 순서로 바꾼다. 더 비싼 계열로는 바꾸지 않고, 대체 값도 허용 목록을 통과해야 한다. 정책에 막힌 값은 대체 없이 거부한다. 요청 값·실제 값·이유를 기록하고, 작업 상세와 설정 화면에 한 줄로 보인다.
- **확인 표시**: 작업이 시작된 뒤 수집한 기록의 메인 세션 모델이 요청한 값과 계열이 다르면 작업 상세에 한 줄로 알린다.

## Capabilities
### New Capabilities
- `model-select`: 쓸 수 있는 모델 목록을 로컬 출처에서 모으는 일, 에이전트·유형·작업별 모델 지정과 우선순위, 시작·재개 인자 반영, 허용 목록 검사, 비용 안내.

### Modified Capabilities
없음. 아직 아카이브된 main spec이 없다(`openspec list --specs` 결과 없음). usage-limit-resume·review-rework에서 이어지는 부분은 그 change의 design·tasks에 교차 항목으로 적었다.

## Impact
- core
  - 새 모듈 `model_select.rs`: 출처 모으기, 정규식 검증, 허용 목록 판정, 우선순위 해석, 비용 비교.
  - `claude_cli`: `TaskSpec.model`, `ResumeSpec.model`, `--model=` 인자.
  - `team_config`: `TypeDef.model`·`TypePatch.model`·`RoleDef.model`.
  - `agents_catalog`: `with_model`(frontmatter `model:` 한 줄 편집).
  - `pricing`: 단가 비교 함수.
  - DB: `task_meta` 컬럼 3개(`model_layer`·`model_arg`·`role_model`, v13)와 대체 기록 2개(`model_requested`·`model_fallback`, 번호 없는 멱등 열 추가).
- 셸: 명령 `model_options`, `save_agent_model`. 기존 `start_task`·팀 설정 저장 경로 보강.
- UI: 공용 `ModelPicker`(Composer, AgentSettings 유형 상세, 에이전트 상세), mock, vitest.
- 다른 change: usage-limit-resume(`ResumeSpec.model`), review-rework(재작업이 원래 모델을 이어받음).
- 비대상: 자격증명·`~/.claude.json`·키체인은 읽지 않는다. 모델 호출이나 네트워크로 목록을 가져오지 않는다. 클라우드 루틴·작성(authoring) 작업의 모델 선택은 후속으로 미룬다.
