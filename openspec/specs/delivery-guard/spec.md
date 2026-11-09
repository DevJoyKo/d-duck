# delivery-guard Specification

## Purpose
D.Duck이 맡긴 작업이 결과를 어디까지 내보낼 수 있는지(전달 범위)를 정한다. 기본은 "PR 초안까지", 사용자가 체크한 프로젝트는 "로컬 작업만"이다. 범위는 규칙 문장, CLI 거부 규칙, (로컬만) PreToolUse 훅, 로컬 git 사후 점검으로 지키고 시작 전과 보고서에 보여 준다.

## Requirements

### Requirement: 전달 범위 결정
- D.Duck이 시작하거나 잇는 모든 로컬 작업(Composer, 다시 시키기, 루틴, 스킬 작성, 사용량 리셋 후 재개)은 시작 직전에 범위를 하나 정해야 한다(SHALL): `draft_pr` 또는 `local_only`.
- 작업의 저장소 루트가 체크한 프로젝트의 저장소 루트와 같거나, 작업 폴더가 체크한 폴더와 같거나 그 아래면 `local_only`여야 한다(SHALL). 그 밖은 `draft_pr`이다.
- 경로 비교는 링크를 푼 정규화 경로를 Unicode NFC로 바꾼 뒤 대소문자를 무시하고 경로 성분 단위로 해야 한다(SHALL).
- worktree(D.Duck worktree, 외부 위치의 `git worktree`)는 공용 저장소의 루트로 비교해야 한다(SHALL).
- 범위는 팀 규칙 주입 여부, 작업 유형, 권한 모드, 페르소나와 상관없이 적용해야 한다(SHALL). 판정에 네트워크를 쓰지 않아야 한다(MUST NOT).

#### Scenario: 체크하지 않은 프로젝트
- **WHEN** 체크 목록이 비어 있고 `/work/app`에서 작업을 시작한다
- **THEN** 범위는 `draft_pr`다

#### Scenario: 체크한 프로젝트의 worktree
- **WHEN** `/work/client`가 체크되어 있고 작업 cwd가 `/work/client/.claude/worktrees/fix-1`이다
- **THEN** 범위는 `local_only`다

#### Scenario: 외부 위치의 git worktree
- **WHEN** `/work/client`가 체크되어 있고 `/tmp/wt`가 `git worktree add`로 만든 그 저장소의 worktree다
- **THEN** `/tmp/wt`에서 시작한 작업의 범위는 `local_only`다

#### Scenario: 대소문자·정규화가 다른 경로
- **WHEN** `/Work/Client`(NFD 한글 폴더명 포함)로 체크했고 `/work/client`(NFC)에서 작업을 시작한다
- **THEN** 범위는 `local_only`다

#### Scenario: 이름이 겹치는 이웃 폴더
- **WHEN** `/work/client`가 체크되어 있고 `/work/client2`에서 작업을 시작한다
- **THEN** 범위는 `draft_pr`다

#### Scenario: 루틴
- **WHEN** 체크한 프로젝트 폴더에서 도는 로컬 루틴이 실행된다
- **THEN** 그 실행도 `local_only` 거부 규칙·훅·규칙 문장을 받는다

### Requirement: 범위를 모르면 시작하지 않음
- 체크 목록을 읽지 못하거나, 저장소 루트·시작 브랜치 판정이 오류로 끝나면 작업을 시작(또는 재개)하지 않아야 한다(MUST NOT). `draft_pr`로 대신하지 않는다.
- 사용자에게 "원격 push 막기 설정을 못 읽었어요"를 보여 줘야 한다(SHALL).
- 저장소가 아닌 폴더(`.git` 없음)는 오류가 아니라 "저장소 없음"으로 판정한다.

#### Scenario: 설정 손상
- **WHEN** 저장된 체크 목록 값이 JSON이 아니다
- **THEN** 작업이 시작되지 않고 "원격 push 막기 설정을 못 읽었어요"가 보인다

