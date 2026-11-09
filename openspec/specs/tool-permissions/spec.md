# tool-permissions Specification

## Purpose
Claude Code 도구 권한을 전역·프로젝트·에이전트 범위에서 한곳에서 보고 고치며, 특정 호출이 왜 허용·거부되는지 설명한다.

## Requirements

### Requirement: 범위별 규칙 보기

권한 탭은 전역, 프로젝트(공유·개인), 에이전트 범위의 allow·ask·deny 규칙과 기본 모드를 보여야 한다(SHALL). 관리자 설정이 있으면 읽기 전용으로 함께 보여야 한다(SHALL). 설정 파일의 `permissions` 밖 키 값은 읽어 보여 주거나 저장하지 않는다(MUST NOT).

#### Scenario: 프로젝트 고르기
- **WHEN** 사용자가 프로젝트 dev_team을 고른다
- **THEN** 그 프로젝트의 공유·개인 규칙이 출처 파일과 함께 보이고, 파일이 없으면 "없음(저장하면 만듭니다)"이 보인다

### Requirement: 규칙 편집

사용자는 규칙을 추가·삭제하고 allow·ask·deny 사이를 옮길 수 있어야 한다(SHALL). 규칙 문법(`Tool`, `Tool(pattern)`, `mcp__server__tool`)이 틀리면 저장되지 않아야 한다(MUST NOT). 저장 전 네이티브 확인 창에 파일 경로와 바뀌는 규칙을 보여야 하고(SHALL), 파일이 그사이 바뀌었으면 저장을 거부하고 다시 불러오게 해야 한다(SHALL). 저장은 다른 키를 그대로 두고 이전 파일을 백업해야 한다(SHALL).

#### Scenario: 다른 키 보존
- **WHEN** 전역 settings에 hooks·env가 있고 사용자가 deny 규칙 하나를 추가해 저장한다
- **THEN** 파일의 hooks·env는 그대로이고 permissions.deny에만 규칙이 더해진다

#### Scenario: 충돌
- **WHEN** 편집하는 동안 Claude Code가 같은 파일을 고쳤다
- **THEN** 저장이 거부되고 "파일이 바뀌었습니다. 다시 불러오세요"가 보인다

### Requirement: 에이전트 도구 제한

에이전트 범위에서 `tools`(허용 목록)와 `disallowedTools`를 고칠 수 있어야 하며(SHALL), 플러그인 에이전트는 읽기 전용이어야 한다(SHALL).

#### Scenario: 리뷰 에이전트에서 쓰기 막기
- **WHEN** 사용자가 reviewer 에이전트의 disallowedTools에 Edit, Write를 넣고 저장한다
- **THEN** 정의 파일 frontmatter에만 반영되고 본문은 그대로다

### Requirement: 실효 확인

사용자가 프로젝트·에이전트·도구 호출을 넣으면 앱은 결과(허용/확인/거부/모드에 따름)와 결정한 규칙·범위를 보여야 한다(SHALL). 모든 범위의 규칙을 합친 뒤 deny → ask → allow 순으로 판단하며(어느 범위든 deny가 이김), 관리자 설정이 관리자 규칙만 허용하면 다른 범위는 무시한다. 결과는 참고용이며 권한 모드·훅·CLI 플래그는 반영하지 않는다고 표시한다(SHALL).

#### Scenario: 거부 규칙 우선
- **WHEN** 전역 allow에 `Bash(git:*)`, 프로젝트 deny에 `Bash(git push:*)`가 있고 `Bash(git push origin main)`을 확인한다
- **THEN** "거부 — 프로젝트(공유) deny `Bash(git push:*)`"가 보인다

### Requirement: 권한 카탈로그와 위험도

권한 탭은 자주 쓰는 규칙 목록을 위험도(높음·중간·낮음), 설명, 일반 권장값과 함께 보여야 한다(SHALL). 사용자는 각 항목을 선택한 범위에서 허용·확인·거부·없음 중 하나로 토글할 수 있어야 하고(SHALL), 토글 결과는 해당 범위 settings의 `permissions` JSON에 반영될 규칙으로 즉시 미리보기되어야 한다(SHALL). 카탈로그에 없는 기존 규칙은 "직접 추가한 규칙"으로 따로 보인다(SHALL).

#### Scenario: 위험한 항목 허용
- **WHEN** 사용자가 위험도 "높음"인 `Bash(git push:*)`를 전역 "허용"으로 토글한다
- **THEN** JSON 미리보기의 allow에 규칙이 생기고, 저장 확인 창에 "위험도 높음 규칙을 허용합니다" 경고가 함께 보인다

#### Scenario: 권장값 적용
- **WHEN** 사용자가 프로젝트 범위에서 "권장값 적용"을 누른다
- **THEN** 추가·변경·제거될 규칙 목록이 보이고, 확인하면 그대로 미리보기에 반영되며 저장 전까지 파일은 바뀌지 않는다

### Requirement: permissions JSON 직접 편집

사용자는 선택한 범위의 `permissions` JSON을 직접 고칠 수 있어야 한다(SHALL). JSON 문법·규칙 형식이 틀리면 저장하지 않고 위치와 이유를 보여야 하며(SHALL), 편집기에는 `permissions` 밖의 키가 나타나지 않아야 한다(MUST NOT).

#### Scenario: 잘못된 JSON
- **WHEN** 사용자가 쉼표를 빠뜨린 JSON으로 저장을 누른다
- **THEN** 저장되지 않고 오류 줄·이유가 보이며 토글 화면은 마지막 올바른 상태를 유지한다
