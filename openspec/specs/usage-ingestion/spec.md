# usage-ingestion Specification

## Purpose
로컬 Claude Code transcript를 증분으로 읽어 사용량·툴·스킬 이벤트를 중복 없이 정규화 저장하고, 원문 데이터는 남기지 않는 수집 규칙을 정의한다.

## Requirements

### Requirement: 수집 대상
앱은 Claude 설정 디렉터리(설정 화면 지정값, 없으면 `CLAUDE_CONFIG_DIR`, 없으면 `~/.claude`)의 `projects/` 하위에서 `<프로젝트>/<세션ID>.jsonl`과 `<프로젝트>/<세션ID>/subagents/agent-<ID>.jsonl`(및 같은 이름의 `.meta.json`)만 읽어야 한다(SHALL). 심볼릭 링크와 `projects/` 밖을 가리키는 경로는 읽지 않아야 하며(MUST NOT), 원본 파일은 수정하지 않는다.

#### Scenario: 서브에이전트 포함
- **WHEN** 세션 S의 transcript와 그 서브에이전트 transcript가 모두 있다
- **THEN** 서브에이전트의 사용량은 세션 S에 연결되고, 서브에이전트 유형별로 구분해 조회할 수 있다

#### Scenario: 심볼릭 링크
- **WHEN** `projects/x/link.jsonl`이 `projects/` 밖 파일을 가리키는 심볼릭 링크다
- **THEN** 그 파일은 수집되지 않는다

### Requirement: 증분 수집
앱은 시작 시 전체 백필을 수행하고, 이후에는 파일 변경을 감지해 새로 추가된 줄만 처리해야 한다(SHALL). 개행으로 끝나지 않은 마지막 줄은 완성될 때까지 처리하지 않아야 한다(SHALL). 파일이 이전 처리 위치보다 작아지면 처음부터 다시 처리해야 한다(SHALL).

#### Scenario: 진행 중인 세션
- **WHEN** 활성 세션의 transcript에 새 응답이 기록된다
- **THEN** 10초 이내에 해당 사용량이 통계에 반영된다

#### Scenario: 쓰는 중인 줄
- **WHEN** 파일의 마지막 줄이 개행 없이 끝나 있다
- **THEN** 그 줄은 건너뛰고, 개행이 추가된 뒤 다음 처리에서 수집된다

### Requirement: 응답 중복 제거
같은 API 응답(같은 message id와 request id)이 여러 줄에 기록되면 한 응답으로 합쳐야 하며(SHALL), 줄마다 토큰 값이 다르면 항목별 최댓값을 써야 한다(SHALL). 같은 파일을 다시 수집해도 결과가 달라지지 않아야 한다(SHALL).

#### Scenario: 블록별 반복 기록
- **WHEN** 하나의 응답이 thinking, text, tool_use 블록으로 나뉘어 3줄에 같은 usage로 기록되어 있다
- **THEN** 그 응답의 토큰은 한 번만 합산된다

#### Scenario: 줄마다 증가하는 usage
- **WHEN** 같은 응답의 줄들에서 output 토큰이 10, 40, 120으로 증가한다
- **THEN** 그 응답의 output 토큰은 120으로 한 번 집계된다

#### Scenario: 재수집
- **WHEN** 사용자가 "전체 다시 수집"을 실행한다
- **THEN** 수집 후 집계 값이 재수집 전과 같다

### Requirement: 집계 제외 레코드
모델이 `<synthetic>`인 응답과 API 오류 메시지로 표시된 응답은 사용량 집계에서 제외해야 한다(SHALL). 툴 호출은 호출 ID당 한 번만 집계해야 한다(SHALL).

#### Scenario: synthetic 응답
- **WHEN** 모델이 `<synthetic>`이고 request id가 없는 응답이 있다
- **THEN** 그 응답은 토큰과 응답 수 집계에 포함되지 않는다

### Requirement: 수집 항목
앱은 응답별로 시각, 세션, 프로젝트, git 브랜치, 모델, input/output/cache write/cache read/thinking 토큰을 저장해야 한다(SHALL). 툴 호출별로 툴 이름, MCP 서버, 스킬 이름, 서브에이전트 유형을 저장해야 한다(SHALL). 세션별 공식 누적 비용은 프로세스 실행 구간마다의 최종값을 합산해 저장해야 한다(SHALL).

#### Scenario: 재개된 세션의 비용
- **WHEN** 한 세션 파일에 서로 다른 실행 구간의 누적 비용 기록이 있고, 구간 1의 최종값이 $2, 구간 2의 최종값이 $3이다
- **THEN** 그 세션의 공식 누적 비용은 $5로 저장된다

