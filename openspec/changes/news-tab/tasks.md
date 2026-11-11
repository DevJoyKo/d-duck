# Tasks

## 1. core

- [ ] 1.1 `news` 모듈: `source_allowed`, 가드 판정(`guard_allows`, 카운터, `GUARD_FLAG`), 프롬프트·인자(`known` 줄 포함). 테스트: 허용·거부 주소(호스트·경로·http·계정·포트·`..`), 가드 키·횟수, 인자 필수 플래그
- [ ] 1.2 `parse_output`: 결과 줄·오류 종류·코드 펜스·스키마·길이·카테고리·버전·날짜·주소·위험 문자·20개·중복. 테스트: 잘못된 항목만 버림, 최상위 오류, 예산·횟수
- [ ] 1.3 마이그레이션 v17, `merge`(덮어쓰지 않음·언어별 500개), `list`(정렬), `known`, `mark_seen`, 상태·매일 설정 저장/읽기, `daily_due`. 테스트: 합치기·중복, 실패 시 유지, 새 것만(id 목록), 언어별, 24시간
- [ ] 1.4 i18n 코드: 확인 창(가져오기·매일 켜기), 비용, 실패 문구 ko/en

## 2. shell

- [ ] 2.1 `news_cmds.rs`: 명령 6개, 실행 공용 함수(가드 자체 검사, cwd 정리, busy·취소), 수동 확인 창, 매일 확인 창, `on_tick`, `news-updated` 이벤트, `main.rs` 가드 모드, 등록

## 3. webview

- [ ] 3.1 nav `news`, 사이드바 항목, App 연결, api 타입·래퍼, i18n `news` ko/en
- [ ] 3.2 `NewsPage`: 목록(정렬·이미 나온 기능·새 소식 점·썸네일), 상세 시트, 빈 상태, 상태 줄, 가져오기·취소·게이트, 매일 스위치. mock 12개. vitest

## 4. 검증·리뷰

- [x] 4.1 설계 리뷰(design-critic + security-privacy-reviewer) 반영 — design.md "설계 리뷰 반영"
- [ ] 4.2 전체 검사(core test·clippy·tsc·vitest·lint), mock 스크린샷(라이트 1440·다크 900)
- [ ] 4.3 테스트 리뷰(qa-verifier + design-critic + brand-reviewer) 반영 — design.md "테스트 리뷰 반영"