#### Scenario: 재개 때 설정 손상
- **WHEN** 사용량 리셋 후 재개 직전에 체크 목록을 못 읽는다
- **THEN** 재개가 실패로 기록되고 `claude`가 실행되지 않는다

### Requirement: 같은 범위 인자를 모든 실행 경로에
- 시작과 재개는 같은 방법으로 만든 범위 인자를 붙여야 한다(SHALL): `--disallowedTools=`(항상), `local_only`면 훅을 담은 `--settings=`.
- 거부 규칙과 훅 설정은 번들 데이터에서만 만들어야 한다(SHALL). 웹뷰가 보낸 값이나 저장한 팀 설정이 빼거나 바꾸지 못해야 한다(MUST NOT).

#### Scenario: 재개 인자
- **WHEN** `local_only` 프로젝트의 작업을 사용량 리셋 후 잇는다
- **THEN** 재개 인자에 `Bash(git push *)`가 든 `--disallowedTools=`와 훅이 든 `--settings=`가 있다

#### Scenario: 웹뷰 필드 무시
- **WHEN** 웹뷰가 시작 요청에 `denyTools: []`를 넣어 보낸다
- **THEN** 무시되고 기본 거부 목록이 붙는다

### Requirement: 잠금 규칙 문구 "PR 초안까지"
- 잠금 규칙 `branch`의 번들 문구는 다음 뜻을 담아야 한다(SHALL): 기본 브랜치에 직접 커밋·push·merge하지 않는다. 작업 브랜치에서 커밋하고, push와 PR은 초안(`gh pr create --draft`)까지만 한다. PR을 머지하거나 초안을 리뷰 준비됨으로 바꾸지 않는다.
- 저장한 팀 설정의 `branch` 덮어쓰기 text는 쓰지 않아야 한다(MUST NOT).

#### Scenario: 저장한 팀 설정이 branch 문구를 바꿈
- **WHEN** 저장한 팀 설정의 `branch` 덮어쓰기 text가 "push 마음대로"다
- **THEN** 규칙 문장은 번들 문구를 쓰고 거부 목록은 그대로다

### Requirement: 기본 거부 목록
- 모든 작업의 거부 목록은 적어도 다음을 담아야 한다(SHALL):
  - 머지·준비됨: `Bash(gh pr merge *)`, `Bash(gh pr ready *)`, `Bash(gh api *merge*)`, `Bash(gh api *ReadyForReview*)`, `Bash(gh api *draft=false*)`, `mcp__*__merge_pull_request`, `mcp__*__update_pull_request(draft:false)`, `mcp__*__create_pull_request(draft:false)`
  - 기본 브랜치 B(`main`, `master`, 감지값)마다: `Bash(git push * B)`, `Bash(git push * B *)`, `Bash(git push *:B)`, `Bash(git push *refs/heads/B*)`
  - 한꺼번에 push: `Bash(git push --all*)`, `Bash(git push * --all*)`, `Bash(git push --mirror*)`, `Bash(git push * --mirror*)`
  - 강제 push: `Bash(git push --force *)`, `Bash(git push * --force)`, `Bash(git push * --force *)`, `Bash(git push -f *)`, `Bash(git push * -f)`, `Bash(git push * -f *)`, `Bash(git push * +*)`
- 시작 브랜치가 기본 브랜치면 `Bash(git push)`, `Bash(git push origin)`, `Bash(git push * HEAD*)`도 담아야 한다(SHALL).
- `--force-with-lease`로 작업 브랜치를 다시 push하는 것은 막지 않는다.
- 감지한 기본 브랜치 이름은 `[A-Za-z0-9._/-]` 1~100자, `-`로 시작하지 않고 `..`가 없을 때만 넣어야 한다(SHALL).

#### Scenario: 머지 명령
- **WHEN** 작업이 `gh pr merge 12 --squash`를 실행하려 한다
- **THEN** 거부 규칙으로 막힌다

#### Scenario: 초안 PR
- **WHEN** 작업 브랜치에서 `git push -u origin fix-1`과 `gh pr create --draft`를 실행한다
- **THEN** 둘 다 거부 규칙에 걸리지 않는다

