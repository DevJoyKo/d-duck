# Proposal

## Why

D.Duck은 Mac 앞에서만 볼 수 있다. 사용자는 휴대폰으로도 에이전트와 작업 상태를 따라가고 싶어 한다. 사용자는 "Mac + Tailscale 웹앱(①)으로 먼저, 나중에 클라우드 중계(②)·네이티브 앱(③)", "첫 버전은 보기만"을 골랐다. Claude 앱도 Remote Control 세션은 보여 주므로, 이 앱은 D.Duck 고유 정보(에이전트 보드, 검토 필요, 지난 작업, 루틴, 사용량)에 집중한다.

## What Changes

- **휴대폰 보기 서버**: D.Duck이 `127.0.0.1`에서만 듣는 읽기 전용 HTTP 서버를 띄운다(설정에서 켤 때만, 기본 꺼짐). 휴대폰은 Tailscale의 `serve` 기능(tailnet 안 HTTPS 프록시)으로 접속한다. 데이터는 사용자 기기 사이에서만 오간다.
- **기기 연결(페어링)**: Mac 설정 화면이 5분짜리 일회용 코드와 QR을 보여 주고, 휴대폰이 코드를 입력하면 기기 토큰을 받는다. 모든 요청은 토큰이 필요하고, 연결된 기기 목록에서 끊을 수 있다.
- **모바일 웹앱(PWA)**: 홈(막힌 작업, 일하는 agent, 검토 용량), 작업(진행·검토 필요·끝난 작업과 처음 지시), 지난 작업, 루틴(로컬·다음 실행), 오늘 사용량. 홈 화면에 추가할 수 있고 15초마다 새로 고친다. 보기 전용이다.
- **Tailscale 안내**: 설치·로그인 여부를 감지해 단계별 안내와 `tailscale serve` 설정 버튼(네이티브 확인)을 준다.

범위 밖: 원격 실행(시작·중지·검토함), 휴대폰 푸시, 세션 흐름(transcript) 보기, 클라우드 중계, 네이티브 앱.

## Capabilities

### New Capabilities

- `mobile-companion`: 휴대폰 보기 서버, 페어링, 모바일 웹앱, Tailscale 연결 안내

### Modified Capabilities

(없음)

## Impact

- core: `mobile` 모듈(읽기 전용 응답 조립, 기기 토큰·페어링 코드, DB v6 `mobile_devices`)
- 셸: 로컬 HTTP 서버 스레드(tiny_http), 설정 명령(켜기·끄기, 페어링 코드, 기기 목록·끊기, Tailscale 상태·serve 설정)
- UI: 설정의 "휴대폰에서 보기", 별도 모바일 진입점(`mobile.html`, 같은 Vite 빌드)
