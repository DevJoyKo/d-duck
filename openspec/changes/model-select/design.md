## Context
요청과 범위는 proposal.md를 본다. 지금 상태는 이렇다.
- `TaskSpec::bg_args`·`ResumeSpec::args`(claude_cli.rs)에는 `--model`이 없다.
- 인라인 역할 `RoleDef`(team_config.rs)는 description·prompt·tools·skills·disallowedTools만 갖고, `deny_unknown_fields`다.
- `AgentDef.model`(agents_catalog.rs)은 frontmatter 값을 표시용으로만 읽는다(`display_safe` 40자).
- frontmatter 줄 편집은 `RawFm`(agents_catalog.rs)이 `tools`·`disallowedTools`·`skills`에 대해 이미 한다. BOM·CRLF를 보존하고, 키가 두 번이거나 여러 줄이면 거부한다.
- 단가표는 `pricing.rs`(번들 `data/pricing.json` + 사용자 덮어쓰기)에 있다. 가장 긴 접두사가 맞는 항목을 고른다.
- 수집 DB `api_calls`에 `session_id`·`agent_id`·`ts`·`model`이 있다. `api_calls_model` 인덱스도 있다.
- 관리 설정은 `permissions::managed()`·`default_managed_roots()`로 이미 찾는다.

### 조사 결과 (2026-11-09, Claude Code 2.1.319, 모델 호출 없음)
| 확인한 것 | 결과 | 근거 |
|---|---|---|
| 모델 목록 명령 | 없다. `claude models --help`·`claude config --help`는 알 수 없는 하위 명령이라 메인 도움말을 그대로 낸다(메인 `--help`와 diff 없음). 하위 명령 목록에도 models·config가 없다 | `claude --help` "Commands:" 절, diff |
| `--model` 도움말 | "Provide an alias for the latest model (e.g. 'fable', 'opus', or 'sonnet') or a model's full name." haiku·`[1m]`·opusplan은 도움말에 없다 | `claude --help` |
| `--agents` JSON | `model` 필드를 받는다. 받는 필드는 description·tools·disallowedTools·model·permissionMode·…·effort다 | https://code.claude.com/docs/en/sub-agents |
| `--agent`로 메인 세션 | "the main thread itself takes on that subagent's tool restrictions and model". resume하면 에이전트 모델도 복원된다. **`--model`과 둘 중 무엇이 이기는지는 문서에 없다** → 실측 1.1(Claude Code 2.1.322, 2026-11-11): `--model`이 이긴다. `--model`은 `--resume`에 이어지지 않는다 | sub-agents 문서, 실측 1.1 |
| 서브에이전트 `model` 값 | `sonnet`·`opus`·`haiku`·`fable`, 전체 id, `inherit` | sub-agents 문서 |
| 별칭 | `default`(별칭 아님, 덮어쓰기 해제)·`best`·`fable`·`sonnet`·`opus`·`haiku`·`sonnet[1m]`·`opus[1m]`·`opusplan`. 어떤 id로 풀리는지는 제공자·CLI 버전에 따라 다르다 | https://code.claude.com/docs/en/model-config |
| 모델 우선순위(CLI) | `/model` > `--model` > `ANTHROPIC_MODEL` > settings `model` > `ANTHROPIC_DEFAULT_MODEL` | model-config 문서 |
| `availableModels` | 계열(`sonnet`)·버전 접두사·전체 id로 맞춘다. 막힌 `--model`·`ANTHROPIC_MODEL`·settings `model`은 "replaces the value at startup with a warning… the session starts on the default model". 관리 설정에 있으면 그 목록만 적용한다. `[]`면 이름 지정이 모두 막힌다. `enforceAvailableModels`는 기본값까지 넓힌다 | model-config 문서 |
| `modelOverrides` | 키는 Anthropic 모델 id, 값은 제공자 문자열(ARN 등). 알 수 없는 키는 무시된다 | model-config 문서 |
| 별칭 고정 변수 | `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU,FABLE}_MODEL`, `ANTHROPIC_CUSTOM_MODEL_OPTION`, `CLAUDE_CODE_SUBAGENT_MODEL`(서브에이전트만) | model-config 문서 |
| 이 기기 설정 | `~/.claude/settings.json`·프로젝트 settings에 모델 키·모델 env 없음. 관리 설정 파일 없음 | 키 이름만 출력하는 스크립트 |
| 이 기기 기록 | `api_calls`에 `claude-opus-5-5`(9293)·`claude-sonnet-5-5`(49)·`claude-haiku-5-5`(36) | `sqlite3 -readonly` 집계 |
| 에이전트 파일 | 이 저장소·사용자 에이전트에 `model:` 줄 없음. 플러그인 캐시에 `model: gemini-3-pro` 하나 | grep |

결론: 기계가 읽을 오프라인 목록은 없다. 목록은 로컬 출처를 모아서 만든다(결정 1).

