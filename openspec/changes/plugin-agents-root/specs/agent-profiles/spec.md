## MODIFIED Requirements

### Requirement: Agent 목록
앱은 사용자 agent(`<설정 디렉터리>/agents/*.md`), 프로젝트 agent(기본 작업 폴더·실행 중인 세션 폴더·루틴 폴더의 `.claude/agents/*.md`), 플러그인 agent, 내장 agent(general-purpose, Explore, Plan, claude-code-guide, statusline-setup)를 모아 보여야 한다(SHALL). 플러그인 agent는 Claude Code가 읽는 곳에서만 찾아야 한다(SHALL): 플러그인 설치 폴더의 `agents/*.md`와 `plugin.json`의 `agents`에 적힌 `.md` 파일(설치 폴더 안의 상대 경로, 링크가 아닌 일반 파일). `plugin.json` `agents`에 적힌 폴더, `..`나 절대 경로, 그 밖의 위치에 있는 `agents` 이름 폴더(예: `docs/agents/`)는 읽지 않아야 한다(SHALL). 설치 폴더는 `<설정 디렉터리>/plugins/installed_plugins.json`에 기록된 `installPath`(그 플러그인의 캐시 폴더 안, 링크 없음; 기록이 여럿이면 모두)를 쓰고, 쓸 수 있는 기록이 없을 때만 `<설정 디렉터리>/plugins/cache/<마켓>/<플러그인>/` 아래 가장 최근에 바뀐 버전 폴더를 쓴다(SHALL). 플러그인 탭과 스킬 탭도 같은 규칙을 쓴다. 각 항목은 이름, 출처, 설명, 모델, 도구 요약을 가진다. frontmatter가 없거나 깨진 파일은 파일 이름을 이름으로 쓰고 "형식 오류"로 표시해야 한다(SHALL). 같은 이름이 여러 출처에 있으면 프로젝트 > 사용자 > 플러그인 순으로 앞선 것을 대표로 두고 나머지도 출처와 함께 보여야 한다(SHALL). 목록 맨 앞에는 "기본 Claude" 카드가 있어야 한다(SHALL).

#### Scenario: 프로젝트 agent 표시
- **WHEN** 기본 작업 폴더에 `.claude/agents/design-critic.md`가 있다
- **THEN** "design-critic" 카드가 출처 "프로젝트 · dev_team"과 frontmatter 설명으로 보인다

#### Scenario: 깨진 정의
- **WHEN** `~/.claude/agents/x.md`에 frontmatter가 없다
- **THEN** "x" 카드가 "형식 오류" 표시와 함께 보이고 편집할 수 있다

#### Scenario: 문서 폴더는 에이전트가 아님
- **WHEN** 플러그인에 `docs/agents/triage-labels.md`(앞부분 없음)만 있고 루트 `agents/`가 없다
- **THEN** 에이전트 탭에 그 플러그인의 에이전트가 없고 "형식 오류" 카드도 없다

#### Scenario: plugin.json에 적힌 파일
- **WHEN** `plugin.json`이 `"agents": ["./extra/helper.md", "./custom/agents", "../x.md"]`를 적었다
- **THEN** 루트 `agents/`의 에이전트와 함께 `helper`만 더 보이고, 폴더와 밖을 가리키는 경로는 무시된다

#### Scenario: 설치 기록이 버전을 정함
- **WHEN** `installed_plugins.json`의 `installPath`가 `1.3.1`이고 같은 플러그인의 다른 버전 폴더가 더 최근에 바뀌었다
- **THEN** `1.3.1` 폴더의 에이전트만 보인다

#### Scenario: 설치 기록이 없음
- **WHEN** 플러그인에 설치 기록이 없고 버전 폴더가 둘 있다
- **THEN** 가장 최근에 바뀐 버전 폴더의 에이전트만 보인다
