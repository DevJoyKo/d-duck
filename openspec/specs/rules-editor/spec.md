# rules-editor Specification

## Purpose
TBD - created by archiving change rules-editor. Update Purpose after archive.

## Requirements

### Requirement: 규칙 목록
메모리 탭의 "규칙"은 전역(`~/.claude/rules/`)과 선택한 프로젝트(`<프로젝트>/.claude/rules/`)의 `.md` 규칙을 하위 폴더까지 보여야 하며(SHALL), 각 규칙의 이름, 하위 폴더, 적용 범위("항상" 또는 `paths` 패턴), 켜짐, git 추적 여부, 심볼릭 링크 여부를 보여야 한다(SHALL). 각 범위 머리에는 한 줄 설명만 둔다(SHALL).

#### Scenario: 프로젝트 규칙 보기
- **WHEN** 사용자가 dev_team을 고르고 "규칙"을 연다
- **THEN** `.claude/rules/` 아래 규칙들이 "항상" 또는 패턴 칩과 함께 보이고, 규칙이 없으면 "아직 규칙이 없어요 · 새 규칙" 안내가 보인다

### Requirement: 규칙 만들기·편집
사용자는 범위(전역/프로젝트)를 골라 새 규칙을 만들고(빈 규칙 또는 템플릿) 본문과 적용 범위 패턴을 고칠 수 있어야 한다(SHALL). 적용 범위는 칩으로 추가·삭제하며 비우면 "항상"이 된다(SHALL). 저장은 `paths` 외 다른 앞부분 필드와 본문 바이트를 보존해야 하고(SHALL), 그사이 파일이 바뀌었으면 거부해야 한다(SHALL). 파일 이름은 영문·숫자·`-`·`_`와 하위 폴더 한 단계까지만 허용한다(SHALL). 패턴에 잘못된 `[`가 있거나 중괄호 확장이 1,000개를 넘으면 경고해야 한다(SHALL).

#### Scenario: 경로 한정 규칙 만들기
- **WHEN** 사용자가 프로젝트 범위에서 템플릿 "API 규칙"으로 만들고 적용 범위에 `src/api/**/*.ts`를 넣어 저장한다
- **THEN** `.claude/rules/api.md`가 `paths` 앞부분과 함께 생기고 목록에 패턴 칩이 보인다

#### Scenario: 충돌
- **WHEN** 편집 중 Claude가 같은 규칙 파일을 고쳤다
- **THEN** 저장이 거부되고 "파일이 바뀌었어요. 다시 불러오세요"가 보인다

### Requirement: 끄기·켜기·삭제
규칙을 끄면 파일은 그대로 두고 Claude Code의 `claudeMdExcludes`에 그 규칙의 절대 경로를 추가해야 한다(SHALL): 프로젝트 규칙은 `<프로젝트>/.claude/settings.local.json`, 전역 규칙은 `~/.claude/settings.json`. 켜면 앱이 넣은 그 항목만 빼야 한다(SHALL). 사용자가 직접 넣은 다른 제외 패턴에 걸린 규칙은 "다른 설정에 의해 꺼짐"으로 읽기 전용 표시해야 한다(SHALL). 삭제는 네이티브 확인 후 휴지통으로 보내야 한다(SHALL).

#### Scenario: 규칙 잠시 끄기
- **WHEN** 사용자가 git이 추적하는 프로젝트 규칙 `testing.md`를 끈다
- **THEN** 파일은 그대로이고 `.claude/settings.local.json`의 `claudeMdExcludes`에 그 절대 경로가 추가되며 목록에 "꺼짐"으로 보이고, 켜면 그 항목만 빠진다

### Requirement: 안전한 쓰기
앱은 규칙 폴더 밖, 심볼릭 링크·하드 링크 파일, 링크된 폴더 아래에는 쓰지 않아야 한다(MUST NOT). 링크 규칙은 본문 편집·삭제를 막고 "공유 링크 · 원본에서 편집"을 표시하되 끄기·켜기는 허용한다(SHALL). 앞부분 형식이 단순하지 않으면 적용 범위를 읽기 전용으로 두고, 앞부분이 깨졌으면 "앞부분 오류 · 항상 적용 중"을 표시해야 한다(SHALL). git이 추적하는 프로젝트 규칙을 고칠 때는 "저장소에 커밋되는 파일" 배지를 보여야 한다(SHALL). 규칙 본문은 DB·로그에 저장하지 않는다(MUST NOT).

#### Scenario: 공유 링크 규칙
- **WHEN** 규칙 파일이 다른 곳을 가리키는 심볼릭 링크다
- **THEN** 편집·삭제가 비활성화되고(끄기는 가능) "공유 링크 · 원본에서 편집"이 보인다
