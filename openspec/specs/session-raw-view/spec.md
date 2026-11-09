# session-raw-view Specification

## Purpose
세션에서 어떤 도구·스킬·서브에이전트를 썼는지 바로 보이게 하고, 필요하면 transcript 원문 JSON을 프롬프트 수준까지 확인하게 한다.

## Requirements

### Requirement: 세션별 도구·스킬 표시

세션 목록의 각 행은 많이 쓴 도구 최대 3개(횟수)와 사용한 스킬·서브에이전트를 보여야 한다(SHALL). 흐름은 스킬 실행, 서브에이전트 시작, 슬래시 커맨드를 일반 도구와 구분해 보여야 하며(SHALL) "스킬·에이전트" 필터를 제공해야 한다(SHALL).

#### Scenario: 스킬을 쓴 세션
- **WHEN** 세션에서 opsx:propose 스킬을 한 번, Explore 서브에이전트를 두 번 썼다
- **THEN** 목록 행에 "opsx:propose"와 "Explore ×2" 칩이 보이고, 흐름에는 해당 시각에 "스킬 · opsx:propose", "서브에이전트 · Explore · 설명" 항목이 보인다

### Requirement: 원문 보기

흐름의 각 항목에서 그 항목을 만든 transcript 기록을 들여쓴 JSON 원문으로 볼 수 있어야 하고(SHALL), 세션 전체 기록을 순서대로 100줄씩 넘겨 볼 수 있어야 한다(SHALL). 원문은 열 때 파일에서 읽고 D.Duck DB·디스크·로그·쿼리 캐시에 남기지 않아야 한다(MUST NOT). 한 기록이 1MB를 넘으면 앞부분만 보이고 잘렸다고 알려야 한다(SHALL).

#### Scenario: 프롬프트 원문
- **WHEN** 사용자가 흐름의 프롬프트 항목에서 "원문"을 누른다
- **THEN** 해당 user 기록의 JSON 전체(message.content 포함)가 들여쓰기되어 보이고 복사할 수 있다

#### Scenario: 저장되지 않음
- **WHEN** 원문을 본 뒤 창을 닫는다
- **THEN** 앱 DB·앱 데이터 폴더·로그에서 그 내용을 찾을 수 없다

### Requirement: 비밀값 가림

원문은 기본으로 비밀값처럼 보이는 문자열을 가려야 하며(SHALL), 사용자가 "가리지 않고 보기"를 켠 경우에만 그대로 보여야 한다(SHALL). 이 선택은 저장하지 않고 창을 닫으면 다시 가림으로 돌아간다(SHALL).

#### Scenario: 토큰이 든 기록
- **WHEN** 도구 입력에 API 키 모양 문자열이 있는 기록을 연다
- **THEN** 기본 화면에서는 가려져 있고, "가리지 않고 보기"를 켜면 원래 값이 보인다
