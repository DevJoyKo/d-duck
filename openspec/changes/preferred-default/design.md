# Design

## 맥락

팀 설정은 번들 기본값 + 저장된 차이(`Stored`)를 합쳐(`merge_in`) 쓴다. 다른 항목(규칙·유형·다듬기 설정)은 기본값과 같으면 저장하지 않고, "기본값으로"는 차이를 지운다. `preferred`만 기본값이 빈 목록이라 `Vec` 하나로 충분했다.

## 결정

### 1. 기본값은 번들 JSON에
- `team_defaults.json`에 `"preferred": [{"kind": "plugin", "name": "mattpocock-skills", "source": "plugin:mattpocock-skills"}]`. `Bundle.preferred: Vec<PreferredItem>`. 언어별 텍스트가 없으므로 ko/en 번들이 같다.
- 번들 값도 `preferred::validate`를 통과해야 한다(테스트로 고정).
- ~~플러그인 항목에는 마켓플레이스가 없다(수용).~~ 보안 리뷰 MEDIUM 1로 대체: 아래 6.

### 2. 저장: `Option<Vec<PreferredItem>>`
- `Stored.preferred: Option<Vec<_>>`, `skip_serializing_if = "Option::is_none"`. 없음 = 기본값, `Some([])` = 비움, `Some(list)` = 그 목록.
- `merge_in`: `stored.preferred.as_deref().unwrap_or(&bundle.preferred)`를 `preferred::clean`.
- `diff`: 검사 뒤 입력이 번들 기본값과 같으면(순서 포함) `None`, 다르면 `Some(입력)`(빈 목록 포함).
- `TeamInput.preferred: Option<Vec<_>>`: 빠짐 = 기본값(차이 없음). 웹뷰는 늘 보낸다.
- `reset_item("preferred")` = `None`. 전체 초기화는 기존대로 팀 설정을 지운다(= `None`).
- 기본값 해시(`bases`)는 두지 않는다. 기본값이 나중에 바뀌어도 "정하지 않은" 사용자는 새 기본값을 받고, 직접 정한 사용자는 자기 목록을 지킨다.

### 3. 이전 저장값
- 지금까지 빈 목록은 직렬화하지 않았다. 그래서 이전 값은 "필드 없음"(= 정하지 않음 → 기본값) 아니면 "비어 있지 않은 목록"이다.
- 이 변경 전에 목록을 일부러 비운 사용자도 필드가 없으므로 기본값을 받는다. 구분할 방법이 없다. 기능이 나온 지 몇 시간이고, 받는 것은 설치된 경우에만 들어가는 안내 한 줄이며, 화면에서 바로 비울 수 있으므로 수용한다.

### 4. 화면
- 이 칸에 "기본값으로" 버튼(`RotateCcw`, 다듬기 칸과 같은 모양)을 목록이 기본값과 다를 때만 보인다. 누르면 초안을 기본값으로 바꾸고, 저장하면 차이가 지워진다. "목록 비우기"는 그대로 빈 목록을 저장한다.
- 기본값 항목이 설치되지 않았으면 기존대로 "설치되지 않아 빠져요"가 보인다. 노랑은 쓰지 않는다.

### 5. 시작 비용
- 이제 대부분 사용자가 목록을 가지므로 `preferred::needed`가 참이 되어, 시작마다 그 폴더의 스킬·에이전트 카탈로그를 읽는다(기존 `loadout` 단계에 포함). 읽기는 로컬 파일 목록이며 고른 항목이 있을 때와 같다. 수용한다.

## 위험

- 기본값이 넣는 줄은 설치된 플러그인의 이름(멤버 짧은 이름)뿐이다. 설명을 넣지 않는 플러그인 묶음 경로이며, 기존 정리·길이 제한을 그대로 탄다. 권한은 바뀌지 않는다.
- ~~꺼진 플러그인(수용)~~ 보안 리뷰 MEDIUM 2로 대체: 아래 6. 원래 문장: 카탈로그는 `enabledPlugins`를 보지 않고 캐시의 최신 버전을 읽는다(preferred-skills 설계 3). 이제 기본값이 모두에게 적용되므로, 껐거나 캐시만 남은 `mattpocock-skills`도 줄에 들어갈 수 있다. Claude가 그 스킬을 찾지 못할 뿐 권한·동작은 바뀌지 않고, 확장 탭과 같은 기준을 지키려고 수용한다.
- D-198 근거 일부("특정 플러그인을 하드코딩하지 않는다")를 대체한다. 설정은 여전히 일반 목록이고, 바뀌는 것은 번들 기본값 하나다.

## 설계 리뷰 반영 (design-critic: BLOCKER 0, HIGH 2, MEDIUM 3, LOW 2)