#### Scenario: 기본 브랜치 push
- **WHEN** 작업이 `git push origin HEAD:refs/heads/main` 또는 `git push --all`을 실행하려 한다
- **THEN** 막힌다

#### Scenario: 감지한 기본 브랜치
- **WHEN** 저장소의 `origin/HEAD`가 `origin/develop`을 가리킨다
- **THEN** 거부 목록에 `Bash(git push * develop)`이 들어간다

#### Scenario: 이상한 기본 브랜치 이름
- **WHEN** 감지한 이름이 `x) Edit(`이다
- **THEN** 그 이름은 버려지고 `main`·`master` 규칙만 들어간다

### Requirement: 기본 브랜치에서 시작하면 worktree
- Composer·다시 시키기 작업이 저장소의 기본 브랜치(또는 분리된 HEAD)에서 시작하려 하면 worktree를 켜서 시작해야 한다(SHALL). Composer에 "기본 브랜치라 worktree에서 해요"를 한 줄로 보여 준다.
- worktree를 만들 수 없는 경로(루틴, 스킬 작성)는 기본 브랜치에서 시작하되 시작 브랜치용 추가 거부 규칙을 붙여야 한다(SHALL).

#### Scenario: main에서 시작
- **WHEN** 사용자가 worktree를 끈 채 `main`이 체크아웃된 폴더에서 작업을 시작한다
- **THEN** 작업은 worktree에서 시작되고 Composer에 "기본 브랜치라 worktree에서 해요"가 보였다

#### Scenario: main에서 도는 루틴
- **WHEN** 루틴이 `main`이 체크아웃된 폴더에서 실행된다
- **THEN** 거부 목록에 `Bash(git push)`와 `Bash(git push * HEAD*)`가 있다

### Requirement: 프로젝트별 "로컬 작업만" 설정
- 설정 화면에 "원격 push 막기" 구역이 있어야 한다(SHALL). 설명은 "체크한 프로젝트는 push·PR 없이 로컬 작업까지만 해요"와 "로컬만 프로젝트는 클라우드 루틴을 쓸 수 없어요" 두 줄이다.
- 구역은 프로젝트 목록의 모든 폴더 프로젝트를 체크박스와 함께 보여 줘야 한다(SHALL). worktree 항목은 따로 체크하지 않고 원래 프로젝트를 따른다. 홈 폴더·폴더 없는 항목은 체크할 수 없다.
- 기본값은 모두 꺼짐이어야 한다(SHALL). 목록에 없던 새 폴더도 꺼짐이다.
- 체크는 바로 저장한다. 체크 해제는 백엔드가 띄우는 네이티브 확인("이 프로젝트에서 push와 PR 초안을 다시 허용할까요?")을 거쳐야 한다(SHALL).
- 체크하면 그 폴더와 저장소 루트를 함께 저장해야 한다(SHALL). 일반 설정 setter로는 바꿀 수 없어야 한다(MUST NOT). 200개까지.
- 목록에 없는 폴더는 Composer에서 폴더 경로로 체크할 수 있어야 한다(SHALL). 백엔드가 경로를 다시 검사한다(실제 디렉터리, 링크 아님, 홈 아님).
- 폴더가 사라진 체크 항목은 "폴더가 없어졌어요"로 남고 체크 상태를 유지한다.
- 프로젝트 고르기는 체크한 프로젝트에 "로컬만" 칩을 보여 줘야 한다(SHALL).

#### Scenario: 체크하면 새 작업에 push 거부
- **WHEN** 사용자가 설정에서 `client`를 체크하고 그 폴더에서 새 작업을 시작한다
- **THEN** 거부 목록에 `Bash(git push *)`·`Bash(gh *)`가 있고 로컬만 훅이 붙는다
- **AND** 팀 규칙에 "원격으로 push하거나 PR을 열지 않는다 · 로컬 작업까지만" 문장이 있다
- **AND** Composer 범위 줄은 "로컬 작업만 · push하지 않아요"다

