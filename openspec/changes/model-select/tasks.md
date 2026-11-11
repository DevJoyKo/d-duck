## 0. 게이트
- [x] 0.1 설계 리뷰: design-critic + security-privacy-reviewer로 proposal·spec·design을 검토한다. 중점 항목은 다음과 같다.
  - 자격증명·env 비접근
  - `claude --help` 실행
  - 정규식
  - 허용 목록 fail closed
  - `--agent`/`--model` 우선순위 잠금

  BLOCKER/HIGH를 해결하고 결과와 조치를 decisions.md "리뷰 기록"과 결정 항목에 남긴다. (확인: 리뷰 기록 2행, 개정된 문서) — 완료: design-critic HIGH 2·MAJOR 2·MINOR 2, security MEDIUM 2·LOW 3을 D-154로 반영
- [x] 0.2 앞 change의 DB 번호(usage-limit-resume v10, delivery-guard·report-evidence·task-lifecycle)를 확인하고 이 change의 `task_meta` 번호를 정한다. `TaskSpec`·`ResumeSpec`을 같이 고치는 change(usage-limit-resume 2.7, review-rework)가 지금 어느 단계인지 확인하고 리베이스 순서를 적는다. (확인: decisions.md 구현 메모)

- [x] 0.3 설계 리뷰(대체 요구사항, D-197): design-critic + security-privacy-reviewer로 spec "지원되지 않는 모델은 자동으로 대체한다"와 design 결정 11을 검토한다. 중점: 정책 우회 불가, 더 비싼 계열 금지, CLI 거부 재시도의 안전성, DB v17 번호. BLOCKER/HIGH를 해결한다. (확인: 리뷰 결과를 PR·보고에 기록) — 완료: design-critic BLOCKER 1·HIGH 2·MAJOR 4·MINOR 2, security HIGH 1·MEDIUM 3·LOW 2를 결정 11 개정으로 반영

## 1. 확인 실행 (사용자 승인 후에만, 사용량 소모)
- [x] 1.1 `--agent=<model: haiku 파일 에이전트> --model=sonnet`으로 짧은 bg 작업 1개를 돌린다. 수집 기록 메인 대화 모델로 어느 쪽이 이기는지 기록한다. 같은 세션 `--resume`에서 `--model`이 유지되는지도 잰다. 결과에 따라 `FILE_AGENT_MODEL_OVERRIDE_VERIFIED`를 바꾸고, design 결정 4를 고친다. 다른 태스크는 이 결과 없이 구현한다. D-144에 따라 한도 실측 때 함께 해도 된다. (확인: decisions.md 실측 항목, 세션 id와 api_calls 모델 값) — 완료(2026-11-11, Claude Code 2.1.322, 코디네이터 실행): `--agent probe-haiku`(`model: haiku`) + `--model sonnet` → 메인 대화 `claude-sonnet-5-5`(세션 74f796a9-7f17-475f-ba72-f0afa3f3b179). `--model` 없이 `--resume` → 사본 1d9be74d가 `claude-opus-5-5`(기본값)로 답함, `--model`은 재개에 이어지지 않음. 목록에 있는 세션 재개는 새 bg id 사본. `FILE_AGENT_MODEL_OVERRIDE_VERIFIED = true`, 결정 4·11 개정, 파일 모델 대체를 거부에서 `--model` 대체로 바꿈

- [ ] 1.2 (사용자 승인 후, 사용량 소모 가능) 단가표·도움말에 없는 id로 `claude --bg --model=<사라진 id>`를 한 번 돌려, 시작 때 거부하는지(종료 코드·stderr 문구)와 세션 안에서 거부하는지를 기록한다. 같은 실행에서 `--fallback-model=<같은 계열 별칭>`이 `--bg`에서 동작하는지, 실패한 시작이 worktree·세션을 남기는지도 잰다. 결과에 맞게 `rejects_model` 문구 목록, `CLI_REJECT_RETRY_MEASURED`, design 결정 11을 고친다. (확인: decisions.md 실측 항목)

## 2. core
- [x] 2.1 `model_select.rs`: `valid_model`(design 결정 2), CLI 도움말 파서(`--model` 절 별칭), 출처 합치기·정렬·`resolved`·`current_default`(결정 1).
  - 입력은 순수 `OptionsEnv`다.
  - 테스트: 2.1.319 `claude --help` 발췌 픽스처, 형식이 바뀐 도움말, `<synthetic>`·공백·101자 값 버림, YAML 예약어·숫자만인 값 거부, 같은 값 출처 합치기, 기본값 추정에서 D.Duck `--model` 세션 제외.
  - (확인: `cargo test -p dduck-core model_select`)
