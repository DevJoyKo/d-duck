# Design: review-routine-runs

## 결정 1. 행 원천을 합친다 (루틴 실행을 `task_meta`에 복제하지 않음)

대안 A: 루틴 시작 때 `task_meta::record_start`로 기록을 하나 더 만든다. `task_meta`는 작업 탭 태그, 작업 기록(목록·삭제·내보내기), 한도 이어서 하기, 전달 범위 점검, 200개 보관 정리가 함께 쓴다. 루틴 실행이 거기 섞이면 작업 기록 200개를 루틴이 밀어내고, 보고서 보관(루틴당 20개, `KEEP_PER_ROUTINE`)과 기록 보관이 서로 다른 규칙으로 같은 폴더를 붙잡는다. 보고서 화면도 `routine: true`를 잃는다.

대안 B(채택): 리뷰 쪽 읽기만 두 표를 합친다. `review_inbox::RECORDS`는 한 SQL 조각이다.

```
(SELECT bg_id, name, cwd, task_type, created_at, reviewed_at, report_id, brief_json, prompt, delivery_alert, wrapped_at, ended_status, 0 AS routine FROM task_meta
 UNION ALL
 SELECT rr.bg_id, r.name, r.cwd, NULL, COALESCE(rr.started_at, rr.scheduled_for), rr.reviewed_at, rr.report_id, NULL, NULL, NULL, rr.wrapped_at,
        <done/failed/stopped 그대로, lost → unknown, 나머지 NULL>, 1
 FROM routine_runs rr JOIN routines r ON r.id = rr.routine_id
 WHERE rr.report_id IS NOT NULL AND rr.bg_id IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM task_meta t WHERE t.bg_id = rr.bg_id AND t.report_id IS NOT NULL)
   AND rr.id = (SELECT MAX(x.id) FROM routine_runs x WHERE x.bg_id = rr.bg_id AND x.report_id IS NOT NULL))
```

`review_inbox::rows`·`pending_count`와 `task_lifecycle::facts`·`lanes`·`board`가 모두 이 조각을 쓴다. 그래서 배지 = 리뷰 대기 칸 = 리뷰 탭 리뷰 전 수가 구조로 맞는다. 하위 질의로 감싸 쓰므로 바깥에서 `WHERE`를 붙일 수 있다. `task_meta`에 같은 bg의 보고서 있는 기록이 있으면 그 기록이 이긴다. 보고서 없는 `task_meta` 행은 루틴 보고서를 가리지 않는다(`task_report::id_for`와 같은 우선순위, 보안 리뷰 LOW). 상관 부질의를 위해 v16에 `routine_runs(bg_id)` 인덱스를 더한다.

- `created_at`: `started_at`(루틴은 claim 때 예정 시각을 넣는다, ms), 없으면 `scheduled_for`. `review_since` 판단에 쓴다.
- `ended_status`: 루틴 상태 `done`·`failed`·`stopped`는 그대로, `lost`(목록에서 사라짐)는 `unknown`, 나머지는 NULL.
- 루틴 지시는 읽지 않는다(`NULL AS prompt`). 목표는 늘 있는 루틴 이름이고, `stored`는 루틴이면 참이다. 루틴 지시는 "작업 지시 저장" 설정 대상이 아니다(원래 동작, 보고서 화면 맡긴 내용에서만 보임).

## 결정 2. 루틴 실행의 리뷰 상태는 `routine_runs`에 둔다 (DB v16)

`ALTER TABLE routine_runs ADD COLUMN reviewed_at INTEGER; ALTER TABLE routine_runs ADD COLUMN wrapped_at INTEGER;`

D-140 (1) "새 컬럼 없음"은 `task_meta` 기준이었다. 루틴 실행에는 상태를 둘 곳이 없어 시각 두 개를 더한다. 쓰기는 모두 공용 도우미 하나를 거친다: 먼저 `task_meta`(보고서 있는 기록), 없으면 그 bg의 보고서 있는 최신 루틴 실행 한 행(`id = MAX(id)`)만 바꾼다(`mark`·`unmark`·`wrap`·`finish_stopped`·`unwrap`). 루틴 상태가 `starting`·`running`·`waiting`이면 표시를 거절한다(심층 방어). 루틴 보관 정리(`set_run_report`가 21번째부터 `report_id`를 지움)나 루틴 삭제(CASCADE)로 보고서가 없어지면 카드도 함께 사라진다. 이것은 보고서 보관 규칙(D-108) 그대로다.