## Goals / Non-Goals
**Goals**
- 앱에 고정 모델 목록 없이, 열 때마다 로컬 출처로 목록을 만든다.
- 유형·파일 에이전트·작업 세 층에서 모델을 고르고, 실제로 쓰일 값을 하나로 정해서 보여 준다.
- 시작·재개·재작업이 같은 모델 값을 쓴다.
- 허용 목록 때문에 CLI가 조용히 다른 모델로 바꾸는 일을 시작 전에 막고, 바뀐 경우 시작 뒤에 알린다.

**Non-Goals**
- 별칭이 지금 어느 id로 풀리는지 정확히 알아내는 일. 그러려면 모델을 부르거나 CLI 내부를 읽어야 한다. "최근 기록 기준" 추정만 보인다.
- 원격 모델 목록(`/v1/models`, 게이트웨이 discovery)·원격 단가.
- `opusplan`·`best`·`[1m]` 항목을 앱이 만들어 넣는 일. 도움말·설정·기록에 나올 때만 보이고, 그 밖에는 직접 입력으로 쓴다.
- 클라우드 루틴·작성(authoring) 작업·로컬 루틴의 모델 선택. 후속 change로 미룬다.
- 앱 안에서 settings `model`을 고치는 일(전역 CLI 기본값 변경). 읽기만 한다.
- effort와 모델의 호환 검사. 4.6 계열에 xhigh 같은 조합은 CLI가 처리한다. 미리보기에 힌트만 보인다(결정 8).

## Decisions

### 1. 목록 출처와 합치기
core `model_select::options(env) -> ModelOptions`. 입력은 순수 데이터(`OptionsEnv`)로 받아 테스트할 수 있게 한다. 셸이 파일·프로세스를 읽어 채운다.

| 출처 | 읽는 법 | 표시 |
|---|---|---|
| CLI | `claude --help`를 고정 인자, 고정 cwd(앱 데이터 폴더, 프로젝트 폴더는 쓰지 않음), 5초 제한, 출력 256KB 상한으로 실행한다. `--model <model>` 줄부터 다음 옵션 줄(`^\s{2}-`) 전까지에서 `'([a-z][a-z0-9-]{0,30}(\[1m\])?)'`를 뽑는다. CLI 경로+`--version` 값으로 캐시한다 | "CLI" |
| 설정 | user·project·local·관리 settings에서 `model`, `availableModels`의 항목, `modelOverrides`의 키를 읽는다. `env` 객체에서는 아래 정해진 이름만 꺼낸다. 앱 프로세스 환경도 같은 이름만 본다 | "설정" |
| 최근 사용 | `SELECT model, MAX(ts) FROM api_calls WHERE ts >= now-30일 AND model != '<synthetic>' GROUP BY model ORDER BY MAX(ts) DESC LIMIT 12` | "최근 사용" |
| 직접 입력 | 웹뷰에서 입력한다. 저장된 유형·에이전트·작업의 현재 값도 목록에 없으면 이 출처로 넣는다 | "직접 입력" |

- env 허용 이름: `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_FABLE_MODEL`, `ANTHROPIC_CUSTOM_MODEL_OPTION`. `env`는 `serde_json::Value`로 받고, 이 이름으로 하나씩 `get`한다. 객체 전체를 구조체로 역직렬화하거나 로그에 남기지 않는다.
- 모든 값은 결정 2의 정규식을 통과해야 한다. 통과하지 못한 값은 버리고 개수만 센다(`dropped`). 이름이 같은 값은 한 항목으로 합치고 출처를 합친다.
- 정렬: 별칭(CLI 순서) → 설정 → 최근 사용(최근 순) → 직접 입력.
- **실제 id(`resolved`)**: 별칭에만 붙인다. 두 가지 중 하나다.
  - `basis: "settings"`: 해당 `ANTHROPIC_DEFAULT_<F>_MODEL` 값이다.
  - `basis: "recent"`: 최근 사용 중 `claude-<별칭>-` 접두사를 가진 가장 최근 id다. "최근 기록 기준"이라고 표시해 추정임을 밝힌다.
  - 둘 다 없으면 비운다.
- **기본값(`current_default`)**: CLI 순서대로 처음 나오는 것이다. `ANTHROPIC_MODEL`(앱 환경·settings env) → settings `model`(관리 > local > project > user) → `ANTHROPIC_DEFAULT_MODEL` → 최근 30일 메인 대화(`agent_id IS NULL`, `<synthetic>` 제외)의 가장 최근 모델("최근 기록") → "계정 기본값". 최근 기록에서는 D.Duck이 모델을 정해 시작한 세션(`task_meta.model_arg`나 `role_model`이 있는 `session_id`)을 뺀다. 그 세션은 CLI 기본값을 보여 주지 않기 때문이다(리뷰 반영 D-154).
- 대안: 앱에 별칭 표를 넣어 두기. 사용자가 static 리스트를 명시적으로 거부했고, 별칭은 CLI 버전마다 바뀐다(문서의 버전 기록 표). 도움말에 haiku가 없는 문제는 최근 사용·직접 입력이 메운다. 도움말 형식이 바뀌면 CLI 출처만 빠진다(spec 시나리오).
- 대안: `claude -p "/model"` 같은 실행. 모델을 부르거나 TTY가 필요해서 버린다.

