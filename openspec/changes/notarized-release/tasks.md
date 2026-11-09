# Tasks

## 1. 스크립트

- [x] 1.1 entitlements.plist(apple-events만, tauri.conf에는 서명 설정 넣지 않음), 빌드 전 APPLE_* 환경 변수 unset
- [x] 1.2 release-local.sh 서명·공증 모드(design 결정 0의 순서, 검증 통과 전 bundle·publish·/Applications 불변, trap 정리, 버전은 tauri.conf에서, --timeout 30m·json 제출 id, 두 값 모두/없음/하나만 분기, 안쪽→바깥 서명, 앱 공증+staple, DMG 서명·공증·staple, spctl·stapler 검증 통과 후에만 배포 위치로, 실패 시 notarytool log, 30분 제한), ad-hoc 경로 회귀 없음 확인(실제 실행) — 하네스로 확인, 실제 release 실행은 사용자 확인 후
- [x] 1.3 `docs/RELEASING.md`(가입·Developer ID Application 인증서·store-credentials는 --password 없이 대화형·실행·검증 명령), 자격 정보 패턴 스캔, 가짜 notarytool(PATH)로 거절·시간 초과 시 세 위치 불변 시험 — `app/scripts/test-release-local.sh` 45/45

## 2. 공증 빌드 확인 (사용자 가입·인증서 준비 후)

- [ ] 2.1 사용자가 인증서·프로필 준비 → 공증 빌드 실행, spctl "Notarized Developer ID" 확인, 다른 macOS 사용자 계정에서 quarantine 붙인 사본을 네트워크 끈 채로 열어 "확인되지 않은 개발자" 차단 없음 확인, 터미널에서 열기·CLI 실행·알림·트레이 동작 확인
- [ ] 2.2 README 2개 설치 안내에서 "그래도 열기"·`xattr -dr` 안내·"not notarized" 문구 정리(공증 빌드 기준), 테스트 리뷰(qa-verifier + design-critic)
