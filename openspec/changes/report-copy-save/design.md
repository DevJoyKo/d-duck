# Design

## Context

- 앱 전체에 `body { user-select: none; cursor: default }`(`src/index.css`)가 걸려 있다. 데스크톱 앱처럼 보이게 하려는 설정이고, 입력창·코드·pre만 예외다.
- 첨부는 `report_attachment(id, index)`가 Rust에서 읽어 정리한 결과(HTML은 srcdoc, Markdown은 SafeMarkdown용 글)로 온다. 원본 파일은 보고서 폴더 `attachments/`에 있다.
- `tauri-plugin-dialog`는 이미 의존성에 있다.

## Goals / Non-Goals

**Goals**
- 읽는 글은 복사할 수 있고, 조작하는 요소는 지금처럼 앱답게 둔다.
- 첨부를 사용자가 고른 위치에만 저장한다.

**Non-Goals**
- 근거 스크린샷·테스트 리포트 파일 저장. 이번에는 경로 복사로 충분하다.
- 보고서 전체 내보내기
- HTML 첨부 원문 복사

## Decisions

1. **선택은 허용 목록 방식으로 연다.**
   - 전역 `none`은 유지하고, 내용 영역에만 `.selectable { user-select: text; cursor: text }` 유틸리티를 붙인다.
   - 대안인 전역 허용 후 조작 요소만 막기는 사이드바·버튼 라벨 선택 같은 앱답지 않은 동작이 여기저기 생기므로 쓰지 않는다.
   - 리뷰 카드는 지금 카드 전체가 `<button>`이다(`ReviewPage.tsx:78-83`). WebKit은 버튼 안의 글을 끌어 선택하지 못한다(design-critic HIGH). 그래서 구조를 바꾼다.
     - 카드는 `<article>`로 바꾸고, 빈 곳을 클릭하면 열리게 한다. 선택된 글이 있으면 열지 않는다.
     - 키보드와 보조기기용으로 "보고서 열기" 버튼을 따로 둔다(포커스 순서 유지).
     - 목표·판정·기준 줄은 `.selectable` 일반 텍스트로 둔다.
     - "리뷰했어요" 버튼은 지금처럼 따로 둔다.
2. **저장은 셸이 모두 처리한다.** 웹뷰는 `(id, index)`만 보낸다.
   - 셸은 작업 기록에서 보고서 폴더를 찾는다.
   - core의 `attachment::read_original`로 원본 바이트를 읽는다. 열기와 같은 규칙(링크 거부, 일반 파일, real path 재확인, 크기 상한, UTF-8)을 적용한다.
   - 그다음 `DialogExt::file().set_file_name(file).add_filter(kind)`로 저장 창을 띄운다.
   - 사용자가 고른 경로에 원자적으로 쓴다. 고른 폴더 안에 `tempfile::NamedTempFile::new_in(parent)`로 임시 파일을 만들고 `persist()`로 바꾼다. 실패하면 임시 파일을 지운다(보안 MEDIUM).
   - 저장 창은 한 번에 하나만 띄운다. 이미 열려 있으면 바로 `Busy`를 돌려준다.
   - 오류는 고정 문구 코드로만 보낸다. 웹뷰에 io 오류나 경로를 보내지 않는다.
   - 웹뷰가 경로를 넘기는 API는 만들지 않는다.
3. **HTML은 원본을 저장한다.**
   - 보고서 폴더는 사용자 프로젝트가 아니라 앱 데이터 폴더에 있다(`lib.rs:3321`). 그래서 저장하면 원본 HTML이 처음으로 앱 밖에 나온다(design-critic MEDIUM). 그래도 사용자가 원하는 것은 에이전트가 만든 산출물이다. 정리한 버전은 링크가 글자로 바뀌어 산출물로 쓸 수 없다.
   - 저장한 HTML에는 `com.apple.quarantine` 표시를 붙인다. 브라우저로 열 때 macOS가 다운로드 파일처럼 다루게 하기 위해서다(보안 LOW).
   - 저장한 파일을 브라우저로 여는 것은 사용자의 선택이다. 그 파일은 이미 사용자 프로젝트 안의 보고서 폴더에 있는 파일과 같다. 앱 안 표시는 계속 sandbox로 한다.
4. **복사는 원문을 다시 읽지 않는다.**
   - Markdown 복사는 이미 받은 `Opened`의 Markdown 글을 쓴다. 원문 그대로임을 확인했다(`attachment.rs:197`, 보안 리뷰).
   - 복사할 때는 화면 표시와 같은 규칙으로 bidi 제어 문자와 보이지 않는 제어 문자를 지운다(`safeMarkdown.ts`의 정리 함수 재사용). Markdown 기호는 그대로 둔다.
   - 클립보드에는 `navigator.clipboard.writeText`를 쓴다. 기존 경로 복사와 같다.

## Risks / Trade-offs

- **선택과 클릭의 충돌**: 카드 클릭과 끌어 선택이 겹친다. 선택 문자열이 있으면 클릭을 무시하는 방식으로 막는다.
- **원본이 바뀌었을 수 있음**: 저장 시점에 읽기 규칙을 다시 검사한다. 규칙을 어기는 파일(링크·상한 초과·비 UTF-8)은 거부한다. 규칙을 지키면서 내용만 바뀐 경우에는 저장 시점의 내용을 저장한다(sig는 비교하지 않음).