### 2. 모델 값 형식
- 정규식은 `^[a-z0-9][a-z0-9._:@-]{0,99}(\[1m\])?$`(대문자 거부)이고, core `model_select::valid_model`과 `src/lib/model.ts`가 같은 표 테스트를 쓴다.
- 이 형식으로 Bedrock id(`us.anthropic.claude-opus-4-8`, `…-v1:0`)와 Vertex 날짜(`@20250929`)를 받는다.
- `/`(게이트웨이 id)·ARN은 받지 않는다(위험 절).
- 글자가 하나도 없는 값(`123`, `1.5`)과 YAML 예약어(`null`·`true`·`false`·`yes`·`no`·`on`·`off`·`y`·`n`, 대소문자 무관)는 거부한다. frontmatter에 따옴표 없이 써도 문자열로 읽히게 하기 위해서다(`~`는 정규식에서 이미 막힌다).
- 저장된 값(팀 설정 `TypeDef.model`, 파일 에이전트 값, `task_meta.model_arg`·`role_model`)도 인자를 만들기 직전(`prepare_with` 끝, `ResumeSpec::args`)에 `valid_model`로 다시 검사한다. 실패하면 조용히 빼지 않고 시작·재개를 거부한다("저장된 모델 이름이 올바르지 않아요").
- `default`·`inherit`는 작업·유형 값으로 거부한다. `inherit`는 파일 에이전트의 기존 값으로만 보존하고, 새로 쓰지 않는다.
- 인자는 늘 `--model=<값>` 한 토큰이다(D-015 방식). 값은 `-`로 시작할 수 없어서 옵션으로 읽힐 수 없다.

### 3. 저장 위치
| 층 | 저장 | 시작 때 |
|---|---|---|
| 작업 | `TaskSpec.model: Option<String>`(웹뷰 입력, `validate`에서 검사). `task_meta.model_layer`(`task`·`agent`·`type`·`cli`)와 실제로 넘긴 값 `task_meta.model_arg`(`--model=` 값, 없으면 NULL)·`task_meta.role_model`(역할 JSON `model`, 없으면 NULL). 표시용 "쓴 모델"은 이 셋에서 계산한다 | 결정 4 |
| 파일 에이전트 | frontmatter `model:` 한 줄. `agents_catalog::with_model(content, Option<&str>)`가 `RawFm`으로 편집한다. 삽입 위치는 `description`·`tools` 다음이다. 같은 키가 둘이거나 여러 줄이면 거부한다. 결과를 다시 읽어 값이 맞는지 확인한다. 쓰기는 기존 에이전트 저장 경로(링크·하드링크·소유자 검사, 원자적 쓰기)를 그대로 탄다 | CLI가 파일을 읽는다 |
| 유형 | `TypeDef.model: Option<String>`, `TypePatch.model: Option<String>`(`""` = 번들 값 없음으로). 번들 유형은 모델이 없다(CLI 기본값) | 인라인 `RoleDef.model` |

- `RoleDef`에 `model: Option<String>`(`skip_serializing_if`)을 더한다. `deny_unknown_fields`는 유지한다.
- `risky_changes`: 모델 변경은 위험 변경으로 세지 않는다. effort와 같은 이유로, 도구·권한을 넓히지 않기 때문이다.

### 4. 우선순위와 인자 만들기
`prepare_with` 끝에서 `model_select::resolve(task, agent_file, type_default) -> Resolved { value: Option<String>, layer }`를 부른다. 결과를 `TaskSpec`의 backend-only 필드(`resolved_model`, `model_layer`)에 담는다.
- **인라인 역할**(번들·추가 유형): 값이 있으면 역할 JSON `model`과 `--model`을 **같은 값**으로 둘 다 넣는다. 그러면 `--agent`와 `--model` 중 무엇이 이기든 결과가 같다.
- **파일 에이전트**(페르소나 또는 유형의 "내 에이전트"): 셸이 `agents_catalog::resolve_for_cwd(defs, name, cwd)`로 `claude --agent`가 실제로 쓸 정의를 고른다. 그 파일에서 frontmatter `model` **원문**을 다시 읽는다. `AgentDef.model`은 표시용(`display_safe` 40자)이라 쓰지 않는다. `primary`도 쓰지 않는다. 원문이 `valid_model`을 통과하지 못하거나 `inherit`이면 "모델 없음"으로 본다(리뷰 반영 D-154).
  - 작업 값이 없으면 `--model`을 넣지 않는다. 파일 값이 있으면 CLI가 쓰고, 없으면 유형 값을 `--model`로 넣는다.
  - 작업 값이 있고 파일에 `model`이 있으면 `--model=<작업 값>`을 넣는다. 실측 1.1(Claude Code 2.1.322, 2026-11-11)에서 `--model`이 이긴다고 확인했다: `model: haiku` 사용자 에이전트를 `--agent`로, `--model sonnet`과 함께 시작한 세션(74f796a9)의 메인 대화가 `claude-sonnet-5-5`로 돌았다. 그래서 컴파일 상수 `FILE_AGENT_MODEL_OVERRIDE_VERIFIED`를 true로 바꿨고, 잠금은 없어졌다. CLI가 바뀌어 false로 되돌리면 이 조합은 다시 거부되고 Composer가 잠긴다(D-141 `AUTO_RESUME_SHIPPED`와 같은 방식).
