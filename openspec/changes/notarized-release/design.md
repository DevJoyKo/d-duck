# Design

## Context

- `release-local.sh`는 `tauri build --bundles app`으로 앱을 빌드하고, ad-hoc `codesign --deep`으로 서명한 뒤, `hdiutil`로 DMG를 만들어 설치한다.
- 앱은 `claude` CLI를 자식 프로세스로 실행하고, osascript로 터미널에 Apple Events를 보낸다. `NSAppleEventsUsageDescription`은 이미 있다.

## Goals / Non-Goals

**Goals:**
- 인증서가 생기면 같은 스크립트로 공증 빌드를 만든다.
- 그 전까지는 지금처럼 동작한다.

**Non-Goals:**
- Mac App Store
- 자동 업데이트(Sparkle 등)
- CI 공증
- Homebrew cask. 공개 뒤 따로 만든다.

## Decisions

0. **전체 순서**(design-critic HIGH)
   1. `tauri build --bundles app`
   2. dSYM 보관
   3. 서명(안쪽에서 바깥쪽으로)
   4. 앱 zip을 공증하고 앱에 staple
   5. staple된 앱으로 DMG를 임시 경로에 만듦
   6. DMG 서명, 공증, staple
   7. 검증(codesign·spctl·stapler)
   8. 통과하면 bundle/dmg와 publish/dmg로 옮기고, 이전 빌드는 휴지통으로 보내고, /Applications에 설치

   8단계 전에 실패하면 셋 중 아무것도 바꾸지 않는다. 임시 STAGE와 TMP_DMG는 `trap`으로 정리한다. ad-hoc 모드도 같은 순서를 따르되 4·6단계 공증과 spctl 검증은 건너뛴다.

1. **서명 순서: 안쪽에서 바깥쪽으로.**
   - 번들 안의 Mach-O를 먼저 `codesign --force --options runtime --timestamp --sign "$ID"`로 서명하고, 앱 번들은 entitlements와 함께 마지막에 서명한다.
   - 서명 대상: `Contents/MacOS/*`, 프레임워크가 있으면 그것까지.
   - `--deep` 서명은 쓰지 않는다. Apple 권고이고, entitlements가 안쪽 바이너리에 잘못 붙는 것을 막는다.
2. **공증 두 번.**
   - 앱: `ditto -c -k --keepParent`로 zip을 만들고 `notarytool submit --keychain-profile "$P" --wait`로 공증한 뒤, `stapler staple`을 앱에 한다.
   - DMG: 그다음 DMG를 만들고 `codesign --timestamp --sign`으로 서명한 뒤, 공증과 staple을 한 번 더 한다.
   - 실패하면 `notarytool log <id>`를 출력하고 종료한다.
   - 이유: 앱에도 티켓이 붙어 있어야 DMG 밖으로 복사해 오프라인에서 열어도 통과한다.
3. **검증.**
   - `codesign --verify --deep --strict`, `spctl -a -vv -t exec`(앱), `spctl -a -vv -t open --context context:primary-signature`(DMG), `stapler validate`를 모두 통과해야 배포 위치로 옮긴다.
   - 그 전에는 임시 경로에서만 작업한다.
4. **entitlements.**
   - `app/src-tauri/entitlements.plist`에는 `com.apple.security.automation.apple-events = true`만 넣는다.
   - Tauri WKWebView는 JIT 예외 없이 동작한다. 공증 빌드에서 실제로 확인한다.
   - 서명의 진실 원천은 스크립트 하나다(design-critic MEDIUM). tauri.conf.json에는 signingIdentity·hardenedRuntime·entitlements를 넣지 않는다.
   - 빌드 전에 `unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH APPLE_SIGNING_IDENTITY APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD`로 tauri 자체 서명·공증을 막는다(보안 MEDIUM).
5. **자격 정보.**
   - 사용자가 직접 `xcrun notarytool store-credentials dduck-notary --apple-id … --team-id …`를 한 번 실행한다. `--password`는 빼서 앱 전용 암호를 대화형으로 입력받는다. 셸 히스토리에 남기지 않기 위해서다(보안 MEDIUM).
   - 스크립트는 `DDUCK_NOTARY_PROFILE`만 읽는다. 인증서는 로그인 키체인에 둔다.
   - 팀 ID는 인증서 이름과 서명에 공개적으로 들어가는 값이다. 그래서 출력에서 가리지 않는다. 다만 저장소에 하드코딩하지 않고, 공개 직전 스캔의 로컬 키워드에 넣는다.

## Risks / Trade-offs

- **WKWebView와 hardened runtime**: 드물게 JIT 예외가 필요할 수 있다. 공증 빌드로 실제 앱을 실행해 확인하고, 필요하면 그 하나만 근거와 함께 더한다.
- **Apple Events 권한 창**: 공증 빌드에서 처음 터미널을 열 때 자동화 허용 창이 뜬다. 정상 동작이며 README에 적는다.
- **공증 대기 시간**: 보통 몇 분이다. 제출마다 `--wait --timeout 30m --output-format json`으로 제출 id를 받는다. 시간이 넘으면 실패로 처리하고 id와 `notarytool info` 명령을 안내한다. 최악의 경우 두 제출로 60분이 걸린다.
- **버전 파일명**: DMG 이름의 버전은 하드코딩하지 않고 tauri.conf.json `version`에서 읽는다(design-critic LOW).
