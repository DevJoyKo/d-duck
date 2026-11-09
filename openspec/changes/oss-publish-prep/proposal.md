# Proposal

## Why

D.Duck을 `DevJoyKo/d-duck` 공개 저장소에 Apache-2.0으로 올린다(D-173). 공개 전 점검에서 다음 문제가 나왔다.
- 남의 세션 기록(`.jsonl`)과 제3자 PDF가 작업 폴더에 있다.
- git 루트가 홈 폴더다.
- 결정 기록과 예제에 개인·업무 정보가 있다.

사용자 결정은 다음과 같다.
- 결정 기록은 요약본만 공개한다.
- PDF와 `.jsonl`은 저장소에 넣지 않는다.
- 캡처와 에셋은 현재 버전과 맞춘다.

## What Changes

- 루트 `.gitignore`에 넣는 것:
  - `*.jsonl`, `*.pdf`, `*.mp4`
  - `openspec/decisions.md`, `openspec/requirements.md`, `openspec/research/`, `agent-system-guide-decisions.md`
  - `publish/`, 빌드 산출물, 로컬 설정
- **테스트 픽스처를 저장하지 않는다.** 수집 테스트가 쓰는 익명 기록(`tests/fixtures/basic/**/*.jsonl`)은 저장소에 넣지 않는다. 테스트가 시작될 때 없으면 생성기(`gen_fixtures.py`, 결정적 출력)로 만든다.
- **결정 요약 `docs/DECISIONS-SUMMARY.md`를 만든다.** 제품·기술·안전 결정과 리뷰 게이트 결과를 개인 맥락 없이 정리한다. README와 문서의 `decisions.md` 링크는 이 요약으로 바꾼다.
- **개인 정보를 걸러낸다.**
  - 대상 1: 예제와 테스트 데이터에 있는 실명 경로, 업무명, 다른 프로젝트 이름, 개인 일정 이름. 일반적인 예시로 바꾼다.
  - 대상 2: 공개할 문서 안의 `/Users/<실명>` 경로
- **에셋을 현재 버전과 맞춘다.**
  - 대상: README, docs, 화면 기획서 HTML이 쓰는 모든 캡처
  - 방법: 한 스크립트(`docs/assets/capture.mjs`)로 mock UI에서 다시 찍는다. 이미지 이름과 화면 상태를 한 표에 둔다.
  - 영어 화면이 생기면 같은 스크립트로 영어판도 찍는다.
- **README에서 PDF 링크를 뺀다.** 화면 기획서는 저장소에 PDF로 넣지 않는다.
- **공개용 git 저장소를 새로 만든다.** `dev_team/`에서 `git init`을 하고, 첫 커밋 전에 비밀값과 개인 정보를 다시 스캔한다. 원격 생성과 push는 사용자 확인 뒤에 한다.

## Capabilities

### New Capabilities
- `public-repository`: 공개 저장소에 들어가는 것과 빠지는 것, 재현 가능한 테스트 픽스처와 에셋

### Modified Capabilities
- 없음

## Impact

- 루트: `.gitignore`, `docs/DECISIONS-SUMMARY.md`, README 2개
- 앱: `crates/dduck-core/tests/*`가 픽스처를 쓰기 전에 생성하는 도우미, `examples/*` 등 개인 정보 걸러내기, mock 데이터 이름
- 문서: `docs/assets/capture.mjs`, 이미지 교체