## 결정 3. 되살리기(backfill) 없음

실제 데이터의 "고아" 폴더 4개는 `routine_runs.report_id`에 앱이 시작 때 기록한 연결이 있다. 결정 1만으로 보이므로 따로 옮길 것이 없다. `report.json`에는 bg id·세션 id가 없다(키: version·status·updatedAt·work·criteria·decisions·blocked·references·attachments). 그러니 시각이나 내용으로 주인을 맞히는 짐작은 하지 않는다. 연결 기록이 없는 폴더는 지금처럼 보이지 않고, 한 시간 뒤 `sweep`이 지운다.

## 결정 4. 리뷰 기준 시각과 업데이트 직후 (설계 리뷰 HIGH 1)

루틴 실행도 `review_since` 뒤에 시작한 것만 리뷰 전·배지에 든다. `review_since`는 리뷰 탭이 처음 돈 시각에 이미 고정되어 있으므로, 그 뒤의 루틴 보고서(루틴당 최대 20개)는 업데이트 직후 리뷰 전으로 한 번에 나타난다. 이것은 의도한 결과다: 사용자가 보고한 문제가 "끝난 일이 리뷰 탭에 안 보인다"이고, 그 보고서들은 실제로 아무도 리뷰하지 않았다. 마이그레이션이 "리뷰했어요"를 대신 채우면 리뷰하지 않은 일을 리뷰했다고 말하게 된다. 그래서 채우지 않는다. 기준 시각 전 루틴 보고서는 전체 필터에서만 보인다.

## 결정 4b. 예전에 "정리 끝" 한 루틴 실행 (설계 리뷰 HIGH 2)

지금까지 루틴 실행의 정리 끝은 세션 키 목록(`WRAPPED_KEYS`)에만 남았다. 이를 지키려고:
- `lane_of`: 기록이 정리 끝(`wrapped_at`)이면 리뷰 대기보다 먼저 정리 끝으로 본다. 기록에는 세션 키를 보지 않는다(기존 그대로, 테스트 리뷰 LOW). `task_meta` 기록은 리뷰 대기 상태에서 정리 끝이 될 수 없으므로(`wrap`·`finish_stopped`가 거절) 기존 작업의 칸은 바뀌지 않는다.
- 리뷰 전(`is_pending`)도 정리 끝 기록은 세지 않는다. 그래서 배지 = 리뷰 대기 칸이 유지된다.
- 폴링마다 `lanes`보다 먼저 도는 `note_ended`가, 목록의 끝난 세션 키가 `WRAPPED_KEYS`에 있고 그 bg의 보고서 있는 루틴 실행이 있으면 `routine_runs.wrapped_at`을 채우고 그 키를 목록에서 뺀다(옮김). 세션 키와 bg id를 함께 아는 것은 그 목록뿐이다. 옮기므로 되돌리기 뒤 다음 폴링이 다시 정리 끝으로 만들지 않는다(테스트 리뷰 FAIL 조치).
- 목록에서 이미 사라진 예전 정리 끝 루틴 실행은 짝을 알 수 없다. 세션 id 앞부분이 bg id와 같아 보여도 CLI가 보장하는 규칙이 아니므로 짐작하지 않는다. 그런 실행은 한 번 리뷰 대기에 다시 나온다(리뷰하지 않은 보고서이므로 틀린 표시는 아니다).

## 결정 5. 쪽 상한

`MAX_OFFSET`을 `MAX_ROWS`(500)에 맞춘다. 웹뷰 `MAX_PAGES`도 500/10 = 50쪽으로 맞춘다. 셸의 offset 거절은 같은 상수를 쓰므로 그대로다.

## 보관과 삭제 (설계 리뷰 MEDIUM)

- 루틴당 21번째부터 보고서 연결이 지워지는 규칙(`KEEP_PER_ROUTINE`, D-108)은 그대로다. 리뷰하지 않은 보고서도 그때 카드가 사라진다. 리뷰 안 한 보고서를 우선 보호하는 것은 보관 규칙을 바꾸는 일이라 이번 범위 밖이다(spec 시나리오로 명시).
- "작업 기록 삭제"는 지금처럼 `task_meta`만 지운다. 루틴 보고서는 루틴이 갖고 있어 루틴을 지우면 함께 지워진다(spec에 명시). 확인창 문구는 바꾸지 않는다.

## 남은 작은 경우

