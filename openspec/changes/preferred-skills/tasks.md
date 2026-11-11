## 0. 게이트
- [x] 0.1 설계 리뷰: design-critic + security-privacy-reviewer. BLOCKER/HIGH를 해결하고 결과를 보고에 남긴다(decisions.md는 메인 체크아웃 전용).  — 완료: BLOCKER·HIGH 0, critic MAJOR 3·MINOR 3, security MEDIUM 3·LOW 4를 design "설계 리뷰 반영"에 반영

## 1. core
- [x] 1.1 `preferred.rs`: `PreferredItem`·`Kind`·`validate`·`Available`·`available(skills, agents)`·`prompt_safe`·`line`. (확인: `cargo test -p dduck-core preferred` — 형식, 중복, 10개, 설치된 것만, 표식·줄바꿈 정리, 2,000바이트, 플러그인 묶음)
- [x] 1.2 `team_config`: `Stored`·`Merged`·`TeamInput`의 `preferred`, `merge_in` 검사, `diff`, `reset_item("preferred")`, `policy_text_with`. (확인: 왕복·틀린 저장값·리셋·줄 위치 테스트)
- [x] 1.3 `loadout::Env.preferred`, `claude_cli::prepare_with`가 줄을 넣는다. (확인: 시스템 지시에 줄이 한 번만, 비면 이전과 같음)
- [x] 1.4 `routines::add_preferred`. (확인: 루틴 정책 문장에 줄이 들어가고 로컬 전용 줄도 남음)

## 2. 셸
- [x] 2.1 `preferred_options` 명령, `loadout_env`가 `Env.preferred`를 채움(고른 항목이 있을 때만), `start_claim`이 `add_preferred` 호출. (확인: `cargo check`, clippy)

## 3. UI
- [x] 3.1 `lib/preferred.ts`(타입·옵션 쿼리·설치 여부), `lib/team.ts` 저장에 `preferred`, mock. (확인: vitest)
- [x] 3.2 `PreferredSection` 팀 규칙 탭, i18n ko·en. (확인: vitest 추가·빼기·설치되지 않음·10개, copyRules)
- [x] 3.3 README.md·README.ko.md 가드레일 절 한 줄. (확인: 읽기)

## 4. 검증·리뷰
- [x] 4.1 `cargo test -p dduck-core`, clippy, tsc, vitest, lint. mock 1436에서 스크린샷 라이트 1440·다크 900.
- [x] 4.2 테스트 리뷰: qa-verifier + design-critic + brand-reviewer. BLOCKER/HIGH/FAIL 해결. — 완료: qa 14/15 PASS(FAIL 1은 spec 문구, 고침), critic MAJOR 2·MINOR 4, brand HIGH 0·MEDIUM 3 반영