- **CLI 기본값**: 아무 층에도 값이 없으면 `--model`도 역할 `model`도 없다.
- 엄격 유형(배포·보안·QA, `FIXED_STRICT`)도 모델을 고를 수 있다. 모델은 권한·도구를 바꾸지 않으므로 엄격 불변식(도구·페르소나 위험)과 무관하다.
  - 대신 엄격 유형에서 기본값보다 단가가 낮은 모델을 고르면 미리보기에 "검토가 얕아질 수 있어요" 한 줄을 보인다.
  - 민감 영역(`sensitive`)도 같다.
- 팀 규칙(`policy_text`)·delivery-guard 인자·`--allowedTools`는 모델과 무관하게 지금처럼 붙는다. 모델 선택이 정책 문구를 바꾸지 않는다.
- 이 순서는 사용자가 정한 것이다(task > agent > type > CLI). 파일 에이전트가 유형보다 앞서는 이유: "내 에이전트"를 고른 유형에서 역할 프롬프트·도구도 파일을 따르기 때문이다(TypeDef.agent 주석).

### 5. 허용 목록 판정
- `model_select::allowed(list, value) -> Allow`. 반환은 `Yes`, `No`, `Unknown`이다.
- 맞춤 규칙(문서): 항목이 계열 이름이면 별칭 같음 또는 `claude-<계열>-` 접두사. 아니면 값이 항목과 같거나 `항목-`로 시작한다. 양쪽의 `[1m]`은 떼고 비교한다.
- `availableModelsMatch` 같은 다른 맞춤 방식 키가 있으면 `Unknown`이다.
- 관리 설정 읽기 실패: 관리 settings 파일이나 MDM plist(`permissions::default_mdm_files`)가 있는데 읽거나 파싱하지 못하면 `allowlist: "unknown"`이다. 서버 관리 설정의 흔적이 있는데 내용을 알 수 없을 때도 같다. 이때는 기본값만 고를 수 있다. 목록 항목과 직접 입력 값은 모두 `Unknown`으로 막는다(리뷰 반영 D-154).
- 출처: 관리 설정에 `availableModels`가 있으면 그것만 쓴다. 없으면 user·project·local을 병합한다(CLI와 같은 "하위가 확장 못 함" 규칙은 관리 설정에만 적용).
- 별칭 값은 계열 항목으로 판정한다. 계열이 막히고 버전 항목만 있으면 CLI는 허용되는 최신 버전으로 바꾼다(문서). 앱은 이걸 `Yes`로 보고, 고르기에 "관리 설정 범위의 버전으로 바뀌어요"를 단다.
- `No`·`Unknown`이면 고르기에서 비활성이다. `start_task`·재개·저장도 거부한다(fail closed). 직접 입력도 같다.
- 기본값 항목은 늘 고를 수 있다. 기본값이 막히면 CLI가 처리한다.
- 막힘 이유 문구(해요체 한 줄):
  - 관리 설정이 막음: "관리 설정이 이 모델을 막아요"
  - 사용자·프로젝트 설정이 막음: "설정의 허용 목록에 없는 모델이에요"
  - `Unknown`(맞춤 방식을 모름): "허용 목록을 확인할 수 없어 막았어요"
  - 관리 설정을 못 읽음: "관리 설정을 읽지 못해 기본값만 쓸 수 있어요"(목록 아래 한 줄)

### 6. 비용 한 줄
- `pricing::compare(table, chosen_id, default_id) -> Option<f64>`는 출력 단가의 비율이다.
- chosen은 직접 id이거나 결정 1의 `resolved`다. 비교 기준은 **그 자리의 기본값**이다(리뷰 반영 D-154).
  - Composer: 작업 층을 뺀 해석 결과(에이전트 파일 → 유형 → CLI 기본값).
  - 유형 상세·에이전트 상세: CLI 기본값(`current_default`).
  - 둘 중 하나라도 단가가 없으면 `None`이다.
- 단가표를 찾기 전에 제공자 앞부분(`us.`·`eu.`·`apac.`·`global.` 다음 `anthropic.`, `anthropic/`)과 `@날짜`·`-v1:0` 같은 뒤 꼬리, `[1m]`을 뗀다. 그다음 `PriceTable::find`(가장 긴 접두사)를 쓴다.
- 비율이 1.2 이상일 때만 "기본값보다 토큰당 약 N배 비싸요 · 앱 단가표 기준"을 보인다. N은 소수 한 자리다.
- 표는 `PriceTable::load`(번들 또는 사용자 로컬 파일)를 그대로 쓴다. 네트워크 접근은 없다.

