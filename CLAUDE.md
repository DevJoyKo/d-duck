# D.Duck

Claude Code 사용자를 위한 로컬 데스크톱 컨트롤 센터(Tauri 2). 결정 요약(공개): `docs/DECISIONS-SUMMARY.md`. 원본 결정 로그 `openspec/decisions.md`는 로컬 전용이다(.gitignore).

## 작업 규칙

- **OpenSpec 스펙 주도**: `/opsx:propose` → `/opsx:apply` → `/opsx:archive`. 활성 change에 미완료 태스크가 없으면 훅이 코드 수정을 차단한다.
- **자율 진행**: 사용자에게 묻지 말고 추천안으로 진행한다. 되돌리기 어렵거나 외부에 영향을 주는 행동(배포, 푸시, 삭제, 결제)만 확인을 받는다.
- **결정 기록**: 모든 결정(제품·기술·프로세스, 리뷰 결과 조치 포함)은 `openspec/decisions.md`에 기록한다. 번복할 때는 기존 항목을 지우지 않고 대체 표시를 한다.
- **다중 리뷰 게이트** (항상 리뷰 에이전트 2개 이상):
  - 설계 단계(change 제안 후, 태스크 그룹 착수 전): `design-critic` + `security-privacy-reviewer`
  - 테스트 단계(태스크 그룹 완료 후, 아카이브 전): `qa-verifier` + `design-critic`
  - BLOCKER/HIGH/FAIL은 해결한 뒤 다음 단계로 넘어간다. 결과와 조치는 `openspec/decisions.md`의 "리뷰 기록"에 남긴다.
- 앱은 Claude 자격증명을 읽거나 저장·전송하지 않는다. transcript 원문은 저장하지 않는다.
