# claude-login Specification

## Purpose
TBD - created by archiving change claude-login. Update Purpose after archive.

## Requirements

### Requirement: 로그인 상태 확인
앱은 `claude auth status --json`으로 CLI 로그인 상태를 확인해야 한다(SHALL). 작업과 같은 CLI 경로·PATH·설정 폴더 환경으로, 셸 없이, stdin 없이 실행하고 5초 안에 끝나야 한다(MUST). 실행 시점은 앱 시작, 창 포커스 복귀(마지막 확인 뒤 60초가 지났을 때만), 사용자의 다시 확인, 로그인 진행 중 2초마다다. 동시에 두 번 실행하지 않는다. CLI를 찾지 못했으면 실행하지 않고 기존 "Claude Code를 찾지 못했어요" 흐름이 우선한다.

#### Scenario: 포커스 확인 속도 제한
- **WHEN** 창이 30초 사이에 다섯 번 포커스를 되찾는다
- **THEN** `auth status`는 그동안 최대 한 번 실행된다

#### Scenario: 동시 요청
- **WHEN** 두 화면이 동시에 상태를 요청한다
- **THEN** `auth status`는 한 번만 실행되고 두 화면이 같은 결과를 받는다

### Requirement: 상태 판정
판정은 로그인됨·로그아웃·알 수 없음 셋 중 하나여야 한다(SHALL). 종료 코드 0이고 `loggedIn`이 true면 로그인됨, 종료 코드 1이고 `loggedIn`이 false면 로그아웃이다. 실행 실패, 시간 초과, JSON 파싱 실패, 다른 종료 코드, 코드와 `loggedIn` 불일치는 알 수 없음이다. 알 수 없음은 기능을 막지 않는다(MUST NOT). stderr 내용은 화면·IPC·로그로 보내지 않는다.

#### Scenario: 로그아웃 판정
- **WHEN** `auth status`가 종료 코드 1로 `{"loggedIn": false, "authMethod": "none", ...}`을 출력한다
- **THEN** 상태는 로그아웃이고 로그인 배너가 보인다

#### Scenario: 파싱 실패는 알 수 없음
- **WHEN** `auth status`가 JSON이 아닌 출력을 내거나 5초 안에 끝나지 않는다
- **THEN** 상태는 알 수 없음이고 배너도 게이트도 없으며 설정에 "로그인 상태를 확인하지 못했어요" [다시 확인]이 보인다

#### Scenario: 코드와 값 불일치
- **WHEN** 종료 코드가 0인데 `loggedIn`이 false다
- **THEN** 상태는 알 수 없음이다

### Requirement: 허용 필드만 보관
앱은 `auth status` 출력에서 `loggedIn`·`authMethod`·`apiProvider`·`subscriptionType`·`planTier`·`loginExpiresAt`만 읽어야 한다(SHALL). `email`·`orgId`·`orgName`·`projectsDirectory`·`configDirectory`와 그 밖의 필드는 DB·설정·로그·화면·IPC 응답 어디에도 남기면 안 된다(MUST NOT). 원문 출력은 판정 직후 메모리에서 버린다.
- `authMethod`·`apiProvider`는 허용 목록 값만 쓰고, 목록 밖이면 "기타"다.
- `subscriptionType`·`planTier`는 값을 그대로 두되 `[a-z0-9_]` 32자 이하일 때만 쓰고 아니면 버린다(실측 `max`·`max_5x`).
- `loginExpiresAt`은 RFC 3339 시각이고 지금부터 ±10년 안일 때만 쓴다.

#### Scenario: 개인 정보 canary
- **WHEN** `email`·`orgId`·`orgName`·`projectsDirectory`·`configDirectory`에 서로 다른 CANARY 값을 넣은 출력을 처리한다
- **THEN** 판정 결과의 직렬화·Debug 출력·IPC 응답·앱 로그(tracing)·DB 파일 바이트 어디에도 CANARY가 없다

#### Scenario: 낯선 값
- **WHEN** `authMethod`가 허용 목록에 없는 문자열이고 `planTier`가 `Max 5x!`다
- **THEN** 연결 방식은 "기타", 요금제는 비어 있고 원래 문자열은 남지 않는다

