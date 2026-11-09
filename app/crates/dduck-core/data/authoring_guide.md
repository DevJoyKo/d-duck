# Claude Code 스킬·커맨드·워크플로 만들기 지침

## 공통
- 요청한 것 하나만 만든다. 이름은 영문 소문자·숫자·`-`로 짓고 64자 이하로 한다.
- 지정한 폴더 밖의 파일은 만들거나 고치지 않는다. 이미 있는 파일은 고치지 않는다.
- 설치되어 있으면 `skill-creator` 스킬(스킬)과 `workflow-authoring` 스킬(워크플로)의 지침을 따른다.
- 비밀값·토큰·개인 경로를 파일에 넣지 않는다.

## 스킬 (`skills/<이름>/SKILL.md`)
- 맨 위 frontmatter에 `name`(폴더 이름과 같게)과 `description`을 쓴다. description은 "언제 쓰는지"를 한 문장으로, 사용자가 실제로 쓸 법한 말(트리거 문구)을 넣는다.
- 본문은 짧게: 언제 쓰나 → 절차 → 하지 말 것. 길어지는 세부는 같은 폴더의 보조 .md 파일로 나누고 본문에서 이름으로 가리킨다.
- 스크립트가 필요하면 `scripts/` 아래에 두고 본문에서 언제 실행하는지 적는다.
- `allowed-tools`, `hooks`, 느낌표와 백틱으로 셸 명령을 실행하는 줄은 사용자가 따로 요청하지 않으면 넣지 않는다.

## 슬래시 커맨드 (`commands/<이름>.md`)
- frontmatter에 `description`과 필요하면 `argument-hint`를 쓴다.
- 사용자가 넘긴 인자는 본문에서 `$ARGUMENTS`(또는 `$1`, `$2`)로 받는다.
- 본문은 Claude에게 주는 지시문이다. 할 일을 순서대로 분명하게 쓴다.

## 워크플로 (`workflows/<이름>.js`)
- 파일의 첫 문장은 `export const meta = { name, description, phases }`이고 순수 객체 리터럴이다(변수·함수 호출·템플릿 문자열 금지).
- `meta.name`은 파일 이름(확장자 제외)과 같아야 한다.
- 단계마다 `phase('제목')`을 부르고, 제목은 `meta.phases`의 `title`과 글자 하나까지 같아야 한다.
- 일은 `agent(프롬프트, { label, phase, schema })`로 맡긴다. 프롬프트에는 그 에이전트가 알아야 할 맥락을 모두 넣는다(다른 에이전트의 대화를 볼 수 없다). 구조화된 결과가 필요하면 `schema`(JSON Schema)를 준다.
- 여러 항목은 `pipeline()`을 기본으로, 모든 결과가 함께 필요할 때만 `parallel()`을 쓴다.
- 스크립트는 TypeScript가 아닌 일반 JavaScript다. `Date.now()`, `Math.random()`, 인자 없는 `new Date()`는 쓸 수 없다. 파일 시스템·Node API도 없다.
- 워크플로는 실행하면 에이전트를 많이 쓸 수 있으니 description에 하는 일을 정확히 적는다.