- [x] 2.2 settings 읽기: user·project·local·관리 settings에서 `model`·`availableModels`·`enforceAvailableModels`·`modelOverrides` 키와 env 허용 이름 7개만 꺼낸다. 앱 프로세스 환경도 같은 이름만 본다. `~/.claude.json`·자격증명은 열지 않는다. 관리 settings·MDM plist·서버 관리 설정 흔적이 있는데 못 읽으면 `allowlist: unknown`이다. (확인: canary 테스트 — `env.ANTHROPIC_AUTH_TOKEN`·`ANTHROPIC_API_KEY` 값과 키 이름이 결과 직렬화에 없음. 깨진 관리 파일 테스트 — 기본값 말고 모두 막힘)
- [x] 2.3 허용 목록 `allowed`(결정 5): 계열·버전 접두사·전체 id, `[1m]` 떼기, `[]`, 관리 설정 단독, 알 수 없는 맞춤 방식은 `Unknown`. (확인: 문서 예시 `["sonnet","claude-opus-4-6"]` 포함 표 테스트)
- [x] 2.4 `pricing::compare`와 비용 문구 함수(1.2배 이상, 소수 한 자리, 기준은 그 자리의 기본값, 제공자 앞부분·날짜·`[1m]` 떼기), effort 힌트(결정 8), 엄격·민감 유형의 "검토가 얕아질 수 있어요" 판정. (확인: 단위 테스트, 단가 없는 id는 None) — 완료: `pricing::compare`·`cost_line`(core), effort·엄격 힌트는 표시 전용이라 `src/lib/model.ts`(`effortHint`·`shallowHint`)에서 계산
- [x] 2.5 `team_config`: `TypeDef.model`·`TypePatch.model`(`""` = 없음), `validate_type`에서 `valid_model` 검사, `diff`·`merge`·`reset_item`, `RoleDef.model`, `risky_changes`에 넣지 않음. (확인: 차이 저장 왕복 테스트, 기존 저장 값 읽기 테스트, `risky_changes` 테스트) — 완료: `model_tests` 2개
- [x] 2.6 `agents_catalog::with_model`: `RawFm`으로 `model:` 한 줄 넣기·바꾸기·지우기. BOM·CRLF를 보존하고, 키가 둘·여러 줄이면 거부한다. `inherit`은 바꾸지 않으면 보존한다. 결과를 다시 읽어 확인한다. 플러그인·내장은 읽기 전용이다. (확인: 바이트 왕복 테스트 — 넣고 지우면 원본과 같음) — 완료: `with_model`·`model_raw`, 바이트 왕복 테스트
- [x] 2.7 `claude_cli`: `TaskSpec.model`(웹뷰 입력, `validate`), backend-only `resolved_model`·`model_layer`, `prepare_with`에서 `resolve` 호출(결정 4). — 완료: `TaskSpec::apply_model`(셸이 `prepare_with` 뒤 파일 모델과 함께 부름), `model_args` 테스트 7개
  - 인라인 역할: 역할 JSON `model`과 `--model=`을 같은 값으로 넣는다.
  - 파일 에이전트: 작업 값이 없으면 `--model`이 없다.
  - `FILE_AGENT_MODEL_OVERRIDE_VERIFIED=false`이면 파일 `model`+작업 값 조합을 거부한다.
  - 인자는 `--` 앞에 한 번만 넣는다.
  - 저장된 값(유형·파일·`ModelPin`)을 인자 만들기 직전에 `valid_model`로 다시 검사하고, 실패하면 거부한다.
  - 재현용 `ModelPin { layer, arg, role }`을 받으면 `resolve`를 건너뛴다. 잠금은 `layer = task`일 때만 본다.
  - (확인: spec "작업별 모델"·"모델 우선순위" 시나리오마다 `bg_args` 테스트)
- [x] 2.8 DB: `task_meta.model_layer`·`model_arg`·`role_model` nullable 컬럼(0.2에서 정한 번호), 시작 때 실제로 넘긴 값을 기록한다. 요청과 다른 모델 비교 함수(결정 9: 메인 대화 첫 행, `<synthetic>` 제외, 계열·접두사 비교, 비교 불가 값 제외). (확인: 마이그레이션 테스트, 비교 표 테스트) — 완료: v13 열 기록(`record_model`·`model_pin`), 비교는 `record_detail.model_other`