### 7. 셸 명령과 UI
- `model_options({ cwd?: string, refresh: bool }) -> ModelOptions`
  - 응답: `{ items: [{ value, sources: ("cli"|"settings"|"recent"|"custom")[], resolved?: { id, basis }, allow: "yes"|"no"|"unknown", blockedBy?: "managed"|"settings"|"unknown", note? }], currentDefault: { value, basis }, cliRead: bool, dropped: number, allowlist: "none"|"managed"|"settings"|"unknown" }`
  - cwd는 기존 프로젝트 검사(`is_safe_cwd`, 신뢰는 묻지 않음 — 정적 읽기만)를 거친다. 캐시는 (CLI 경로, 버전, cwd) 키이고 react-query 항목 하나다(D-138 방식, gcTime 무한).
- `save_agent_model({ id, model: string | null })`: 카탈로그 id만 받는다. 기존 `save_agent_tools`와 같은 쓰기 계획·확인 경로를 쓴다.
- 팀 설정 저장은 기존 `TeamInput` → `diff`에 `model`이 실린다.
- `ModelPicker` 컴포넌트 하나를 Composer·유형 상세·에이전트 상세가 같이 쓴다.
  - 네이티브 `<select>`이고(키보드·스크린리더를 그대로 쓴다, 구현 때 팝오버 listbox 대신 고름), 항목은 한 줄이다: `opus · claude-opus-5-5 (최근 기록 기준) · CLI`. 출처는 칩 대신 글자로 붙인다.
  - 첫 줄은 "기본값 (지금: … · 출처)", 마지막 줄은 "직접 입력"(입력칸 + 검증 한 줄)이다. 머리에 "목록 새로 고침" 버튼이 있다.
  - 막힌 항목은 흐리게 보이고 이유를 붙인다. 비용 줄은 고르기 아래에 둔다.
  - 문구는 해요체다.
- Composer는 "생각 깊이" 옆에 "모델"을 둔다. 미리보기(`showPreview`)에 "모델: … · 층" 한 줄을 넣는다.
- 작업 상세에 결정 9의 알림을 둔다.

### 8. effort 힌트
문서의 effort 표는 모델마다 다르다(4.6 계열은 xhigh 없음). 앱은 막지 않는다. 고른 값이 `-4-6` id이고 effort가 xhigh이면 미리보기에 "이 모델은 xhigh를 지원하지 않을 수 있어요" 한 줄만 보인다. 판정 규칙은 id 접두사 하나뿐이다.

### 9. 요청과 다른 모델 알림
- 폴러가 `task_meta.session_id`를 채운 뒤(usage-limit-resume 3.1) 비교한다. 비교 대상은 `api_calls`에서 그 session의 `agent_id IS NULL`이고 `model != '<synthetic>'`인 첫 행의 `model`이다. 기대 값은 `model_arg`, 없으면 `role_model`이다. 둘 다 없으면(층이 `agent`·`cli`) 비교하지 않는다.
- 요청 값이 별칭이면 계열(`claude-<별칭>-`)로 비교한다. 전체 id면 `[1m]`과 제공자 앞부분(`us.anthropic.`)을 떼고 접두사로 비교한다.
- 다르면 작업 상세에 한 줄을 보인다. 저장은 하지 않고 조회 때 계산한다.
- `best`·`opusplan`·`default`처럼 계열이 정해지지 않은 값은 비교하지 않는다.

### 10. 다른 change와의 연결
- **usage-limit-resume**
  - `ResumeSpec.model: Option<String>`. `args()`는 `--resume=` 뒤에 `--model=`을 붙인다.
  - 재개는 층을 다시 계산하지 않고 **시작 때 넘긴 인자를 재현**한다(리뷰 반영 D-154). `prepare_resume`에 backend-only `ModelPin { layer, arg, role }`을 넣으면 `resolve`를 건너뛰고 `--model=`은 `model_arg`, 역할 JSON `model`은 `role_model`을 그대로 쓴다. 두 값은 `valid_model`로 다시 검사한다.
  - 파일 에이전트+작업 값 잠금은 상수가 false일 때만 다시 본다(지금은 true라 걸리지 않음).
  - 실측 1.1(Claude Code 2.1.322, 2026-11-11): `--model` 없이 `--resume`한 세션(사본 1d9be74d)은 기본 모델 `claude-opus-5-5`로 답했다. `--model`은 재개에 이어지지 않으므로, 재개마다 `ModelPin`의 `--model=`을 다시 넘긴다(지금 구현). 목록에 아직 있는 세션을 재개하면 새 bg id의 사본이 생긴다("already running in the background, so this started a copy").
  - `task_meta`에 값이 없는 이전 작업(NULL)은 `--model`도 역할 `model`도 넣지 않는다.
  - 재개 직전 재확인(3.2)에 허용 목록 검사(결정 5)를 더한다. 걸리면 버튼+확인으로만 잇는다. 모델 때문에 막힌 재개(허용 목록, 대체 없음, 파일 에이전트 모델, 손상된 기록)는 지금은 모두 `Reason::RiskChanged`로 보인다. 모델 전용 이유 코드는 후속이다.
  - 기록 읽기는 `task_meta::PinRead`다. 층이 없는 이전 작업은 `layer = cli`·인자 없음으로 재현하고(층 재계산 없음), 층이 네 값이 아니면 거부한다. 값은 거르지 않고 넘겨 `check_pin`이 손상된 값을 거부한다(QA 리뷰 HIGH).
  - 그 change의 design·tasks에 교차 항목을 적었다.
