## 1. 수정

- [x] 1.1 `plugins::plugin_agent_dirs`(루트 `agents/` + plugin.json `agents` 경로, 정규화한 루트 기준)를 두고 에이전트 카탈로그가 최신 버전 폴더마다 이것만 훑게 한다
- [x] 1.2 회귀 테스트: `docs/agents` 문서는 빠지고, 루트 agents와 plugin.json 경로는 보이며, 플러그인 형식 오류 카드가 없다

## 2. 검증

- [ ] 2.1 cargo test·clippy 통과, 실제 사용자 데이터(mattpocock-skills)로 형식 오류 카드가 사라짐 확인
- [ ] 2.2 리뷰(qa-verifier + design-critic), 결과를 decisions.md 리뷰 기록에
