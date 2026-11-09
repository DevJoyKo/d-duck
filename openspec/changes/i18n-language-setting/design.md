# Design

## Context

현재 상태와 제약은 다음과 같다.

- 화면 문구는 컴포넌트와 `src/lib/*`(예: `format.ts`의 `STATUS_LABEL`, `auth.ts`의 `COPY`, `theme.ts`의 `THEME_LABEL`)에 한국어로 직접 들어 있다. 한글이 들어간 비테스트 줄이 약 2,100개, 파일이 70개다.
- 날짜는 `format.ts`의 `toLocaleString("ko-KR")`로 고정되어 있다.
- Rust에서는 dduck-core와 src-tauri에 한글 문자열 줄이 약 1,600개 있다. 대부분은 테스트·주석·프롬프트이고, 사용자용 오류는 `Error` → 문자열로 IPC를 거쳐 그대로 화면에 나온다.
- 트레이 메뉴(`src-tauri/src/lib.rs`)와 알림, 네이티브 확인 창은 Rust가 한국어로 만든다.
- 설정은 DB `settings` 키-값(`settings.rs`, `set_user_pref`)에 저장된다. 테마만 localStorage에 있다.
- `copyRules.test.ts`가 해요체와 "리뷰" 용어를 소스 전체에서 검사한다.
- 새 의존성은 꼭 필요할 때만 넣는다. 자격증명과 transcript 원문은 다루지 않는다.

## Goals / Non-Goals

**Goals:**
- 타입으로 보장되는 키, 빠진 번역은 빌드나 테스트에서 실패하는 구조
- 한 번에 바꾸지 않고 화면 묶음별로 옮겨도 앱이 깨지지 않는 점진 이행
- Rust OS 표면과 UI가 같은 언어 값을 쓰는 것

**Non-Goals:**
- 세 번째 언어(구조는 허용하지만 이번 범위에 넣지 않음)
- 오른쪽에서 왼쪽으로 쓰는 언어(RTL)
- 사용자 데이터 번역
- Claude CLI 자체 출력의 번역
- 문서·영상·DMG 이름 번역

## Decisions

1. **자체 경량 i18n을 쓴다. i18next는 쓰지 않는다.**
   - 구성: `src/i18n/ko.ts`(기준 카탈로그, `as const`), `src/i18n/en.ts`(`Messages` 타입을 만족해야 컴파일됨), `t(key, params)`, `useT()` 훅, React context로 언어 전달.
   - 복수형은 `Intl.PluralRules`로 `{one, other}`를 고른다.
   - 대안인 i18next+react-i18next는 기능이 많지만 의존성이 2개 늘고 키 타입 안전성을 얻으려면 추가 설정이 필요하다. 언어가 2개이고 문구가 단순하므로 자체 구현으로 충분하다.
   - 키는 화면 단위 네임스페이스로 둔다(`tasks.board.title`).
2. **언어 값은 DB 설정 `ui_language`(`system`|`ko`|`en`, 기존 snake_case 키 규칙)에 저장하고, localStorage를 캐시로 둔다.**
   - Rust(트레이·알림·확인 창)가 읽어야 하므로 DB가 원본이다.
   - 첫 화면이 깜빡이지 않도록 UI는 localStorage 캐시로 먼저 그린 뒤 DB 값으로 맞춘다.
   - `system`의 해석은 UI에서는 `navigator.language`, Rust에서는 `sys-locale`로 한다(새 크레이트 1개, 작고 순수 Rust). 대안인 macOS `defaults read`는 플랫폼 의존이라 쓰지 않는다.
   - `ui_language`는 `USER_PREFS` 허용 목록에 세 값으로 넣는다. `set_language`는 `set_user_pref`를 거쳐 저장하고, Rust가 트레이 메뉴를 다시 만든다.
   - localStorage 캐시, DB 값, task_meta의 언어 값은 모두 세 값만 받는다. 그 밖의 값은 `system`으로 본다. 그래서 `Intl` RangeError가 나지 않는다.
   - sys-locale 결과는 `ko`로 시작하는지만 판정한다. 문자열 자체는 화면과 로그에 남기지 않는다.
   - `<html lang>`은 현재 언어로 맞춘다(`index.html`의 고정 `en`과 RoutinesPage의 `lang="ko"`를 정리한다).
3. **백엔드 문구는 기존 코드 체계를 넓힌다. 새 타입은 만들지 않는다.** (design-critic HIGH 반영)
   - 이미 코드로 오는 오류는 UI 카탈로그 매핑만 더한다: `CliError.kind`, `LimitReason`, `ReportErrorCode`, `PreviewReason`.
   - 문제는 `CliError::InvalidInput { reason: &'static str }` 57곳에 들어 있는 한국어 문장이다. 이를 `reason: ReasonCode`(직렬화는 문자열 코드)와 `params`(코드마다 타입이 정해진 구조체)로 바꾼다.
   - 함께 보내는 `text`는 Rust 메시지 테이블이 현재 언어로 만든 대체 문장이다. UI가 코드를 모를 때만 쓴다.
   - 오류가 아닌 백엔드 표시 문장도 같은 방식으로 바꾼다. 대상은 한도 배너 `text`(`limit_resume.rs` 585·607), 로그인 상태 줄 등이다. UI가 받는 것은 코드, 시각(ms), 숫자이고, 문장과 시각 형식은 UI가 만든다.
   - Rust 메시지 테이블은 `dduck-core/src/i18n.rs`의 `msg(lang, code, &Params)`다.
     - 모든 문자열 params에 `display_safe`를 강제한다.
     - URL은 host만, 경로는 basename만 받는 생성자로 넣는다.
     - 테스트는 두 가지를 확인한다: 모든 코드에 두 언어가 있는지, userinfo가 들어간 URL이 새지 않는지.
   - UI의 `t()`는 문자열만 반환한다. 링크 같은 서식은 JSX 슬롯으로 넣는다. 치환은 한 번만 하고 결과를 다시 치환하지 않는다. `dangerouslySetInnerHTML`은 쓰지 않는다.

