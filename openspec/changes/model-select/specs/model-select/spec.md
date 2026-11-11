# Spec Delta

## Purpose

사용자가 에이전트(역할·파일 에이전트)와 작업마다 Claude 모델을 고를 수 있게 한다. 고를 수 있는 목록은 설치된 CLI·설정·수집 기록 같은 로컬 출처에서 그때그때 모은다.

## ADDED Requirements

### Requirement: 로컬 출처로 만드는 모델 목록
앱은 모델 목록을 앱 안에 고정해 두지 않아야 한다(MUST NOT). 목록은 로컬 출처를 합쳐서 만들어야 한다(SHALL). 출처는 설치된 CLI 도움말의 `--model` 별칭, settings의 모델 키와 정해진 모델 환경 변수, 수집 DB에서 최근 30일 동안 쓴 모델이다. 같은 값은 한 항목으로 합치고 출처 표시를 모두 달아야 한다(SHALL). 목록을 만들면서 모델을 부르거나 네트워크에 접속해서는 안 된다(MUST NOT).

#### Scenario: 세 출처를 합친다
- **WHEN** CLI 도움말에 `'fable'`·`'opus'`·`'sonnet'`이 있고, 최근 30일 기록에 `claude-opus-5-5`·`claude-haiku-5-5`가 있고, 설정 `model`이 `sonnet`이다
- **THEN** 목록에 `fable`(CLI), `opus`(CLI), `sonnet`(CLI·설정), `claude-opus-5-5`(최근 사용), `claude-haiku-5-5`(최근 사용)가 각각 한 번씩 나온다

#### Scenario: CLI 도움말 형식이 바뀌었다
- **WHEN** `claude --help` 출력에서 `--model` 설명을 찾지 못하거나 따옴표 친 별칭이 없다
- **THEN** CLI 출처 항목만 빠진다. 다른 출처와 직접 입력은 그대로 쓸 수 있다. 목록 아래에 "CLI에서 별칭을 못 읽었어요" 한 줄이 보인다

#### Scenario: 새로 고침
- **WHEN** 사용자가 목록의 새로 고침을 누른다
- **THEN** 캐시를 버리고 도움말·설정·기록을 다시 읽는다. 모델 호출이나 네트워크 요청은 없다

#### Scenario: 형식이 이상한 값은 버린다
- **WHEN** 설정이나 기록에 `<synthetic>`, 공백이 든 값, 100자가 넘는 값이 있다
- **THEN** 그 값은 목록에 나오지 않는다

### Requirement: 설정은 모델 키만 읽는다
설정 파일에서는 `model`·`availableModels`·`enforceAvailableModels`·`modelOverrides`의 키, 그리고 `env` 안의 정해진 모델 변수 이름만 읽어야 한다(SHALL). `env`의 다른 값(예: API 키·토큰), `~/.claude.json`, 자격증명 파일, 키체인은 읽지 않아야 한다(MUST NOT). 읽은 값은 저장하거나 기록에 남기지 않는다(MUST NOT). 다만 사용자가 작업에 고른 모델 id는 예외로 저장한다.

#### Scenario: env에 토큰이 같이 있다
- **WHEN** 사용자 settings `env`에 `ANTHROPIC_DEFAULT_OPUS_MODEL`과 `ANTHROPIC_AUTH_TOKEN`이 함께 있다
- **THEN** 목록 응답·로그·DB 어디에도 토큰 값이나 토큰 키 이름이 없다. opus 별칭의 실제 id는 그 변수의 값으로 표시된다

### Requirement: 기본값 항목
모든 모델 고르기의 첫 항목은 "기본값"이어야 한다(SHALL). 최근 기록으로 기본값을 짐작할 때는 D.Duck이 모델을 정해 시작한 세션과 `<synthetic>` 기록을 빼야 한다(SHALL). 기본값을 고르면 그 층에서는 모델을 정하지 않은 것으로 본다. 기본값 항목에는 그 자리에서 실제로 쓰일 값과 그 값이 어디서 왔는지를 한 줄로 보여야 한다(SHALL). 예: 에이전트 설정, 유형 설정, CLI 설정, 최근 기록.

