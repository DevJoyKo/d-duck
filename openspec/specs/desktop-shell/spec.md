# desktop-shell Specification

## Purpose
DevDeck 데스크톱 앱의 창, 트레이 상주, 백그라운드 동작, Claude CLI 감지 등 모든 탭이 공유하는 앱 수명주기를 정의한다.

## Requirements

### Requirement: 좌측 사이드바 탐색
앱은 메인 창 왼쪽 사이드바에 위쪽 "작업", 아래쪽 "사용 통계"·"설정" 메뉴를 제공해야 한다(SHALL). 사이드바에는 앱 이름(D.Duck), 검토가 필요한 작업 수, Claude CLI 연결 상태를 표시해야 한다(SHALL). 마지막으로 연 페이지는 다음 실행 때 복원되어야 한다(SHALL).

#### Scenario: 첫 실행
- **WHEN** 사용자가 앱을 처음 실행한다
- **THEN** 메인 창이 열리고 "작업" 페이지가 선택되어 있다

#### Scenario: 페이지 복원
- **WHEN** 사용자가 "사용 통계"를 연 상태로 앱을 종료했다가 다시 실행한다
- **THEN** "사용 통계" 페이지가 선택된 상태로 열린다

#### Scenario: 검토 필요 배지
- **WHEN** 답을 기다리는 작업 1개와 실패한 작업 1개가 있다
- **THEN** 사이드바 "작업" 메뉴에 2가 표시된다

### Requirement: 트레이 상주와 백그라운드 동작
앱은 메뉴바(macOS) 또는 시스템 트레이(Windows/Linux)에 아이콘을 표시해야 한다(SHALL). 메인 창을 닫아도 앱은 종료되지 않고, 수집과 세션 상태 감시를 계속해야 한다(SHALL). 트레이 메뉴는 "창 열기"와 "종료"를 제공해야 한다(SHALL).

#### Scenario: 창 닫기
- **WHEN** 사용자가 메인 창을 닫는다
- **THEN** 트레이 아이콘이 남아 있고, 이후 새로 기록된 transcript도 수집된다

#### Scenario: 완전 종료
- **WHEN** 사용자가 트레이 메뉴에서 "종료"를 선택한다
- **THEN** 앱 프로세스와 앱 내부의 수집·감시 작업이 모두 종료되고, 앱이 실행했던 Claude 백그라운드 세션은 계속 실행된다

### Requirement: Claude CLI 감지
앱은 시작할 때 `claude` 실행 파일의 경로와 버전을 감지하고, 실제로 사용할 경로를 화면에 표시해야 한다(SHALL). Finder 등 셸 밖에서 실행되어 PATH가 없어도 감지해야 한다(SHALL). CLI가 없으면 작업 탭 기능을 비활성화하고 이유를 표시하며(SHALL), 지원 범위(2.1.x) 밖이면 경고를 표시해야 한다(SHALL). 사용 통계 탭은 CLI 없이도 동작해야 한다(SHALL).

#### Scenario: 셸 밖 실행
- **WHEN** 앱이 PATH 환경변수 없이 실행되고 `claude`가 nvm 경로에만 설치되어 있다
- **THEN** 앱은 그 경로를 찾아 표시하고 작업 탭이 활성화된다

### Requirement: CLI 경로와 설정 디렉터리 지정
사용자는 설정 화면에서 CLI 경로와 Claude 설정 디렉터리를 직접 지정할 수 있어야 한다(SHALL). CLI 경로는 파일 선택 창으로만 지정하고, 실행 가능한 일반 파일이며 이름이 `claude`(Windows는 `claude.exe`/`claude.cmd`)이고 `--version` 출력이 Claude Code 형식일 때만 받아들여야 한다(SHALL).

#### Scenario: 다른 실행 파일 지정
- **WHEN** 사용자가 이름이 `claude`가 아닌 실행 파일을 고른다
- **THEN** 앱은 저장을 거부하고 이유를 표시한다

