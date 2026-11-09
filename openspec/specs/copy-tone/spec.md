# copy-tone Specification

## Purpose
TBD - created by archiving change copy-sweep. Update Purpose after archive.

## Requirements

### Requirement: 문구 통일
앱의 화면 문구는 해요체여야 하며(SHALL) 설명은 항목마다 한 줄이어야 한다(SHALL). 리뷰 관련 용어는 '리뷰'로 통일해야 하고(SHALL) 사용자 범위는 모든 화면에서 같은 이름("전역")으로 불러야 한다(SHALL).

#### Scenario: 혼용 없음
- **WHEN** 금지 패턴 테스트가 화면 소스 문자열을 검사한다
- **THEN** '습니다'로 끝나는 화면 문구와 '검토 용량/검토함/검토 필요/검토 대기'가 없다