- **review-rework**(원래 review-rework-evidence에서 나뉨, D-149 (2))
  - "새 작업으로"는 `model_layer = task`일 때만 Composer 초안의 모델 칸에 `model_arg`를 채운다. 다른 층이면 모델 칸을 기본값으로 둔다. 그러면 지금 에이전트·유형 기본값이 적용되고 파일 에이전트 잠금도 걸리지 않는다. 원래 값이 지금 기본값과 다르면 "원래 작업은 X로 돌았어요" 한 줄을 보인다.
  - "같은 세션에서 이어서"는 재개와 같은 `ModelPin` 재현을 쓴다.
  - 그 change에는 이미 같은 내용이 있다: design "실행 조건 이월", tasks 1.3·1.4·2.3, spec 시나리오 "모델 유지". 이 change는 그 change가 쓰는 계약 두 가지를 제공한다. 하나는 `task_meta.model_layer`·`model_arg`·`role_model` 열 이름과 NULL 의미("기록 없음, 모델 인자 없음")이고, 다른 하나는 `ResumeSpec.model`·`ModelPin` 재현 규칙이다. 이 계약을 바꾸면 그 change에 알린다.

### 11. 지원되지 않는 모델의 자동 대체 (사용자 요청 2026-11-11, D-197)
요청: 고른 모델이 더는 지원되지 않으면 시작을 실패시키지 말고 자동으로 대체한다. 설계 리뷰(0.3) 반영본이다.

**지원 판정** `model_select::supported(value, &Known) -> bool`. `Known`은 순수 데이터다(도움말 별칭, 설정 값, 최근 7일 모델). 다음 중 하나면 지원된다.
- 계열 별칭(`FAMILIES`, `[1m]` 포함)과 문서 별칭 `best`·`opusplan`. 도움말에 haiku가 없어도 계열 별칭은 CLI가 푼다.
- 지금 `claude --help` 별칭.
- 설정의 모델 값: `model`, `modelOverrides` 키, 정해진 env 값. **`availableModels` 항목은 빼다.** 허용 범위일 뿐 지원 증거가 아니다(리뷰 BLOCKER).
- 최근 **7일** 기록의 모델(`normalize_id`로 비교). 30일이면 은퇴한 id가 한 달 동안 지원됨으로 남는다(리뷰 MAJOR).
- 단가표: `price_key(value)`가 어떤 접두사와 같거나 `접두사-YYYYMMDD`다. `PriceTable::find`의 "가장 긴 접두사"는 쓰지 않는다. 그 규칙이면 `claude-opus-5-9`도 `claude-opus-5`로 "안다"가 된다.
- 대안: 모델을 불러 확인하기. 비용이 들고 네트워크를 쓴다(비대상).

**대체 대상** `model_select::fallback(requested, &FallbackCtx) -> Result<Replacement, &'static str>`.
- 순서는 늘 **형식 검사 → 요청 값의 허용 목록 검사(막히면 거부, 대체 없음) → 지원 판정 → 대체 대상 검사**다. 그래서 대체가 정책을 돌아가는 길이 되지 않는다.
- `Replacement::Alias(f)`: 같은 계열 별칭. 계열은 `family_of(value)`다. 별칭이면 그 계열, `opusplan`은 opus, 그 밖에는 `normalize_id` 결과가 `claude-`로 시작하고 `-`로 나눈 낱말에 계열 이름이 딱 하나 있을 때 그 계열이다(`claude-3-5-sonnet-…`도 잡는다). `[1m]`은 떼서 넘긴다(1M 문맥은 단가가 더 높다).
- 대체 별칭은 `allowed`가 안내 문구 없이 `Yes`일 때만 쓴다. "관리 설정 범위의 버전으로 바뀌어요"(버전 항목으로 좁혀짐)는 막힘으로 본다. CLI가 그 별칭을 다시 사라진 버전으로 좁힐 수 있기 때문이다(리뷰 BLOCKER).
- `Replacement::Default`: 계열을 모르면 기본값이다(사용자 요구 2). 인라인 역할은 `--model`과 역할 `model`을 모두 뺀다.
- 같은 계열 별칭이 막히면 기본값을 본다. 이때 기본값의 계열이 요청 계열보다 높거나(haiku < sonnet < opus < fable) 계열을 모르면 거부한다("대신 쓸 모델이 없어 시작하지 않았어요 · 더 비싼 모델로는 바꾸지 않아요"). 계열을 모르는 요청은 비교할 계열이 없어서 규칙 3이 적용되지 않는다. 대신 대체 사실이 작업 상세에 남는다(보안 리뷰 MEDIUM, 사용자 요구 2를 따름).
- 대체 값도 `valid_model`을 통과해야 한다. `--model=<값>` 한 토큰 형식은 그대로다.

