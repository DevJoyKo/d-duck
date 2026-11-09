# Spec Delta

## Purpose
공개 저장소에 무엇이 들어가고 무엇이 빠지는지 정하고, 저장소만 받아도 테스트와 문서 이미지를 재현할 수 있게 한다.

## ADDED Requirements

### Requirement: 저장소에서 빼는 것
공개 저장소에는 다음이 없어야 한다(MUST NOT):
- Claude 세션 기록과 모든 `.jsonl` 파일
- PDF 파일
- 원본 결정 기록(`openspec/decisions.md`)
- 한국어 화면 기획서(HTML·PDF). 영문 화면 기획서 HTML만 공개한다
- 작업 이력(`openspec/changes/archive/`), 남은 확인 목록, 내부 소개 자료(`docs/service-intro/`)
- 사내 구상 문서(`openspec/requirements.md`, `openspec/research/`)
- 개인 운영 메모
- 빌드 산출물과 배포 묶음(`publish/`, 소스 압축 파일), 공개 준비 체크리스트 같은 내부 운영 문서
- 로컬 설정

`.gitignore`가 이를 막아야 한다(SHALL). 첫 커밋 전 검사에서 위 파일이 하나라도 추적 대상이면 실패해야 한다(SHALL).

#### Scenario: 무시 확인
- **WHEN** 작업 폴더에 세션 기록 `.jsonl`, 화면 기획서 PDF, `openspec/decisions.md`가 있는 상태에서 `git add -A`를 한다
- **THEN** `git status`의 추적 대상에 이 파일들이 없다

### Requirement: 재현 가능한 테스트 픽스처
수집 테스트가 쓰는 익명 기록 픽스처는 저장소에 넣지 않고, 테스트가 쓰기 전에 생성기로 만들어야 한다(SHALL). 생성 결과는 항상 같아야 하며(SHALL), 생성기를 실행할 수 없으면 테스트는 이유를 말하고 실패해야 한다(SHALL).

#### Scenario: 생성기를 실행할 수 없음
- **WHEN** python3가 없는 환경에서 픽스처가 없는 상태로 테스트를 실행한다
- **THEN** 픽스처를 쓰는 테스트가 "python3로 tests/fixtures/gen_fixtures.py를 실행할 수 없어요" 같은 이유와 함께 실패한다

#### Scenario: 처음 받은 저장소
- **WHEN** 저장소를 새로 받아 `cargo test -p dduck-core`를 실행한다
- **THEN** 픽스처가 생성되고 수집 테스트가 통과한다

### Requirement: 결정 요약
저장소에는 제품·기술·안전 결정과 리뷰 게이트 결과를 정리한 결정 요약 문서가 있어야 한다(SHALL). 이 문서에는 개인 일정, 다른 프로젝트 이름, 실명 경로, 회사 정보가 없어야 한다(MUST NOT). 공개 문서의 결정 기록 링크는 요약을 가리켜야 하고(SHALL), 공개 문서 안의 상대 링크 중 깨진 것은 0개여야 한다(SHALL).

#### Scenario: 링크
- **WHEN** README의 안전 설계 문단에서 결정 링크를 누른다
- **THEN** 저장소 안의 결정 요약이 열린다

### Requirement: 개인 정보 없음
저장소에 들어가는 파일에는 작성자의 실명 경로(`/Users/<실명>`), 이메일, 다른 프로젝트 이름, 개인 일정, 회사·업무 이름이 없어야 한다(MUST NOT). 테스트용 가짜 비밀값은 그대로 둘 수 있다(CANARY·mock 표시).

#### Scenario: 스캔
- **WHEN** 첫 커밋 전에 개인 키워드와 비밀값 패턴으로 추적 대상 파일을 스캔한다
- **THEN** 일치하는 것은 허용 목록(테스트용 가짜 값)에 있는 것뿐이다

### Requirement: 에셋과 현재 버전 일치
README와 docs에 쓰는 화면 캡처는 한 스크립트로 현재 mock UI에서 다시 만들 수 있어야 하며(SHALL), 이미지 이름마다 페이지·상태·언어·테마를 적은 목록이 있어야 한다(SHALL). 문서가 참조하는 이미지는 모두 존재해야 한다(SHALL). 영문 문서(README.md, 영문 화면 기획서)는 영어 화면 캡처만 써야 하고, 한국어 문서는 한국어 캡처를 써야 한다(SHALL). 참조되지 않는 오래된 캡처는 지운다(SHALL).

#### Scenario: 다시 찍기
- **WHEN** 디자인이 바뀐 뒤 캡처 스크립트를 실행한다
- **THEN** 목록의 모든 이미지가 새 화면으로 바뀌고, 문서가 참조하는 이미지 중 없는 것이 0개다

### Requirement: 기여 안내
저장소에는 일반적인 오픈소스 운영 문서가 있어야 한다(SHALL): 기여 안내(개발 환경, 브랜치·커밋 규칙, DCO 서명, PR 흐름, 스펙 변경과 테스트 기준, 개인정보 규칙), 행동 강령, 보안 제보 방법(공개 이슈가 아닌 비공개 경로), 이슈·PR 템플릿, PR마다 테스트를 돌리는 CI. 문서에는 관리자의 개인 연락처를 넣지 않고 GitHub 기능(비공개 보안 제보, 이슈)으로 연락하게 해야 한다(SHALL).

#### Scenario: 처음 기여하는 사람
- **WHEN** 기여자가 README의 Contributing 링크를 따라간다
- **THEN** 개발 환경을 띄우고, 이슈를 고르고, 스펙과 테스트를 갖춘 PR을 여는 절차를 한 문서에서 따라 할 수 있고, PR 템플릿이 같은 체크 항목을 보여 준다

#### Scenario: 보안 문제 발견
- **WHEN** 기여자가 취약점을 발견한다
- **THEN** SECURITY.md가 공개 이슈 대신 GitHub 비공개 보안 제보를 쓰라고 안내한다
