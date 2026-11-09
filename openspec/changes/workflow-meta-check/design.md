# Design

## Context

`authoring.rs::workflow_meta`는 meta를 정규식으로 읽고 실행하지 않는다(D-086). Claude Code는 meta를 실제 JS 파서로 읽어 순수 리터럴만 받는다. 이 규칙은 QA가 CC 2.1.319 바이너리의 오류 문구로 확인했다.

## Goals / Non-Goals

**Goals:**
- meta 부분에 대한 앱의 판정이 CC와 같은 결과를 내게 한다. 본문 스크립트 전체의 문법 오류는 범위 밖이다. CC는 스크립트 전체를 파싱하므로 본문 오류는 앱이 놓칠 수 있고, 이 한계를 화면 안내에 적는다.
- 실행은 하지 않는다.

**Non-Goals:**
- 워크플로 본문 전체 검사(phase 호출 일치 등)

## Decisions

1. **작은 JS 리터럴 토크나이저를 Rust로 직접 쓴다.**
   - 첫 문장 `export const meta =` 뒤의 객체 리터럴을 읽는다.
   - 받는 것: 문자열(작은따옴표·큰따옴표, 이스케이프 처리), 보간 없는 템플릿, 숫자, true/false/null, 배열, 객체(식별자·문자열 키, 끝 쉼표), 괄호로 감싼 값(결정 6)
   - 그 밖의 토큰(`${`, `...`, 식별자 값, 함수 호출, `:` 타입 주석 위치의 타입)은 거부한다.
   - 주석은 건너뛴다.
   - 대안인 JS 엔진 내장(boa·quickjs)은 의존성이 크고 실행 위험이 있어 쓰지 않는다.
2. **이유는 코드로 돌려준다.** 예: `notFirstStatement`, `notLiteral`, `missingName`, `missingDescription`, `nameMismatch`. UI 카탈로그가 문구를 만든다(i18n D-171).
3. **목록 표시.** `skills_catalog`의 워크플로 항목에 `cc_readable: bool`과 `reason`을 추가한다.
4. **UI 즉시 검사.** 같은 파서를 셸 명령으로 부르지 않는다. 최소한의 TS 판정(이름 일치·첫 문장 형태)만 입력 중에 하고, 최종 판정은 저장 명령의 Rust 결과로 한다.

5. **상한과 견고성**(보안 리뷰)
   - 상한: 중첩 깊이 32, 토큰 1만 개, 배열·객체 원소 1천 개. 넘으면 `tooComplex`로 거부한다. 재귀 파서도 스택이 넘치지 않는다.
   - 읽는 범위: meta 객체가 닫히는 곳에서 멈춘다. 닫히지 않은 문자열·주석은 끝까지 한 번만 훑는다.
   - panic 없음: 바이트 인덱스는 char 경계에서만 자른다. `\u{110000}`, 짝 없는 서로게이트, 잘린 이스케이프에도 panic하지 않고 `notLiteral`을 낸다. proptest로 임의 입력을 넣어 panic 0을 확인한다.
   - 이유 표현: reason은 값을 담지 않는 enum 코드다. 위치가 필요하면 줄 번호 숫자만 둔다. 파일 내용, 토큰, 값은 reason에 넣지 않는다.
   - 이름 비교: meta.name은 디코딩한 원문 값과 파일 이름을 비교한다. 예: 보이지 않는 문자가 섞이면 다른 이름으로 본다. display_safe는 화면에 보일 때만 쓴다.
   - UI 즉시 검사: eval과 new Function을 쓰지 않는다.

