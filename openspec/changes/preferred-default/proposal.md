# "먼저 살펴볼 스킬·에이전트" 기본값

## Why

사용자 요청: Matt Pocock의 스킬을 "먼저 살펴볼 스킬·에이전트"의 기본값으로 둔다. 지금(preferred-skills, D-198)은 기본값이 빈 목록이라, 저장된 목록이 곧 전체 값이다. 기본값을 두려면 "한 번도 정하지 않음(기본값을 씀)"과 "사용자가 비움(빈 목록 유지)"을 구분해야 한다.

## What Changes

- 번들 기본값(`data/team_defaults.json`의 `preferred`)이 플러그인 하나가 된다: `{kind: plugin, name: mattpocock-skills, source: plugin:mattpocock-skills}`(마켓플레이스 `claude-plugins-official`).
- 저장값 `Stored.preferred`가 `Option`이 된다. 없음 = 기본값, `[]` = 사용자가 비움. 다른 팀 설정 항목처럼 기본값과 같은 목록은 저장하지 않는다.
- "기본값으로"(이 칸)와 "전체 기본값으로"는 기본값(mattpocock-skills)으로 돌아간다. 팀 규칙 탭의 이 칸에 "기본값으로" 버튼을 둔다(목록이 기본값과 다를 때만).
- 시작할 때는 지금처럼 설치된 항목만 넣는다. 플러그인이 없는 사용자는 시스템 지시가 그대로이고, 설정 화면에는 "설치되지 않아 빠져요"가 보인다.
- README(영어·한국어) 가드레일 절의 이 설정 설명에 기본값을 적는다.

## Capabilities

### Modified Capabilities
- `preferred-skills`: 기본값과 저장 의미.

## Impact

- core: `routines`(기본값이면 루틴도 카탈로그를 읽음, 테스트 갱신), `team_config`(Bundle·Stored·TeamInput·merge·diff·reset_item), `data/team_defaults.json`, 테스트.
- webview: `PreferredSection` "기본값으로", `AgentSettings`가 기본값 전달, mock 기본값·초기화, i18n, vitest.
- 문서: README.md·README.ko.md.
- 이전 저장값: 지금까지 빈 목록은 저장하지 않았으므로(`skip_serializing_if = "Vec::is_empty"`) 모두 "정하지 않음"으로 읽혀 기본값을 받는다.
- 아카이브 순서: `preferred-skills`를 먼저 아카이브해야 이 MODIFIED 델타가 적용된다.