**파일 에이전트** 실측 1.1(Claude Code 2.1.322, 2026-11-11)에서 `--model`이 `--agent`의 모델을 이긴다고 확인했다(결정 4). 그래서 에이전트 파일의 `model:`이 지원되지 않으면 다른 층과 같은 규칙으로 대체하고, 대체 값을 `--model=`로 넣는다. 계열을 모르면 기본값의 값을 알 때만 그 값을 `--model`로 넣는다. `--model`을 빼면 CLI가 파일 값을 다시 쓰기 때문이다. 기본값을 모르면 거부한다("대신 쓸 모델이 없어 시작하지 않았어요…"). 허용 목록 검사는 그대로다. 파일 값이 정책에 막히면 대체 없이 거부한다. `FILE_AGENT_MODEL_OVERRIDE_VERIFIED`를 false로 되돌리면 이전처럼 거부한다("에이전트 파일의 모델을 쓸 수 없어요…").

**CLI 거부** 1.2 실측 전에는 **다시 시작하지 않는다.** `rejects_model(stderr_tail, value)`은 만들어 두되, 재시도는 상수 `CLI_REJECT_RETRY_MEASURED = false` 뒤에 둔다. 실패한 `--bg`가 worktree·세션·훅을 남겼는지 모르고, 정책 거부 문구와 섞일 수 있으며, 네이티브 승인을 바뀐 인자에 다시 써야 하기 때문이다(보안 리뷰 HIGH·MEDIUM, 설계 리뷰 MAJOR). 지금은 거부 문구가 맞으면 실패 메시지 끝에 "이 모델을 쓸 수 없는 것 같아요 · 다른 모델을 골라 주세요" 한 줄을 더하고, 다음 시작부터는 그 값이 지원 판정에서 빠지지 않는다(저장하지 않음). 문구 판정은 `availableModels`·`policy`·`organization`·`managed`·`permission`·`access`가 있으면 거부로 보지 않는다. stderr는 판정에만 쓰고 DB·timing·로그에 넣지 않는다.
- 대안: CLI의 `--fallback-model <model>`(도움말 2.1.322: "when the default model is overloaded or not available"). `--bg`에서의 동작과 "default model"의 뜻이 측정되지 않았다. 1.2에서 함께 재고, 맞으면 대체 별칭을 이 옵션으로도 넘긴다(허용 목록을 통과한 값만).

**층별 적용**
| 요청 값 | 대체 시 인자 |
|---|---|
| 작업·유형(인라인 역할) | `--model`과 역할 `model`을 같은 대체 값으로(기본값이면 둘 다 없음) |
| 작업·유형(파일 에이전트 위) | `--model`만 대체 값으로. 기본값이면 `--model` 없음 |
| 에이전트 파일 `model:` | `--model=<대체 별칭>`, 계열을 모르면 `--model=<알려진 기본값>`, 둘 다 없으면 거부 |
| 재개 `ModelPin` | `arg`·`role`을 같은 규칙으로 대체한다. 층은 그대로 둔다. `layer = agent`이면 파일 원문을 다시 읽어 위 행을 적용한다 |

**기록** `task_meta.model_requested TEXT`, `task_meta.model_fallback TEXT`(`unsupported`만, 1.2 뒤 `cli_rejected`). 버전 번호를 쓰지 않는다. `migrate` 끝의 멱등 단계가 `pragma table_info`로 열이 없을 때만 `ADD COLUMN`한다. 그래서 PR #5의 v16과 순서가 어떻든 부딪히지 않는다(리뷰 HIGH). `model_requested`는 `valid_model`을 통과한 값만 저장한다(100자 이하). `model_arg`·`role_model`은 늘 **실제로 넘긴 값**이다. 그래서 재개는 대체된 값을 그대로 재현한다. 재개에서 다시 대체하면 두 열을 새 값으로 바꾸고, `model_requested`는 비어 있을 때만 채운다.

**표시**
- 작업 상세: `model_requested`가 있으면 "요청한 X를 쓸 수 없어 Y로 실행했어요"(Y가 없으면 "기본값으로") 한 줄. 결정 9 알림은 실제로 넘긴 값과 비교한다.
- 설정 화면: `model_options`가 저장된 값(팀 설정 유형 모델, 이 cwd의 에이전트 파일 모델)을 "직접 입력" 출처 항목으로 넣고, 지원되지 않으면 `replacement: { kind: "alias", to } | { kind: "default" } | { kind: "none" }`을 단다. 고르기 아래에 "지원되지 않는 모델 · Y로 대체돼요"(기본값이면 "기본값으로 대체돼요") 또는 "… · 대신 쓸 모델이 없어요" 한 줄을 보인다. 파일 에이전트 상세도 같은 줄이다(실측 1.1 뒤).
- 대체는 노랑을 쓰지 않는다. 보조 글자색 한 줄이다.

