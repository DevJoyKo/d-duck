# agent-profiles Specification

## Purpose
로컬에 정의된 Claude agent를 Duck 캐릭터 카드로 보여 주고, 어떤 agent가 지금 일하는지와 그 상태를 표시하며, agent 정의 파일 편집과 작업 시작 시 페르소나·작업 유형 선택을 제공한다.

## Requirements

### Requirement: Agent 목록
앱은 사용자 agent(`<설정 디렉터리>/agents/*.md`), 프로젝트 agent(기본 작업 폴더·실행 중인 세션 폴더·루틴 폴더의 `.claude/agents/*.md`), 플러그인 agent(`<설정 디렉터리>/plugins/cache/**/agents/*.md`), 내장 agent(general-purpose, Explore, Plan, claude-code-guide, statusline-setup)를 모아 보여야 한다(SHALL). 각 항목은 이름, 출처, 설명, 모델, 도구 요약을 가진다. frontmatter가 없거나 깨진 파일은 파일 이름을 이름으로 쓰고 "형식 오류"로 표시해야 한다(SHALL). 같은 이름이 여러 출처에 있으면 프로젝트 > 사용자 > 플러그인 순으로 앞선 것을 대표로 두고 나머지도 출처와 함께 보여야 한다(SHALL). 목록 맨 앞에는 "기본 Claude" 카드가 있어야 한다(SHALL).

#### Scenario: 프로젝트 agent 표시
- **WHEN** 기본 작업 폴더에 `.claude/agents/design-critic.md`가 있다
- **THEN** "design-critic" 카드가 출처 "프로젝트 · dev_team"과 frontmatter 설명으로 보인다

#### Scenario: 깨진 정의
- **WHEN** `~/.claude/agents/x.md`에 frontmatter가 없다
- **THEN** "x" 카드가 "형식 오류" 표시와 함께 보이고 편집할 수 있다

### Requirement: 첫 페이지
사이드바 맨 위 메뉴는 "에이전트"여야 하고(SHALL), 저장된 페이지가 없을 때 앱은 이 페이지로 시작해야 한다(SHALL).

#### Scenario: 처음 실행
- **WHEN** 사용자가 앱을 처음 연다
- **THEN** 에이전트 페이지가 보이고 사이드바의 "에이전트"가 선택되어 있다

### Requirement: Duck 캐릭터와 상태 문장
각 카드는 Duck 캐릭터와 그 아래 한 줄 상태 문장을 보여야 한다(SHALL). 동작은 일하는 중 = 자판 두드리기, 답 기다림 = 손 들기, 실패 = 느낌표 팻말, 최근 끝남 = 체크 팻말, 쉬는 중 = 졸기다. 상태 문장은 서브에이전트 메타의 짧은 작업 설명(최대 60자, 제어 문자 제거) 또는 작업 이름을 쓰며 저장하지 않아야 한다(MUST NOT). 움직임 줄이기 설정에서는 정지 자세만 보여야 한다(SHALL).

#### Scenario: 움직임 줄이기
- **WHEN** macOS에서 동작 줄이기가 켜져 있다
- **THEN** Duck은 자세만 바뀌고 움직이지 않으며 상태 문장은 그대로 보인다

### Requirement: 일하는 상태 판정
카드 상태는 실행 중인 세션의 서브에이전트 메타(끝남 표시가 아직 없고 최근 5분 안에 기록이 바뀐 것)와 페르소나로 시작한 세션(D.Duck 작업 또는 transcript의 agent 설정)의 상태로 정해야 한다(SHALL). 우선순위는 답 기다림 > 실패 > 일하는 중 > 10분 안에 끝남 > 쉬는 중이며, 같은 agent의 동시 진행 수를 표시해야 한다(SHALL). 목록에 없는 agent 종류가 일하면 "기타" 카드로 보여야 한다(SHALL). 상태는 작업 목록 폴링(15초)마다 갱신한다.

