# Design: plugin-bulk-update

## 결정 1. 무엇을 고를 수 있나 (3가지 상태 규칙 그대로)

- 체크박스: `locked == null`(전역 범위, D-083) 이고 업데이트 상태가 `available` 또는 `unknown`. 단일 버튼이 활성인 조건과 같다(`latest`만 비활성).
- "모두 업데이트 (N)": `available`만. `unknown`은 실제로 최신일 수 있고 업데이트는 마켓플레이스 코드를 내 권한으로 다시 받는 일이라, 사용자가 직접 체크할 때만 넣는다.
- 선택은 화면 필터·검색과 무관하게 유지된다(확인 창이 실제 대상을 모두 적는다). 진행 중에는 체크박스·머리 버튼·단일 업데이트·토글이 모두 잠긴다.
- 최신·잠긴 줄에는 체크박스 대신 같은 폭의 빈 칸(줄 맞춤). 잠긴 이유는 기존 줄 아래 문구 그대로.

## 결정 2. IPC: 명령 2개 + 이벤트 1개

- `update_plugins(ids: string[]) -> BulkResult`: 확인 창부터 끝까지 셸이 맡는다. 웹뷰가 확인을 건너뛸 길이 없다(토큰·승인 상태를 웹뷰에 남기지 않음).
- `stop_plugin_updates()`: 멈춤 깃발만 세운다(인자 없음, 진행 중이 아니면 아무 일 없음).
- 이벤트 `plugin-update-progress {id, status}`: 줄 상태(`running`/`finished`) 갱신용. 최종 상태는 명령 반환값이 기준.
- `BulkResult { confirmed: bool, items: [{ id, status }] }`, `status`는 `waiting | running | finished{outcome} | skipped{reason} | stopped`. `outcome`은 기존 `Outcome`(done·needsApproval·locked·unchanged·timeout·failed).

## 결정 3. 셸 흐름 (`update_plugins`)

1. `PluginBusy`를 묶음 전체 동안 잡는다(단일 업데이트·토글과 겹치지 않음). 멈춤 깃발을 내린다.
2. ids: 입력 최대 50개, 중복 제거, `valid_id` 아닌 것은 `skipped{badId}`. 실행 대상은 15개까지(확인 창에 모두 적을 수 있는 수), 넘는 것은 `skipped{tooMany}`.
3. CLI 목록을 다시 읽고 core `bulk_plan(ids, installed, local_updates)`: 설치 안 됨 → `notInstalled`, 전역 아님 → `locked`, 최신 → `latest`. 나머지가 실행 대상(현재 버전, 새 버전 또는 확인 불가 문구).
4. 실행 대상이 없으면 창 없이 `confirmed: false`와 건너뜀만 돌려준다.
5. `confirm_lock`을 잡고 네이티브 경고 창 하나: 제목 "플러그인 N개 업데이트", 줄마다 `· 이름@마켓플레이스 (범위) 현재 → 새 버전`(대상 전부, 최대 15줄, 확인 불가면 "새 버전 확인 불가(최신일 수 있음)"), 건너뛴 수, 단일 업데이트와 같은 위험 문단(마켓플레이스 코드가 다음 시작부터 내 권한으로 실행, 추가 명령 승인 요구 시 실행하지 않고 터미널 안내), "하나씩 차례로 하고 하나가 실패해도 나머지는 계속해요". 버튼 "업데이트"/"취소". 취소면 `confirmed: false`.
6. 각 항목 직전에 목록과 `local_updates`를 다시 읽고 `recheck`: 동의한 스냅샷과 범위·버전·lastUpdated·창에 보인 새 버전 상태가 다르면 `skipped{changed}`(창이 열린 동안, 앞 항목이 도는 동안 바뀐 것 모두).
7. core `run_bulk`: 대상마다 차례로 — 멈춤 깃발이면 남은 것 모두 `stopped`, 아니면 `running` 알림 → 한 개 업데이트 → `finished` 알림. 한 개 업데이트는 단일과 같은 `update_one`(= `plugin_action(Update, id, scope)` 120초 제한 + 다시 읽은 목록으로 `judge_update`). 오류(CLI 실행 불가, 목록 읽기 실패)는 그 줄 `failed`로 바꾸고 계속한다.
8. 단일 `update_plugin`도 같은 `update_one`과 확인 창 재료(`update_target`)를 쓴다.

## 결정 4. 화면

