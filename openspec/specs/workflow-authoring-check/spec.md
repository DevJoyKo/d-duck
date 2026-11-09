# workflow-authoring-check Specification

## Purpose
앱에서 만들거나 목록에 보이는 워크플로가 Claude Code에 실제로 등록되는지 사용자가 저장 전과 목록에서 알 수 있게 한다.

## Requirements

### Requirement: Claude Code와 같은 meta 규칙
워크플로 파일은 첫 문장이 `export const meta = { … }`이고, 그 값이 순수 리터럴(문자열·숫자·불리언·null·배열·객체, 보간·스프레드·함수 호출·식별자·타입 주석 없음)이며, `name`과 `description`이 빈 문자열이 아닌 문자열로 있을 때만 유효하다(SHALL). 음수 숫자는 받고, 빈 칸 배열과 예약 키(`__proto__` 등)는 거부한다(SHALL). 이 검사는 meta 부분만 대상으로 하며, 본문 스크립트의 문법 오류는 확인하지 않는다(범위 밖). 앱은 유효하지 않은 워크플로를 저장하지 않아야 하고(MUST NOT), 올바른 문자열 이스케이프는 받아야 한다(SHALL). 검사는 파일을 실행하지 않는다(MUST NOT).

#### Scenario: 템플릿 보간
- **WHEN** meta의 description에 `${x}`가 들어간 템플릿 문자열을 쓰고 저장한다
- **THEN** 저장되지 않고 "meta에 템플릿 보간을 쓸 수 없어요 · 보간을 빼고 따옴표 문자열로 써 주세요"처럼 이유와 고칠 방법이 입력 중에 보인다

#### Scenario: 괄호와 이스케이프 키
- **WHEN** meta가 `({ name: ('triage'), \u0064escription: 'd' })`이다
- **THEN** Claude Code처럼 괄호를 벗기고 키를 디코딩해 유효로 본다

#### Scenario: 이스케이프
- **WHEN** description이 `'it\'s fine'`이다
- **THEN** 유효로 보고 저장된다

### Requirement: 이름 일치 즉시 확인
만들기 화면에서 본문의 `meta.name`이 이름 칸과 다르면 입력하는 동안 이유를 보여 주고 저장 버튼을 꺼야 한다(SHALL). 고치기 화면에서 기존 파일의 `meta.name`이 파일 이름과 다르면 저장은 허용하고, Claude Code가 등록하는 이름을 경고로 보여 줘야 한다(SHALL).

#### Scenario: 이름만 바꿈
- **WHEN** 만들기 화면에서 본문을 고친 뒤 이름 칸을 `triage`에서 `triage-v2`로 바꾼다
- **THEN** 저장 버튼이 꺼지고 "meta.name을 triage-v2로 맞춰 주세요"가 보인다

#### Scenario: 기존 파일의 다른 meta.name
- **WHEN** 고치기 화면에서 `triage.js`의 meta.name을 `plan`으로 바꾼다
- **THEN** 저장 버튼은 켜져 있고 "Claude Code에는 plan으로 등록돼요"가 보이며, 저장하면 그대로 저장된다

### Requirement: 목록에 못 읽는 워크플로 표시
확장 목록은 Claude Code가 읽지 못하는 워크플로(앱이 만들지 않은 파일 포함)에 "Claude Code가 못 읽어요" 표시와 이유 한 줄을 붙여야 한다(SHALL).

#### Scenario: 손으로 만든 파일
- **WHEN** `~/.claude/workflows/x.js`의 meta에 함수 호출이 있다
- **THEN** 목록의 x 항목에 표시와 "meta 값은 리터럴이어야 해요 · 변수·함수 호출 대신 값을 직접 써 주세요"가 보인다

#### Scenario: 등록 이름이 다른 파일
- **WHEN** `~/.claude/workflows/notes.js`의 meta.name이 `sync-notes`다
- **THEN** 목록의 notes 항목에 "등록 이름 다름" 배지와 "Claude Code에는 sync-notes로 등록돼요"가 보인다