### Requirement: 연결 방식 표시
로그인됨 상태에서 앱은 연결 방식을 한 줄로 보여 줘야 한다(SHALL). 로그인 버튼은 로그아웃 상태에서만 보인다(MUST).
- `claude.ai`: "Claude 계정으로 연결돼 있어요"(요금제가 있으면 함께).
- `oauth_token`: "토큰으로 연결돼 있어요".
- `api_key`·`api_key_helper`·`third_party` 또는 `apiProvider`가 `firstParty`가 아님: "다른 방식으로 연결돼 있어요".
- 자세히 줄: "로그아웃은 터미널에서 `claude auth logout`으로 해요".

#### Scenario: 클라우드 공급자
- **WHEN** `loggedIn`이 true이고 `authMethod`가 `third_party`다
- **THEN** 설정에 "다른 방식으로 연결돼 있어요"가 보이고 로그인 버튼과 배너는 없다

#### Scenario: 환경 토큰
- **WHEN** `loggedIn`이 true이고 `authMethod`가 `oauth_token`이다
- **THEN** "토큰으로 연결돼 있어요"가 보이고 모델 호출 기능은 잠기지 않는다

### Requirement: 모델 호출 기능의 게이트
로그아웃 상태에서 작업 시작, Claude에게 만들기, 요청 다듬기, Claude 분석, 로컬 루틴 실행(예약·지금 실행), 한도 후 재개(자동·직접)는 CLI 실행 전에, 그리고 claim·네이티브 확인 창보다 먼저 멈춰야 한다(SHALL). 백엔드는 고정 오류 코드 `login_required`를 돌려주고 화면은 "Claude에 로그인해야 이 기능을 쓸 수 있어요" [로그인]을 보여 준다. 검사는 백엔드 명령 앞에서 해야 한다(MUST). 알 수 없음이면 통과한다.

#### Scenario: 로그아웃에서 작업 시작
- **WHEN** 로그아웃 상태에서 작업 시작을 누른다
- **THEN** `claude --bg`는 실행되지 않고 작성 내용은 그대로이며 한 줄 안내와 [로그인]이 보인다

#### Scenario: 화면 우회
- **WHEN** 로그아웃 상태에서 웹뷰가 작업 시작 명령을 직접 호출한다
- **THEN** 백엔드가 `login_required`를 돌려주고 CLI를 실행하지 않는다

#### Scenario: 확인 창보다 먼저
- **WHEN** 로그아웃 상태에서 루틴 "지금 실행"이나 "직접 이어서"를 누른다
- **THEN** 네이티브 확인 창이 뜨지 않고 루틴·재개 행은 claim되지 않는다

#### Scenario: 예약된 루틴
- **WHEN** 로그아웃 상태에서 로컬 루틴 시각이 된다
- **THEN** 실행하지 않고 그 회차를 `login_required`로 기록한다

#### Scenario: 자동 재개 보류
- **WHEN** 로그아웃 상태에서 한도 재개 행의 due 시각이 된다
- **THEN** 행은 claim되지 않고 `auto`로 남으며, 로그인 뒤 다음 확인 주기에 평소처럼 재개된다

### Requirement: claude.ai 로그인 전용 기능
클라우드 루틴 목록·편집은 `authMethod`가 `claude.ai`일 때만 실행해야 한다(SHALL). 로그아웃이면 모델 호출 게이트와 같은 안내, 다른 방식이면 "클라우드 루틴은 Claude 계정으로 로그인해야 써요"를 보여 준다. MCP 상태 확인은 막지 않고, claude.ai 로그인이 아니면 claude.ai 커넥터 줄에 "Claude 계정 로그인이 필요해요"를 붙인다.

#### Scenario: 토큰 연결에서 클라우드 루틴
- **WHEN** `authMethod`가 `oauth_token`일 때 클라우드 루틴 탭을 연다
- **THEN** CLI를 실행하지 않고 "클라우드 루틴은 Claude 계정으로 로그인해야 써요"가 보인다

### Requirement: 로그인 없이 쓰는 기능과 배너
세션 목록·transcript·통계·아티팩트·메모리·규칙·확장·플러그인 목록·터미널 열기는 로그인 없이 동작해야 한다(SHALL). 로그아웃 상태에서는 앱 위에 배너 하나만 보인다: "Claude에 로그인해야 작업을 맡길 수 있어요" [로그인]. 작업 화면의 한도 배너는 로그아웃 동안 숨기고 로그인하면 다시 보인다(MUST). 투어의 CLI 상태 자리에는 CLI가 있고 로그아웃이면 로그인 한 줄을 같은 자리에 보여 준다.