- 플러그인 묶음 머리(h2 줄) 오른쪽: "모두 업데이트 (N)"(N=0이면 숨김), "선택한 것 업데이트 (N)"(N=0이면 비활성). 둘 다 작은 보조 버튼(노랑 없음, 주 버튼 없음). 고를 수 있는 플러그인이 하나도 없으면 둘 다 숨긴다.
- 진행 중: 머리에 "업데이트 중 2/3" 상태와 "멈추기" 버튼(누르면 "멈추는 중…" 비활성).
- 줄 상태(기존 메모 자리, 한 줄): 대기 · 업데이트 중… · 완료(기존 updated 문구) · 실패/승인 필요/바뀐 것 없음(기존 noteFor 문구 재사용) · 건너뜀(이유) · 멈춤. `aria-live`는 머리 상태 한 곳만(`role=status`), 줄마다 알리지 않는다.
- 끝나면 `["plugins"]`를 다시 읽고 머리 아래 요약 한 줄(`role=status`): 0이 아닌 것만 "3개 업데이트, 1개 실패, 1개 승인 필요, 1개 바뀐 것 없음, 2개 멈춤, 1개 건너뜀". 끝난 줄은 선택에서 빠진다(실패·멈춤은 남겨 다시 시도 가능).
- 확인 창 취소: 아무것도 바뀌지 않음(선택 유지).

## 보안·프라이버시

- 입력은 id 목록뿐. 범위·버전·실행 여부는 다시 읽은 CLI 목록으로 정한다. 같은 `plugin_action`(인자 배열, `--json -s <scope> <id>`, stdin 닫힘, 시간 초과 시 프로세스 그룹 종료, stderr는 분류에만)을 쓴다. `-y`·`--accept-command` 없음.
- 확인 창 문구는 `display_safe` 길이 제한 값만(id 100자, 버전 40자).
- 이벤트·반환값에는 id와 상태만. stderr·경로·MCP 설정은 없다.
- 입력 50개·실행 15개 상한, 한 번에 하나(`PluginBusy`), 각 120초 제한.

## 설계 리뷰 반영 (design-critic HIGH 2·MEDIUM 4·LOW 1, security HIGH 1·MEDIUM 2·LOW 4)

- **진행 상태가 화면 수명에 묶임(critic HIGH)**: 묶음 상태(줄 상태·진행·요약)는 컴포넌트가 아니라 모듈 저장소(`lib/pluginBulk.ts`, `useSyncExternalStore`)에 둔다. 이벤트 구독과 명령 Promise도 저장소가 가진다. 다른 탭에 갔다 와도 그대로 보인다. 새 IPC는 없다(앱 새로고침 때만 잃고, 그때는 기존 "다른 플러그인 작업 중" 오류가 보인다).
- **필터로 머리가 사라짐(critic HIGH)**: 버튼 줄은 묶음 h2가 아니라 목록 위 별도 띠로 둔다. 칩이 전체·플러그인이거나, 진행 중·요약이 있으면 검색 결과와 무관하게 보인다. N은 전체 플러그인 기준.
- **이름 없는 대상(security HIGH, critic MEDIUM)**: 실행 대상 상한 15 = 확인 창 줄 수. "외 N개" 없음. 화면도 15개까지만 고를 수 있고(넘으면 체크박스 비활성 + "한 번에 15개까지" 안내), "모두"는 앞 15개.
- **동의 뒤 변경(security MEDIUM)**: 재확인을 항목마다 직전에, 창에 보인 새 버전 상태까지 비교.
- **이유에 원문(security MEDIUM)**: 상태는 닫힌 enum뿐(`Outcome`·`Skip`). 셸 오류는 문자열 없이 `failed`로 바꾼다. 직렬화 테스트로 카나리 없음 확인.
- **같은 id 여러 범위(LOW)**: `bulk_plan`은 `scope == User` 항목을 고르고, 전역 항목이 둘 이상이면 `locked`.
- **시나리오 보강(critic MEDIUM)**: 창이 열린 동안 바뀜, 승인 필요여도 계속, 진행 중 토글 거부, 15개 상한 시나리오 추가.
- **취소·대상 없음(critic MEDIUM)**: 취소면 반환값을 무시(줄·선택 그대로). 대상 0이면 창 없이 요약 한 줄("1개 건너뜀")만.
- **선택에 남는 줄(critic MEDIUM)**: `done`만 선택에서 뺀다.
- **잠금 범위·a11y(critic LOW)**: 진행 중 AddSheet 설치는 셸 `PluginBusy`가 막고 기존 오류 문구가 보인다(화면 잠금은 범위 밖). 체크박스 이름 "{name} 선택".
- 반영 안 함: 취소 뒤 쿨다운(확인 창 반복은 사용자가 누를 때만, confirm_lock이 겹침을 막음), 앱 종료 시 고아 프로세스(단일 업데이트와 같은 기존 동작, 후속).
