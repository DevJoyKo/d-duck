# Proposal

## Why

워크플로 만들기 QA(2026-11-09, 근거: scratchpad/wfqa)에서 결함 2건이 나왔다.
1. **(MEDIUM) 앱 검사가 Claude Code보다 느슨하다.** Claude Code는 `meta`가 첫 문장이고 순수 리터럴이 아니면 그 워크플로를 건너뛴다. 그런데 앱은 정규식으로만 검사해서, Claude Code가 건너뛰는 파일도 저장하고 목록에 정상처럼 보여 준다. 예: 템플릿 보간, 스프레드, 함수 호출, 식별자 값, TS 타입 주석(`authoring.rs:156-176`). 반대로 `'it\'s'` 같은 올바른 이스케이프는 거부한다.
2. **(LOW) 이름을 바꿔도 저장 버튼이 켜져 있다.** 본문을 고친 뒤 이름을 바꾸면 `meta.name`은 옛 이름 그대로인데 저장 버튼이 켜져 있다. 오류는 저장할 때에야 나온다(`AddSheet.tsx:459`).

## What Changes

- **`meta` 검사를 Claude Code 규칙과 맞춘다.** 첫 문장이 `export const meta = {...}`인지, 값이 순수 리터럴(문자열·숫자·불리언·null·배열·객체)인지, `name`·`description`이 있는지, `phases` 제목이 문자열인지 본다. 작은 토큰화 파서로 검사하고 코드는 실행하지 않는다. 문자열 이스케이프는 올바르게 받는다.
- 확장 목록에서 Claude Code가 읽지 못하는 워크플로에 "Claude Code가 못 읽어요" 표시와 이유 한 줄을 단다. 앱이 만들지 않은 파일도 같다.
- 만들기·고치기 화면에서 본문의 `meta.name`이 이름과 다르거나 meta가 규칙에 어긋나면, 입력하는 동안 이유를 보여 주고 저장 버튼을 끈다.

## Capabilities

### New Capabilities
- `workflow-authoring-check`: 워크플로 meta 검사와 표시

### Modified Capabilities
- 없음(skill-authoring 메인 스펙의 저장 규칙을 좁힌다)

## Impact

- `crates/dduck-core/src/authoring.rs`(meta 파서), `skills_catalog.rs`(목록 표시)
- `app/src/components/AddSheet.tsx`·`AuthoringEditor.tsx`·`ExtensionsPage.tsx`, extensions 카탈로그 ko/en
- 테스트: QA probe의 경우(fncall, phvar, spread, tmpl, tsann, esc)를 Rust 테스트로
