# Design

## Context

공개 전 점검의 BLOCKER·HIGH 목록과 사용자 결정은 decisions.md D-172~174에 있다. 현재 git 루트가 홈 폴더이므로, 이 폴더를 그대로 push하지 않는다.

## Goals / Non-Goals

**Goals:**
- 작업 폴더는 그대로 쓰면서 공개 저장소에는 안전한 것만 들어가게 한다.
- 처음 받은 사람이 테스트를 돌릴 수 있게 한다.

**Non-Goals:**
- 기존 히스토리 보존. 새 저장소이므로 히스토리가 없다.
- 영상과 DMG 배포. DMG는 GitHub Releases에 올리고 저장소에는 넣지 않는다.

## Decisions

1. **새 저장소를 `dev_team/`에 만든다.**
   - `dev_team/`에서 `git init`을 하면 홈 폴더 저장소 안에 중첩 저장소가 생긴다. 홈 폴더 저장소는 커밋이 없어 영향이 없다.
   - 사본 폴더(`publish/github`) 방식은 쓰지 않는다. 이전 사본은 낡았고, 두 곳을 맞춰 유지하는 비용이 크다.
   - 비공개 파일은 `.gitignore`로 막는다. 첫 커밋 전에 `git ls-files`를 스캔해 확인한다.
2. **픽스처는 테스트 도우미가 만든다.**
   - 도우미는 dduck-core `src/testkit.rs`의 `ensure_basic()`이다(`#[doc(hidden)] pub mod`). 단위 테스트(`prompt.rs:90`)와 통합 테스트 3곳(`ingest_fixture.rs:31`, `analytics_fixture.rs:14`, `ipc_contract.rs:24`)이 모두 이 함수를 부른다. tests/common에 두면 src 단위 테스트가 쓸 수 없다(design-critic HIGH).
   - 생성기 경로와 출력 폴더는 `CARGO_MANIFEST_DIR`로 만든 절대 경로로 넘긴다.
   - 동작 1: `basic/` 아래 `.jsonl`이 없으면 `python3 -I gen_fixtures.py basic`을 실행한다.
   - 동작 2: 여러 테스트가 동시에 부르는 경우는 `std::sync::Once`와 파일 잠금으로 막는다.
   - 생성기 출력은 결정적이다. 지금 커밋된 것과 바이트가 같은 것을 확인했다.
   - 대안인 생성기를 Rust로 다시 쓰는 방식은 비용이 크다. python3는 macOS CLT에 들어 있다.
3. **결정 요약은 새로 쓴다.**
   - 이전 `publish/github/docs/DECISIONS-SUMMARY.md`는 D-130까지만 들어 있다. 그래서 D-001~D-174와 리뷰 기록을 주제별로 다시 요약한다.
   - 주제: 데이터·개인정보, 작업 실행·안전선, 리뷰·근거, UI·문구, 배포
4. **공개 범위**(design-critic HIGH·MEDIUM)
   - 공개하는 것
     - 앱 소스
     - `openspec/specs/`, 진행 중 change, `openspec/config.yaml`
     - `CLAUDE.md`, `.claude/`(agents·commands·hooks·skills·settings.json), `docs/features`·`docs/design`·`docs/assets`·영문 화면 기획서 HTML
     - README, LICENSE, NOTICE
   - 공개하지 않는 것
     - `openspec/changes/archive/`: 개인 맥락과 실명 경로가 들어 있는 작업 이력이다. 요구사항은 메인 스펙에 합쳐져 있다.
     - `openspec/follow-ups.md`, `docs/service-intro/`(내부 소개 자료·영상), `docs/improvement-proposals.md`
     - 그 밖에 앞서 정한 것
   - 공개 파일 안의 링크는 `decisions.md` → `docs/DECISIONS-SUMMARY.md`로 바꾼다. `research/`·PDF·archive를 가리키는 링크는 지운다.
   - 첫 커밋 전에 추적 대상 md 전체를 대상으로 링크 검사를 해서, 깨진 상대 링크가 0이어야 한다.

5. **캡처 스크립트는 하나로 둔다.**
   - 파일은 `docs/assets/capture.mjs`이고 Playwright로 mock 서버(1430)를 찍는다.
   - 목록 `docs/assets/shots.json`에 이미지마다 다음을 적는다: `{file, page, query, actions, lang, theme, size}`.
   - 화면 기획서 HTML의 `img/`, README의 `docs/assets/screenshots/`, 영상 프레임 등 문서가 참조하는 이미지를 모두 이 목록에 넣는다.
   - 참조 검사 스크립트로 문서 속 이미지 경로와 목록을 대조한다.
