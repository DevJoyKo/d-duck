# Design

## Context

- 사용자 결정: ①(Mac + Tailscale 웹앱) 먼저, 보기 전용.
- 이 Mac에는 Tailscale이 아직 없다(설치·로그인은 사용자 몫).
- 원칙: 자격증명 비접근, transcript 원문 비저장(보내지도 않음), 웹뷰·네트워크 입력은 신뢰하지 않음.

## Decisions

### M1. 서버
- 셸에 `tiny_http` 서버 스레드. `127.0.0.1:47613`(고정, 충돌 시 47614–47619 중 첫 빈 포트)에만 bind.
- 메서드: GET만 + `POST /api/pair` 하나. 그 밖은 405. 요청 본문 4KB 상한, 동시 처리 4개(스레드 풀), 응답에 `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy`(self만), `Referrer-Policy: no-referrer`.
- 정적 파일: Tauri 자산 해석기(`app.asset_resolver()`)로 `mobile.html`과 그 자산만 제공(경로는 해석기에 있는 키만). 토큰 없이 받을 수 있다(코드는 공개돼도 무방, 데이터는 API에만).
- 켜기/끄기: 설정 `mobile_enabled`(UI 선호 허용 목록이 아니라 전용 명령 + 네이티브 확인). 끄면 서버 `unblock()` 후 스레드 종료.

### M2. 페어링과 토큰 (`devdeck-core::mobile`)
- 코드: 6자리 숫자, 5분 유효, 한 번만, 실패 5회면 폐기. 메모리에만 둔다(앱 재시작 시 사라짐).
- 성공 시 토큰 32바이트(`getrandom` → hex 64자) 발급, DB v6 `mobile_devices(id, name, token_sha256, created_at, last_seen_at)`에 해시만 저장. 기기 이름은 웹앱이 보낸 값(`display_safe` 40자) 또는 "휴대폰".
- 검증: `Authorization: Bearer <hex64>` → SHA-256 비교(상수 시간 비교). 성공 시 last_seen_at 갱신(1분에 한 번).
- 기기 상한 10대.

### M3. API (읽기 전용 JSON)
- `GET /api/overview`: 막힌 작업(이름·상태·유형), 에이전트 활동(agent·state·count·detail), 검토 용량(load/capacity), 마지막 폴링 시각.
- `GET /api/tasks`: 현재 세션(이름 또는 제목, 상태, 그룹, 유형, 페르소나, 시작 시각, 폴더 끝 두 단계).
- `GET /api/tasks/<key>/prompt`: 처음 지시(D.Duck 저장본 → transcript 첫 지시, 데스크톱과 같은 값).
- `GET /api/history`, `GET /api/history/<bg_id>`: 지난 작업 목록(지시 길이), 지시.
- `GET /api/routines`: 로컬 루틴(이름·일정 문장·켜짐·다음 실행·마지막 결과·진행 중), 지시문 없음.
- `GET /api/usage/today`: 오늘 비용·토큰·응답 수.
- 모두 기존 core 함수 재사용, 셸이 상태(`agents`, DB)에서 조립.

### M4. 모바일 웹앱
- Vite 두 번째 진입점 `mobile.html` + `src/mobile/main.tsx`. React + TanStack Query, 같은 디자인 토큰·Duck 아바타 재사용, 하단 탭(홈·작업·지난 작업·루틴·사용량).
- 토큰은 `localStorage`(이 기기 브라우저에만). 401이면 페어링 화면.
- `manifest.webmanifest`(이름 D.Duck, 아이콘, standalone). 서비스 워커는 두지 않는다(no-store 데이터, 캐시 불필요).
- 15초 폴링, 실패 시 마지막 데이터와 "마지막 <시각>".

### M5. Tailscale
- 감지: `/Applications/Tailscale.app/Contents/MacOS/Tailscale` 또는 PATH의 `tailscale`. `tailscale status --json`(5초)에서 `BackendState`, `Self.DNSName`만 읽는다.
- 공개: 네이티브 확인 후 `tailscale serve --bg --https=443 http://127.0.0.1:<port>`. 해제: `tailscale serve --https=443 off`. tailnet 안에서만 보이는 HTTPS(공개 인터넷 노출 `funnel`은 쓰지 않음).
- 주소: `https://<Self.DNSName에서 끝 . 제거>/`.

## Risks / Trade-offs

- tailnet에 다른 사람·기기가 있으면 주소에 접근할 수 있다 → 기기 토큰이 경계.
- 같은 Mac의 다른 프로세스가 `127.0.0.1` 포트에 접근 가능 → 토큰 없으면 데이터 없음.
- 지시 원문이 휴대폰 화면(브라우저)에 보인다 → 데스크톱과 같은 범위, 저장은 하지 않음(no-store).