#### Scenario: 읽기 기능
- **WHEN** 로그아웃 상태에서 세션 기록과 통계를 연다
- **THEN** 평소처럼 보인다

#### Scenario: 한도 배너와 겹침
- **WHEN** 한도 대기 작업이 있고 로그아웃 상태다
- **THEN** 작업 화면에는 로그인 배너만 보이고, 로그인하면 한도 배너가 돌아온다

#### Scenario: 투어
- **WHEN** CLI는 찾았고 로그아웃 상태에서 투어를 연다
- **THEN** "Claude Code를 찾지 못했어요"가 있던 자리에 로그인 한 줄과 [로그인]이 보인다

### Requirement: 기존 로그인 오류 문구 통일
클라우드 루틴·요청 다듬기·Claude 분석의 기존 로그인 실패 문구는 게이트 문구로 바꿔야 한다(SHALL). 이 실패가 나면 상태를 한 번 다시 확인한다.

#### Scenario: 늦은 실패
- **WHEN** 상태가 알 수 없음이라 통과한 요청 다듬기가 로그인 오류로 끝난다
- **THEN** "Claude에 로그인해야 이 기능을 쓸 수 있어요" [로그인]이 보이고 상태 확인이 한 번 실행된다

### Requirement: 브라우저 로그인 실행
[로그인]은 `claude auth login --claudeai`를, 보조 선택 "Console 계정으로 로그인"은 `--console`을 실행해야 한다(SHALL). 인자는 고정 문자열이고 화면에서 받은 값을 붙이면 안 된다(MUST NOT). `--email`·`--sso`는 쓰지 않는다. 작업과 같은 환경으로, 셸 없이, stdin null로, 자체 프로세스 그룹에서 실행한다. 진행 중에는 "브라우저에서 로그인해 주세요" [취소]를 보여 주고 2초마다 상태를 확인한다(최대 5분). 로그인됨이 되면 진행을 닫고 배너를 없애고 잠겼던 화면을 새로 고친다.

#### Scenario: 성공
- **WHEN** [로그인]을 누르고 브라우저에서 승인한다
- **THEN** 다음 확인에서 로그인됨이 되고 배너가 사라지며 작업 시작이 가능해진다

#### Scenario: 시간 초과
- **WHEN** 5분 동안 로그인됨이 되지 않는다
- **THEN** 자식이 끝나고 "로그인이 끝나지 않았어요" [다시 시도] [터미널에서 로그인]이 보인다

### Requirement: 로그인 프로세스 수명
로그인은 한 번에 하나여야 한다(SHALL). 확인과 실행은 한 잠금 안에서 한다. 취소, 시간 초과, 성공 뒤 남은 자식, 앱 종료, 작업 객체 해제 때 자식 프로세스 그룹을 끝낸다(MUST). 그룹 종료는 자식을 회수하기 전에만 한다(PID 재사용 방지). stdout·stderr는 상한을 넘어도 EOF까지 읽어 버려 자식이 막히지 않게 한다. 출력은 저장·로그하지 않고, 토큰·코드를 파싱하거나 입력받지 않는다.

#### Scenario: 취소
- **WHEN** 진행 중에 [취소]를 누른다
- **THEN** 자식 프로세스 그룹이 끝나고 상태가 다시 확인되며 로그인 버튼이 다시 보인다

#### Scenario: 중복 실행
- **WHEN** 로그인 진행 중에 다른 화면의 [로그인]을 누른다
- **THEN** 두 번째 `claude auth login`은 실행되지 않고 진행 화면이 앞으로 온다

#### Scenario: 앱 종료
- **WHEN** 로그인 진행 중에 앱을 끝낸다
- **THEN** `claude auth login` 프로세스 그룹이 남지 않는다

#### Scenario: 많은 출력
- **WHEN** 자식이 1MB를 출력한다
- **THEN** 자식은 막히지 않고 앱은 64KB까지만 검사한다