#### Scenario: 서브에이전트가 일하는 중
- **WHEN** 실행 중인 세션에 `agentType: "design-critic"`이고 `finished` 키가 없으며 1분 전에 기록된 서브에이전트가 있다
- **THEN** design-critic 카드의 Duck이 자판을 두드리고, 아래에 "일하는 중 · <작업 설명>"이 보인다

#### Scenario: 페르소나 작업이 답을 기다림
- **WHEN** D.Duck이 `--agent qa-verifier`로 시작한 작업이 답 기다림 상태다
- **THEN** qa-verifier 카드의 Duck이 손을 들고 "답을 기다려요 · <작업 이름>"이 보이며, 카드를 누르면 작업 상세로 이동한다

#### Scenario: 중단된 흔적
- **WHEN** `finished` 키가 없는 서브에이전트 기록이 30분째 바뀌지 않았다
- **THEN** 그 agent 카드는 일하는 중으로 보이지 않는다

#### Scenario: 아무 일도 없음
- **WHEN** 어떤 세션도 그 agent를 쓰지 않는다
- **THEN** Duck이 졸고 "쉬는 중"이 보인다

### Requirement: 정의 저장 확인
저장 전에 백엔드가 네이티브 확인 창으로 파일 이름·경로, 바뀐 줄 수, 저장될 frontmatter 내용을 보여 줘야 한다(SHALL). 본문이나 한 줄짜리 `name`·`description` 값만 바뀐 경우가 아니면 "이 agent가 실행할 수 있는 일이 바뀔 수 있습니다" 경고를 더하고(SHALL), `hooks`·`mcpServers`·확인을 건너뛰는 권한이 들어 있으면 항상 표시해야 한다(SHALL). 승인 직후 쓰기 전에 파일이 바뀌지 않았는지 다시 확인해야 한다(SHALL).

#### Scenario: 들여쓴 hooks 변경
- **WHEN** 저장 내용에서 `hooks:` 아래 들여쓴 줄만 바뀌었다
- **THEN** 확인 창에 권한 경고와 새 frontmatter가 보인다

### Requirement: 정의 파일 편집
사용자·프로젝트 agent는 앱에서 정의 파일 전체를 열어 고치고 저장할 수 있어야 한다(SHALL). 웹뷰는 경로가 아니라 목록의 항목 ID로만 파일을 지정해야 하며(SHALL), 백엔드는 저장 직전에 그 파일이 탐색된 agent 디렉터리 안의 일반 파일이고 256KB 이하인지 다시 확인해야 한다(SHALL). 파일은 임시 파일에 쓴 뒤 바꿔치기하고 원래 권한을 유지해야 한다(SHALL). 연 뒤 밖에서 바뀌었으면 덮어쓰지 않고 알려야 한다(SHALL). 플러그인·내장 agent, 소유자가 다르거나 다른 사용자가 쓸 수 있는 파일, 심볼릭 링크 경로는 읽기 전용이다(SHALL).

#### Scenario: 설명 고치기
- **WHEN** 사용자가 design-critic의 설명을 고치고 저장을 눌러 확인 창에서 승인한다
- **THEN** 파일이 바뀌고 카드의 설명이 새 값으로 보인다

#### Scenario: 권한 필드 변경 경고
- **WHEN** 사용자가 `tools:` 줄을 바꿔 저장한다
- **THEN** 확인 창에 권한 변경 경고가 보이고, 취소하면 파일이 바뀌지 않는다

#### Scenario: 밖에서 바뀐 파일
- **WHEN** 편집기를 연 뒤 다른 편집기에서 같은 파일을 고쳤다
- **THEN** 저장하지 않고 "파일이 밖에서 바뀌었습니다. 다시 불러오세요"를 보인다

