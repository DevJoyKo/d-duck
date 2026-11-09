# Proposal

## Why

지금 배포하는 DMG는 ad-hoc 서명만 되어 있다(D-160). 그래서 다른 Mac에서 처음 열면 Gatekeeper가 "확인되지 않은 개발자"로 막고, 사용자가 시스템 설정에서 "그래도 열기"를 눌러야 한다. 공개 배포를 앞두고 있어 Developer ID 서명과 Apple 공증으로 이 차단을 없앤다. 처음 열 때 "인터넷에서 다운로드한 앱" 확인 창은 공증된 앱에도 뜨며, 이는 정상 동작이다. Mac App Store는 쓰지 않는다. 샌드박스 안에서는 `~/.claude` 읽기, CLI 실행, git 다루기 같은 핵심 기능이 동작하지 않기 때문이다(사용자 결정 2026-11-09).

## What Changes

- **`release-local.sh`에 서명·공증 모드를 추가한다.** 환경 변수 `DDUCK_SIGN_IDENTITY`(Developer ID Application 인증서 이름)와 `DDUCK_NOTARY_PROFILE`(notarytool 키체인 프로필 이름)이 있으면 다음 순서로 진행한다.
  1. hardened runtime과 타임스탬프로 서명한다.
  2. 앱을 공증하고 staple한다.
  3. DMG를 만들고 서명한다.
  4. DMG를 공증하고 staple한다.
  5. `spctl`로 확인한다.

  두 변수가 없으면 지금처럼 ad-hoc으로 서명한다. 하나만 있으면 실패하고 이유를 알려 준다.
- **entitlements 파일을 추가한다.** 터미널 열기(osascript)에 필요한 Apple Events 권한만 넣는다. 넓은 예외 권한은 쓰지 않는다.
- **자격 정보는 스크립트와 저장소에 두지 않는다.** Apple ID, 앱 암호, 팀 ID는 사용자가 `xcrun notarytool store-credentials`로 키체인에 한 번 저장하고, 스크립트는 프로필 이름만 참조한다.
- **README 설치 안내**: 공증된 빌드부터는 "그래도 열기" 안내를 지운다.
- **사용자 준비 안내 문서 `docs/RELEASING.md`**: 가입, 인증서 만들기, store-credentials, 실행 방법을 적는다.

## Capabilities

### New Capabilities
- `release-signing`: 배포 빌드의 서명·공증·검증 규칙

### Modified Capabilities
- 없음

## Impact

- `app/scripts/release-local.sh`, `app/src-tauri/entitlements.plist`(새 파일), `tauri.conf.json`의 macOS 번들 설정(hardened runtime)
- `docs/RELEASING.md`, README 2개
- 비용: Apple Developer Program 연 $99. 사용자가 가입한다.