#### Scenario: 아무 설정도 없다
- **WHEN** 모델 관련 설정이 하나도 없고 최근 메인 세션 기록이 `claude-opus-5-5`다
- **THEN** 기본값 항목은 "기본값 (지금: claude-opus-5-5 · 최근 기록)"이다

#### Scenario: Composer에서 유형 기본값이 있다
- **WHEN** 리뷰 유형의 기본 모델이 `sonnet`이고 사용자가 Composer에서 리뷰 유형을 고른다
- **THEN** 모델 고르기의 기본값 항목은 "기본값 (지금: sonnet · 유형 설정)"이다

### Requirement: 직접 입력 검증
사용자는 목록에 없는 모델 id를 직접 입력할 수 있어야 한다(SHALL). 입력은 소문자 영숫자로 시작하고 `[a-z0-9._:@-]`만 쓰며 100자 이하여야 한다. 끝에 `[1m]`을 붙일 수 있다. 이 형식이 아니면 거부해야 한다(MUST). `default`·`inherit`, 글자가 없는 값, YAML 예약어(`null`·`true`·`false`·`yes`·`no`·`on`·`off`·`y`·`n`)는 받지 않는다(MUST NOT). 저장된 값도 시작·재개 인자를 만들기 직전에 같은 규칙으로 다시 검사하고, 실패하면 거부해야 한다(MUST). 검증은 웹뷰와 Rust에서 같은 규칙으로 해야 한다(SHALL).

#### Scenario: 허용되는 입력
- **WHEN** 사용자가 `claude-opus-4-6[1m]`, `us.anthropic.claude-opus-4-8`, `claude-sonnet-4-5@20250929` 중 하나를 입력한다
- **THEN** 받아들여지고 "직접 입력" 출처로 표시된다

#### Scenario: 거부되는 입력
- **WHEN** 입력이 `-p`, `opus --dangerously-skip-permissions`, `Opus`, `a/b`, `inherit`, `null`, `true`, `123`, 101자 문자열 중 하나다
- **THEN** 저장·시작을 거부하고 "모델 이름 형식이 아니에요" 한 줄을 보인다. 웹뷰를 거치지 않은 요청도 Rust가 같은 이유로 거부한다

### Requirement: 에이전트별 기본 모델
작업 유형(D.Duck 역할)과 편집 가능한 파일 에이전트마다 기본 모델을 정할 수 있어야 한다(SHALL). 유형의 값은 팀 설정에 차이로 저장하고, 시작할 때 인라인 역할 정의의 `model`로 넣어야 한다(SHALL). 파일 에이전트는 frontmatter의 `model:` 한 줄만 바꾸고 다른 바이트(BOM·줄끝 포함)는 보존해야 한다(SHALL). 기본값으로 되돌리면 그 줄을 지운다. 플러그인·내장 에이전트와 링크·하드링크 파일에는 쓰지 않아야 한다(MUST NOT).

#### Scenario: 유형 기본 모델
- **WHEN** 사용자가 리뷰 유형의 기본 모델을 `sonnet`으로 저장하고, 그 유형으로 작업을 시작한다
- **THEN** 시작 인자의 `--agents` JSON에서 그 역할의 `model`이 `"sonnet"`이다

#### Scenario: 파일 에이전트 줄 편집
- **WHEN** CRLF 줄끝을 쓰는 `design-critic.md`에 `model:` 줄이 없고, 사용자가 `opus`를 저장한다
- **THEN** frontmatter에 `model: opus` 한 줄이 CRLF로 더해지고 나머지 바이트는 같다. 다시 기본값으로 저장하면 원래 파일과 바이트가 같다

#### Scenario: 여러 줄로 쓴 값
- **WHEN** 파일의 `model` 키가 두 번 나오거나 값이 여러 줄이다
- **THEN** 저장을 거부하고 "정의 편집에서 고쳐 주세요"를 보인다