#### Scenario: 웹뷰가 임의 경로를 요청
- **WHEN** 웹뷰가 목록에 없는 ID나 경로 문자열로 저장을 요청한다
- **THEN** 백엔드가 거부하고 아무 파일도 쓰지 않는다

### Requirement: 작업 시작 옵션
새 백그라운드 작업 창은 작업 유형(개발·보안·배포·리서치·커뮤니케이션·검토·QA, 선택 안 함 가능)과 페르소나(기본 Claude 또는 agent 목록 중 하나)를 고를 수 있어야 한다(SHALL). 페르소나는 백엔드가 그 작업 폴더에서 쓸 수 있는 agent(사용자·플러그인·내장, 또는 그 폴더나 상위 폴더의 프로젝트 agent)인지 확인한 뒤 `--agent=<name>` 한 인자로 넘겨야 한다(SHALL). 그 agent 정의에 `hooks`·`mcpServers`·확인을 건너뛰는 권한이 있거나 읽을 수 없으면 위험 모드와 같은 네이티브 확인을 받아야 한다(SHALL). 작업 유형과 페르소나는 D.Duck DB에 작업 ID별로 저장하고, 작업 카드와 상세에 표시해야 한다(SHALL). 작업 유형은 Claude에 넘기는 지시를 바꾸지 않는다.

#### Scenario: 페르소나로 작업 시작
- **WHEN** 사용자가 페르소나 "qa-verifier", 유형 "QA"를 고르고 작업을 시작한다
- **THEN** `claude --bg ... --agent=qa-verifier`로 시작되고, 카드에 "QA" 태그와 "qa-verifier"가 보인다

#### Scenario: 목록에 없는 페르소나
- **WHEN** 웹뷰가 목록에 없는 agent 이름으로 시작을 요청한다
- **THEN** 작업을 시작하지 않고 오류를 표시한다

#### Scenario: 다른 폴더의 프로젝트 agent
- **WHEN** 작업 폴더가 side-project인데 dev_team 프로젝트에만 있는 design-critic을 페르소나로 요청한다
- **THEN** 작업을 시작하지 않고 "이 폴더에서 쓸 수 없는 agent입니다"를 표시하며, Composer 목록에도 보이지 않는다

#### Scenario: 위험한 설정을 가진 페르소나
- **WHEN** 고른 agent 정의에 `permissionMode: bypassPermissions`나 `hooks`가 있다
- **THEN** 앱이 네이티브 확인 창에 그 값을 보여 주고, 승인해야만 시작한다

### Requirement: 역할 색과 배치 순서
Duck 색은 agent의 역할(보안·QA·리뷰·배포·리서치·설계·문서·개발·범용)에 따라 정해야 하며 같은 역할은 같은 색이어야 한다(SHALL). 카드에는 역할 칩을 단다. 배치는 "이름순"과 "사용자 지정"(순서 편집) 중 고를 수 있어야 하고(SHALL), 일하는 agent는 항상 앞에 놓여야 한다(SHALL). 순서 편집 중에는 상태 변화로 카드가 움직이지 않는다. 선택한 순서는 이 기기에만 저장된다.

#### Scenario: 일하는 agent 앞으로
- **WHEN** 이름순 배치에서 qa-verifier가 일하기 시작한다
- **THEN** qa-verifier 카드가 기본 Claude 앞쪽 그룹으로 옮겨지고, 끝나면 이름 순서 자리로 돌아간다

### Requirement: 테마 전환
사이드바 상단의 버튼으로 시스템·밝음·어두움 테마를 차례로 바꿀 수 있어야 하고(SHALL), 선택은 다음 실행에도 유지되며 첫 화면부터 적용되어야 한다(SHALL). 저장소를 쓸 수 없으면 시스템 테마를 따른다.

#### Scenario: 어두운 테마
- **WHEN** 사용자가 테마 버튼을 두 번 누른다
- **THEN** 앱 전체가 어두운 테마가 되고, 앱을 다시 열어도 유지된다