- [x] 2.9 대체(design 결정 11, D-197): `Known`·`supported`(계열·문서 별칭, 도움말, 설정 값(`availableModels` 제외), 최근 7일 기록, 단가표 접두사 또는 `-YYYYMMDD`), `family_of`(별칭·`opusplan`·id 낱말), `fallback`(같은 계열 별칭 → 기본값, 더 비싼 계열 금지, 대체 값 허용 목록 검사(안내 문구 붙은 Yes는 막힘), 파일 에이전트 모델은 1.1 뒤 `--model`로 대체, 요청 값이 막히면 대체 없이 거부), `rejects_model`(stderr 문구), `options`의 저장 값 항목에 `replacement`. `TaskSpec`·`ResumeSpec`에 대체 적용(층별 표). DB 멱등 열 추가(`model_requested`·`model_fallback`, 번호 없음)와 기록·읽기. (확인: spec "지원되지 않는 모델은 자동으로 대체한다" 시나리오마다 단위 테스트, 마이그레이션 테스트) — 완료: `supported`·`family_of`·`fallback`·`fix`·`apply_fallback`·`rejects_model`, DB는 번호 없는 멱등 열 추가

## 3. 셸
- [x] 3.1 `model_options` 명령: `claude --help`를 앱 데이터 폴더 cwd·5초 제한·256KB 상한으로 실행하고, (CLI 경로, 버전, cwd)로 캐시한다. `refresh`면 캐시를 비운다. cwd 검사, 최근 사용 SQL(30일·12개), 관리 설정 루트는 `permissions::default_managed_roots`를 쓴다. (확인: 가짜 CLI 통합 테스트 — 도움말 실패 시 `cliRead: false`이고 나머지 출처는 유지, 모델 호출 인자 없음) — 완료: `model_cmds::model_options`, 가짜 CLI 테스트 `help_cli`
- [x] 3.2 `start_task`·팀 설정 저장·`save_agent_model`에서 허용 목록을 검사한다(fail closed). 페르소나·"내 에이전트"는 `resolve_for_cwd`가 고른 정의 파일에서 `model` 원문을 다시 읽어 넘긴다(`valid_model` 실패·`inherit`는 모델 없음). `save_agent_model`은 기존 에이전트 저장 경로(링크·하드링크·소유자·원자적 쓰기)를 쓴다. (확인: 막힌 값으로 시작하면 CLI가 실행되지 않는 통합 테스트, 프로젝트 에이전트가 사용자 에이전트를 가리는 cwd에서 프로젝트 파일 값을 쓰는 테스트) — 완료: `start_task`·`save_team_config`·`save_agent_model` 허용 목록 검사, 파일 모델은 `resolve_for_cwd` 정의 파일에서 읽음. 셸 통합 테스트는 core 테스트로 대신함

- [x] 3.3 대체 연결: `start_task`·재개가 `Known`·허용 목록·기본값으로 대체를 적용하고 기록한다. 재개의 에이전트 층은 파일 원문을 다시 읽는다. `claude --bg`가 실패하고 `rejects_model`이 맞으면 재시도 없이 안내 한 줄을 붙인다(1.2 전). stderr는 판정에만 쓴다. (확인: 대체 인자 테스트, 거부 문구 안내 테스트, 정책 낱말이 있으면 안내 없음) — 완료: 재시도 없음(1.2 전), `hint_rejected` 테스트

## 4. UI
- [x] 4.1 `src/lib/model.ts`: 정규식(Rust와 같은 표 테스트), 출처 라벨, 기본값 줄·비용 줄·막힘 이유 문구(해요체 한 줄). (확인: vitest, `copyRules.test.ts` 통과) — 완료
- [x] 4.2 `ModelPicker` 공용 컴포넌트(결정 7)를 만든다. — 완료: 네이티브 select + 직접 입력(팝오버 listbox 대신)
  - 구성: 기본값 줄, 출처 칩, 실제 id·"최근 기록 기준", 막힌 항목 흐림+이유, 직접 입력과 검증 줄, 새로 고침, 비용 줄.
  - 키보드(↑↓·Enter·Esc가 이 팝오버만 닫음)와 aria listbox를 지원한다.
  - mock 출처 4종을 만든다: CLI 실패, 관리 허용 목록, 기록 없음, env 고정.
  - (확인: vitest, 라이트·다크·900px 스크린샷)
- [x] 4.3 AgentSettings에 붙인다. — 완료
  - 유형 상세 "기본 모델"을 effort 옆에 둔다. "내 에이전트" 유형은 파일 값이 먼저라는 한 줄을 보인다.
  - 에이전트 상세 "기본 모델"은 `save_agent_model`로 저장한다. 플러그인·읽기 전용이면 보기만 하고 "복사해서 내 것으로"를 안내한다.
  - (확인: vitest, 스크린샷)