#### Scenario: 해제하면 초안 PR 기본값으로
- **WHEN** 사용자가 체크를 풀고 네이티브 확인에서 "허용"을 누른 뒤 그 폴더에서 새 작업을 시작한다
- **THEN** 거부 목록에 `Bash(git push *)`가 없고 훅도 없다
- **AND** Composer 범위 줄은 "PR 초안까지 · 머지는 사람이 해요"다

#### Scenario: 해제 확인에서 취소
- **WHEN** 사용자가 체크를 풀다가 네이티브 확인에서 취소한다
- **THEN** 체크가 그대로 남는다

#### Scenario: 일반 setter로 해제 시도
- **WHEN** 웹뷰가 일반 설정 setter로 `local_only_projects` 키를 빈 값으로 보낸다
- **THEN** 거부된다

#### Scenario: 새 폴더
- **WHEN** 목록에 없던 `/work/new-client`를 Composer에서 고른다
- **THEN** 범위 줄은 "PR 초안까지 · 머지는 사람이 해요"이고 "로컬만으로 바꾸기" 버튼이 있다
- **AND** 버튼을 누르면 그 폴더가 체크되고 줄이 "로컬 작업만 · push하지 않아요"로 바뀐다

#### Scenario: 이미 실행 중인 작업
- **WHEN** 작업이 실행 중일 때 그 프로젝트를 체크한다
- **THEN** 실행 중인 작업의 인자는 바뀌지 않고 설정 구역에 "실행 중인 작업은 그대로예요"가 보인다

### Requirement: "로컬 작업만" 강제
- `local_only` 작업의 거부 목록은 기본 목록에 더해 다음을 담아야 한다(SHALL): `Bash(git push *)`, `Bash(git * push*)`, `Bash(gh *)`, `mcp__github__*`, `mcp__*__create_pull_request`, `mcp__*__merge_pull_request`, `mcp__*__push_files`, 그 밖의 GitHub MCP 쓰기 도구 이름.
- `local_only` 작업에는 D.Duck이 직접 제공하는 PreToolUse 훅이 붙어야 한다(SHALL). 훅은 도구 호출 JSON을 stdin으로 읽고 다음을 막아야 한다(SHALL):
  - 어떤 형태든 git push: `-C`·`-c` 같은 전역 옵션, `env`·`timeout` 같은 래퍼, 절대 경로 `git`, `sh -c`·`bash -c` 안, push로 풀리는 git 별칭
  - 모든 `gh`
  - GitHub류 MCP 도구
- 훅은 명령을 해석할 수 없으면 막아야 한다(SHALL, fail closed). 훅 자체가 실패하거나 시간을 넘겨도 호출은 막혀야 한다(SHALL).
- 훅은 막지 않는 호출에 허용 결정을 출력하지 않아야 한다(MUST NOT). 다른 권한 검사를 건너뛰게 하지 않기 위해서다.
- 훅이 무시될 수 있는 환경(관리 설정이 사용자 훅을 막거나 모든 훅을 끔)이면 `local_only` 작업을 시작하지 않아야 한다(MUST NOT).
- `local_only` 작업의 권한 모드가 auto나 bypassPermissions면, 그 모드에서 거부 규칙과 훅이 지켜진다는 실측이 기록되기 전까지 시작을 거부해야 한다(SHALL).
- 팀 규칙에 `[안전]` 줄 "원격으로 push하거나 PR을 열지 않는다 · 로컬 작업까지만 한다. 끝나면 커밋한 브랜치 이름을 보고한다"가 붙어야 한다(SHALL). 팀 규칙 주입이 꺼진 경로라도 이 줄은 붙인다.

#### Scenario: 전역 옵션으로 우회
- **WHEN** `local_only` 작업이 `git -C . push origin fix-1`을 실행하려 한다
- **THEN** 훅이 막는다

#### Scenario: 셸 안의 push
- **WHEN** `local_only` 작업이 `sh -c 'cd sub && /usr/bin/git push'`를 실행하려 한다
- **THEN** 훅이 막는다