#### Scenario: 파일에 이미 있는 inherit
- **WHEN** 파일 에이전트의 `model:` 값이 `inherit`다
- **THEN** 고르기는 "기본값 (메인 대화 모델을 따라요)"로 보이고, 사용자가 바꾸지 않으면 그 줄은 그대로다

### Requirement: 작업별 모델
새 작업 폼에서 이번 작업의 모델을 고를 수 있어야 한다(SHALL). 고른 값은 시작 인자에 `--model=<값>` 하나로 넣어야 한다(SHALL). 값이 `-`로 시작하거나 별도 인자로 쪼개져 들어가서는 안 된다(MUST NOT). 작업에서 고르지 않으면 아래 우선순위 요구사항에 따른다. 어느 층에도 값이 없으면 `--model` 인자를 넣지 않는다.

#### Scenario: 작업에서 고른다
- **WHEN** 사용자가 Composer에서 `haiku`를 고르고 시작한다
- **THEN** `claude --bg` 인자에 `--model=haiku`가 정확히 한 번 있고, 프롬프트 구분자 `--`보다 앞에 있다

#### Scenario: 고르지 않는다
- **WHEN** 유형·에이전트·작업 어디에도 모델이 없다
- **THEN** 시작 인자에 `--model`이 없고 역할 JSON에도 `model`이 없다

### Requirement: 모델 우선순위
앱은 실제로 쓸 모델을 다음 순서로 하나 정해야 한다(SHALL): 작업에서 고른 값 > 에이전트 파일의 `model` > 작업 유형 기본값 > CLI 기본값. 정한 값과 그 층을 시작 전 미리보기와 작업 상세에 보여야 한다(SHALL). 인라인 역할로 시작하면 역할 JSON의 `model`과 `--model`이 서로 다른 값이어서는 안 된다(MUST NOT).

#### Scenario: 작업 값이 이긴다
- **WHEN** 유형 기본값이 `sonnet`이고 Composer에서 `opus`를 고른다
- **THEN** 역할 JSON `model`과 `--model`이 모두 `opus`이고, 미리보기에 "모델: opus · 이번 작업"이 보인다

#### Scenario: 에이전트 파일이 유형보다 앞선다
- **WHEN** 유형이 "내 에이전트" `design-critic`을 쓰고, 그 파일의 `model`이 `opus`이고, 유형 기본값이 `haiku`다
- **THEN** 작업 모델은 `opus`(에이전트 설정)이고, 시작 인자에 `--model=haiku`가 없다

#### Scenario: 파일 에이전트 위에 작업 값을 고른다
- **WHEN** 파일 에이전트(`model: opus`)를 페르소나로 고르고 Composer에서 `sonnet`을 고른다
- **THEN** 확인 실행(design 1.1)에서 `--model`이 `--agent`의 모델보다 앞선다고 기록되기 전에는 Composer 모델 고르기가 잠긴다. 그때 "이 에이전트는 파일에 정한 모델(opus)로 돌아요" 한 줄이 보인다. 기록된 뒤에는 `--model=sonnet`으로 시작한다

### Requirement: 관리 설정을 읽지 못하면 막는다
관리 settings 파일·MDM plist·서버 관리 설정이 있는데 읽거나 해석하지 못하면, 기본값을 뺀 모든 값(목록 항목과 직접 입력)을 막아야 한다(MUST). 목록 아래에 이유를 한 줄로 보여야 한다(SHALL).

#### Scenario: 관리 파일이 깨졌다
- **WHEN** 관리 settings 파일이 있고 JSON으로 읽히지 않는다
- **THEN** 모든 항목이 "허용 목록을 확인할 수 없어 막았어요"로 비활성이다. 목록 아래에 "관리 설정을 읽지 못해 기본값만 쓸 수 있어요"가 보인다. 기본값으로는 시작할 수 있다

### Requirement: 허용 목록 검사
설정에 `availableModels`가 있으면 그 목록이 막는 값은 고르기에서 비활성으로 보이고 이유를 달아야 한다(SHALL). 그 값으로는 시작·재개·저장을 거부해야 한다(MUST). 관리 설정에 목록이 있으면 그 목록만 적용한다. 앱이 판정하지 못하는 형식이면 막는 쪽으로 다뤄야 한다(MUST).

