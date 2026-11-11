## MODIFIED Requirements

### Requirement: 사이드바 묶음
사이드바는 "일"과 "Claude 구성" 두 묶음으로 나뉘어야 하고(SHALL), 소식·사용 통계·설정은 그 아래, 현재 프로젝트 선택기는 맨 아래에 있어야 한다(SHALL).

#### Scenario: 사이드바 구성
- **WHEN** 앱을 연다
- **THEN** "일: 에이전트·작업·루틴·아티팩트", "Claude 구성: 확장·MCP·권한·메모리", 그 아래 소식·사용 통계·설정, 맨 아래 현재 프로젝트가 보인다

#### Scenario: 소식 페이지 주소
- **WHEN** 기억된 페이지나 `?page=`가 `news`이다
- **THEN** 소식 탭이 열린다
