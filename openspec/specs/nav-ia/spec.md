# nav-ia Specification

## Purpose
TBD - created by archiving change nav-ia. Update Purpose after archive.

## Requirements

### Requirement: 확장 탭
스킬·커맨드·워크플로·플러그인은 "확장" 탭 하나에서 보여야 한다(SHALL). 필터 칩으로 종류를 좁히고 이름으로 검색할 수 있어야 하며(SHALL), 각 항목은 출처(내 것·프로젝트·플러그인 이름)를 보여야 한다(SHALL). 플러그인을 펼치면 그 플러그인이 주는 스킬·커맨드·MCP가 보여야 한다(SHALL). 기존 켜기/끄기·업데이트·링크 설치·작성·편집은 그대로 동작해야 한다(SHALL).

#### Scenario: 플러그인이 준 스킬 찾기
- **WHEN** 사용자가 확장 탭에서 "스킬" 칩을 누르고 "design"을 검색한다
- **THEN** 내 스킬과 플러그인이 준 스킬이 함께 나오고 플러그인 것은 플러그인 이름 배지가 붙는다

### Requirement: MCP 탭
사이드바에 "MCP" 탭이 있어야 하며(SHALL), 설정된 MCP 서버를 서비스 아이콘·이름·범위(전역·프로젝트·플러그인 제공)와 함께 목록으로 보여야 한다(SHALL). 상태 확인은 버튼을 눌렀을 때만 해야 한다(SHALL). 설정 화면에는 MCP 목록 대신 MCP 탭으로 가는 링크만 둔다(SHALL).

#### Scenario: MCP 바로 보기
- **WHEN** 사용자가 사이드바의 MCP를 누른다
- **THEN** 연결된 서버들이 아이콘과 함께 보이고 "상태 확인"을 누르면 연결 상태가 표시된다

### Requirement: 사이드바 묶음
사이드바는 "일"과 "Claude 구성" 두 묶음으로 나뉘어야 하고(SHALL), 사용 통계·설정은 그 아래, 현재 프로젝트 선택기는 맨 아래에 있어야 한다(SHALL).

#### Scenario: 사이드바 구성
- **WHEN** 앱을 연다
- **THEN** "일: 에이전트·작업·루틴·아티팩트", "Claude 구성: 확장·MCP·권한·메모리", 그 아래 사용 통계·설정, 맨 아래 현재 프로젝트가 보인다

### Requirement: 잠긴 항목 복사해서 내 것으로
플러그인·내장 등 읽기 전용(잠김) 스킬·커맨드·워크플로에는 "복사해서 내 것으로"가 있어야 한다(SHALL). 누르면 저장 위치(내 것 `~/.claude/…` 또는 현재 프로젝트 `.claude/…`)와 새 이름(기본 `<이름>-custom`, 겹치면 `-2`…)을 고르는 작은 확인이 뜨고, 확인하면 그 항목의 폴더 전체(보조 파일·LICENSE 포함)를 복사해 편집기로 바로 열어야 한다(SHALL). 원본은 바꾸지 않아야 하고(MUST NOT) 기존 항목을 덮어쓰지 않아야 한다(MUST NOT). 복사본에는 원본 출처가 남아야 한다(SHALL).

#### Scenario: 플러그인 스킬 고쳐 쓰기
- **WHEN** 사용자가 잠긴 플러그인 스킬 `frontend-design`에서 "복사해서 내 것으로"를 누르고 내 것·기본 이름으로 확인한다
- **THEN** `~/.claude/skills/frontend-design-custom/`이 생기고 편집기가 열리며, 목록에서 원본은 잠긴 채, 복사본은 "내 것 · frontend-design에서 복사"로 보인다
