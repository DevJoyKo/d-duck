# Tasks

## 1. 백엔드

- [x] 1.1 `news.rs`: 프롬프트에 적용 필드, `check_item`의 필드 단위 검증(`key_fact` 24자, `adopt`, `apply_steps` 1–3·160자·한 줄, `verify`), `APPLY_RISKY_RE`·`#` 줄 제거. 테스트: 정상·긴 값·위험 단계(파이프·bypassPermissions·Bash(*)·apiKeyHelper·hooks command)·모르는 키·필드 없는 옛 응답
- [x] 1.2 마이그레이션 v18(열 4개), `merge`·`list`가 새 열을 저장·반환, `apply_steps` JSON. 테스트: v17 데이터 유지·NULL 반환·왕복
- [x] 1.3 `claude_cli.rs`: `Origin::News`, `NEWS_REFERENCE_HEADER`, `Origin` parse·as_str, `apply_origin`을 `matches!` 분기로, `ReasonCode::NewsPlanOnly`·`NewsNoRemote`(ko/en), task_meta·db 출처 저장과 "다시 맡기기" 복원. 테스트: 프로젝트 폴더에서 acceptEdits·dontAsk·원격 거절, 머리글만 있는 초안, 두 머리글, `clean_origin("news")` 왕복
- [ ] 1.4 확인 창 예상 비용 문구 ko/en: 3.2의 실제 가져오기 비용으로 정함

## 2. 화면

- [ ] 2.1 api 타입(`NewsItem` 새 필드, `TaskOrigin` "news"), mock 시드에 적용 필드(9개)·없음(3개)
- [ ] 2.2 `FactCard`·적용 태그, 줄·시트 썸네일 교체, 고정폭 판별, 말줄임·전체 보기. vitest: 명령(고정폭)·수치·태그·대체 카드·adopt만 있음·한글 24자 말줄임과 title
- [ ] 2.3 시트 "적용 제안"(단계·확인·대상 선택·버튼), `draftFromNews`(목표·완료 고정, 소식 글은 머리글 아래 `> ` 인용), `NewsPage onDraft`→`App.openDraft`(교체 확인), 대상 기본 recent[0]. vitest: 초안 내용·고정 문구·인용·대상 없음 안내·시작 안 함
- [ ] 2.3a 원문 링크: 줄의 "원문 ↗"(줄 열기와 분리, 기존 원문 열기 명령 재사용), 적용 제안의 주소 글자, 초안 인용의 주소 줄. vitest: 줄 클릭과 분리·초안 주소
- [ ] 2.4 작성 창: "소식에서" 배지, `planOnlyFor` 헬퍼로 계획 잠금(출처·머리글), 다듬기 권장·다듬은 뒤 출처 유지. TS·Rust 머리글 같음 테스트. i18n ko/en

## 3. 검사·리뷰

- [x] 3.1 설계 리뷰(design-critic + security-privacy-reviewer) 반영 — design.md "설계 리뷰 반영"
- [ ] 3.2 전체 검사(core test·clippy·tsc·vitest·lint), 실제 가져오기 1회(사용자 승인, cost_usd 기록), mock 스크린샷(목록·시트·작성 창, 라이트·다크)
- [ ] 3.3 테스트 리뷰(qa-verifier + design-critic + brand-reviewer) 반영 — design.md "테스트 리뷰 반영"
