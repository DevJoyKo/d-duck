# 리뷰 탭에 루틴 실행 보고서도 보이기

## Why

사용자 보고: "리뷰 탭에 끝난 작업이 다 안 보여요." 실제 데이터(읽기 전용 조사, 2026-11-11)에서 보고서 폴더는 6개인데 리뷰 탭이 읽는 `task_meta`에는 보고서가 붙은 기록이 2개뿐이었다. 나머지 4개(`7ab1aaa7…`·`53ce4544…`·`9259d393…`·`b062649a…`)는 모두 `routine_runs.report_id`에 이미 연결된 루틴 실행 보고서다(run 2·5·4·1, bg `bba9b3f7`·`f3ea1594`·`66baf45b`·`ec445fdb`).

원인: 루틴 실행은 `start_claim`(src-tauri/src/lib.rs)에서 보고서 폴더를 붙이고 `task_report::set_run_report`로 `routine_runs.report_id`에 저장한다. 그런데 리뷰 탭(`review_inbox::rows`·`pending_count`)과 작업 보드의 리뷰 대기·정리 칸(`task_lifecycle::facts`·`lanes`·`board`)은 `task_meta WHERE report_id IS NOT NULL`만 읽는다. review-inbox spec도 "루틴 실행 … 보이지 않는다(SHALL NOT 표시)"라고 정해 두었다(D-140). 보고서 화면(`task_report::target`)은 이미 루틴 실행을 읽을 수 있으니, 목록에서만 빠져 있다.

함께 본 것: 작업 `fec1ec43`(09:15:26 시작)은 `task_meta.remote = 1`(휴대폰 원격 제어)이라 설계대로 보고서가 없다(`attach_report`가 원격에는 붙이지 않음). 보고서 `9259d393`은 `routine_runs` run 4(bg `66baf45b`)에 앱이 직접 기록한 것이다. 둘은 관계가 없다. 그리고 리뷰 탭은 offset 200(21쪽)까지만 받는데 목록은 500개까지 세므로, 22쪽부터 오류가 난다.

## What Changes

- 리뷰 탭·사이드바 배지·작업 보드 리뷰 대기/정리 칸이 `task_meta` 기록과 함께, 보고서가 붙은 루틴 실행(`routine_runs.report_id`)도 읽는다. 둘은 하나의 행 원천(같은 SQL)을 써서 배지·리뷰 대기 칸·리뷰 탭 수가 늘 같다.
- 루틴 실행의 리뷰했어요·정리 끝 상태를 저장할 곳으로 `routine_runs.reviewed_at`·`wrapped_at`을 더한다(DB v16). 리뷰했어요·되돌리기·정리 끝·끝내고 넘기기가 루틴 실행에도 같은 규칙으로 동작한다.
- 루틴 실행 카드는 폴더·시각 줄 앞에 "루틴"을 붙여 구분한다. 묶음은 유형 없음("기본")이다.
- 쪽 넘김 상한을 목록 상한(500개, 50쪽)에 맞춘다.
- 이미 쌓인 보고서: 되살리기(backfill)는 따로 하지 않는다. 고아처럼 보이던 4개는 `routine_runs`에 앱이 기록한 연결이 이미 있어서 이 변경만으로 보인다. `report.json`에는 작업 id가 없어 그 내용으로 주인을 짐작하지 않는다.

## Capabilities

### Modified Capabilities
- `review-inbox`: 리뷰 대상에 보고서가 붙은 루틴 실행 포함, 쪽 수 상한.
- `task-lifecycle`: 리뷰 대기·정리 칸과 정리 끝이 루틴 실행 기록도 다룬다.

## Impact

- core: `db.rs`(v16), `review_inbox.rs`, `task_lifecycle.rs`.
- shell: 코드 변경 없음(명령 시그니처 그대로). 알림 "리뷰 대기"·트레이 수는 core 결과를 따라 루틴도 포함한다.
- webview: `ReviewPage.tsx`(쪽 상한·루틴 표시), `api.ts`(`routine` 필드), mock.
- 개인정보: 새로 저장하는 값은 시각 두 개뿐이다. 목록은 루틴 지시를 읽지 않고 루틴 이름을 목표로 싣는다.
