# 기능 목록 · TC 매핑 (유지보수용)

OpenSpec에 정리한 기능을 Feature ID로 묶고, 시나리오마다 QA용 TC를 연결한 문서다.

| 파일 | 내용 |
|---|---|
| [FEATURES.md](FEATURES.md) | 영역별 기능 목록: Feature ID, 상태, 요약, 근거 change, 핵심 코드, TC 수 |
| [TEST-CASES.md](TEST-CASES.md) | 기능별 TC: 조건 → 기대 결과, 자동 테스트 연결 또는 수동 절차 |
| [tc-mapping.csv](tc-mapping.csv) | 같은 TC를 스프레드시트로 정리했다. 실행 결과·실행일·비고 열은 QA 회차 기록용이다 |
| `catalog.json` | 원본 데이터. 위 세 파일은 여기서 만든다 |
| `tools/render.py` | catalog.json으로 위 세 파일을 다시 만든다 |
| `tools/check.py` | 연결된 테스트가 아직 있는지, OpenSpec에 ID 없는 요구사항이 생겼는지 점검한다 |

## ID 규칙

- **Feature ID** `F-<영역>-<NN>`: OpenSpec 요구사항(`### Requirement`) 하나가 기능 하나다. 번호는 한 번 정하면 바꾸지 않는다. 기능을 없애면 상태를 `폐기`로 두고 번호는 비워 둔다.
- **TC ID** `TC-<영역>-<NN>-<MM>`: 그 요구사항의 시나리오(`#### Scenario`)와 1:1이다.
- **영역 코드**

| 코드 | 영역 |
|---|---|
| SHL | 앱 셸·탐색 |
| ING | 기록 수집 |
| USG | 사용량 |
| TSK | 작업 |
| BRF | 작업 맡기기 |
| RPT | 보고서·리뷰 |
| AGT | 에이전트·팀 |
| EXT | 확장(스킬·MCP·플러그인·규칙·메모리) |
| ART | 아티팩트·루틴 |
| SAF | 계정·안전·한도 |
| MOB | 모바일 연동 |
| TUR | 둘러보기 |

  capability와 영역의 대응은 catalog.json의 `domains`에 있다.

## 상태 값

| 상태 | 뜻 |
|---|---|
| 구현됨 | 코드가 있고 빌드에 들어 있다 |
| 일부 구현 | 요구사항 일부만 구현됐다. 근거는 status_note에 있다 |
| 미구현 | 스펙만 있다 |
| 보류 | 코드는 있지만 스파이크 확인 전이라 꺼져 있다. 예: `AUTO_RESUME_SHIPPED=false` |

## 회귀 테스트 실행

```bash
cd app
cargo test -p dduck-core        # Rust 코어
npx vitest run                  # UI 단위
npm run dev:mock                # 수동 TC용 mock UI (http://localhost:1430)
```

## 유지보수 절차

1. OpenSpec change에 요구사항이나 시나리오를 더하거나 바꾼다.
2. `python3 docs/features/tools/check.py`를 돌리면 ID 없는 요구사항과 TC 없는 시나리오가 나온다.
3. catalog.json에 새 기능을 넣는다. 그 영역의 다음 번호를 쓰고 상태·코드·테스트 연결을 채운다.
4. 테스트 이름을 바꾸거나 지웠다면 check.py가 알려 준다. 연결을 고친다.
5. `python3 docs/features/tools/render.py`로 문서를 다시 만든다.
6. QA 회차는 tc-mapping.csv를 복사해 실행 결과 열을 채운다.

change를 아카이브하면 spec이 `openspec/specs/<capability>/spec.md`로 옮겨진다. 그때 catalog의 `change` 값은 그대로 두고, 문서의 spec 경로만 새 위치로 읽는다.

## 매핑하면서 찾은 스펙과 구현의 차이 (2026-11-09)

정리하는 동안 아래 차이를 찾았다. 어느 쪽에 맞출지 정한 뒤 스펙이나 코드를 고치고, 이 목록에서 지운다.

| 기능 | 차이 |
|---|---|
| F-SHL-02 | 스펙의 기본 페이지와 다르게, nav-ia 이후 기본 페이지는 '에이전트'다 |
| F-SHL-18 | `src/mock/memory.ts`에 옛 이름 "DevDeck"이 남아 있다 |
| TUR | 스펙의 투어 카드 높이는 540px인데 구현은 480px이다 |
| EXT (MCP) | 탭을 열면 상태 확인이 자동으로 실행된다(D-138). 이보다 앞선 스펙에는 "열기만으로 실행 안 함"이라고 되어 있다 |
| TC-AGT-01-02 | 안내 문구가 스펙과 다르다 |
| TC-BRF-05-02 | 스펙은 권고 문장을 목표에 넣는데, 구현은 목표를 비우고 참고 인용으로 넣는다 |
| TC-RPT-04-01 | 스펙은 "충족 2 · 근거 없음 1"을 한 줄로 보이는데, 구현은 줄과 배지를 따로 보인다 |
| F-ART-06·17 | nav-ia 이후 사이드바에서 루틴이 아티팩트보다 위에 있다 |
| F-ART-03 | 빈 상태 문구가 스펙은 "없습니다"이고 코드는 "없어요"다. 코드가 문구 규칙(해요체)에 맞다 |
| F-TSK-02 | 스펙의 4열 보드가 task-lifecycle에서 5칸 보드로 바뀌었다 |
| 작업 정리 테스트 | 테스트 이름에 "리뷰 탭에 완료"가 있지만 그 내용을 확인하는 단언이 없다 |

미구현 기능: review-rework(F-RPT-33~39), mobile-companion(F-MOB-01~05), 모델 선택의 화면 연결(F-BRF-13·14).
보류 기능: 스파이크 확인 전이라 꺼 둔 것이다. F-RPT-08 `report_guard::MEASURED`, F-SAF-29·30·33 `AUTO_RESUME_SHIPPED`.