### Requirement: 브라우저가 안 열릴 때의 링크
CLI가 출력한 URL은 https이고 호스트가 정확히 `claude.ai`·`platform.claude.com`·`console.anthropic.com` 중 하나이며, 사용자 정보·포트가 없고 2048자 이하일 때만 "브라우저에서 직접 열기" 링크로 제공해야 한다(SHALL). URL은 백엔드 메모리에만 두고 화면에는 링크 유무만 보낸다(MUST). 링크를 누르면 백엔드가 검사를 통과한 URL 문자열만 기본 브라우저로 연다.

#### Scenario: 허용 호스트
- **WHEN** CLI가 `https://claude.ai/oauth/authorize?...`를 출력한다
- **THEN** "브라우저에서 직접 열기"가 보이고 누르면 그 주소가 열린다

#### Scenario: 낯선 호스트
- **WHEN** CLI 출력에 `https://claude.ai.example.com/...`이나 `http://claude.ai/...`이 있다
- **THEN** 링크는 보이지 않는다

#### Scenario: URL 비노출
- **WHEN** 로그인 진행 중 IPC 응답과 앱 로그를 살핀다
- **THEN** URL 문자열이 없다

### Requirement: 터미널 대체 경로
앱 안 실행으로 끝낼 수 없을 때(자식이 로그인되지 않은 채 끝남, 코드 붙여넣기 안내, 시간 초과) 앱은 [터미널에서 로그인]을 제공해야 한다(SHALL). 누르면 앱 안 자식을 먼저 끝내고, 네이티브 확인 뒤 Terminal.app에서 같은 방식 플래그로 `claude auth login`을 연다. 앱과 같은 설정 폴더(프로필)로 로그인해야 한다(MUST). 설정 폴더가 기본값이면 `CLAUDE_CONFIG_DIR`을 지운 채 실행한다. 값은 osascript에 argv로만 넘기고 플래그는 고정 문자열이다. 앱은 터미널 출력을 읽지 않고 상태 확인만 계속한다. Automation 권한이 거부되면 명령 복사를 보여 준다.

#### Scenario: 코드 붙여넣기 요구
- **WHEN** CLI 출력에 코드 붙여넣기 안내가 나온다
- **THEN** 앱은 코드를 받지 않고 [터미널에서 로그인]을 보여 준다

#### Scenario: 같은 프로필
- **WHEN** 앱 설정 폴더가 기본값이고 사용자 셸에 `CLAUDE_CONFIG_DIR`이 설정되어 있다
- **THEN** 터미널 명령은 `CLAUDE_CONFIG_DIR`을 지우고 실행되어 앱과 같은 프로필에 로그인한다

#### Scenario: 확인 후 실행
- **WHEN** [터미널에서 로그인]을 누르고 확인 창에서 취소한다
- **THEN** Terminal.app이 열리지 않는다

### Requirement: 만료 안내
로그인됨이고 `authMethod`가 `claude.ai`이며 `loginExpiresAt`이 지금부터 3일 안이면 앱은 "Claude 로그인이 곧 만료돼요" [다시 로그인]을 설정과 작업 시작 화면에 한 줄로 보여 줘야 한다(SHALL). 기능을 막지 않는다.

#### Scenario: 2일 남음
- **WHEN** `loginExpiresAt`이 지금부터 2일 뒤다
- **THEN** 안내 한 줄이 보이고 작업 시작은 막히지 않는다

#### Scenario: 10일 남음
- **WHEN** `loginExpiresAt`이 10일 뒤다
- **THEN** 안내가 없다

### Requirement: 자격증명 비접근
앱은 Claude 자격증명(키체인 항목, `.credentials.json`, OAuth 코드·토큰, `CLAUDE_CODE_OAUTH_TOKEN`·`ANTHROPIC_API_KEY`·`ANTHROPIC_AUTH_TOKEN` 값)을 읽거나 저장·전송하면 안 된다(MUST NOT). 로그인 셸에서 이 환경 변수들을 가져와 자식에게 넘기지 않는다. 토큰 만들기(`claude setup-token`) 기능은 제공하지 않는다.

#### Scenario: 환경 변수 비전달
- **WHEN** 로그인 셸에 `CLAUDE_CODE_OAUTH_TOKEN`이 설정되어 있다
- **THEN** 앱이 실행하는 `auth status`·`auth login` 자식 환경에는 앱 자체 환경에 없던 그 변수가 없다