루틴 상태가 `running`으로 남았는데 CLI 목록에는 없으면(스케줄러가 아직 `lost`로 바꾸기 전), 카드는 리뷰 전에 보이지만 "리뷰했어요"가 거절된다. `routines::track`이 다음 폴링에 상태를 고치므로 그대로 둔다.

## 알림·트레이

`lanes`가 루틴 실행도 리뷰 대기로 보므로, 보고서가 붙은 루틴 실행이 끝나면 완료 알림이 "리뷰 대기"를 붙이고 트레이 툴팁 리뷰 대기 수에도 든다(셸 코드는 그대로, `lanes`·`pending_count` 결과만 달라짐).

## 정리 점검표

루틴 실행은 worktree를 쓰지 않는다. 정리 점검표(`task_lifecycle::record`)는 지금처럼 `task_meta` 기록만 읽어 루틴 실행에는 "확인 못 했어요"를 보인다. 정리 끝 버튼은 점검표와 상관없이 동작한다(기존 규칙). 점검표 확장은 범위 밖이다.

## 보안·프라이버시

- 새 저장 값은 시각 두 개다. transcript·자격증명을 읽지 않는다.
- SQL 조각은 상수이고, 사용자 값은 `?` 매개변수로만 넣는다.
- 웹뷰에 보고서 id·경로를 보내지 않는다(기존 `Item` 그대로, `routine: bool`만 더함).
- bg id 검사(`valid_bg_id`)·보고서 id 검사(`valid_id`)는 합친 행에도 그대로 적용한다.

## 결정 6. 보고서 없는 D.Duck 작업은 전체에만 (D-196)

사용자 보고 2: 작업 보드에서 시작한 작업 중 보고서 없이 시작한 것(예: 조사 작업, 원격 조작 작업)은 리뷰 탭에 전혀 없다. 리뷰할 근거가 없으므로 리뷰 대상은 아니지만, "끝난 일 목록"에서 빠지지 않게 전체 필터에만 둔다.

- 원천은 같은 `RECORDS`이며 `report_id IS NULL`인 `task_meta` 행이다(루틴 실행은 보고서가 있을 때만 원천에 있다). D.Duck 밖 세션은 `task_meta`에 없으니 들어오지 않는다.
- `build`는 보고서 있는 행과 없는 행을 나눈다. 리뷰 전·배지·"다른 프로젝트에 N개"·`has_any`·`running`·500 상한은 보고서 있는 행만으로 계산한다. 그래서 배지 = 리뷰 대기 칸이 그대로다. 전체 목록은 둘을 합쳐 최신 순으로 정렬한 뒤 500개로 자른다.
- 실행 중(마지막 목록 기준)이거나 목록을 아직 못 읽었으면(`known=false`) 넣지 않는다. 끝났는지 모르기 때문이다.
- 같은 bg에 루틴 보고서가 있으면 보고서 없는 `task_meta` 행은 뺀다(카드 하나).
- 카드: `CardReport::None`, `ended`(`task_meta.ended_status` 코드). 응답에 `noReport` 수를 더해, 보고서 설정이 꺼져 있고 보고서 기록이 없어도 범위·필터를 숨기지 않는다.
- "작업 보기": CLI 목록에 있으면 기존 `dduck-open-task`(보드 서랍), 없으면 `#record=<bg>`로 작업 탭에 가서 지난 작업 행을 펼친다(행이 20개 뒤여도 보이게). 새 백엔드 명령은 없다.
- 원격 조작 칩: 기존 화면에 그런 칩이 없어 더하지 않는다.
- 스킬 작성 작업("Claude에게 부탁하기")도 `task_meta` 기록이라 함께 보인다(지난 작업과 같은 범위).
- 테스트 리뷰 반영(D-196): 보고서가 꺼져 있고 보고서 없는 작업만 있어도 "작업 보고서가 꺼져 있어요 · 설정 열기"를 보인다. 이름도 목표도 없는 기록은 "이름을 남기지 않은 작업이에요"(시각 먼저). 지난 작업으로 열린 행은 펼침 버튼에 포커스를 준다. 칩 문구는 결정대로 "보고서 없음"이다. `rows`는 보고서 없는 행도 읽기 상한(5000) 안에서 읽는다. 보고서 있는 행이 4800개를 넘어야 배지와 어긋나는데, `task_meta` 200개·루틴당 20개 보관이라 사실상 생기지 않아 그대로 둔다.
