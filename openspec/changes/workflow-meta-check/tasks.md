# Tasks

## 1. 검사

- [x] 1.1 Rust meta 리터럴 토크나이저와 이유 코드(값 없는 enum, 줄 번호만), 깊이 32·토큰 1만·원소 1천 상한(tooComplex), char 경계·잘못된 이스케이프 panic 없음 proptest, 원문 name 비교, design 결정 6의 규칙별 테스트(음수·sparse·예약 키·숫자 형식·이스케이프·중복 키·빈 name/description), QA probe 경우(fncall, phvar, spread, tmpl, tsann, esc)와 기존 통과 경우 테스트
- [x] 1.2 저장 경로에서 새 검사 사용, 순수 명령 check_workflow_meta(text, name), skills_catalog 항목에 cc_readable·reason·registered_name과 목록 시나리오 Rust 테스트

## 2. 화면

- [x] 2.1 AddSheet·AuthoringEditor 입력 중 400ms 디바운스로 check_workflow_meta 호출해 이유 표시·저장 버튼 끄기, 본문 문법은 검사하지 않는다는 한 줄 안내, 확장 목록 "Claude Code가 못 읽어요" 표시, ko/en 문구, vitest

## 3. 검증

- [x] 3.1 cargo test·tsc·vitest, mock 스크린샷
- [x] 3.2 테스트 리뷰(qa-verifier + design-critic) · 지적 8건 수정(괄호·키 이스케이프·BigInt 받기, 고치기 이름 경고, 이유별 고칠 방법, 조사, 등록 이름 배지, 400ms 가짜 타이머)
