# Tasks

## 1. core

- [ ] 1.1 `mobile` 모듈: 페어링 코드(5분·1회·5회 실패 폐기), 토큰 발급·해시 저장·상수 시간 검증, DB v6 `mobile_devices`, 기기 목록·끊기·상한, 테스트
- [ ] 1.2 응답 조립용 직렬화 타입(overview·tasks·history·routines·usage) — 루틴 지시문·경로 전체·transcript가 들어가지 않음을 테스트

## 2. 셸

- [ ] 2.1 tiny_http 서버(127.0.0.1 bind, GET+pair, 헤더, 본문 상한, 스레드 풀, 켜기·끄기), 자산 해석기로 mobile 파일 제공, 라우팅·401·405 통합 테스트
- [ ] 2.2 설정 명령: 켜기(네이티브 확인)·끄기, 페어링 코드 만들기, 기기 목록·끊기, Tailscale 상태·serve 켜기(네이티브 확인)·끄기

## 3. UI

- [ ] 3.1 모바일 진입점과 화면(페어링, 홈, 작업, 지난 작업, 루틴, 사용량, 연결 끊김 표시), manifest
- [ ] 3.2 데스크톱 설정 "휴대폰에서 보기"(상태, Tailscale 안내, 코드·QR, 기기 목록)
- [ ] 3.3 Playwright(모바일 뷰포트)로 페어링·각 탭·401 확인

## 4. 통합

- [ ] 4.1 로컬(127.0.0.1)에서 실제 서버로 페어링·데이터 확인, LAN 주소로는 연결 안 됨 확인

## Workflow follow-up

- 설계 리뷰(design-critic + security-privacy-reviewer) → 구현 → 테스트 리뷰(qa-verifier + design-critic) 후 아카이브.
