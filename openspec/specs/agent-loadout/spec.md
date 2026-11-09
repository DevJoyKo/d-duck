# agent-loadout Specification

## Purpose
TBD - created by archiving change agent-loadout. Update Purpose after archive.

## Requirements

### Requirement: 역할별 기본 장비
에이전트 설정에서 각 역할(작업 유형)과 편집 가능한 에이전트 정의마다 기본 스킬과 MCP 서버를 고를 수 있어야 한다(SHALL). 스킬은 시작할 때 미리 불러오는 목록이고 MCP는 그 에이전트가 쓸 서버 목록이다(고르지 않은 서버는 `disallowedTools`로 막는다). 플러그인을 고르면 그 플러그인의 스킬이 한 번에 담겨야 한다(SHALL). 미리 불러오는 스킬의 합계 크기를 보여야 한다(SHALL).

#### Scenario: 디자인 역할 장비
- **WHEN** 사용자가 디자인 유형에 frontend-design 스킬과 figma MCP를 고르고 저장한다
- **THEN** 다음에 디자인 유형으로 시작한 작업의 에이전트 정의에 `skills: [frontend-design]`과 figma를 뺀 서버마다 `disallowedTools: mcp__<서버>__*`가 들어가고 `mcpServers`는 들어가지 않는다

#### Scenario: 플러그인 묶음
- **WHEN** 사용자가 장비에서 플러그인 하나를 고른다
- **THEN** 그 플러그인이 주는 스킬이 스킬 목록에 추가되고 MCP 쪽에는 "플러그인 MCP는 플러그인을 켜면 항상 켜져요" 한 줄이 보인다

### Requirement: 안전한 저장
파일 에이전트는 frontmatter의 `skills`·`disallowedTools` 줄만 바꾸고 다른 바이트는 보존해야 한다(SHALL). 링크·하드링크 파일, 플러그인 에이전트에는 쓰지 않아야 하며(MUST NOT) 플러그인 에이전트는 "복사해서 내 것으로"를 제안해야 한다(SHALL). MCP는 이미 설정된 서버 이름 참조만 허용하고 인라인 서버 정의(명령·환경 변수)는 만들지 않아야 한다(MUST NOT). 목록에 없는 스킬·서버 이름은 경고를 보여야 한다(SHALL).

#### Scenario: 플러그인 에이전트
- **WHEN** 사용자가 플러그인이 준 에이전트의 장비를 열었다
- **THEN** 읽기 전용으로 보이고 "플러그인 에이전트는 MCP 지정이 무시돼요 · 복사해서 내 것으로" 안내가 있다
