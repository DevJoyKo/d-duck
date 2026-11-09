# Spec Delta

## Purpose
배포용 macOS 빌드를 Developer ID로 서명하고 Apple 공증을 받아, 내려받은 사용자가 "확인되지 않은 개발자" 차단 없이 열 수 있게 하는 규칙을 정한다.

## ADDED Requirements

### Requirement: 서명·공증 모드
릴리스 스크립트는 서명 인증서 이름과 공증 프로필 이름이 모두 주어지면 Developer ID 서명(hardened runtime, 보안 타임스탬프)과 Apple 공증을 거친 앱과 DMG를 만들어야 한다(SHALL). 앱과 DMG에는 공증 티켓을 붙여야 한다(SHALL, staple). 끝에는 Gatekeeper 평가(`spctl`)와 staple 검증이 앱과 DMG 모두 통과해야 한다(SHALL). 통과하기 전에는 빌드 출력 위치(bundle/dmg), 공유 위치(publish/dmg), 설치 위치(/Applications)를 하나도 바꾸지 않아야 하며, 실패하면 셋 모두 이전 상태로 남아야 한다(SHALL). 공증 대기가 시간 제한을 넘으면 실패로 처리하고 제출 id를 출력해야 한다(SHALL).

#### Scenario: 공증 빌드
- **WHEN** 두 값을 설정하고 릴리스 스크립트를 실행한다
- **THEN** `spctl -a -vv`가 앱에 대해 "source=Notarized Developer ID"를 보고하고, `xcrun stapler validate`가 앱과 DMG 모두 통과한다

#### Scenario: 공증 거절
- **WHEN** 공증이 Invalid로 끝난다(시험에서는 PATH의 가짜 notarytool로 흉내 낸다)
- **THEN** 스크립트는 notarytool 로그를 보여 주고 실패하며, bundle/dmg·publish/dmg의 기존 DMG와 /Applications의 앱은 바뀌지 않는다

#### Scenario: 처음 열기
- **WHEN** quarantine 표시가 붙은 공증 빌드를 다른 macOS 사용자 계정에서 네트워크를 끈 채 연다
- **THEN** "확인되지 않은 개발자" 차단 없이 "인터넷에서 다운로드한 앱" 확인 창만 뜨고, 열기를 누르면 실행된다

### Requirement: 기본은 ad-hoc
두 값이 모두 없으면 지금처럼 ad-hoc 서명 빌드를 만들어야 한다(SHALL). 하나만 있으면 무엇이 빠졌는지 말하고 실패해야 한다(SHALL).

#### Scenario: 설정 없음
- **WHEN** 두 값 없이 실행한다
- **THEN** ad-hoc 서명 DMG가 만들어지고 "공증되지 않은 빌드" 안내가 출력된다

### Requirement: 자격 정보 비보관
Apple ID, 앱 전용 암호, 인증서 개인 키는 스크립트·저장소·로그·셸 히스토리에 들어가지 않아야 한다(MUST NOT). 공증은 사용자가 키체인에 저장한 프로필 이름으로만 해야 하며(SHALL), 환경에 다른 공증 자격(APPLE_ID 등)이 있어도 빌드 도구의 자동 서명·공증이 쓰지 않게 해야 한다(SHALL). 팀 ID는 서명에 공개적으로 들어가는 값이라 비밀로 다루지 않되 저장소에 하드코딩하지 않는다(SHALL).

#### Scenario: 저장소 스캔
- **WHEN** 저장소에서 Apple ID 이메일, 앱 암호 형식(xxxx-xxxx-xxxx-xxxx), 사용자 팀 ID(공개 직전 스캔의 로컬 비공개 키워드 목록)를 검색한다
- **THEN** 일치하는 것이 없다

### Requirement: 최소 권한
hardened runtime 예외 권한은 앱이 실제로 쓰는 것만 둬야 한다(SHALL). 지금 필요한 것은 Apple Events(터미널에서 열기) 하나다. JIT, 서명되지 않은 메모리, 라이브러리 검증 끄기 같은 예외는 쓰지 않는다(MUST NOT). 공증 빌드에서도 터미널에서 열기, CLI 실행, 알림, 트레이가 동작해야 한다(SHALL).

#### Scenario: 공증 빌드 기능
- **WHEN** 공증 빌드를 설치하고 작업을 터미널에서 연다
- **THEN** 처음 한 번 macOS 자동화 권한을 묻고, 허용하면 터미널이 열린다
