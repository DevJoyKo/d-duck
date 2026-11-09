# app-identity Specification

## Purpose
앱의 이름·실행 파일·데이터 파일·코드 이름을 D.Duck 하나로 맞추고, 이름이 바뀌어도 사용자 데이터가 이어지게 한다.

## Requirements

### Requirement: 옛 이름 제거

현재 쓰이는 코드·설정·실행 파일·데이터 파일 이름에는 옛 이름(DevDeck, devdeck)이 남지 않아야 한다(MUST NOT). 당시 결정을 적은 기록 문서(결정 로그, 리서치, 기존 change)는 고치지 않는다(SHALL).

#### Scenario: 이름 검색
- **WHEN** 앱 소스(app/ 아래, 빌드 산출물 제외)에서 `devdeck`을 대소문자 구분 없이 찾는다
- **THEN** 옛 DB 파일 이름을 한 번 이전하는 코드(상수 하나와 그 테스트) 말고는 결과가 없다

### Requirement: 데이터 이어 쓰기

앱은 시작할 때 새 데이터 파일(dduck.db)이 없고 옛 파일(devdeck.db)이 있으면 옛 파일과 보조 파일(-wal, -shm)의 이름을 바꿔 그대로 써야 한다(SHALL). 새 파일이 이미 있으면 옛 파일을 건드리지 않는다(SHALL).

#### Scenario: 업데이트 후 첫 실행
- **WHEN** 옛 이름의 DB에 사용 기록이 있는 상태에서 새 버전을 처음 켠다
- **THEN** 사용 통계·작업 기록·루틴이 그대로 보이고 데이터 폴더에는 dduck.db만 있다