#### Scenario: 별칭
- **WHEN** 저장소에 `alias.p = push`가 있고 작업이 `git p`를 실행하려 한다
- **THEN** 훅이 막는다

#### Scenario: 해석 불가
- **WHEN** 명령에 닫히지 않은 따옴표가 있다
- **THEN** 훅이 막는다

#### Scenario: 평범한 명령
- **WHEN** `local_only` 작업이 `git commit -m "정리"`나 `npm test`를 실행한다
- **THEN** 훅은 결정을 내지 않고 평소 권한 흐름을 따른다

#### Scenario: 위험 모드
- **WHEN** `local_only` 프로젝트에서 bypassPermissions 작업을 시작하려 하고 실측 기록이 없다
- **THEN** 시작이 거부되고 이유가 한 줄로 보인다

### Requirement: 로컬만 프로젝트와 클라우드 루틴
- 클라우드 루틴은 원격에서 돌아 거부 규칙과 훅이 닿지 않는다. 저장소가 `local_only` 프로젝트(그 저장소의 `origin` 주소가 같은 owner/repo)인 클라우드 루틴은 D.Duck에서 만들거나 실행하지 않아야 한다(MUST NOT).

#### Scenario: 로컬만 저장소의 클라우드 루틴 실행
- **WHEN** `client` 저장소를 쓰는 클라우드 루틴을 D.Duck에서 실행하려 한다
- **THEN** 거부되고 "로컬만 프로젝트는 클라우드 루틴을 쓸 수 없어요"가 보인다

### Requirement: 범위 표시
- Composer는 시작 버튼 위에 현재 폴더 기준 범위를 한 줄로 보여 줘야 한다(SHALL): `draft_pr`는 "PR 초안까지 · 머지는 사람이 해요", `local_only`는 "로컬 작업만 · push하지 않아요". 값은 시작 때와 같은 판정에서 와야 한다(SHALL).
- 시작한 작업의 범위는 작업 기록에 고정 코드로 남기고 보고서 "맡긴 내용" 칩("PR 초안까지"/"로컬만")으로 보여 줘야 한다(SHALL). 기록이 없는 옛 작업은 칩을 보이지 않는다.

#### Scenario: 보고서 칩
- **WHEN** `local_only`로 시작한 작업의 보고서를 연다
- **THEN** "맡긴 내용"에 "로컬만" 칩이 있다

### Requirement: 로컬 git 사후 점검
- 사후 점검은 다음 둘뿐이어야 한다(SHALL). 둘 다 로컬 git 정보만 읽고 네트워크를 쓰지 않는다.
  - `default_moved`: 작업 시작 때 기록한 기본 브랜치 로컬 커밋 id와 점검 때의 값이 다르다.
  - `push_detected`: 작업 시간 범위 안에 `refs/remotes/origin/*` reflog에 "update by push" 항목이 있다. `draft_pr`는 기본 브랜치 ref만, `local_only`는 모든 ref를 본다.
- reflog에서 시각과 메시지 종류만 읽어야 한다(SHALL). 이름·메일·커밋 내용은 저장하지 않는다(MUST NOT).
- 결과는 작업 기록에 두 값으로 남기고, 보고서 "확인할 것"과 리뷰 탭 카드 경고 표시에 써야 한다(SHALL).

#### Scenario: 기본 브랜치 로컬 커밋
- **WHEN** 루틴이 `main`에서 시작해 `main`에 커밋했다
- **THEN** 보고서에 "기본 브랜치에 새 커밋이 생겼어요"가 있다

#### Scenario: 로컬만 프로젝트에서 push
- **WHEN** `local_only` 작업 시간 안에 `refs/remotes/origin/fix-1` reflog에 "update by push"가 생겼다
- **THEN** 보고서에 "원격으로 push한 기록이 있어요"가 있고 리뷰 탭 카드에 경고 표시가 있다

#### Scenario: 초안 PR 범위의 작업 브랜치 push
- **WHEN** `draft_pr` 작업이 `fix-1`을 push했다
- **THEN** 경고가 없다