4. **OS 표면 문구는 Rust 테이블에서 직접 만든다.**
   - 대상은 트레이, OS 알림, 휴대폰 원격 알림(`remote_alerts`), 확인 창, 파일 선택 창 제목이다. 문구 키는 같은 `i18n.rs`에 둔다.
   - 작업 이름에는 기존처럼 `display_safe(..,64)`를 적용한다.
   - **번들 데이터**(`data/team_defaults.json` 68줄, `data/permission_catalog.json` 49줄)는 문구 필드를 `{ko, en}`으로 바꾼다. 읽을 때 현재 언어 판을 고른다.
   - 사용자가 바꾼 유형과 역할은 지금처럼 문자열 하나로 저장한다. 그래서 번역하지 않는다. 기본값과 같은지는 저장 시 표시(`custom`)로 구분한다.
   - 기본값에 있는 Claude용 고정 지시는 시작 언어 판을 쓴다.
5. **형식은 `Intl`을 쓴다.**
   - `format.ts`의 함수가 현재 locale(`ko-KR`/`en-US`)을 인자나 context로 받는다.
   - 상대 시간에는 `Intl.RelativeTimeFormat`을 쓴다.
   - 통화는 지금처럼 `$`로 둔다.
6. **Claude 응답 언어는 앱이 붙이는 고정 지시의 끝에 한 줄로 넣는다.**
   - ko: "사람이 읽는 출력은 한국어로 써요", en: "Write human-readable output in English".
   - 보고서 JSON 스키마의 키와 판정 enum은 그대로 둔다.
   - 작업을 시작할 때의 언어를 `task_meta`에 기록한다. 이어하기나 재작업은 그 언어를 따른다.
7. **문구 규칙 검사는 언어별로 나눈다.**
   - `copyRules.test.ts`는 ko 카탈로그와 Rust ko 열만 본다.
   - en 규칙은 다음을 확인한다.
     - sentence case
     - 문장 끝 마침표 일관성
     - "review"·"global" 용어 통일
     - 금지어 "Please kindly" 등
   - 하드코딩 금지 검사의 범위는 다음과 같다.
     - UI: `src/components/**`와 `src/lib/**`의 문자열 리터럴
     - Rust: dduck-core와 src-tauri의 `#[cfg(test)]` 밖 문자열 리터럴
     - 제외: 주석, 테스트, mock, `i18n.rs`와 카탈로그 파일, 로그 전용 문자열, 허용 목록(예: "한국어" 선택지 이름)
8. **이행 순서(점진)**
   - 기반(i18n 모듈, 설정, 형식) → 공통(사이드바, ui, 상태 라벨) → 화면 묶음 7개 → Rust 표면 → 프롬프트 → 검사 강제.
   - 하드코딩 검사는 처음에는 경고 목록으로 돌리다가, 마지막 태스크에서 실패로 바꾼다.

## Risks / Trade-offs

- **영어 문구가 더 길다** → 900px 레이아웃이 깨질 수 있다. 말줄임과 툴팁을 쓰고, 영어 스크린샷 리뷰를 게이트로 둔다.
- **이행 중 혼합 언어** → 묶음별로 옮기는 동안 영어 화면에 한글이 섞인다. 마지막 검사를 강제하기 전에는 설정의 English에 "(미완성)"을 표시하지 않고, 모든 묶음을 끝낸 뒤 출시한다(중간 빌드는 내부용).
- **mock 데이터는 한국어로 남는다** → mock은 사용자 데이터 역할이므로 그대로 둔다. 테스트 스냅샷은 언어를 고정해(`ko`) 기존 테스트를 유지한다.
- **Rust 코드화 범위** → 모든 `Error`를 코드화하지는 않는다. 화면에 나오는 경로만 하고, 나머지는 "알 수 없는 오류예요/Something went wrong" + 로그로 둔다.
- **프롬프트 언어 지시** → Claude가 지시를 어길 수 있다. 근거 요구와 판정 enum은 언어와 무관해서 기능에 영향이 없다.
- **리뷰 게이트**: 테스트 단계에는 qa-verifier와 design-critic이 필수다(CLAUDE.md). 화면 문구와 레이아웃이 크게 바뀌므로, UI 테스트 단계 기본 구성대로 brand-reviewer를 더한다(`.claude/agents/brand-reviewer.md`, D-109 엄격 기준).
- **sys-locale 추가** → 새 의존성 1개다. 라이선스는 MIT/Apache라 THIRD_PARTY.md에 적는다.

## Open Questions

없음. 기본 언어 규칙(ko OS → 한국어, 그 밖 → 영어)과 지원 언어(ko·en)는 요청에 따랐다.
