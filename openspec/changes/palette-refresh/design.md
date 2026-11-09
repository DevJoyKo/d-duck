# Design

## Context

- 색은 `app/src/index.css`의 `--color-*` 토큰(Tailwind 4 `@theme`)에서 온다. 다크는 `prefers-color-scheme`와 `[data-theme=dark]` 두 곳에 같은 값이 들어 있다.
- 사이드바는 `--color-rail`(#222854, 다크 #10132b)과 `--color-rail-ink`를 쓴다.
- 오리 그림은 SVG 컴포넌트로 되어 있고, 역할 색은 `roleColors.ts`에서 온다.

## Goals / Non-Goals

**Goals:**
- 토큰만 바꿔서 대부분의 화면이 따라오게 한다.
- 사이드바를 중성색으로 바꾼다.

**Non-Goals:**
- 레이아웃 변경(B 시안의 3구역 보드 등)
- (D-177로 대체) 오리 그림은 다듬은 조약돌 그림체로 바꾼다(tasks 2.4)
- 타입 스케일 전면 개편(2.3에서 단계별로만, 별도 리뷰)

## Decisions

1. **토큰을 교체하고 이름은 유지한다.**
   - `ink`·`paper`·`sheet`·`line` 등은 이름을 그대로 두고 값만 바꾼다.
   - `rail`/`rail-ink`는 `side`/`side-ink`/`side-active`로 바꾼다. 사이드바만 쓰는 토큰이다.
   - 새 토큰:
     - `well`: 칸 바탕
     - `shadow-e1`, `shadow-e2`: 그림자 단계
   - 다크 값을 두 곳에 중복으로 두는 구조는 유지하고, 테스트로 두 곳이 같은지 확인한다.
   - 대안: 새 이름 체계로 전부 바꾸면 수백 곳을 고쳐야 해서 위험이 크다.
2. **반투명 사이드바**
   - `backdrop-filter: blur(30px)`와 rgba 바탕을 쓴다.
   - Tauri 창 자체의 vibrancy는 쓰지 않는다. 창 설정과 플랫폼마다 다르기 때문이다.
3. **위계**
   - 공용 Sheet·카드 클래스에서 `border`를 `ring-[0.5px] ring-line`과 `shadow-e1`로 바꾼다.
   - 대기 카드는 `shadow-e2`와 `ring-2 ring-duck`을 쓴다.
4. **하드코딩 정리**(design-critic HIGH)
   - 대상 1: 남색 hex 값 `#222854`, `#10132b`, `#151935`, `#1c2143`, `#0b0e22`, `#343a68`, `#2c3158`
   - 대상 2: 남색 rgba 그림자 `rgba(15,18,40,…)`
   - 대상 3: select 화살표 data URI 색, `text-white`·`white/10` 같은 사이드바 전용 흰색
   - 대상 4: Tailwind 기본 팔레트색(`emerald-*`, `red-*`, `amber-*` 등)
   - 알려진 위치: LogView.tsx:28·69(xterm 테마와 바탕), TourDeck.tsx:113, LoginBits.tsx:152, TaskReport.tsx:820·827 외, AgentsPage.tsx:186, Sidebar.tsx:18·24·53·140, StartChecklist.tsx:70, index.css:115
   - 위 값은 모두 토큰으로 바꾼다. 오리 SVG는 제외한다.
   - xterm 테마는 토큰 값을 읽어 만든다.
   - 다크 토큰 `chart-2`, `duck-soft`, `duck-stage-quiet`, `line`도 흑연 체계로 다시 정한다.
   - 이 검사를 vitest에 추가한다.
5. **대비 검증**
   - 토큰 쌍(글자/바탕)의 대비를 계산하는 테스트를 추가한다. 기준은 4.5:1이다.

## Risks / Trade-offs

- **오리가 묻힘**: 다크 흑연 바탕에서 남색 외곽선 오리가 덜 보일 수 있다. 오리 무대(`duck-stage`) 바탕을 다크에서 밝게 두어 대비를 유지한다.
- **동시 작업**: i18n 기반 작업이 Sidebar를 고치는 중이다. 그 작업이 끝난 뒤에 착수한다.