- [x] 4.4 Composer에 붙인다. — 완료
  - "모델"을 생각 깊이 옆에 둔다. 기본값 줄은 지금 층(유형·에이전트·CLI)을 보인다.
  - (1.1 뒤 잠금 없음) 미리보기 "모델: … · 층", effort·엄격 힌트를 넣는다.
  - `startTask`에 `model`을 보낸다. 다시 맡기기·초안은 `model_layer = task`일 때만 모델 칸을 채우고, 아니면 기본값과 "원래 작업은 X로 돌았어요" 한 줄을 보인다.
  - (확인: vitest, 스크린샷)
- [x] 4.5 작업 상세에 "모델: … · 층"과 요청과 다른 모델 알림 한 줄을 넣는다. 기록이 없는 이전 작업은 "기록 없음"이다. (확인: vitest, mock) — 완료(지난 작업 펼침)

- [x] 4.6 대체 표시: 작업 상세 "요청한 X를 쓸 수 없어 Y로 실행했어요"(기본값이면 "기본값으로"), 유형·에이전트 상세 "지원되지 않는 모델 · Y로 대체돼요"/"… · 대신 쓸 모델이 없어요". ko·en, 조사 처리. (확인: vitest, copyRules, 스크린샷) — 완료

## 5. 다른 change와의 연결
- [x] 5.1 (usage-limit-resume) `ResumeSpec.model`, `args()`가 `--model=`을 붙인다. `prepare_resume`은 `task_meta`의 `ModelPin`(`model_layer`·`model_arg`·`role_model`)으로 시작 인자를 재현하고 층을 다시 계산하지 않는다. 잠금은 `layer = task`일 때만 보고, 걸리면 기존 RiskChanged로 버튼만 남는다. 재개 직전 허용 목록 검사를 더한다. 그 change tasks 2.8과 같은 항목이다. (확인: 재개 인자 테스트 — 유형 기본값이 바뀌어도 원래 값 유지, 에이전트 층 작업은 잠금에 안 걸림, NULL이면 없음, 막히면 자동 재개 없음) — 완료: `limit_cmds::resume`가 `model_pin`으로 재현·대체·기록, `ResumeSpec.model_args`
- [x] 5.2 (review-rework) 그 change의 design "실행 조건 이월"과 tasks 1.3·1.4·2.3이 이 change의 `task_meta.model_layer`·`model_arg`·`role_model`(NULL = 모델 인자 없음), `ResumeSpec.model`·`ModelPin` 재현, "새 작업으로"는 `layer = task`일 때만 모델 칸을 채움 계약과 맞는지 확인한다. 이름·의미가 달라졌으면 양쪽 문서를 맞춘다. (확인: 두 change 문서 대조 메모, 재작업 초안·인자 테스트가 그 change에 있음) — 완료: review-rework design "실행 조건 이월"·tasks 1.6과 열 이름·의미 일치 확인. `model_arg`는 실제로 넘긴 값(대체 포함)이라 의미 변화 없음

## 6. 통합 검증
- [ ] 6.1 (부분: core 통합 테스트·mock 스크린샷만, 셸 끝-끝은 실제 앱 확인 필요) 가짜 `claude`(도움말 픽스처)와 픽스처 settings·DB로 끝까지 돌린다. 흐름: 목록 → 유형 기본값 저장 → Composer에서 작업 값 → 시작 인자 확인 → 가짜 기록으로 다른 모델 알림. 관리 허용 목록 거부와 env 토큰 canary도 확인한다. 근거 파일을 scratchpad `qa-evidence/model-select/`에 남긴다(D-145). (확인: 통합 테스트 로그와 스크린샷)

## 7. 리뷰
- [x] 7.1 테스트 리뷰: qa-verifier + design-critic + brand-reviewer(UI 변경, D-109·D-148) + agent-skills:code-reviewer(D-148)를 붙인다. BLOCKER/HIGH/FAIL을 해결한 뒤 decisions.md "리뷰 기록"에 결과와 조치를 기록한다. (확인: 리뷰 기록 4행) — 완료(3종): qa-verifier HIGH 1→수정 후 PASS, design-critic MAJOR 4·MINOR 4 반영·일부 문서화, brand-reviewer PASS(HIGH 0, MEDIUM 3 반영). code-reviewer는 이번 지시 범위 밖이라 돌리지 않음

## Workflow follow-up
- 리뷰 통과 후 `/opsx:archive model-select`