6. **CC 규칙 세부**(design-critic HIGH·MEDIUM, CC 2.1.319 바이너리 오류 문구 근거. 테스트 리뷰 뒤 CC 근사(acorn, `scratchpad/wfqa/ccsim/check.mjs`·`qa-evidence/workflow-meta-check/ccsim2.txt`)와 다시 맞춤)
   - **값**
     - 받는 것: 음수 단항(`-1`), 문자열, 보간 없는 템플릿, 숫자, true·false·null, 배열, 객체
     - 괄호: CC 파서는 괄호를 노드로 남기지 않는다(paren.js 받음). 그래서 값(`name: ('x')`, `((1))`)과 meta 전체(`({...})`)의 괄호를 벗겨 읽는다. 괄호 안이 리터럴 하나가 아니면(`(1, 2)`, `('a' + 'b')`) 거부한다. 괄호 중첩도 깊이 32 상한에 든다.
     - 거부하는 것:
       - 그 밖의 단항(`!`, `+`, `~`)
       - 빈 칸 배열(`[1,,2]`, sparse)
       - 예약 키(`__proto__`, `constructor`, `prototype`)
       - 계산된 키(`[k]`)
       - 식별자·숫자·문자열이 아닌 키
       - 정규식 리터럴. CC 근사는 Literal로 받지만, CC가 그 값(객체)을 meta에서 어떻게 다루는지 근거가 없어 보수적으로 거부한다(notLiteral).
     - `__proto__`·`constructor`·`prototype` 키와 짝 없는 서로게이트(`\uD800`)도 CC 근사는 받는다. 앱은 거부를 유지한다. 앞의 셋은 앱이 meta를 객체로 다룰 때 프로토타입 키와 섞이지 않게 하려는 것이고, 짝 없는 서로게이트는 Rust 문자열(UTF-8)로 담을 수 없어 이름 비교·표시를 할 수 없기 때문이다. 이 경우 앱 표시가 CC보다 엄격하다(알려진 차이).
   - **숫자**: 10진·소수(`.5`)·지수(`1e3`)·16진·8진(`0o`)·2진(`0b`)·`_` 구분자·BigInt(`1n`, `0x1Fn`)를 받는다. BigInt는 CC 근사에서 Literal이라 받는다(big.js). 음수 BigInt(`-1n`)는 CC가 음수 단항을 숫자에만 허용하므로 거부한다(unsupportedUnary). 레거시 8진(`017`)과 소수·지수 BigInt(`1.5n`)는 파서 오류라 거부한다(parseError). 이전 문구 "BigInt는 strict 모드 오류"는 틀렸다(strict 모드 오류는 레거시 8진뿐).
   - **문자열 이스케이프**
     - 받는 것: `\n` `\t` `\\` `\'` `\"` `\xHH` `\uHHHH` `\u{...}`(최대 10FFFF), 줄 이어쓰기(백슬래시+개행)
     - 거부하는 것: `\0` 뒤에 숫자가 오는 레거시 8진, 따옴표 안의 맨 개행
   - **키**: 같은 키가 여러 번 나오면 마지막 값을 쓴다. 식별자 키의 유니코드 이스케이프(`\u0064escription`, `\u{6E}ame`)는 디코딩해 그 이름으로 본다(idesc.js 받음). 예약 키 검사는 디코딩한 뒤에 한다. 디코딩한 글자가 식별자 글자가 아니면 parseError다.
   - **name·description**: 빈 문자열이 아닌 문자열이어야 한다. 공백만 있는 description은 CC가 받는다. 앱의 기존 공백 거부는 CC에 맞춰 풀고, 경고로만 둔다.
   - **phases**: 형식은 강제하지 않는다. CC 근거를 찾지 못했다.
   - **이유 코드**: notFirstStatement, parseError, notLiteral, templateInterpolation, unsupportedUnary, sparseArray, reservedKey, unsupportedKey, missingName, emptyName, missingDescription, emptyDescription, nameMismatch, tooComplex
   - **문구**: 이유마다 "이유 · 고칠 방법" 한 줄이다(ko 해요체, en sentence case). 입력 중 표시와 저장 거부가 같은 문장을 쓴다. Rust는 `i18n::Fail::Meta*`에, UI는 `extensions.wfcheck.reason.*`에 두고, Rust 테스트가 두 쪽 문장이 같은지 확인한다. 저장 거부에는 " · N번째 줄"을 붙인다.
   - **조사**: 이름 뒤 "로/으로"는 받침으로 고른다(`i18n::ro`, `src/i18n/josa.ts`, 같은 표). 라틴 문자·숫자 이름은 읽는 소리로 본다(숫자 0·3·6, m·n·ng 끝 → 으로). 영어는 조사가 없다.
7. **입력 중 검사**(design-critic HIGH): 입력 중에는 400ms 디바운스 뒤 Rust 검사 명령(`check_workflow_meta(text, name)`, 순수 함수, 파일 접근 없음)을 invoke로 불러 같은 판정을 보여 준다. TS 쪽 별도 판정은 두지 않는다. 결정 4는 이 결정으로 대체한다.
8. **목록 이름**: 목록 이름은 파일 이름이다. 손으로 만든 파일의 meta.name이 파일 이름과 다르면 "등록 이름 다름" 배지와 함께 "Claude Code에는 <meta.name>(으)로 등록돼요"를 보여 준다. CC는 meta.name으로 등록한다(QA 1단계 근거).
   - **고치기**: 기존 파일을 고칠 때는 meta.name이 파일 이름과 달라도 저장한다(`authoring::check_existing`). 입력 중에는 같은 "Claude Code에는 …로 등록돼요" 경고만 보이고 저장 버튼은 켜져 있다. 이 경고에 쓰려고 `check_workflow_meta`는 nameMismatch일 때만 display_safe한 `registeredName`을 돌려준다(사용자 자신의 입력, 목록의 registeredName과 같은 처리). 만들기 화면은 지금처럼 막는다.
