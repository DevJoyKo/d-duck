# Tasks

## 1. 토큰

- [x] 1.1 index.css 라이트·다크 토큰 교체(A 체계: ink #24283B·paper #F5F6F8·sheet #FFF / 다크 흑연 #1D1E23·#27282F·#ECEEF4, 상태색), side·side-ink·side-active·well·shadow-e1·e2 추가, 다크 두 블록 일치 테스트, 대비 4.5:1 테스트
- [x] 1.2 하드코딩 색 → 토큰(design 결정 4의 목록 전체, xterm 테마 포함, 오리 SVG 제외), 남색 hex·rgba·Tailwind 기본색 검사 테스트

## 2. 컴포넌트

- [x] 2.1 Sidebar·StartChecklist 중성 반투명·선택 항목 표시, 흰 글자·white/10 제거(i18n 언어 전환 작업 뒤 착수)
- [x] 2.2 ui.tsx Sheet·카드·버튼 위계(ring+shadow), 작업 보드 칸 well, 대기 카드 e2+노랑 링(선택 링·포커스 링과 다른 모양), 노랑 허용 목록 밖의 노랑 제거, 다크 오리 무대 대비

- [x] 2.3 (재리뷰 반영, 범위: PageHeader·Tasks·Agents) 작업 카드 줄 구성 - 1줄: 20px RoleDuck(aria-hidden)+제목(14 semibold, min-w-0 flex-1 truncate)+보고서 아이콘(14px, aria-label), 알약 없음 / 2줄: 12 ink-3 "역할 · 페르소나" 한 줄 truncate / 3줄: 프롬프트 13 ink-2 line-clamp-2 / 4줄: 경로(mono truncate)+상태(shrink-0 whitespace-nowrap), KIND_LABEL은 상세 서랍으로 / 경고 줄(보고서 형식 오류·한도): 아이콘+12px. 한도 동작(LimitBits)은 FollowUps와 같은 구분선 밑 한 줄로 합치고 주 동작 1개만 primary, 안내는 12px 한 줄+title. 900px 상세 서랍: 보드 오른쪽 여백을 서랍 실제 폭에 맞춤(pr-[564px] 고정 제거). 역할명은 i18n 키(lib/agents.ts:31). 머리 요약: 0인 항목 빼기, 영어 복수형, aria-live 없음, 이름·경로 없음 테스트. 카드 선택 버튼 aria-labelledby=제목. 버튼: quiet는 secondary 별칭 유지, plain만 추가. 타입 정리: 10·11·15·17px -> 20·14·13·12(위 범위만). Agents 화면 알약(1개)은 유지.

- [x] 2.4 오리 교체(D-177): duck-A SVG를 DuckAvatar·RoleDuck 컴포넌트로(역할 12종·상태 4종, 32·20px 단순화판, CSS 변수 색), 앱 아이콘(src-tauri/icons 전체 규격) 교체, 오리 그림 노랑 예외를 노랑 검사에 반영 (남음: Windows용 Square*Logo·icon.ico는 11-07 그대로, src-tauri 담당이 교체)

- [x] 2.5 루틴 사용 로그(클라우드 실행 기록·로그 패널, 로컬 실행 기록 표)가 긴 줄·긴 토큰(URL·JSON·경로)에서 화면 경계를 넘는 문제: 먼저 mock에서 재현 스크린샷, 원인(flex/grid 자식 min-w-0 누락, 표 셀, break-words 한계) 확인 후 패널은 max-w-full min-w-0, 로그 줄은 [overflow-wrap:anywhere]+whitespace-pre-wrap, 표는 table-fixed+셀 truncate+title, 로그 상자는 세로 max-h 스크롤. D-168 앱 전체 줄바꿈 기준과 맞추고 같은 패턴(세션 원문·보고서 명령 출력·MCP 오류)도 점검, 1440·900 회귀 스크린샷

- [x] 2.6 아티팩트 페이지 크기를 창 크기에 맞춰 동적으로: 목록 영역 실측(ResizeObserver)으로 열 수(카드 최소 폭 기준)×잘리지 않는 행 수(최소 1행), 창 변경 시 보던 첫 카드가 있는 페이지로, 기존 페이지 넘김·검색 1쪽 복귀 유지, 테스트(계산 함수 단위 + 1440·900 스크린샷에서 마지막 행 안 잘림)

- [x] 2.7 작업 상세 서랍의 Claude 화면 미러링(LogView, xterm) 출력 깨짐: 실제 `claude logs` 출력으로 재현(백그라운드 작업 하나의 로그 바이트를 scratchpad에 저장, 내용은 저장소에 넣지 않음), 원인 확인(원본 터미널 폭과 xterm cols 180 불일치, 한글 등 넓은 문자 폭(unicode11 addon), 폰트 대체로 칸 어긋남, lastScreen 자르기 위치가 이스케이프 중간, 서랍 폭보다 넓은 캔버스) 후 수정: 원본 폭 추정·맞춤 또는 컨테이너 폭에 맞춘 축소, 넓은 문자 처리, 회귀 테스트(lastScreen 단위 + 저장한 바이트로 렌더 스냅숏), 라이트·다크 스크린샷

## 3. 검증

- [x] 3.1 tsc·vitest(버튼 안 버튼 0), 다크 900px 사이드바 blur 성능 확인, Playwright 스크린샷 화면 8개 × (라이트 1440·라이트 900·다크 900) + 작업 상세 서랍 열림 900
- [x] 3.2 테스트 리뷰(qa-verifier + design-critic + brand-reviewer, 엄격 D-109), 지적 조치·decisions 기록