#### Scenario: 사용자 설정이 막는다
- **WHEN** 관리 설정이 없고 사용자 settings `availableModels`가 `["sonnet"]`이다
- **THEN** `opus` 항목은 비활성이고 "설정의 허용 목록에 없는 모델이에요"가 붙는다

#### Scenario: 관리 설정이 막는다
- **WHEN** 관리 설정 `availableModels`가 `["sonnet", "haiku"]`이고 사용자가 `claude-opus-5-5`를 직접 입력한다
- **THEN** 시작을 거부하고 "관리 설정이 이 모델을 막아요" 한 줄을 보인다. CLI는 실행하지 않는다

#### Scenario: 저장된 값이 손상됐다
- **WHEN** 팀 설정 DB의 유형 모델 값이 손으로 `opus x`로 바뀌었다
- **THEN** 그 유형으로 시작을 거부하고 "저장된 모델 이름이 올바르지 않아요"를 보인다. `--model` 없이 조용히 시작하지 않는다

#### Scenario: 빈 목록
- **WHEN** 관리 설정 `availableModels`가 `[]`다
- **THEN** 기본값만 고를 수 있다

### Requirement: 비용 안내
고른 모델의 실제 id를 알고, 앱에 든 단가표에서 그 id의 출력 단가가 그 자리의 기본값(Composer는 작업 값을 뺀 에이전트·유형·CLI 기본값, 설정 화면은 CLI 기본값)보다 높으면 고르기 아래에 한 줄로 알려야 한다(SHALL). 단가는 앱에 든 표나 사용자가 덮어쓴 로컬 표에서만 가져와야 한다(SHALL). 원격으로 가져와서는 안 된다(MUST NOT). 실제 id나 단가를 모르면 비용 줄을 보이지 않는다.

#### Scenario: 더 비싼 모델
- **WHEN** 기본값이 `claude-sonnet-5-5`이고 사용자가 실제 id가 `claude-opus-5-5`로 알려진 `opus`를 고른다
- **THEN** "기본값보다 토큰당 약 N배 비싸요 · 앱 단가표 기준" 한 줄이 보인다. N은 두 출력 단가의 비율을 소수 첫째 자리에서 반올림한 값이다

#### Scenario: 유형 기본값이 기준이다
- **WHEN** Composer에서 유형 기본값이 `claude-opus-5-5`이고 사용자가 `claude-opus-5-5`를 고른다
- **THEN** 비용 줄이 없다

#### Scenario: 제공자 앞부분
- **WHEN** 사용자가 `us.anthropic.claude-opus-5-5`를 직접 입력하고 기본값이 `claude-sonnet-5-5`다
- **THEN** 단가는 `claude-opus-5-5`로 찾아 비용 줄을 보인다

#### Scenario: 단가를 모르는 모델
- **WHEN** 사용자가 단가표에 없는 id를 직접 입력한다
- **THEN** 비용 줄이 없다

### Requirement: 이어 하기와 재작업은 원래 모델을 쓴다
앱은 시작 때 모델의 층과 실제로 넘긴 `--model` 값·역할 `model` 값을 작업 기록에 남겨야 한다(SHALL). 한도 뒤 재개와 "같은 세션에서 이어서" 재작업은 층을 다시 계산하지 않고 그 인자를 그대로 재현해야 한다(SHALL). "새 작업으로"는 층이 작업일 때만 모델 칸을 채운다(SHALL). 재개 직전에도 허용 목록을 다시 검사한다(SHALL).

#### Scenario: 한도 뒤 재개
- **WHEN** `--model=haiku`로 시작한 작업이 한도에 닿았다가 재개된다
- **THEN** 재개 인자에 `--model=haiku`가 있다

#### Scenario: 유형 기본값으로 돈 작업의 재개
- **WHEN** 유형 기본값 `sonnet`으로 시작한 작업이 한도에 닿았고, 그 사이 유형 기본값이 `opus`로 바뀌었다
- **THEN** 재개 인자와 역할 JSON `model`은 시작 때와 같은 `sonnet`이다. 파일 에이전트 잠금으로 재개가 막히지 않는다