## Risks / Trade-offs
- [`--agent`의 모델과 `--model` 중 무엇이 이기는지 문서에 없음] → 실측 1.1(Claude Code 2.1.322, 2026-11-11)로 `--model`이 이긴다고 확인했다. 인라인 역할은 여전히 두 값을 같게 넣는다. CLI가 바뀌면 상수 하나로 이전 잠금으로 돌아간다.
- [`claude --help` 문구는 계약이 아님] → 파싱 실패는 CLI 출처만 비운다. 픽스처 테스트에 2.1.319 출력 발췌를 넣는다. 도움말에 haiku가 없는 것도 그대로 받아들인다(최근 사용·직접 입력으로 보완).
- ["최근 기록 기준" 실제 id는 추정] → 표시를 "최근 기록 기준"으로 밝힌다. 비용 줄은 이 추정으로도 계산하되 "앱 단가표 기준"을 붙인다. 틀리면 결정 9 알림이 사후에 잡는다.
- [관리 설정을 못 읽으면 기본값만 남아 불편함] → 막는 쪽이 맞다. 이유 한 줄을 보이고 기본값은 늘 쓸 수 있다.
- [프로젝트 settings는 신뢰 안 된 저장소일 수 있음] → 값은 정규식을 통과한 모델 이름만 목록에 쓴다. 실행에 쓰이는 것은 사용자가 고른 값뿐이다. 프로젝트 settings의 `availableModels`는 막는 쪽으로만 작동한다.
- [settings `env`에 토큰·API 키가 같이 있음] → 정해진 이름만 `get`한다. canary 테스트로 토큰 값이 응답·로그·DB에 없음을 고정한다.
- [정규식이 게이트웨이 id(`a/b`)·ARN을 막음] → 그런 사용자는 CLI 설정(`ANTHROPIC_MODEL` 등)으로 기본값을 쓰면 된다. 기본값 항목은 늘 고를 수 있다. 필요하면 후속으로 넓힌다.
- [허용 목록 판정이 CLI와 다를 수 있음] → `Unknown`은 막는 쪽이다. 결정 9 알림이 사후에 잡는다.
- [지원 판정이 틀림(새 모델이 단가표보다 먼저 나옴)] → 같은 계열 별칭으로 대체되므로 대개 같은 최신 모델이 된다. 설정 화면이 미리 "지원되지 않는 모델"을 보여 사용자가 고칠 수 있다. 작업 상세에 대체 사실이 남는다.
- [오래됐지만 아직 살아 있는 id(일부러 고정한 옛 버전)] → 7일 안에 안 썼고 단가표에 없으면 같은 계열 별칭으로 바뀐다. 단가표에 `claude-opus-4-5`·`claude-opus-4-1`·`claude-sonnet-4-5`를 더해 줄였다. 같은 계열 안에서는 비용 비교를 하지 않는다(별칭은 보통 더 싼 최신 버전).
- [CLI 거부 문구 미측정] → 1.2 전에는 재시도하지 않고 안내 한 줄만 더한다. 세션 안에서 API가 거부하는 경우는 잡지 못한다(1.2에서 `--fallback-model`과 함께 잰다).
- [비싼 모델을 기본으로 둔 유형이 쌓이면 비용 증가] → 유형 상세에도 같은 비용 줄을 보인다. 작업 상세에 실제 모델을 보인다.

## Migration Plan
- DB: `task_meta`에 nullable `model_layer TEXT`, `model_arg TEXT`, `role_model TEXT`를 더한다(v13, 이미 main에 있음). 대체 기록 `model_requested TEXT`, `model_fallback TEXT`는 버전 번호 없이 `migrate` 끝의 멱등 단계(열이 없을 때만 `ADD COLUMN`)로 더한다. 열린 PR #5(v16)와 순서가 바뀌어도 부딪히지 않는다. 기존 행은 NULL이고 "기록 없음"으로 보인다. 재개에서 NULL이면 `--model`을 넣지 않는다(지금 동작과 같음).
- 팀 설정: `TypePatch.model`은 선택 필드다. 이전 저장 값은 그대로 읽히고 모델 없음이 된다.
- 되돌리기: 기능을 빼도 저장된 `model` 필드는 serde `default`로 무시된다. 파일 에이전트에 쓴 `model:` 줄은 CLI가 원래 지원하는 필드라 그대로 둬도 된다.

## Open Questions
- settings의 `modelPicker` 행을 출처로 쓸지. 문서 settings-reference에만 형식이 있어 아직 확인하지 않았다. 출처를 하나 더하는 일이라 spec·태스크 구조는 바뀌지 않는다.