- HIGH 리뷰 게이트: CLAUDE.md는 단계마다 리뷰어 2명을 요구하지만, 이번 change는 작다며 사용자가 설계 1회(design-critic)·테스트 1회(qa-verifier)로 정했다. 사용자 지시를 따르고 여기 남긴다.
- HIGH 결정 기록: tasks 4.4 추가. decisions.md는 로컬 전용이라 D-202 줄과 D-198 일부 대체 표시를 지시자에게 넘긴다.
- MEDIUM routines 테스트·셸 주석: Impact와 1.1에 추가.
- MEDIUM 꺼진 플러그인: 위험에 적고 수용.
- MEDIUM "기본값인데 플러그인이 없다" 테스트: 1.1에 추가.
- LOW 멤버 상한: 스펙 시나리오에 "10개까지, 외 N개" 명시.
- LOW mock 저장 폴백 `input.preferred ?? []`: 2.1에서 기본값으로 고친다.

## 테스트 리뷰 반영 (qa-verifier: 시나리오 9 PASS·1 수동 확인, FAIL·BLOCKER·HIGH 0)

- MEDIUM 이전에 일부러 비운 사용자: 설계 3에서 수용한 그대로.
- LOW 테스트 공백: 저장값 `"preferred":null`과 전체 초기화(`Stored::default`)가 기본값으로 읽히는지 core 테스트에 더했다. AgentSettings 통합(비우기→저장→다시 읽기)은 core 왕복 테스트와 화면 단위 테스트로 나눠 덮는 것으로 수용.
- LOW mock의 "설치되지 않음" 샘플: 기본값 mattpocock-skills가 mock 옵션에 없어서 같은 표시가 보인다. 3항목 샘플은 기본값 화면을 보이려고 뺐다.

## 6. 보안 리뷰 반영 (PASS, MEDIUM 2)

- **마켓플레이스**: `PreferredItem.marketplace`(플러그인 출처만, 선택). 기본값은 `claude-plugins-official`을 적는다. 고르기에서 플러그인 출처 항목을 고르면 마켓플레이스도 저장한다. `Available.marketplace`는 스킬·에이전트 파일 경로의 `plugins/cache/<마켓>/…`에서 얻고, 플러그인 묶음은 (마켓, 이름)으로 나눈다. 마켓을 알 수 없는 플러그인 항목은 목록에 넣지 않는다.
- **모호함**: 마켓을 적지 않은 항목(이 기능 첫 버전에서 저장한 것)은 같은 이름이 둘 이상의 마켓에 있으면 줄에서 빼고, 화면에 "같은 이름이 여러 마켓플레이스에 있어 빠져요"를 보인다.
- **켜짐**: `preferred::plugin_env`가 `<config>/plugins/installed_plugins.json`(허용 필드 `scope`·`projectPath`만)과 `enabledPlugins`(user `<config>/settings.json`, project `<cwd>/.claude/settings.json`, local `<cwd>/.claude/settings.local.json`, local이 먼저)만 읽는다. 다른 설정 값은 역직렬화하지 않는다. 파일은 1MB까지. 설치됨 = user·managed 기록, 또는 project·local 기록의 `projectPath`가 작업 폴더이거나 그 위. 켜짐 = 앞의 순서로 처음 정한 값이 `true`.
- 설정 화면(폴더 없음)은 user 설정과 알려진 프로젝트 폴더마다의 결과 중 하나라도 켜짐이면 켜짐으로 보인다. 시작은 그 작업 폴더만 본다.
- managed 설정 파일의 `enabledPlugins`는 읽지 않는다(수용: 관리형 배포에서 켜진 플러그인이 꺼짐으로 보일 수 있음, 안내 줄이 빠질 뿐이다). 작업 폴더의 상위 폴더 `.claude` 설정은 보지 않는다.
- 꺼진 플러그인 출처 스킬·에이전트도 같은 규칙으로 빠진다.

## v0.1.2 노트 문장

- "먼저 살펴볼 스킬·에이전트"의 기본값이 `mattpocock-skills` 플러그인이 됐어요. v0.1.1에서 이 목록을 일부러 비웠다면 기본값이 다시 들어가니, 팀 규칙 탭에서 다시 비워 주세요.

## 보안 반영 뒤 QA (qa-verifier 1회: BLOCKER·HIGH·MEDIUM 0, LOW 4)

- LOW 1 모르는 scope: `user`·`managed`·없음만 어디서나 설치됨, 그 밖의 값은 세지 않도록 고쳤다(테스트 `odd@m`).
- LOW 2 하위 폴더의 `.claude`만 봄: 설계 6에서 수용.
- LOW 3 마켓 없는 옛 항목이 있으면 같은 이름 옵션이 모두 숨음: 옛 항목을 빼고 다시 고르면 된다. 수용.
- LOW 4 경계 테스트(1MB 초과, managed, canonical 캐시, project 설정만 켬): 시간 때문에 미룬다.