#### Scenario: 에이전트 층 작업의 새 작업 재작업
- **WHEN** 파일 에이전트 `model: opus`로 돈 작업(층 = 에이전트)에서 "새 작업으로" 재작업을 연다
- **THEN** Composer 모델 칸은 기본값이고 잠기지 않는다

#### Scenario: 그 사이 허용 목록이 바뀌었다
- **WHEN** 재개 전에 관리 설정이 `haiku`를 막게 바뀌었다
- **THEN** 자동 재개를 하지 않고, 버튼으로만 잇게 한다. 이유는 기존 재개 이유 "위험 설정이 바뀜"(`risk_changed`)으로 보인다. 모델 전용 문구는 후속이다

### Requirement: 요청과 다른 모델 알림
작업이 시작된 뒤 수집한 기록에서 그 세션 메인 대화의 첫 모델 id(`<synthetic>` 제외)가 요청한 값과 계열이 다르면 작업 상세에 한 줄로 알려야 한다(SHALL). 계열은 opus·sonnet·haiku·fable 이름이나 id 앞부분으로 비교한다. 비교할 수 없으면 아무것도 보이지 않는다.

#### Scenario: CLI가 기본 모델로 바꿨다
- **WHEN** `--model=opus`로 시작했는데 그 세션 메인 대화의 첫 기록 모델이 `claude-sonnet-5-5`다
- **THEN** 작업 상세에 "요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요" 한 줄이 보인다

### Requirement: 지원되지 않는 모델은 자동으로 대체한다
작업·유형·재개 기록에서 정해진 모델이 지금 지원되지 않으면, 앱은 시작을 실패시키지 않고 대체 모델로 시작해야 한다(SHALL). "지원됨"은 다음 중 하나다: 계열 별칭(`opus`·`sonnet`·`haiku`·`fable`, `[1m]` 포함), 문서에 있는 별칭(`best`·`opusplan`), 지금 `claude --help`의 별칭, 설정의 `model`·`modelOverrides` 키·정해진 env 값, 최근 7일 기록의 모델, 앱 단가표에 있는 id(접두사와 같거나 뒤에 `-YYYYMMDD` 날짜만 붙은 것). `availableModels` 항목은 지원 증거가 아니다. 이 밖의 값은 "지원되지 않음"이다.

대체 순서는 다음과 같다(SHALL).
1. 같은 계열 별칭. 계열은 별칭 이름이나 id 안의 `opus`·`sonnet`·`haiku`·`fable` 낱말로 정한다. `[1m]`은 떼고 넘긴다.
2. 계열을 정할 수 없으면 기본값(`--model`을 넣지 않음).
3. 요청한 계열보다 비싼 계열(haiku < sonnet < opus < fable)로 바꾸어서는 안 된다(MUST NOT). 같은 계열 별칭이 허용 목록에 막혀 기본값으로 가야 하는데, 기본값의 계열을 모르거나 더 비싸면 시작을 거부한다.
4. 대체 모델도 허용 목록을 안내 문구 없이 통과해야 한다(MUST). 요청한 값이 허용 목록에 막혔으면 대체하지 않고 지금처럼 거부한다(MUST). 대체는 정책을 돌아가는 길이 아니다.

에이전트 파일의 `model:`이 지원되지 않으면, `--agent`와 `--model`의 우선순위가 확인되기 전(design 1.1)에는 시작을 거부하고 고칠 곳을 알려야 한다(SHALL). CLI가 시작 때 모델을 거부한 경우, 실측(1.2) 전에는 다시 시작하지 않고 실패 메시지에 안내 한 줄만 더한다(SHALL).

앱은 요청한 값, 실제로 넘긴 값, 이유를 작업 기록에 남겨야 한다(SHALL). 재개는 실제로 넘긴 값을 재현하고, 그 값이 그 사이 지원되지 않게 됐으면 같은 규칙으로 다시 대체해야 한다(SHALL).

