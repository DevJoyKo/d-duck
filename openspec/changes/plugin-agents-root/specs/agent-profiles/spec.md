## ADDED Requirements

### Requirement: 플러그인 에이전트를 읽는 폴더
플러그인 에이전트는 Claude Code가 읽는 폴더에서만 찾아야 한다(SHALL): 플러그인 설치 폴더(가장 최근 버전)의 `agents/`와 `plugin.json`의 `agents` 경로(설치 폴더 안). 그 밖의 위치에 있는 `agents` 이름 폴더(예: `docs/agents/`)는 에이전트로 읽지 않는다(SHALL). 플러그인 탭의 에이전트 목록과 같은 규칙이다.

#### Scenario: 문서 폴더는 에이전트가 아님
- **WHEN** 플러그인에 `docs/agents/triage-labels.md`(앞부분 없음)만 있고 루트 `agents/`가 없다
- **THEN** 에이전트 탭에 그 플러그인의 에이전트가 없고 "형식 오류" 카드도 없다

#### Scenario: plugin.json에 적힌 경로
- **WHEN** `plugin.json`이 `"agents": ["./extra/agents"]`를 적고 그 폴더에 에이전트가 있다
- **THEN** 루트 `agents/`의 에이전트와 함께 그 에이전트도 보인다
