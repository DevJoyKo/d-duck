# Proposal

## Why

사용자가 리뷰 탭과 작업 보고서에서 두 가지 불편을 겪었다. 하나는 첨부(Markdown·HTML)를 파일로 받을 수 없다는 점이고, 다른 하나는 보고서 글을 블록으로 잡아 복사할 수 없다는 점이다.
- 원인 1: 첨부 헤더에는 크게 보기·닫기만 있고 저장 동작이 없다(`ReportEvidence.tsx` 첨부 헤더).
- 원인 2: 앱 전체에 `body { user-select: none }`이 걸려 있고, 입력창과 코드만 예외다(`index.css`).

## What Changes

- 보고서와 리뷰 카드의 내용 영역에서 글을 선택해 복사할 수 있게 한다(⌘C). 내용 영역은 목표, 판정, 완료 기준, 결정, 근거 설명, 명령 출력, Markdown 첨부 본문이다. 버튼·탭·사이드바처럼 조작하는 부분은 지금처럼 선택되지 않는다.
- 첨부 헤더에 두 동작을 추가한다.
  - **저장**: macOS 저장 창을 띄워 원본 `.md`·`.html` 파일을 저장한다.
  - **복사**: Markdown 원문을 복사한다. HTML 첨부에는 이 동작이 없다.
- 바뀌지 않는 것:
  - 첨부를 읽는 규칙(링크 거부, 크기 상한, UTF-8)
  - 화면 안 HTML 격리(sandbox iframe + CSP)
  - 근거 파일 경로 복사

## Capabilities

### New Capabilities
- `report-copy-save`: 보고서 내용 선택·복사와 첨부 저장·복사

### Modified Capabilities
- 없음

## Impact

- **UI**
  - `src/index.css`: 선택 가능 영역 클래스 추가
  - `ReportEvidence.tsx`, `TaskReport.tsx`, `ReviewPage.tsx`: 내용 영역 표시, 첨부 헤더 버튼
- **셸**: `save_report_attachment(id, index)` 명령을 추가한다. tauri-plugin-dialog 저장 창을 쓰며, 이 플러그인은 이미 의존성에 있다.
- **core**: 첨부 원본을 검사된 읽기 경로로 읽는 함수
- **테스트**
  - vitest: 선택 클래스, 버튼, 복사
  - Rust: 원본 읽기 규칙, 저장 경로 검증
