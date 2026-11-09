# Tasks

## 1. 선택·복사

- [x] 1.1 `index.css` `.selectable` 유틸리티, TaskReport·ReportEvidence 내용 영역에 적용, ReviewPage 카드 `<button>` → `<article>`+"보고서 열기" 버튼(빈 곳 클릭은 선택 문자열 없을 때만 열기), vitest
- [x] 1.2 Markdown 첨부 "복사" 버튼(원문에서 bidi·제어 문자만 제거, "복사했어요" 1.5초)(Opened의 Markdown = 원문, attachment.rs:197), vitest

## 2. 첨부 저장

- [x] 2.1 core `attachment::read_original(report_dir, &Attachment)` — 열기와 같은 검사, 원본 바이트, Rust 테스트(링크·상한·비UTF-8·정상)
- [x] 2.2 셸 `save_report_attachment(id, index)`: 보고서 폴더 해석, 원본 읽기, 저장 창(기본 이름·필터), NamedTempFile::new_in(parent)+persist, HTML은 quarantine 표시, 저장 창 하나만(Busy), 취소는 Ok(false), 오류는 고정 문구 코드
- [x] 2.3 첨부 헤더 "저장" 버튼과 결과 한 줄, mock에서 동작, vitest

## 3. 검증

- [x] 3.1 cargo test·clippy, tsc, vitest, Playwright: 리뷰 카드·보고서에서 실제 끌어 선택 후 선택 문자열 확인, 스크린샷(라이트 1440·다크 900): 선택된 글, 첨부 헤더 버튼
- [x] 3.2 테스트 리뷰(qa-verifier + design-critic), 지적 조치·decisions 기록
