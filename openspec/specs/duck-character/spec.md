# duck-character Specification

## Purpose
TBD - created by archiving change duck-scarf-polish. Update Purpose after archive.

## Requirements

### Requirement: 오리 스카프 마감
오리의 역할 색 스카프는 목(부리 아래)에 몸 곡선을 따라 둘러져야 하며(SHALL), 어떤 포즈에서도 몸 윤곽선 밖으로 나오지 않아야 한다(MUST NOT). 앞쪽에 작은 매듭이 있어야 하고(SHALL) 몸 윤곽선은 스카프 위로 끊김 없이 보여야 한다(SHALL). 여러 오리가 한 화면에 있어도 서로의 모양에 영향을 주지 않아야 한다(SHALL).

#### Scenario: 모든 포즈
- **WHEN** 에이전트 화면에 다섯 포즈의 오리가 여러 색으로 보인다
- **THEN** 스카프가 모두 몸 안에서 곡선으로 감기고 매듭이 앞쪽에 있다