### Requirement: 명령과 스킬 호출 기록
Skill 도구 호출은 스킬 호출(경로: 도구)로 기록해야 한다(SHALL). 사용자가 직접 입력한 슬래시 명령은 사용자 입력 레코드에서만 추출하고, 내장 명령은 "명령"으로, 그 밖의 명령은 스킬 호출(경로: 슬래시)로 구분해 기록해야 한다(SHALL). 이름은 영숫자와 `:_.-`로 된 64자 이하일 때만 저장한다(SHALL).

#### Scenario: 두 경로의 스킬 호출
- **WHEN** 한 세션에서 Skill 도구로 `openspec-propose`가 1회, 슬래시 명령 `/opsx:propose`가 1회 실행되었다
- **THEN** 두 호출이 모두 스킬 호출로 기록되고, 호출 경로(도구/슬래시)가 구분된다

#### Scenario: 내장 명령
- **WHEN** 사용자가 `/upgrade`를 실행했다
- **THEN** 그 호출은 명령으로 기록되고 스킬 호출 수에는 포함되지 않는다

#### Scenario: 툴 결과 속 태그
- **WHEN** 툴 결과나 첨부 레코드의 본문에 명령 태그 문자열이 들어 있다
- **THEN** 그 문자열은 명령이나 스킬로 기록되지 않는다

### Requirement: 원문 비저장
앱은 프롬프트, 응답 텍스트, thinking, 툴 입력·출력, 명령 인자, 스킬 인자, 서브에이전트 작업 설명, `history.jsonl` 내용을 저장해서는 안 된다(MUST NOT). 이 금지는 DB뿐 아니라 로그 파일, 오류·패닉 메시지, 화면 저장소에도 적용된다. 파싱 오류는 파일 경로, 줄 번호, 오류 종류만 기록해야 한다(SHALL).

#### Scenario: 원문 유출 검사
- **WHEN** 저장이 금지된 모든 자유 텍스트 위치에 고유 표식 문자열을 넣은 transcript를 수집한다
- **THEN** 앱 DB 파일(WAL 포함)과 앱 로그 어디에서도 그 표식이 발견되지 않는다

### Requirement: 세션 제목 저장 선택
세션 제목(Claude가 생성한 요약 제목)은 저장하되, 설정에서 끌 수 있어야 한다(SHALL). 끄면 저장된 제목을 모두 삭제하고 이후 수집하지 않아야 한다(SHALL). 제목, 프로젝트 경로, 브랜치는 알림 본문과 로그에 넣지 않아야 한다(MUST NOT).

#### Scenario: 알림과 로그의 메타데이터
- **WHEN** 세션 제목·경로·브랜치에 고유 표식이 들어간 세션이 입력 대기로 바뀌고 수집 오류도 발생한다
- **THEN** 알림 본문과 앱 로그 어디에도 그 표식이 없다

#### Scenario: 제목 저장 끄기
- **WHEN** 사용자가 세션 제목 저장을 끈다
- **THEN** 기존 세션 제목이 삭제되고 세션 목록에는 세션 ID 앞부분이 대신 표시된다

### Requirement: 손상·과대 레코드 내성
파싱할 수 없는 줄, 알 수 없는 레코드 유형, 16MB를 넘는 줄은 건너뛰고 수집을 계속해야 한다(SHALL). 건너뛴 줄 수는 파일별로 기록해 설정 화면에서 확인할 수 있어야 한다(SHALL).

#### Scenario: 깨진 JSON 줄
- **WHEN** transcript 중간에 JSON이 아닌 줄이 있다
- **THEN** 그 줄만 건너뛰고 나머지 줄은 정상 수집되며, 건너뛴 줄 수가 1 증가한다

#### Scenario: 거대한 줄
- **WHEN** transcript에 17MB짜리 한 줄이 있다
- **THEN** 그 줄은 건너뛰고 건너뛴 줄 수가 1 증가하며, 앱 메모리 사용이 그 줄 크기만큼 늘지 않는다

### Requirement: 데이터 파일 보호
앱 DB 파일은 소유자만 읽고 쓸 수 있는 권한으로 만들어야 한다(SHALL).

#### Scenario: 파일 권한
- **WHEN** macOS 또는 Linux에서 앱이 DB를 처음 만든다
- **THEN** DB 파일 권한은 0600, 앱 데이터 디렉터리 권한은 0700이다