#### Scenario: 사라진 전체 id는 같은 계열 별칭으로
- **WHEN** 유형 기본 모델이 `claude-opus-3-9`이고, 그 id가 CLI 도움말·설정·최근 기록·단가표 어디에도 없다
- **THEN** 시작 인자와 역할 JSON `model`이 `opus`다. 작업 기록에 요청 `claude-opus-3-9`, 실제 `opus`, 이유 `unsupported`가 남는다

#### Scenario: 계열을 모르는 값은 기본값으로
- **WHEN** 작업에서 직접 입력한 `acme-large-2`가 어디에도 없다
- **THEN** `--model`이 없고 역할 JSON에도 `model`이 없다. 기록의 실제 값은 기본값이다

#### Scenario: 지원되는 값은 그대로
- **WHEN** 작업 값이 `claude-haiku-4-5-20251001`이고 단가표에 `claude-haiku-4-5`가 있다
- **THEN** `--model=claude-haiku-4-5-20251001` 그대로다. 대체 기록이 없다

#### Scenario: 정책에 막힌 값은 대체하지 않는다
- **WHEN** 관리 설정 `availableModels`가 `["sonnet"]`이고 작업 값이 지원되지 않는 `claude-opus-3-9`다
- **THEN** 시작을 거부하고 "관리 설정이 이 모델을 막아요"를 보인다. `opus`나 기본값으로 시작하지 않는다

#### Scenario: 대체 별칭이 버전으로 좁혀지고 기본값이 더 비싸다
- **WHEN** 작업 값이 지원되지 않는 `claude-haiku-3-1`이고, 사용자 설정 `availableModels`가 `["claude-haiku-3-1", "opus"]`이고, 기본값이 `opus`다
- **THEN** `haiku`는 `claude-haiku-3-1`로 좁혀지므로 대체 대상이 아니다. 기본값은 더 비싼 계열이다. 시작을 거부하고 "대신 쓸 모델이 없어 시작하지 않았어요 · 더 비싼 모델로는 바꾸지 않아요"를 보인다

#### Scenario: CLI가 시작 때 거부했다
- **WHEN** `claude --bg`가 실패하고 stderr에 그 모델 이름과 "model … not found" 같은 거부 문구가 있다
- **THEN** 다시 시작하지 않는다. 실패 메시지 끝에 "이 모델을 쓸 수 없는 것 같아요 · 다른 모델을 골라 주세요" 한 줄이 붙는다. stderr에 `policy`·`organization`·`availableModels` 같은 낱말이 있으면 그 줄을 붙이지 않는다

#### Scenario: 에이전트 파일의 사라진 모델
- **WHEN** 파일 에이전트의 `model:`이 지원되지 않는 `claude-sonnet-3-9`이고 작업 값이 없다
- **THEN** 시작을 거부하고 "에이전트 파일의 모델을 쓸 수 없어요 · 에이전트 설정에서 바꿔 주세요"를 보인다

#### Scenario: 재개 때 기록된 모델이 사라졌다
- **WHEN** `--model=claude-opus-4-1`로 시작한 작업을 재개하는데 그 사이 그 id가 지원되지 않게 됐다
- **THEN** 재개 인자에 `--model=opus`가 있고, 기록에 대체가 남는다

#### Scenario: 작업 상세 알림
- **WHEN** 작업 기록에 요청 `claude-opus-3-9`, 실제 `opus`가 있다
- **THEN** 작업 상세에 "요청한 claude-opus-3-9를 쓸 수 없어 opus로 실행했어요" 한 줄이 보인다. 실제가 기본값이면 "…기본값으로 실행했어요"다

#### Scenario: 설정 화면 표시
- **WHEN** 유형 상세에 저장된 모델이 지원되지 않는 `claude-opus-3-9`다
- **THEN** 모델 고르기 아래에 "지원되지 않는 모델 · opus로 대체돼요" 한 줄이 보인다. 대체할 모델이 없으면 "지원되지 않는 모델 · 대신 쓸 모델이 없어요"다