#### Scenario: 설정 디렉터리 변경
- **WHEN** 사용자가 설정 디렉터리를 `~/.claude-work`로 지정한다
- **THEN** 수집 대상이 그 디렉터리의 `projects/`로 바뀌고, 앱이 실행하는 `claude`도 같은 설정 디렉터리를 쓴다

#### Scenario: CLI 미설치
- **WHEN** PATH와 알려진 설치 위치 어디에도 `claude`가 없다
- **THEN** 작업 탭에 "Claude CLI를 찾을 수 없음" 안내가 표시되고 새 작업 실행이 비활성화되며, 사용 통계 탭은 정상 동작한다

#### Scenario: 지원 범위 밖 버전
- **WHEN** 감지된 `claude --version`이 2.1.x가 아니다
- **THEN** 작업 탭에 감지된 버전과 "검증되지 않은 버전" 경고가 표시된다

### Requirement: 자격증명 비접근
앱은 Claude 인증 정보(OAuth 토큰, API 키, 키체인 항목, `.credentials*` 파일, 세션 키 파일)를 읽거나 저장하거나 전송해서는 안 된다(MUST NOT). 앱이 Claude 설정 디렉터리 안에서 읽는 파일은 `projects/` 하위 transcript로 한정해야 한다(SHALL).

#### Scenario: 자격증명 파일 존재
- **WHEN** Claude 설정 디렉터리에 `.credentials.json`과 `sessions/*.key` 파일이 있다
- **THEN** 앱은 그 파일들을 열지 않는다

### Requirement: 앱 프로세스의 네트워크 비사용
앱 프로세스(화면과 백엔드)는 네트워크 요청을 하지 않아야 한다(MUST NOT). 앱이 실행한 `claude` 자식 프로세스의 통신은 예외다. 화면은 앱에 번들된 리소스만 불러와야 한다(SHALL).

#### Scenario: 오프라인 사용
- **WHEN** 네트워크가 끊긴 상태에서 앱을 사용한다
- **THEN** 사용 통계, 세션 목록, 로그 보기, 중지가 동작하고, 새 작업 실행은 CLI 오류를 그대로 표시한다

#### Scenario: 화면에서 외부 요청 시도
- **WHEN** 화면 코드가 외부 URL로 요청을 보내려 한다
- **THEN** 요청이 차단된다

### Requirement: 앱 종료와 재열기
트레이 "종료"와 OS의 앱 종료 명령(macOS Cmd+Q 등)은 앱을 완전히 종료해야 한다(SHALL). 창이 숨겨진 상태에서 Dock(macOS) 아이콘을 누르거나 앱을 다시 실행하면 기존 창이 다시 나타나야 하며(SHALL), 두 번째 인스턴스가 생기지 않아야 한다(MUST NOT).

#### Scenario: Cmd+Q 종료
- **WHEN** macOS에서 앱이 앞에 있을 때 사용자가 Cmd+Q를 누른다
- **THEN** 앱 프로세스가 종료되고 트레이 아이콘이 사라진다

#### Scenario: 두 번째 실행
- **WHEN** 앱이 이미 실행 중인데 사용자가 앱을 다시 실행한다
- **THEN** 새 프로세스는 곧바로 종료되고 기존 앱의 창이 나타난다

#### Scenario: 숨긴 창 다시 열기
- **WHEN** 창을 닫아 숨긴 뒤 Dock 아이콘을 누른다
- **THEN** 같은 창이 다시 나타나고 앱 인스턴스는 하나다

### Requirement: 크로스플랫폼 빌드
앱은 macOS용 `.dmg` 설치 파일로 빌드되어야 한다(SHALL). 같은 코드베이스로 Windows와 Linux 빌드가 가능해야 하며, OS별 경로(홈 디렉터리, 앱 데이터 디렉터리)는 하드코딩하지 않아야 한다(MUST NOT).

#### Scenario: macOS 빌드 산출물
- **WHEN** macOS에서 릴리스 빌드를 실행한다
- **THEN** 설치 가능한 `.dmg` 파일이 생성된다
