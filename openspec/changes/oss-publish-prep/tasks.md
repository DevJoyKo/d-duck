# Tasks

## 1. 저장소 경계

- [x] 1.1 루트 `.gitignore`(openspec/changes/archive·openspec/follow-ups.md·docs/service-intro·docs/improvement-proposals.md·jsonl·pdf·mp4·zip·tar.gz 등 소스·배포 묶음·PUBLISH-CHECKLIST 등 내부 운영 문서·한국어 화면 기획서(docs/screen-spec/index.html)·decisions.md·requirements.md·research·agent-system-guide-decisions.md·publish·빌드 산출물·.env*·CLAUDE.local.md·.claude/settings.local.json·*.tsbuildinfo·.DS_Store)
- [x] 1.2 픽스처 도우미 dduck-core `testkit::ensure_basic()`(CARGO_MANIFEST_DIR 절대 경로, 없으면 생성, Once+잠금, 실패 시 이유), prompt.rs:90·ingest_fixture·analytics_fixture·ipc_contract에 연결, 픽스처 지운 상태에서 cargo test 통과, 두 번 생성 해시 같음 테스트, python3 없을 때 이유 메시지 테스트

## 2. 내용 정리

- [x] 2.1 개인 정보 걸러내기: examples(record_probe.rs 실명 경로·업무명 포함)·테스트 데이터·mock·tour.test.ts 금지 패턴(개인 키워드를 일반 패턴으로)·openspec/specs·진행 중 change·.claude/·공개 문서의 실명 경로·업무명·다른 프로젝트·개인 일정(공개 전 점검 HIGH 5·7, publish/PUBLISH-CHECKLIST.md 2절 참고), 테스트 통과
- [x] 2.2 `docs/DECISIONS-SUMMARY.md`(D-001~D-174, 주제별, 개인 맥락 없음), 공개 파일 전체(README·CLAUDE.md·.claude/·app/README·docs·openspec 공개분)의 decisions.md 링크를 요약으로 교체, research·PDF·archive 링크 제거, 추적 md 상대 링크 검사 0건
- [x] 2.3 THIRD_PARTY.md에 MPL-2.0(lightningcss, cssparser)·OFL-1.1(Pretendard)·Unicode-3.0 추가

- [x] 2.4 기여 안내(일반 오픈소스 운영 방식): CONTRIBUTING.md(영어, 개발 환경·브랜치·커밋·DCO sign-off·PR 흐름·OpenSpec 변경·테스트·리뷰 기준·개인정보 규칙·문구 규칙), CODE_OF_CONDUCT.md(Contributor Covenant 2.1), SECURITY.md(GitHub 비공개 보안 제보), .github/ISSUE_TEMPLATE(bug·feature·config), PULL_REQUEST_TEMPLATE.md, .github/workflows/ci.yml(PR마다 cargo test·clippy·tsc·vitest·lint, macOS), 라벨 안내(good first issue·help wanted), README 기여 절 링크

## 3. 에셋

- [x] 3.1 `docs/assets/capture.mjs` + `shots.json`: 문서가 참조하는 모든 캡처(README, screen-spec/img, service-intro 프레임) 목록화, 현재 버전으로 다시 찍기, 참조 검사(없는 이미지 0·안 쓰는 캡처 삭제)
- [x] 3.2 영어 화면 준비 후 en 캡처 추가: README.md·영문 화면 기획서(index.en.html, docs/D.Duck-screen-spec.pdf는 로컬 전용)는 en 캡처만, README.ko.md·한국어 기획서는 ko 캡처. 영문 산출물에 한글 캡처 0개 확인
- [x] 3.3 화면 기획서 전체 재촬영: 한국어(index.html, img/ ko 캡처)·영문(index.en.html, img/en/ 캡처) 모두 새 디자인으로, 사실 보정(현재 구현과 일치), 두 PDF(로컬 전용) 다시 생성
- [x] 3.4 DMG 갱신: palette-refresh·i18n·report-copy-save 반영 후 release-local.sh로 빌드·서명 검증·설치, publish/dmg 최신화(로컬 전용, 공개 저장소에는 Releases로만)
- [x] 3.5 소개 영상 갱신(docs/service-intro/video 2분 이내·소리 없음·자막): 새 디자인·새 오리 화면으로 다시 녹화, 한국어판(다나까 자막)과 영어판(영어 UI·영어 자막) 두 개, 선명도(1920x1200 이상, 텍스트 또렷), 로컬 전용(.gitignore) — 공개 시 영어판을 Releases 자산으로 올리고 README에 링크

## 4. 공개

- [x] 4.1 선행: CI 명령 전부 로컬 통과(eslint 오류 0, vitest 전부, clippy는 examples의 unwrap 정리 후 --all-targets -D warnings로 강화), 3.1 재캡처 완료·미참조 이미지 0·남는 이미지 눈 확인. `git init` 후 `git rev-parse --show-toplevel`이 dev_team인지 확인, `git add -A` 후 추적 파일 스캔(`/Users/` 뒤 me·x 외 이름, 로컬 전용 `publish/private-terms.txt`의 실명·다른 프로젝트·개인 일정·업무 키워드(`publish/scan-private.sh`), 이메일, 비밀값 패턴, jsonl·pdf·zip·db 0), 첫 커밋
- [ ] 4.2 (사용자 승인 받음, D-182) `gh repo create DevJoyKo/d-duck --public`, push, `gh repo edit --enable-discussions`, `gh api -X PUT repos/DevJoyKo/d-duck/private-vulnerability-reporting`, 설명·토픽 설정, Releases에 DMG
- [x] 4.3 Medium 초안(한국어 본문 + 영어판, publish/medium/ 로컬): 동기·이름 유래(사용자 원문), 테크 리드식 운영 4단계, 핵심 화면(새 캡처), 안전 설계, 만든 방식(스펙 주도·리뷰 게이트), 설치·GitHub 링크. 게시는 사용자가 직접
