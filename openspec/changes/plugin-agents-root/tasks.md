## 1. 수정

- [x] 1.1 `plugins::plugin_agent_files`(루트 `agents/*.md` + plugin.json `agents`의 `.md` 파일, 정규화한 설치 폴더 안·링크 아님, 폴더 항목 무시)를 두고 플러그인 탭과 에이전트 탭이 함께 쓴다
- [x] 1.2 `plugins::plugin_roots`: installed_plugins.json `installPath`(검증, 여럿이면 모두) 우선, 기록이 없을 때만 가장 최근 버전 폴더. 에이전트 탭·스킬 탭이 쓴다
- [x] 1.3 회귀 테스트: `docs/agents`만 있는 플러그인, manifest 파일 형식·폴더·`..`·절대 경로, 기록 우선·기록 없음 최신, 버전 폴더 링크, 스킬 탭 기록 우선

## 2. 검증

- [x] 2.1 cargo test·clippy·tsc·vitest·lint 통과, 실제 사용자 데이터로 mattpocock 형식 오류 카드가 사라짐 확인(agents_probe)
- [ ] 2.2 리뷰(qa-verifier + design-critic), 결과를 decisions.md 리뷰 기록에
