# D.Duck 테스트 케이스(TC) 매핑

> 자동 생성: `docs/features/tools/render.py`. TC ID = `TC-<영역>-<기능번호>-<시나리오번호>`, 시나리오는 OpenSpec spec의 `#### Scenario`와 1:1이다.

TC 527개 · 자동 245 · 부분 자동 215 · 수동 67

- **자동**: 연결된 테스트가 시나리오의 조건과 기대 결과를 확인한다. 회귀는 `cargo test -p dduck-core`, `npx vitest run`(app/)으로 돌린다.
- **부분 자동**: 일부만 자동이다. 나머지는 수동 절차로 확인한다.
- **수동**: 앱(또는 `npm run dev:mock`)에서 절차대로 확인한다. `[스파이크]`는 실제 한도·로그인처럼 재현 비용이 큰 확인이다.


## SHL · 앱 셸·탐색

### F-SHL-01

**문구 통일** · 구현됨 — copy-sweep 금지 패턴 테스트가 화면·Rust 소스를 검사 · spec: `openspec/specs/copy-tone/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-01-01 | 혼용 없음 | WHEN 금지 패턴 테스트가 화면 소스 문자열을 검사한다 / THEN '습니다'로 끝나는 화면 문구와 '검토 용량/검토함/검토 필요/검토 대기'가 없다 | 자동 | `src/lib/copyRules.test.ts::has no 합니다체 in the Korean text`<br>`src/lib/copyRules.test.ts::says 리뷰, not 검토`<br>`src/lib/copyRules.test.ts::calls the user scope 전역 (no 사용자 범위, 내 것(모든 프로젝트), 내 플러그인)`<br>`src/lib/copyRules.test.ts::detects the formal endings it is meant to forbid` |

### F-SHL-02

**좌측 사이드바 탐색** · 구현됨 — Sidebar·App에 메뉴·배지·마지막 페이지 복원. 단 기본 페이지는 nav-ia 이후 '에이전트' · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-02-01 | 첫 실행 | WHEN 사용자가 앱을 처음 실행한다 / THEN 메인 창이 열리고 "작업" 페이지가 선택되어 있다 | 수동 | 수동: 앱 데이터 없는 새 설치 상태 → 앱 실행 → 메인 창이 열리고 기본 페이지가 선택됨(현재 코드 기본값은 '에이전트'라 스펙 '작업'과 다름을 확인) |
| TC-SHL-02-02 | 페이지 복원 | WHEN 사용자가 "사용 통계"를 연 상태로 앱을 종료했다가 다시 실행한다 / THEN "사용 통계" 페이지가 선택된 상태로 열린다 | 부분 자동 | `src/components/NavIa.test.tsx::a remembered `plugins` opens 확장 with the 플러그인 chip`<br>`src/lib/nav.test.ts::keeps current pages and maps old ids to 확장 with a chip`<br>수동: 사용 통계 탭을 연 채 앱 종료 → 다시 실행 → 사용 통계 탭이 선택된 상태로 열림 |
| TC-SHL-02-03 | 검토 필요 배지 | WHEN 답을 기다리는 작업 1개와 실패한 작업 1개가 있다 / THEN 사이드바 "작업" 메뉴에 2가 표시된다 | 부분 자동 | `src/lib/brief.test.ts::counts unreviewed code tasks, including finished ones`<br>수동: 답 대기 작업 1개·보고서 없는 실패 작업 1개 준비 → 사이드바 확인 → '작업' 메뉴 배지에 2 |

### F-SHL-03

**트레이 상주와 백그라운드 동작** · 구현됨 — lib.rs에 TrayIconBuilder('창 열기'·'종료'), CloseRequested 시 숨김, watcher 유지 · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-03-01 | 창 닫기 | WHEN 사용자가 메인 창을 닫는다 / THEN 트레이 아이콘이 남아 있고, 이후 새로 기록된 transcript도 수집된다 | 수동 | 수동: 앱 실행 → 메인 창 닫기 → 메뉴바 아이콘이 남고, 터미널에서 claude로 응답 하나 생성 후 창 열기 → 사용 통계에 반영됨 |
| TC-SHL-03-02 | 완전 종료 | WHEN 사용자가 트레이 메뉴에서 "종료"를 선택한다 / THEN 앱 프로세스와 앱 내부의 수집·감시 작업이 모두 종료되고, 앱이 실행했던 Claude 백그라운드 세션은 계속 실행된다 | 수동 | 수동: 앱에서 백그라운드 작업 하나 시작 → 트레이 '종료' → D.Duck 프로세스는 사라지고 `claude agents`에서 그 작업은 계속 실행 중 |

### F-SHL-04

**Claude CLI 감지** · 구현됨 — cli_path.rs가 설정값→알려진 위치(nvm 포함)→로그인 셸 탐색 순으로 감지 · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-04-01 | 셸 밖 실행 | WHEN 앱이 PATH 환경변수 없이 실행되고 `claude`가 nvm 경로에만 설치되어 있다 / THEN 앱은 그 경로를 찾아 표시하고 작업 탭이 활성화된다 | 부분 자동 | `crates/dduck-core/src/cli_path.rs::nvm_highest_version_wins_in_empty_env`<br>`crates/dduck-core/src/cli_path.rs::nvm_style_symlink_to_claude_exe_is_accepted`<br>`crates/dduck-core/src/cli_path.rs::setting_takes_priority_and_probe_is_last_resort`<br>수동: Finder에서 앱 실행(claude는 nvm 경로에만 설치) → 설정·작업 탭 확인 → nvm 경로가 표시되고 새 작업 실행이 활성 |

### F-SHL-05

**CLI 경로와 설정 디렉터리 지정** · 구현됨 — validate()가 이름·실행권한·--version 형식 검사, effective_config_dir 우선순위, CLAUDE_CONFIG_DIR 전달 · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-05-01 | 다른 실행 파일 지정 | WHEN 사용자가 이름이 `claude`가 아닌 실행 파일을 고른다 / THEN 앱은 저장을 거부하고 이유를 표시한다 | 부분 자동 | `crates/dduck-core/src/cli_path.rs::rejects_bad_candidates`<br>수동: 설정 → CLI 경로 선택에서 이름이 claude가 아닌 실행 파일 선택 → 저장 거부와 이유 문구 표시 |
| TC-SHL-05-02 | 설정 디렉터리 변경 | WHEN 사용자가 설정 디렉터리를 `~/.claude-work`로 지정한다 / THEN 수집 대상이 그 디렉터리의 `projects/`로 바뀌고, 앱이 실행하는 `claude`도 같은 설정 디렉터리를 쓴다 | 부분 자동 | `crates/dduck-core/src/cli_path.rs::effective_config_dir_priority`<br>`crates/dduck-core/tests/claude_cli_fake.rs::start_background_passes_argv_cwd_and_config_dir`<br>`crates/dduck-core/tests/claude_cli_fake.rs::default_config_dir_is_not_passed`<br>`crates/dduck-core/src/settings.rs::config_dir_must_exist`<br>수동: 설정에서 설정 디렉터리를 ~/.claude-work로 지정 → 그 디렉터리 projects/에 transcript 생성 → 사용 통계에 반영, 새 작업이 같은 디렉터리로 실행됨 |
| TC-SHL-05-03 | CLI 미설치 | WHEN PATH와 알려진 설치 위치 어디에도 `claude`가 없다 / THEN 작업 탭에 "Claude CLI를 찾을 수 없음" 안내가 표시되고 새 작업 실행이 비활성화되며, 사용 통계 탭은 정상 동작한다 | 수동 | 수동: claude가 없는 환경(설정 경로 비우고 PATH·알려진 위치 없음)에서 앱 실행 → 작업 탭에 CLI 없음 안내·새 작업 비활성, 사용 통계 탭은 정상 |
| TC-SHL-05-04 | 지원 범위 밖 버전 | WHEN 감지된 `claude --version`이 2.1.x가 아니다 / THEN 작업 탭에 감지된 버전과 "검증되지 않은 버전" 경고가 표시된다 | 부분 자동 | `crates/dduck-core/src/cli_path.rs::accepts_valid_cli_and_flags_unsupported_version`<br>수동: --version이 3.0.0을 출력하는 가짜 claude를 CLI 경로로 지정 → 작업 탭 확인 → 감지 버전과 '검증되지 않은 버전' 경고 표시 |

### F-SHL-06

**자격증명 비접근** · 구현됨 — discover.rs가 projects/ 하위 허용 패턴만 수집, 자격증명 이름 거부 · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-06-01 | 자격증명 파일 존재 | WHEN Claude 설정 디렉터리에 `.credentials.json`과 `sessions/*.key` 파일이 있다 / THEN 앱은 그 파일들을 열지 않는다 | 부분 자동 | `crates/dduck-core/src/discover.rs::only_allowed_files_are_found`<br>`crates/dduck-core/src/discover.rs::forbidden_names_rejected_even_if_pattern_matches`<br>수동: 설정 디렉터리에 .credentials.json·sessions/x.key 생성 → 앱 실행·전체 다시 수집 → fs_usage/opensnoop로 두 파일 open 없음 확인 |

### F-SHL-07

**앱 프로세스의 네트워크 비사용** · 구현됨 — 엄격 CSP(connect-src ipc만). 예외: D-087로 링크 설치용 http.rs(GitHub 2개 호스트) · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-07-01 | 오프라인 사용 | WHEN 네트워크가 끊긴 상태에서 앱을 사용한다 / THEN 사용 통계, 세션 목록, 로그 보기, 중지가 동작하고, 새 작업 실행은 CLI 오류를 그대로 표시한다 | 수동 | 수동: Wi-Fi 끄기 → 사용 통계·세션 목록·로그 보기·중지 동작 확인, 새 작업 실행 → CLI 오류가 그대로 표시 |
| TC-SHL-07-02 | 화면에서 외부 요청 시도 | WHEN 화면 코드가 외부 URL로 요청을 보내려 한다 / THEN 요청이 차단된다 | 수동 | 수동: 빌드된 앱에서 웹 인스펙터 열기 → fetch('https://example.com') 실행 → CSP 위반으로 차단됨 |

### F-SHL-08

**앱 종료와 재열기** · 구현됨 — single_instance 플러그인(show_main), RunEvent::Reopen, 종료 처리 · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-08-01 | Cmd+Q 종료 | WHEN macOS에서 앱이 앞에 있을 때 사용자가 Cmd+Q를 누른다 / THEN 앱 프로세스가 종료되고 트레이 아이콘이 사라진다 | 수동 | 수동: macOS에서 앱을 앞에 두고 Cmd+Q → 프로세스 종료, 메뉴바 아이콘 사라짐 |
| TC-SHL-08-02 | 두 번째 실행 | WHEN 앱이 이미 실행 중인데 사용자가 앱을 다시 실행한다 / THEN 새 프로세스는 곧바로 종료되고 기존 앱의 창이 나타난다 | 수동 | 수동: 앱 실행 중 Finder에서 D.Duck 다시 실행 → 새 프로세스 즉시 종료, 기존 창이 앞으로 나옴(ps로 인스턴스 1개) |
| TC-SHL-08-03 | 숨긴 창 다시 열기 | WHEN 창을 닫아 숨긴 뒤 Dock 아이콘을 누른다 / THEN 같은 창이 다시 나타나고 앱 인스턴스는 하나다 | 수동 | 수동: 창 닫아 숨김 → Dock 아이콘 클릭 → 같은 창이 다시 나타나고 인스턴스 1개 |

### F-SHL-09

**크로스플랫폼 빌드** · 구현됨 — Tauri 번들 설정과 release-local.sh로 DMG 산출(메모리: bundle/dmg) · spec: `openspec/specs/desktop-shell/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-09-01 | macOS 빌드 산출물 | WHEN macOS에서 릴리스 빌드를 실행한다 / THEN 설치 가능한 `.dmg` 파일이 생성된다 | 수동 | 수동: macOS에서 릴리스 빌드(release-local.sh) 실행 → bundle/dmg에 .dmg 생성, 열어서 설치 가능 |

### F-SHL-10

**오리 스카프 마감** · 일부 구현 — D-128로 목 스카프 대신 역할별 아이템(허리띠·모자·넥타이)으로 바뀜; 매듭은 버클로 대체 · spec: `openspec/specs/duck-character/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-10-01 | 모든 포즈 | WHEN 에이전트 화면에 다섯 포즈의 오리가 여러 색으로 보인다 / THEN 스카프가 모두 몸 안에서 곡선으로 감기고 매듭이 앞쪽에 있다 | 부분 자동 | `src/components/DuckAvatar.test.tsx::keeps the buckle in the other poses`<br>`src/components/DuckAvatar.test.tsx::draws the belt but no buckle in the working pose`<br>수동: 에이전트 화면에서 다섯 포즈·여러 색 오리 확인 → 아이템이 몸 윤곽선 안에 있고 앞쪽 매듭(버클)이 보임(일하는 포즈는 버클 없음) |

### F-SHL-11

**확장 탭** · 구현됨 — ExtensionsPage가 칩·검색·출처 표시, 플러그인 행 펼침 · spec: `openspec/specs/nav-ia/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-11-01 | 플러그인이 준 스킬 찾기 | WHEN 사용자가 확장 탭에서 "스킬" 칩을 누르고 "design"을 검색한다 / THEN 내 스킬과 플러그인이 준 스킬이 함께 나오고 플러그인 것은 플러그인 이름 배지가 붙는다 | 부분 자동 | `src/components/NavIa.test.tsx::스킬 chip + search finds my skill and, inside its plugin, the plugin's`<br>`src/components/NavIa.test.tsx::puts a plugin's skills inside it, marks the current project's and project plugins, enabled first`<br>수동: 확장 탭 → '스킬' 칩 → 'design' 검색 → 내 스킬과 플러그인 스킬이 함께 보이고 플러그인 것은 플러그인 이름 아래(배지)로 구분 |

### F-SHL-12

**MCP 탭** · 구현됨 — McpPage·mcp_config 정적 파싱. D-138로 탭을 열면 상태 확인 1회 자동 실행(스펙 '버튼 눌렀을 때만'과 다름) · spec: `openspec/specs/nav-ia/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-12-01 | MCP 바로 보기 | WHEN 사용자가 사이드바의 MCP를 누른다 / THEN 연결된 서버들이 아이콘과 함께 보이고 "상태 확인"을 누르면 연결 상태가 표시된다 | 부분 자동 | `src/components/NavIa.test.tsx::lists servers from the settings files at once and loads the connectors from home when the tab opens (D-138)`<br>`src/components/NavIa.test.tsx::settings links to the MCP tab`<br>`src/lib/connectorIcons.test.ts::matches the real connector names`<br>수동: 사이드바 MCP → 서버가 아이콘과 함께 보임 → '다시 확인' → 연결 상태 표시 |

### F-SHL-13

**사이드바 묶음** · 구현됨 — 사이드바 묶음 구현. 이름은 '작업 공간'(D-130), 프로젝트 선택기는 D-127로 사이드바에서 빠짐 · spec: `openspec/specs/nav-ia/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-13-01 | 사이드바 구성 | WHEN 앱을 연다 / THEN "일: 에이전트·작업·루틴·아티팩트", "Claude 구성: 확장·MCP·권한·메모리", 그 아래 사용 통계·설정, 맨 아래 현재 프로젝트가 보인다 | 부분 자동 | `src/components/NavIa.test.tsx::shows 작업 공간 and Claude 구성 in order, then usage and settings, and no project picker`<br>수동: 앱 열기 → 사이드바 확인 → '작업 공간: 에이전트·작업·리뷰·루틴·아티팩트', 'Claude 구성: 확장·MCP·권한·메모리', 그 아래 사용 통계·설정 |

### F-SHL-14

**잠긴 항목 복사해서 내 것으로** · 구현됨 — authoring::copy_item이 폴더 복사·이름 변경·copied-from 표시, UI 복사 패널 · spec: `openspec/specs/nav-ia/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-14-01 | 플러그인 스킬 고쳐 쓰기 | WHEN 사용자가 잠긴 플러그인 스킬 `frontend-design`에서 "복사해서 내 것으로"를 누르고 내 것·기본 이름으로 확인한다 / THEN `~/.claude/skills/frontend-design-custom/`이 생기고 편집기가 열리며, 목록에서 원본은 잠긴 채, 복사본은 "내 것 · frontend-design에서 복사"로 보인다 | 자동 | `src/components/NavIa.test.tsx::copies a locked plugin skill to mine from inside its plugin and opens the editor`<br>`src/components/NavIa.test.tsx::names copies <name>-custom, then -2`<br>`crates/dduck-core/src/authoring.rs::copy_item_copies_the_folder_renames_and_never_overwrites`<br>`crates/dduck-core/src/skills_catalog.rs::copies_name_their_origin` |

### F-SHL-15

**탭별 프로젝트 선택기 (D-127, 사이드바 선택기 대체)** · 구현됨 — settings page_projects(탭별) + ProjectPicker·ProjectContext(D-127·D-130) · spec: `openspec/specs/project-switcher/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-15-01 | 프로젝트 바꾸기 | WHEN 사용자가 권한 탭의 선택기에서 dev_team을 고른다 / THEN 권한 탭만 dev_team 기준으로 바뀌고 다른 탭은 각자 고른 프로젝트를 유지하며, 앱을 다시 열어도 권한 탭은 dev_team이다 | 부분 자동 | `src/components/ProjectSwitcher.test.tsx::opens with ⌘P on the tab, searches, picks with Enter and saves the id for that tab only`<br>`src/components/ProjectSwitcher.test.tsx::each tab keeps its own: a tab with its own pick stays, a tab without one follows the most recent`<br>`crates/dduck-core/src/settings.rs::page_projects_take_known_pages_and_ids_only`<br>수동: 권한 탭 선택기에서 dev_team 선택 → 메모리 탭은 이전 선택 유지 → 앱 재시작 → 권한 탭이 dev_team |

### F-SHL-16

**같은 자리의 범위 전환** · 구현됨 — ScopeBar가 [전역 \| 프로젝트] 전환, 없으면 '프로젝트 고르기'로 그 탭 선택기를 엶 · spec: `openspec/specs/project-switcher/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-16-01 | 프로젝트 없이 권한 화면 | WHEN 현재 프로젝트가 없는 상태로 권한 화면을 연다 / THEN [전역 \| 프로젝트 고르기]가 보이고, 프로젝트 고르기를 누르면 그 탭의 선택기가 열린다 | 자동 | `src/components/ProjectSwitcher.test.tsx::without a project shows [전역 | 프로젝트 고르기], and the project side opens the picker` |

### F-SHL-17

**홈 폴더** · 구현됨 — ScopeBar 홈 폴더 처리와 projects.rs 홈 거부 · spec: `openspec/specs/project-switcher/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-17-01 | 홈을 고름 | WHEN 사용자가 홈 폴더를 현재 프로젝트로 고르고 규칙 화면을 연다 / THEN 전역 규칙만 보이고 프로젝트 쪽에는 "홈 폴더는 전역과 같아요"가 보인다 | 자동 | `src/components/ProjectSwitcher.test.tsx::home as project: rules show the global side only, with the home line`<br>`crates/dduck-core/src/projects.rs::home_and_config_inode_are_refused` |

### F-SHL-18

**옛 이름 제거** · 일부 구현 — db.rs 이전 상수 외에 src/mock/memory.ts 목 데이터에 'DevDeck' 문자열이 남아 있음 · spec: `openspec/specs/app-identity/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-18-01 | 이름 검색 | WHEN 앱 소스(app/ 아래, 빌드 산출물 제외)에서 `devdeck`을 대소문자 구분 없이 찾는다 / THEN 옛 DB 파일 이름을 한 번 이전하는 코드(상수 하나와 그 테스트) 말고는 결과가 없다 | 수동 | 수동: app/에서 node_modules·target·dist 제외하고 `grep -ril devdeck .` → db.rs 말고는 결과 없어야 함(현재 src/mock/memory.ts도 나옴) |

### F-SHL-19

**데이터 이어 쓰기** · 구현됨 — db::adopt_legacy가 devdeck.db(+wal/shm)를 한 번 이름 바꿈 · spec: `openspec/specs/app-identity/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-19-01 | 업데이트 후 첫 실행 | WHEN 옛 이름의 DB에 사용 기록이 있는 상태에서 새 버전을 처음 켠다 / THEN 사용 통계·작업 기록·루틴이 그대로 보이고 데이터 폴더에는 dduck.db만 있다 | 부분 자동 | `crates/dduck-core/src/db.rs::legacy_database_is_adopted_once`<br>수동: 앱 데이터 폴더에 사용 기록 있는 devdeck.db만 두고 새 버전 실행 → 사용 통계·작업·루틴이 그대로 보이고 폴더에는 dduck.db만 |

### F-SHL-20

**언어 선택** · 구현됨 — ui_language(system\|ko\|en) DB 저장 + localStorage 캐시, 설정 항목과 사이드바 빠른 전환 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-20-01 | 처음 실행, 영어 OS | WHEN 언어를 고른 적이 없고 OS 기본 언어가 en-US다 / THEN 사이드바·설정·빈 상태 문구가 영어로 보이고, 설정의 언어 값은 "System language"다 | 부분 자동 | `src/i18n/i18n.test.ts::system means Korean only for a ko locale`<br>`crates/dduck-core/src/i18n.rs::system_rule_is_ko_prefix_only`<br>수동: en-US OS에서 새 데이터 폴더로 실행 → 사이드바·설정이 영어, 언어 값 "System language" |
| TC-SHL-20-02 | 처음 실행, 한국어 OS | WHEN 언어를 고른 적이 없고 OS 기본 언어가 ko-KR이다 / THEN 지금과 같은 한국어 화면이 보인다 | 부분 자동 | `src/i18n/i18n.test.ts::system means Korean only for a ko locale`<br>수동: ko-KR OS에서 새 데이터 폴더로 실행 → 한국어 화면 |
| TC-SHL-20-03 | 사이드바 빠른 전환 | WHEN 한국어 화면에서 사이드바의 "언어 바꾸기" 메뉴를 열고 "English"를 고른다 / THEN 화면이 바로 영어로 바뀌고 버튼 이름은 "Change language"가 되며, 설정 페이지의 언어 값도 English다 | 자동 | `src/components/LanguageSetting.test.tsx::switches from the menu, names itself in the current language, and saves the same value` |
| TC-SHL-20-04 | 직접 고름 유지 | WHEN 한국어 OS에서 "English"를 고르고 앱을 종료한 뒤 다시 연다 / THEN 첫 화면부터 영어로 보인다 | 부분 자동 | `src/i18n/i18n.test.ts::applies the backend's answer for system, sets <html lang> and caches the choice`<br>수동: 한국어 OS에서 English 선택 후 종료·재실행 → 첫 화면부터 영어 |

### F-SHL-21

**바로 적용** · 구현됨 — useT 재렌더(재마운트 없음), set_language 뒤 트레이 다시 만들기, 백엔드 문장은 쿼리 다시 읽기 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-21-01 | 초안 유지 | WHEN 작업 시키기 창에 목표를 적은 채 설정에서 언어를 English로 바꾸고 돌아온다 / THEN 창의 라벨과 버튼은 영어이고 적어 둔 목표는 그대로다 | 자동 | `src/components/LanguageSetting.test.tsx::keeps a draft typed before the change` |
| TC-SHL-21-02 | 트레이 | WHEN 언어를 English로 바꾼다 / THEN 트레이 메뉴가 "Open window"·"Quit" 등 영어로 바로 바뀐다 | 수동 | 수동: 설정에서 English 선택 → 메뉴 막대 트레이가 "Open window"·"Quit"로 바로 바뀜 |

### F-SHL-22

**화면 문구 카탈로그** · 구현됨 — ko 기준 카탈로그 + en Shape 타입, 하드코딩 한글 검사(허용 목록 사유 표기) · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-22-01 | 키 누락 | WHEN 개발자가 한국어 카탈로그에만 새 키를 넣는다 / THEN 카탈로그 완전성 테스트가 그 키 이름과 함께 실패한다 | 자동 | `src/i18n/i18n.test.ts::has the same keys in Korean and English`<br>`src/lib/hardcodedKorean.test.ts::has no Korean literal outside the catalogs except the allowed ones` |
| TC-SHL-22-02 | 복수형 | WHEN 영어에서 리뷰 전 작업이 1개, 3개다 / THEN 배지 접근 이름이 각각 "1 task to review", "3 tasks to review"다 | 자동 | `src/i18n/i18n.test.ts::picks English singular and plural with Intl.PluralRules` |

### F-SHL-23

**사용자 데이터는 번역하지 않음** · 구현됨 — 카탈로그는 앱 라벨만, 사용자·Claude 내용은 그대로 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-23-01 | 한국어 목표, 영어 화면 | WHEN 영어 화면에서 목표가 "로그인 버그 고치기"인 작업 카드를 본다 / THEN 카드 라벨은 영어이고 목표 줄은 "로그인 버그 고치기" 그대로다 | 부분 자동 | `src/components/TasksEnglish.test.tsx::draws the board with English labels only`<br>수동: mock ?lang=en에서 한국어 목표 카드 → 라벨 영어, 목표 원문 |

### F-SHL-24

**앱 기본 제공 데이터** · 구현됨 — team_defaults·permission_catalog {ko,en}, pick_lang, 사용자 수정은 custom으로 원문 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-24-01 | 기본 유형 이름 | WHEN 영어 화면에서 이름을 바꾸지 않은 기본 작업 유형과 이름을 "백엔드 담당"으로 바꾼 유형을 본다 / THEN 앞의 것은 영어 이름이고, 뒤의 것은 "백엔드 담당" 그대로다 | 부분 자동 | `crates/dduck-core/src/i18n.rs::pick_lang_takes_one_side_of_every_pair`<br>수동: 영어 화면 팀 설정에서 기본 유형은 영어, 이름 바꾼 유형은 원문 |
| TC-SHL-24-02 | 권한 라벨 | WHEN 영어 화면에서 에이전트 권한 목록을 연다 / THEN 권한 라벨과 설명이 영어다 | 자동 | `src/components/AgentsEnglish.test.tsx::permissions page has no app-added Korean on either tab` |

### F-SHL-25

**백엔드가 주는 사용자 문구** · 구현됨 — ReasonCode+params+대체 text, Fail 표(문자열 오류는 현재 언어), offCode·scopeCode·connectionCode·origin 코드 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-25-01 | 시작 거절 | WHEN 영어 화면에서 로컬 작업만 프로젝트에 bypass 모드로 작업을 시작해 백엔드가 거절한다 / THEN 하단에 영어 거절 이유와 영어 고치기 링크가 보인다 | 부분 자동 | `src/lib/apiErrors.test.ts::words an invalidInput code from the catalog, with Rust's sentence only for an unknown code`<br>수동: mock ?lang=en 로컬만 프로젝트 bypass 시작 → 영어 거절 이유와 고치기 링크 |
| TC-SHL-25-02 | URL 노출 없음 | WHEN 원격 주소가 `https://user:token@example.com/x.git`인 로컬만 프로젝트의 push 거절 문구가 나온다 / THEN 화면과 로그에는 `example.com`만 보이고 `user:token`은 어디에도 없다 | 자동 | `crates/dduck-core/src/i18n.rs::url_params_keep_only_the_host`<br>`crates/dduck-core/src/i18n.rs::reason_serializes_code_params_and_fallback_text` |
| TC-SHL-25-03 | 모르는 코드 | WHEN 화면 카탈로그에 없는 새 오류 코드가 온다 / THEN 백엔드가 같이 보낸 현재 언어 문장이 보이고 코드 이름만 보이지 않는다 | 자동 | `src/lib/apiErrors.test.ts::words an invalidInput code from the catalog, with Rust's sentence only for an unknown code` |

### F-SHL-26

**OS 표면 문구** · 구현됨 — 트레이·알림·확인 창·파일 창 제목을 i18n.rs Code 표로, 두 언어·같은 자리표시자 테스트 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-26-01 | 알림 | WHEN 언어가 English이고 작업이 입력을 기다린다 / THEN OS 알림 제목과 본문이 영어다(작업 이름은 원문) | 부분 자동 | `crates/dduck-core/src/i18n.rs::every_code_has_both_languages_with_the_same_placeholders`<br>수동: English로 두고 작업이 입력을 기다리게 함 → OS 알림 제목·본문 영어, 작업 이름 원문 |

### F-SHL-27

**날짜·숫자 형식** · 구현됨 — format.ts dateTime·relativeTime·num locale 인자, 24시간 고정 지점 유지 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-27-01 | 날짜 | WHEN 2026-11-09 20:19(로컬)를 목록 시각으로 한국어와 영어 화면에서 본다 / THEN 한국어는 `ko-KR` {month:2-digit, day:2-digit, hour:2-digit, minute:2-digit} 결과("11. 09. 오후 08:19"), 영어는 `en-US` 같은 옵션 결과("11/09, 08:19 PM")로 보인다 — 두 값 모두 `Intl.DateTimeFormat` 실측값으로 테스트에 고정한다 | 자동 | `src/i18n/i18n.test.ts::formats list times per language (Intl values measured and fixed)` |
| TC-SHL-27-02 | 백엔드가 만든 시각 문장 | WHEN 사용량 한도 배너가 리셋 시각을 보여 준다 / THEN 시각은 화면이 현재 언어 형식으로 만든 것이고, 백엔드가 만든 "오전/오후" 문장이 영어 화면에 나오지 않는다 | 자동 | `src/lib/apiErrors.test.ts::builds the banner from numbers like Rust does` |

### F-SHL-28

**Claude 응답 언어** · 구현됨 — 고정 지시 끝 응답 언어 한 줄, task_meta.lang 기록, 이어하기·재작업·이어하기 프롬프트가 시작 언어를 따름 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-28-01 | 영어 그릴링 | WHEN 영어 화면에서 질문 만들기를 누른다 / THEN 질문과 선택지가 영어로 오고, 사용자가 한국어로 쓴 목표는 그대로 인용된다 | 부분 자동 | `crates/dduck-core/src/i18n.rs::reply_line_and_lang_codes`<br>수동: 영어 화면에서 질문 만들기 → 질문·선택지 영어, 한국어 목표는 인용 그대로 |

### F-SHL-29

**언어별 문구 규칙** · 구현됨 — copyRules가 ko 카탈로그·Rust ko 열·번들 ko만 보고, en 규칙(sentence case·용어·톤) 검사 추가 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-29-01 | 한국어 규칙 유지 | WHEN 한국어 카탈로그에 "검토 대기"나 "~합니다"로 끝나는 문구가 들어간다 / THEN 문구 규칙 테스트가 실패한다 | 자동 | `src/lib/copyRules.test.ts::has no 합니다체 in the Korean text`<br>`src/lib/copyRules.test.ts::says 리뷰, not 검토` |

### F-SHL-30

**영어 레이아웃** · 일부 구현 — 5.3 스크린샷 리뷰 전 · spec: `openspec/changes/i18n-language-setting/specs/app-language/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SHL-30-01 | 좁은 창 | WHEN 영어로 900px 창에서 작업 보드와 작업 시키기 하단을 본다 / THEN 칸 제목과 "Start task" 버튼이 한 줄에 보이고 겹치지 않는다 | 수동 | 수동: mock ?lang=en, 900px 창에서 작업 보드와 작업 시키기 하단 → 칸 제목과 "Start task" 한 줄, 겹침 없음 |

## ING · 기록 수집

### F-ING-01

**수집 대상** · 구현됨 — discover.rs 허용 패턴·심볼릭/하드 링크 거부, 서브에이전트 세션 연결 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-01-01 | 서브에이전트 포함 | WHEN 세션 S의 transcript와 그 서브에이전트 transcript가 모두 있다 / THEN 서브에이전트의 사용량은 세션 S에 연결되고, 서브에이전트 유형별로 구분해 조회할 수 있다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/tests/analytics_fixture.rs::distributions` |
| TC-ING-01-02 | 심볼릭 링크 | WHEN `projects/x/link.jsonl`이 `projects/` 밖 파일을 가리키는 심볼릭 링크다 / THEN 그 파일은 수집되지 않는다 | 자동 | `crates/dduck-core/src/discover.rs::only_allowed_files_are_found` |

### F-ING-02

**증분 수집** · 구현됨 — 시작 백필 + watcher 증분, 미완성 줄 보류, 축소 시 재처리 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-02-01 | 진행 중인 세션 | WHEN 활성 세션의 transcript에 새 응답이 기록된다 / THEN 10초 이내에 해당 사용량이 통계에 반영된다 | 부분 자동 | `crates/dduck-core/tests/ingest_fixture.rs::incremental_partial_line_and_idempotency`<br>`crates/dduck-core/tests/ingest_fixture.rs::shrunk_file_is_reprocessed`<br>수동: 앱 실행 중 터미널에서 claude로 응답 하나 생성 → 10초 안에 사용 통계 응답 수가 늘어남 |
| TC-ING-02-02 | 쓰는 중인 줄 | WHEN 파일의 마지막 줄이 개행 없이 끝나 있다 / THEN 그 줄은 건너뛰고, 개행이 추가된 뒤 다음 처리에서 수집된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::incremental_partial_line_and_idempotency`<br>`crates/dduck-core/src/ingest.rs::capped_reader_handles_partial_and_long_lines` |

### F-ING-03

**응답 중복 제거** · 구현됨 — message id+request id 기준 upsert, 항목별 최댓값 병합 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-03-01 | 블록별 반복 기록 | WHEN 하나의 응답이 thinking, text, tool_use 블록으로 나뉘어 3줄에 같은 usage로 기록되어 있다 / THEN 그 응답의 토큰은 한 번만 합산된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations` |
| TC-ING-03-02 | 줄마다 증가하는 usage | WHEN 같은 응답의 줄들에서 output 토큰이 10, 40, 120으로 증가한다 / THEN 그 응답의 output 토큰은 120으로 한 번 집계된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations` |
| TC-ING-03-03 | 재수집 | WHEN 사용자가 "전체 다시 수집"을 실행한다 / THEN 수집 후 집계 값이 재수집 전과 같다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::incremental_partial_line_and_idempotency` |

### F-ING-04

**집계 제외 레코드** · 구현됨 — synthetic·API 오류 응답 제외, tool_use_id 기준 1회 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-04-01 | synthetic 응답 | WHEN 모델이 `<synthetic>`이고 request id가 없는 응답이 있다 / THEN 그 응답은 토큰과 응답 수 집계에 포함되지 않는다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations` |

### F-ING-05

**수집 항목** · 구현됨 — api_calls·tool_calls·cost_segments 저장 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-05-01 | 재개된 세션의 비용 | WHEN 한 세션 파일에 서로 다른 실행 구간의 누적 비용 기록이 있고, 구간 1의 최종값이 $2, 구간 2의 최종값이 $3이다 / THEN 그 세션의 공식 누적 비용은 $5로 저장된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/tests/analytics_fixture.rs::sessions_and_detail` |

### F-ING-06

**명령과 스킬 호출 기록** · 구현됨 — parse.rs 슬래시 명령 추출, commands.rs 내장 명령 분류 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-06-01 | 두 경로의 스킬 호출 | WHEN 한 세션에서 Skill 도구로 `openspec-propose`가 1회, 슬래시 명령 `/opsx:propose`가 1회 실행되었다 / THEN 두 호출이 모두 스킬 호출로 기록되고, 호출 경로(도구/슬래시)가 구분된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/tests/analytics_fixture.rs::distributions` |
| TC-ING-06-02 | 내장 명령 | WHEN 사용자가 `/upgrade`를 실행했다 / THEN 그 호출은 명령으로 기록되고 스킬 호출 수에는 포함되지 않는다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/src/commands.rs::classifies_builtins_and_skills`<br>`crates/dduck-core/tests/analytics_fixture.rs::distributions` |
| TC-ING-06-03 | 툴 결과 속 태그 | WHEN 툴 결과나 첨부 레코드의 본문에 명령 태그 문자열이 들어 있다 / THEN 그 문자열은 명령이나 스킬로 기록되지 않는다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/src/parse.rs::slash_command_must_start_message` |

### F-ING-07

**원문 비저장** · 구현됨 — 파서가 허용 필드만 보관, 로깅 매크로 금지 정적 검사 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-07-01 | 원문 유출 검사 | WHEN 저장이 금지된 모든 자유 텍스트 위치에 고유 표식 문자열을 넣은 transcript를 수집한다 / THEN 앱 DB 파일(WAL 포함)과 앱 로그 어디에서도 그 표식이 발견되지 않는다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::no_canary_reaches_storage`<br>`crates/dduck-core/tests/no_content_logging.rs::no_logging_macros_in_app_sources`<br>`crates/dduck-core/src/parse.rs::invalid_json_has_category_only`<br>`crates/dduck-core/src/parse.rs::debug_output_redacts_metadata` |

### F-ING-08

**세션 제목 저장 선택** · 구현됨 — set_store_titles·clear_titles, 제목 없으면 세션 ID 앞부분 표시 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-08-01 | 알림과 로그의 메타데이터 | WHEN 세션 제목·경로·브랜치에 고유 표식이 들어간 세션이 입력 대기로 바뀌고 수집 오류도 발생한다 / THEN 알림 본문과 앱 로그 어디에도 그 표식이 없다 | 부분 자동 | `crates/dduck-core/tests/ingest_fixture.rs::no_canary_reaches_storage`<br>`crates/dduck-core/tests/no_content_logging.rs::no_logging_macros_in_app_sources`<br>`crates/dduck-core/src/parse.rs::debug_output_redacts_metadata`<br>수동: 제목·경로·브랜치에 고유 표식이 든 세션을 입력 대기로 만들고 깨진 줄도 넣음 → macOS 알림 본문과 앱 로그에서 표식 검색 → 없음 |
| TC-ING-08-02 | 제목 저장 끄기 | WHEN 사용자가 세션 제목 저장을 끈다 / THEN 기존 세션 제목이 삭제되고 세션 목록에는 세션 ID 앞부분이 대신 표시된다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::turning_titles_off_scrubs_them_from_disk_and_keeps_them_out`<br>`crates/dduck-core/tests/ingest_fixture.rs::title_storage_can_be_disabled`<br>`src/lib/logic.test.ts::falls back to the session id prefix`<br>`crates/dduck-core/src/ingest.rs::titles_off_leaves_no_bytes_with_many_rows` |

### F-ING-09

**손상·과대 레코드 내성** · 구현됨 — 깨진 줄·16MB 초과 줄 건너뛰고 ingest_files.skipped_lines 기록 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-09-01 | 깨진 JSON 줄 | WHEN transcript 중간에 JSON이 아닌 줄이 있다 / THEN 그 줄만 건너뛰고 나머지 줄은 정상 수집되며, 건너뛴 줄 수가 1 증가한다 | 자동 | `crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations`<br>`crates/dduck-core/src/parse.rs::invalid_json_has_category_only` |
| TC-ING-09-02 | 거대한 줄 | WHEN transcript에 17MB짜리 한 줄이 있다 / THEN 그 줄은 건너뛰고 건너뛴 줄 수가 1 증가하며, 앱 메모리 사용이 그 줄 크기만큼 늘지 않는다 | 부분 자동 | `crates/dduck-core/tests/ingest_fixture.rs::oversized_line_is_skipped_and_following_lines_parse`<br>`crates/dduck-core/src/ingest.rs::capped_reader_handles_partial_and_long_lines`<br>수동: 17MB 한 줄이 든 transcript 준비 → 전체 다시 수집 → 건너뛴 줄 수 +1, 활동 모니터에서 메모리가 17MB 이상 늘지 않음 |

### F-ING-10

**데이터 파일 보호** · 구현됨 — db::open이 디렉터리 0700·파일 0600으로 생성 · spec: `openspec/specs/usage-ingestion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ING-10-01 | 파일 권한 | WHEN macOS 또는 Linux에서 앱이 DB를 처음 만든다 / THEN DB 파일 권한은 0600, 앱 데이터 디렉터리 권한은 0700이다 | 자동 | `crates/dduck-core/src/db.rs::file_permissions_are_owner_only` |

## USG · 사용량

### F-USG-01

**기간 선택** · 구현됨 — period.ts 로컬 시간대 기간 계산, 백엔드에 UTC 오프셋 전달 · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-01-01 | 기간 변경 | WHEN 사용자가 기간을 "최근 30일"로 바꾼다 / THEN 탭의 모든 수치와 차트가 30일 범위로 다시 계산된다 | 부분 자동 | `src/lib/logic.test.ts::7d includes today and the six previous days`<br>`src/lib/logic.test.ts::custom is inclusive and order-insensitive`<br>`src/lib/logic.test.ts::sends the UTC offset with the sign the backend expects`<br>`crates/dduck-core/tests/analytics_fixture.rs::local_timezone_buckets`<br>수동: 사용 통계 탭 → 기간을 '최근 30일'로 변경 → 요약 카드·추이·분포가 모두 30일 범위로 바뀜 |

### F-USG-02

**요약 지표** · 구현됨 — analytics::summary와 UsagePage 빈 상태 문구(해요체로 바뀜) · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-02-01 | 데이터 없음 | WHEN 선택한 기간에 수집된 데이터가 없다 / THEN 요약 카드는 0을 표시하고 "이 기간의 사용 기록이 없습니다" 안내가 나타난다 | 부분 자동 | `crates/dduck-core/tests/analytics_fixture.rs::empty_period_is_zero`<br>수동: 기록이 없는 사용자 지정 기간 선택 → 요약 카드가 모두 0이고 '이 기간의 사용 기록이 없어요' 안내 |

### F-USG-03

**시계열 추이** · 구현됨 — Bucket::for_range가 2일 이하면 시간 단위 · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-03-01 | 오늘 선택 | WHEN 기간이 "오늘"이다 / THEN 추이 차트가 시간 단위로 표시된다 | 부분 자동 | `crates/dduck-core/tests/analytics_fixture.rs::local_timezone_buckets`<br>수동: 기간 '오늘' 선택 → 추이 차트 가로축이 시간 단위 |

### F-USG-04

**분포 분석** · 구현됨 — analytics::distribution(상위 N+기타, 호출 경로), DistributionList · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-04-01 | 툴 Top N | WHEN 사용자가 툴 분포를 연다 / THEN 호출 수 기준 내림차순으로 상위 10개 툴과 "기타" 합계가 표시된다 | 부분 자동 | `crates/dduck-core/tests/analytics_fixture.rs::distributions`<br>`src/components/DistributionList.test.tsx::shows count and share for each item, including the 기타 bucket`<br>수동: 툴 11개 이상 쓴 기간 선택 → 툴 분포 → 호출 수 내림차순 상위 10개와 '기타' 합계 |
| TC-USG-04-02 | 스킬 호출 경로 | WHEN 사용자가 스킬 분포를 연다 / THEN 스킬별 호출 수가 Skill 도구 호출과 슬래시 명령 호출로 구분되어 표시된다 | 자동 | `crates/dduck-core/tests/analytics_fixture.rs::distributions`<br>`src/components/DistributionList.test.tsx::splits skill calls by tool and slash` |
| TC-USG-04-03 | 내장 명령 분리 | WHEN 기간 내 `/upgrade` 2회와 `/opsx:propose` 1회가 기록되어 있다 / THEN 스킬 분포에는 `opsx:propose`만, 명령 분포에는 `/upgrade`가 표시된다 | 자동 | `crates/dduck-core/tests/analytics_fixture.rs::distributions`<br>`crates/dduck-core/tests/ingest_fixture.rs::fixture_expectations` |

### F-USG-05

**필터** · 구현됨 — Query.projects/models가 summary·timeseries·distribution 공통 적용 · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-05-01 | 프로젝트 필터 | WHEN 사용자가 프로젝트 "dev_team"만 선택한다 / THEN 모든 지표가 해당 프로젝트 세션만으로 다시 계산된다 | 부분 자동 | `crates/dduck-core/tests/analytics_fixture.rs::filters_apply_everywhere`<br>수동: 프로젝트 필터에서 dev_team만 선택 → 요약·추이·분포가 모두 dev_team 세션 기준으로 바뀜 |

### F-USG-06

**비용 추정** · 구현됨 — pricing.rs 단가표, 미등록 모델 비용 0·표시 · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-06-01 | 미등록 모델 | WHEN 단가표에 없는 모델의 응답이 수집되어 있다 / THEN 해당 모델 행에 "단가 미등록"이 표시되고 총 비용에는 포함되지 않는다 | 자동 | `crates/dduck-core/tests/analytics_fixture.rs::unpriced_model_reported`<br>`crates/dduck-core/src/pricing.rs::unknown_model_has_no_price`<br>`src/components/DistributionList.test.tsx::marks unpriced models exactly as the backend sends them (cost null)` |

### F-USG-07

**세션 목록과 상세** · 구현됨 — analytics::sessions·session_detail(원문 컬럼 없음) · spec: `openspec/specs/usage-analytics/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-07-01 | 세션 상세 열기 | WHEN 사용자가 세션 목록에서 한 세션을 선택한다 / THEN 그 세션의 토큰·비용·툴·스킬 집계가 표시되고, 메시지 원문은 표시되지 않는다 | 부분 자동 | `crates/dduck-core/tests/analytics_fixture.rs::sessions_and_detail`<br>수동: 세션 목록에서 세션 하나 선택 → 토큰·비용·툴·스킬 집계가 보이고 메시지 원문은 없음 |

### F-USG-08

**앱 안 점검** · 구현됨 — insights::local 점검(근거·효과·링크), 표본 부족 처리 · spec: `openspec/specs/usage-insights/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-08-01 | 캐시 비율이 낮음 | WHEN 최근 7일 입력 토큰 중 캐시 읽기 비율이 20% 미만이고 응답이 100개 이상이다 / THEN "캐시가 잘 쓰이지 않습니다" 카드가 비율과 예상 절감, 원인 후보와 함께 보인다 | 부분 자동 | `crates/dduck-core/src/insights.rs::local_checks_fire_with_evidence`<br>수동: 캐시 읽기 20% 미만·응답 100개 이상인 7일 기록 → '사용 개선 분석' → 캐시 카드가 비율·예상 절감·원인 후보와 함께 보임 |
| TC-USG-08-02 | 데이터가 적음 | WHEN 기간 안 응답이 20개 미만이다 / THEN "분석할 기록이 부족합니다"가 보이고 점검 카드는 없다 | 부분 자동 | `crates/dduck-core/src/insights.rs::small_samples_are_not_checked`<br>수동: 응답 20개 미만 기간 선택 → '사용 개선 분석' → '분석할 기록이 부족해요'만 보이고 카드 없음 |

### F-USG-09

**Claude 권고** · 구현됨 — 익명·버킷 요약, WebSearch/WebFetch 가드 훅, 예산·타임아웃. tasks 3.2 실제 1회 실행은 사용자 확인 대기 · spec: `openspec/specs/usage-insights/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-09-01 | 요약 확인 | WHEN 사용자가 "Claude에게 분석 받기"를 누른다 / THEN 확인 창에 모델별 토큰·비용, 도구별 횟수·오류율 같은 숫자만 보이고 취소하면 아무것도 실행되지 않는다 | 부분 자동 | `crates/dduck-core/src/insights.rs::summary_is_anonymous_and_bucketed`<br>`crates/dduck-core/src/insights.rs::summary_stays_under_cap_with_many_names`<br>`crates/dduck-core/src/insights.rs::args_are_guarded`<br>`crates/dduck-core/src/insights.rs::guard_limits_hosts_and_counts`<br>수동: 'Claude에게 분석 받기' 클릭 → 확인 창에 숫자 요약만 보임 → 취소 → 실행 흔적(claude 프로세스) 없음 |
| TC-USG-09-02 | 실패 | WHEN 로그인 만료나 120초 초과로 실패한다 / THEN 오류가 보이고 앱 안 점검 결과는 그대로 남는다 | 부분 자동 | `crates/dduck-core/src/insights.rs::fake_cli_run`<br>`crates/dduck-core/src/insights.rs::output_states`<br>수동: 로그아웃 상태에서 'Claude에게 분석 받기' 실행 → 오류 표시, 앱 안 점검 카드는 그대로 |

### F-USG-10

**결과 보관** · 구현됨 — insights::save/load/delete로 마지막 1개만 settings에 보관 · spec: `openspec/specs/usage-insights/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-USG-10-01 | 다시 열기 | WHEN 앱을 다시 켜고 사용 통계를 연다 / THEN 마지막 권고와 실행 시각·비용이 보이고 "지우기"가 있다 | 부분 자동 | `crates/dduck-core/src/insights.rs::stores_only_the_last_result`<br>수동: 권고 1회 받은 뒤 앱 재시작 → 사용 통계 → 마지막 권고·시각·비용과 '지우기' 표시, 지우면 사라짐 |

## TSK · 작업

### F-TSK-01

**세션 상태 목록** · 구현됨 — CLI agents 폴링·status 정규화·끝난 세션 24h 보존(tracker)이 있음 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-01-01 | 끝난 백그라운드 세션 | WHEN 앱이 백그라운드 세션의 완료를 관측한 지 1시간이 지났다 / THEN 그 세션은 "완료" 열에 완료 상태로 계속 표시된다 | 자동 | `crates/dduck-core/src/tracker.rs::ended_sessions_visible_for_24h_from_first_observation` |
| TC-TSK-01-02 | 상태 필드 누락 | WHEN CLI가 반환한 세션 항목에 state 필드가 없고 status만 있다 / THEN status 값(busy→작업 중, waiting→입력 대기, idle→대기)으로 상태를 정해 표시하고 오류를 내지 않는다 | 자동 | `crates/dduck-core/src/claude_cli.rs::status_normalization`<br>`crates/dduck-core/src/claude_cli.rs::parses_real_agents_json_shapes` |

### F-TSK-02

**상태 보드** · 일부 구현 — 4열 보드는 task-lifecycle 5칸(확인 필요·리뷰 대기·정리, 완료 칸 없음)으로 대체됨 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-02-01 | 상태 열 | WHEN 세션 A는 입력 대기, B는 작업 중, C는 완료, D는 쉬는 중, E는 실패 상태다 / THEN D는 "대기 중", B는 "진행 중", A와 E는 "검토 필요", C는 "완료" 열에 표시되고, A 카드만 노란색으로 강조된다 | 부분 자동 | `src/lib/logic.test.ts::puts sessions in queued, running, review, reviewWait, wrapup columns by lane, newest first`<br>수동: 입력 대기·작업 중·완료·쉬는·실패 세션을 하나씩 둔다 → 작업 탭을 연다 → 쉬는=대기 중, 작업 중=진행 중, 입력 대기·실패=확인 필요 칸에 있고 입력 대기 카드·칸 머리만 노란색인지 본다 |

### F-TSK-03

**세션 종류별 조작 범위** · 구현됨 — canControl로 대화형 세션은 로그·중지 비활성, 터미널 열기·강제 종료는 공통 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-03-01 | 대화형 세션 선택 | WHEN 사용자가 다른 터미널에서 실행 중인 대화형 세션을 선택한다 / THEN "이 폴더에서 터미널 열기"와 "강제 종료"를 쓸 수 있고, 로그와 중지는 보이지 않으며 안내 문구가 표시된다 | 부분 자동 | `src/lib/logic.test.ts::interactive sessions cannot be controlled`<br>수동: 다른 터미널에서 claude 대화형 세션 실행 → D.Duck 작업 탭에서 선택 → '이 폴더에서 터미널 열기'·'강제 종료'만 보이고 로그·중지는 없으며 안내 문구가 보인다 |

### F-TSK-04

**세션 강제 종료** · 구현됨 — force_stop: bg는 stop 먼저, 대화형은 네이티브 확인 후 procs::terminate(소유·이름 재확인, 3초 후 SIGKILL) · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-04-01 | 응답 없는 작업 강제 종료 | WHEN 사용자가 진행 중인 세션에서 "강제 종료"를 두 번 확인한다 / THEN 세션 프로세스가 끝나고, 다음 갱신 주기에 목록에서 상태가 바뀐다 | 수동 | 수동: 응답 없는 세션 하나를 진행 중으로 둔다 → 상세에서 '강제 종료'→'강제 종료하기'(대화형은 네이티브 창도 승인) → 프로세스가 끝나고 다음 갱신에서 상태가 바뀐다 |
| TC-TSK-04-02 | 재사용된 pid | WHEN 목록의 pid가 이미 다른 프로그램에 재사용되었다 / THEN 앱은 그 프로세스에 신호를 보내지 않고 "종료하지 못했습니다" 오류를 표시한다 | 부분 자동 | `crates/dduck-core/src/procs.rs::refuses_non_claude_and_bogus_pids`<br>`crates/dduck-core/src/procs.rs::identification_rules`<br>수동: (오류 문구만 수동) 비-claude pid로 바뀐 상황은 재현 어려움 → procs 테스트로 신호 미전송 확인, 화면에서 KillFailed 시 '종료하지 못했습니다' 류 오류가 보이는지 본다 |

### F-TSK-05

**새 백그라운드 작업 실행** · 구현됨 — start_background(--bg, -- 뒤 프롬프트), 오류 매핑, 시작 직후 1초 폴링·시작 카드 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-05-01 | 정상 실행 | WHEN 사용자가 존재하는 디렉터리와 프롬프트를 입력하고 "실행"을 누른다 / THEN 백그라운드 작업이 시작되고, 10초 이내에 해당 세션이 "작업 중" 그룹에 나타난다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::start_background_passes_argv_cwd_and_config_dir`<br>`src-tauri/src/lib.rs::polls_every_second_for_a_while_after_a_start`<br>`src/components/StartingCards.test.tsx::keeps a started task until it shows up in the agents list`<br>수동: 존재하는 폴더와 프롬프트로 '실행' → 10초 안에 그 세션이 '진행 중' 칸에 나타난다 |
| TC-TSK-05-02 | 옵션처럼 보이는 프롬프트 | WHEN 프롬프트가 `--dangerously-skip-permissions`나 `-p x`처럼 `-`로 시작한다 / THEN 그 값은 CLI 옵션이 아닌 프롬프트 텍스트로만 전달된다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/tests/claude_cli_fake.rs::start_background_passes_argv_cwd_and_config_dir` |
| TC-TSK-05-03 | 신뢰되지 않은 작업 디렉터리 | WHEN 지정한 디렉터리가 Claude에서 아직 신뢰(trust)되지 않아 CLI가 실패한다 / THEN "이 디렉터리에서 claude를 한 번 실행해 신뢰를 승인하세요" 안내와 CLI 오류 메시지를 표시한다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::error_mapping`<br>`src/lib/logic.test.ts::maps CLI errors`<br>수동: 신뢰 안 된 새 폴더로 실행 → 신뢰 확인 창에서 취소 → 신뢰 승인 안내와 CLI 오류가 보인다 |
| TC-TSK-05-04 | CLI 실행 실패 | WHEN CLI가 0이 아닌 종료 코드로 끝난다 / THEN 종료 코드와 오류 출력의 마지막 부분을 화면에 표시한다 | 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::error_mapping`<br>`src/lib/logic.test.ts::maps CLI errors` |

### F-TSK-06

**실행 입력 검증** · 구현됨 — TaskSpec::validate(폴더·이름·프롬프트 크기)와 웹뷰 validateTaskName·시작 막힘 목록 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-06-01 | 잘못된 디렉터리 | WHEN 존재하지 않는 디렉터리를 지정한다 / THEN 실행하지 않고 "디렉터리를 찾을 수 없음" 오류를 표시한다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>수동: 새 작업에 없는 폴더 경로를 넣고 시작 → 실행되지 않고 폴더를 찾을 수 없다는 오류가 보인다 |
| TC-TSK-06-02 | 잘못된 이름 | WHEN 작업 이름이 `--force`이다 / THEN 실행하지 않고 이름 규칙 오류를 표시한다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`src/lib/logic.test.ts::validates task names like the backend`<br>수동: 작업 시키기 창에서 시작을 막는 조건을 만든다 → 하단 문구와 시작 버튼 상태를 본다 → 막힌 이유가 보이고 시작되지 않는다 |

### F-TSK-07

**위험 권한 모드 확인** · 구현됨 — 위험 모드는 DangerApproval(네이티브 확인) 없이는 CLI를 실행하지 않음 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-07-01 | 위험 모드 선택 | WHEN 사용자가 bypassPermissions 모드를 선택하고 "실행"을 누른다 / THEN 위험을 설명하는 네이티브 확인 대화상자가 나타나고, 승인해야만 실행된다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::dangerous_mode_needs_native_approval`<br>`crates/dduck-core/src/claude_cli.rs::dangerous_modes`<br>수동: 권한 bypassPermissions로 '실행' → 위험 설명 네이티브 창이 뜨고, 취소하면 시작 안 되고 승인하면 시작된다 |
| TC-TSK-07-02 | 화면 우회 시도 | WHEN 웹뷰에서 실행 명령을 직접 호출해 위험 모드로 실행을 요청한다 / THEN 백엔드가 네이티브 확인 대화상자를 띄우고, 거부하면 실행하지 않는다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::dangerous_mode_needs_native_approval`<br>수동: 개발자 도구 콘솔에서 start_task를 permissionMode=bypassPermissions로 직접 호출 → 네이티브 창이 뜨고 거부 시 시작되지 않는다 |

### F-TSK-08

**로그 보기** · 구현됨 — logs는 2MB 꼬리·마지막 화면만, xterm 렌더러로 표시 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-08-01 | 실행 중 로그 | WHEN 사용자가 작업 중인 백그라운드 세션의 로그를 연다 / THEN 터미널에서 보이는 것과 같은 최근 화면이 표시되고, 2초 이내에 최신 화면으로 바뀐다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::last_screen_keeps_tail_from_final_clear`<br>`crates/dduck-core/tests/claude_cli_fake.rs::agents_logs_stop`<br>`src/lib/logic.test.ts::keeps the tail from the final clear`<br>수동: 작업 중인 bg 세션의 로그를 연다 → 터미널과 같은 최근 화면이 보이고 2초 안에 바뀐다 |
| TC-TSK-08-02 | HTML 주입 | WHEN 로그 출력에 `<img src=x onerror=alert(1)>` 문자열이 들어 있다 / THEN 그 문자열이 글자 그대로 표시되고 스크립트는 실행되지 않는다 | 수동 | 수동: bg 작업에 `<img src=x onerror=alert(1)>`를 출력하게 한다 → 로그를 연다 → 문자열이 글자 그대로 보이고 알림창이 뜨지 않는다 |

### F-TSK-09

**작업 중지** · 구현됨 — stop 명령과 확인 후 중지 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-09-01 | 중지 | WHEN 사용자가 작업 중인 백그라운드 세션에서 "중지"를 누르고 확인한다 / THEN 다음 갱신 주기 안에 해당 세션이 "완료" 열에 중지 상태로 표시된다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::agents_logs_stop`<br>수동: 작업 중 bg 세션에서 '중지'→확인 → 다음 갱신에 중지 상태(멈췄어요)로 끝난 칸에 보인다 |

### F-TSK-10

**터미널 열기** · 구현됨 — open_terminal: attach 명령(셸 인용), 대화형은 폴더에서 새 터미널, 실패 시 명령 복사 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-10-01 | 입력 대기 세션 응답 | WHEN 사용자가 입력 대기 백그라운드 세션에서 "터미널에서 열기"를 누른다 / THEN 기본 터미널 앱이 열리고 해당 세션에 연결된다 | 수동 | 수동: 입력 대기 bg 세션에서 '터미널에서 열기' → 기본 터미널이 열리고 그 세션에 붙는다 |
| TC-TSK-10-02 | 터미널 제어 권한 거부 | WHEN macOS가 터미널 제어(Automation) 권한을 거부한다 / THEN 권한 안내와 함께 `claude attach <id>` 명령을 복사하는 버튼이 표시된다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::attach_command_is_quoted`<br>`crates/dduck-core/tests/claude_cli_fake.rs::ids_starting_with_a_dash_never_reach_the_cli`<br>수동: 시스템 설정에서 D.Duck의 터미널 자동화 권한을 끈다 → '터미널에서 열기' → 권한 안내와 `claude attach <id>` 복사 버튼이 보인다 |

### F-TSK-11

**입력 대기 알림** · 구현됨 — tracker 전이 1회 알림, 고정 본문, settings::notify_waiting로 끄기 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-11-01 | 대기 전환 알림 | WHEN 작업 중이던 세션이 입력 대기로 바뀐다 / THEN 세션 이름을 담은 OS 알림이 한 번 표시된다 | 부분 자동 | `crates/dduck-core/src/tracker.rs::transitions_after_priming`<br>`crates/dduck-core/src/tracker.rs::errors_do_not_reset_state_or_repeat_notifications`<br>`crates/dduck-core/src/tracker.rs::notification_body_is_fixed_and_bounded`<br>수동: 작업 중 세션이 질문을 하게 한다 → macOS 알림이 세션 이름과 함께 한 번만 뜬다 |
| TC-TSK-11-02 | 알림 끄기 | WHEN 설정에서 입력 대기 알림을 끈 상태로 세션이 입력 대기로 바뀐다 / THEN OS 알림은 표시되지 않고 목록과 트레이 표시만 바뀐다 | 수동 | 수동: 설정에서 입력 대기 알림을 끈다 → 세션을 입력 대기로 만든다 → OS 알림은 없고 목록·트레이 표시만 바뀐다 |

### F-TSK-12

**트레이의 입력 대기 표시** · 구현됨 — 트레이 set_title 개수와 입력 대기 메뉴, 항목 클릭 시 창·선택 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-12-01 | 트레이에서 세션 열기 | WHEN 입력 대기 세션 2개가 있고 사용자가 트레이 메뉴의 첫 항목을 누른다 / THEN 트레이에 "2"가 표시되어 있고, 앱 창이 열리며 그 세션이 선택된다 | 수동 | 수동: 입력 대기 세션 2개를 만든다 → 메뉴 막대 트레이에 '2'를 확인 → 트레이 메뉴 첫 항목 클릭 → 앱 창이 열리고 그 세션이 선택된다 |

### F-TSK-13

**기본 작업 폴더** · 구현됨 — 네이티브 폴더 선택과 default_cwd 설정, Composer 미리 채움 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-13-01 | 기본 폴더 미리 채움 | WHEN 기본 작업 폴더가 `/w/app`으로 설정된 상태에서 "새 작업"을 연다 / THEN 작업 폴더 칸에 `/w/app`이 채워져 있고, 다른 폴더를 골라도 다음 새 작업은 다시 `/w/app`으로 시작한다 | 부분 자동 | `crates/dduck-core/src/settings.rs::default_cwd_roundtrip`<br>수동: 설정에서 기본 작업 폴더를 고른다 → '새 작업'을 열어 폴더가 채워졌는지 보고 다른 폴더로 바꿔 시작 → 다시 '새 작업'을 열면 기본 폴더로 돌아와 있다 |

### F-TSK-14

**처음 지시 보기** · 구현됨 — prompt::first_prompt로 transcript에서 첫 지시를 읽어 카드 2줄·상세 표시 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-14-01 | 카드의 처음 지시 | WHEN 보드에 "auth 테스트를 고쳐 줘"로 시작한 세션이 있다 / THEN 그 카드에 지시의 앞부분이 보이고, 카드를 열면 지시 전체가 보인다 | 부분 자동 | `crates/dduck-core/src/prompt.rs::finds_the_first_typed_instruction`<br>`crates/dduck-core/src/prompt.rs::skips_commands_meta_and_tool_results`<br>수동: 짧은 지시로 작업을 시작한다 → 보드 카드에 지시 앞부분 2줄이 보이고 카드를 열면 전체가 보인다 |

### F-TSK-15

**폴더 신뢰 확인 후 바로 시작** · 구현됨 — trust_then_retry: 네이티브 확인→터미널 열기→설정 읽기 감시→자동 재시작 · spec: `openspec/specs/local-agent-tasks/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-15-01 | 신뢰되지 않은 폴더에서 시작 | WHEN 사용자가 신뢰되지 않은 폴더로 작업을 시작하고, 확인 창에서 "터미널에서 신뢰하기"를 고른 뒤 터미널에서 신뢰를 승인한다 / THEN 3분 안에 승인하면 작업이 자동으로 시작되어 '진행 중'에 나타난다 | 부분 자동 | `crates/dduck-core/src/trust.rs::reads_trust_for_folder_and_ancestors`<br>수동: 신뢰 안 된 새 폴더로 작업 시작 → '터미널에서 신뢰하기' → 터미널에서 신뢰 승인 → 3분 안에 작업이 자동으로 '진행 중'에 나타난다 |
| TC-TSK-15-02 | 신뢰를 거절 | WHEN 사용자가 확인 창에서 "취소"를 고른다 / THEN 작업은 시작되지 않고 "이 디렉터리에서 claude를 한 번 실행해 신뢰를 승인하세요" 안내가 표시된다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::error_mapping`<br>`src/lib/logic.test.ts::maps CLI errors`<br>수동: 신뢰 안 된 폴더로 시작 → 확인 창에서 '취소' → 작업이 시작되지 않고 신뢰 승인 안내가 보인다 |

### F-TSK-16

**세션별 도구·스킬 표시** · 구현됨 — 세션 행 topTools·skills·subagents 집계와 흐름의 스킬·서브에이전트·커맨드 항목 · spec: `openspec/specs/session-raw-view/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-16-01 | 스킬을 쓴 세션 | WHEN 세션에서 opsx:propose 스킬을 한 번, Explore 서브에이전트를 두 번 썼다 / THEN 목록 행에 "opsx:propose"와 "Explore ×2" 칩이 보이고, 흐름에는 해당 시각에 "스킬 · opsx:propose", "서브에이전트 · Explore · 설명" 항목이 보인다 | 부분 자동 | `crates/dduck-core/tests/sessions_skills_contract.rs::session_rows_carry_tool_skill_and_subagent_counts`<br>`crates/dduck-core/src/timeline.rs::skills_agents_commands_spans_and_subagent_links`<br>수동: 스킬 1회·Explore 서브에이전트 2회를 쓴 세션을 수집 → 세션 목록 행의 칩과 흐름 항목, '스킬·에이전트' 필터를 본다 |

### F-TSK-17

**원문 보기** · 구현됨 — raw::read로 span·100줄 페이지를 파일에서 읽음, 저장·로그 없음 정적 검사 · spec: `openspec/specs/session-raw-view/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-17-01 | 프롬프트 원문 | WHEN 사용자가 흐름의 프롬프트 항목에서 "원문"을 누른다 / THEN 해당 user 기록의 JSON 전체(message.content 포함)가 들여쓰기되어 보이고 복사할 수 있다 | 자동 | `crates/dduck-core/src/raw.rs::item_spans_return_full_records_masked_by_default`<br>`crates/dduck-core/src/raw.rs::pages_hold_100_records_and_long_lines_are_masked_then_cut` |
| TC-TSK-17-02 | 저장되지 않음 | WHEN 원문을 본 뒤 창을 닫는다 / THEN 앱 DB·앱 데이터 폴더·로그에서 그 내용을 찾을 수 없다 | 부분 자동 | `crates/dduck-core/tests/sessions_skills_contract.rs::timeline_is_never_logged_or_stored`<br>수동: 고유 문자열이 든 프롬프트의 원문을 연 뒤 창을 닫는다 → 앱 DB·앱 데이터 폴더·로그를 grep해 그 문자열이 없는지 본다 |

### F-TSK-18

**비밀값 가림** · 구현됨 — 원문 기본 가림, reveal 플래그로만 원값 · spec: `openspec/specs/session-raw-view/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-18-01 | 토큰이 든 기록 | WHEN 도구 입력에 API 키 모양 문자열이 있는 기록을 연다 / THEN 기본 화면에서는 가려져 있고, "가리지 않고 보기"를 켜면 원래 값이 보인다 | 자동 | `crates/dduck-core/src/raw.rs::item_spans_return_full_records_masked_by_default`<br>`crates/dduck-core/src/raw.rs::secret_named_keys_mask_everything_under_them` |

### F-TSK-19

**리뷰를 해야 끝나는 작업** · 구현됨 — statusLabel·finished_body가 리뷰 전 작업을 '오리가 끝냈어요 · 리뷰 대기'로 표시 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-19-01 | 끝남 알림 | WHEN 보고서가 붙은 작업을 에이전트가 끝내고 끝남·실패 알림이 켜져 있다 / THEN macOS 알림은 "오리가 끝냈어요 · 리뷰 대기: <작업 이름>"이고 "완료"가 없다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::tooltip_and_notification_copy` |
| TC-TSK-19-02 | 상태 칩 | WHEN 사용자가 작업 탭에서 끝났지만 리뷰하지 않은 작업 카드를 본다 / THEN 상태 칸은 "오리가 끝냈어요 · 리뷰 대기"이다 | 자동 | `src/lib/logic.test.ts::never calls an unreviewed task 완료`<br>`src/components/TaskLifecycle.test.tsx::리뷰 대기 holds the review badge's rows, failed and stopped report tasks included, and never says 완료` |

### F-TSK-20

**바꾸지 않는 "완료"·"끝" 문구** · 구현됨 — 상태 라벨에서 완료 제거(정리 끝만 완료) · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-20-01 | 문구 점검 | WHEN 앱 소스에서 작업 상태 "완료" 문자열을 찾는다 / THEN 정리 끝 상태와 바꾸지 않는 넷 말고는 남아 있지 않다 | 부분 자동 | `src/lib/logic.test.ts::never calls an unreviewed task 완료`<br>`src/components/TaskLifecycle.test.tsx::리뷰 대기 holds the review badge's rows, failed and stopped report tasks included, and never says 완료`<br>수동: app/src와 Rust 소스에서 '완료'를 grep → 완료 기준·온보딩 (완료)·포즈·튜토리얼·정리 끝 상태 외 작업 상태 문구가 없는지 확인한다 |

### F-TSK-21

**작업 보드 칸** · 구현됨 — 5칸 보드(대기 중→진행 중→확인 필요→리뷰 대기→정리), 세션 없어도 보드 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-21-01 | 실행 중인 세션 없음 | WHEN 살아 있는 세션은 없고 리뷰 대기 작업이 2개 있다 / THEN 빈 화면 대신 보드가 보이고 리뷰 대기 칸에 카드 2개가 있다 | 자동 | `src/components/TaskLifecycle.test.tsx::with no live session the board still shows 리뷰 대기 and 정리`<br>`src/components/TaskLifecycle.test.tsx::has 대기 중 → 진행 중 → 확인 필요 → 리뷰 대기 → 정리 and no 완료 column` |

### F-TSK-22

**리뷰 대기 칸** · 구현됨 — 리뷰 대기 칸 = review_inbox pending, 40장 상한, 리뷰했어요 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-22-01 | 배지와 같은 수 | WHEN 리뷰 탭 배지가 3이고 그중 하나는 실패로 끝난 보고서 작업이다 / THEN 리뷰 대기 칸 수가 3이고, 실패 작업은 "실패했어요 · 리뷰 대기"로 리뷰 대기 칸에 있으며 확인 필요 칸에는 없다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::review_column_equals_the_badge_with_failed_and_stopped_tasks`<br>`src/components/TaskLifecycle.test.tsx::리뷰 대기 holds the review badge's rows, failed and stopped report tasks included, and never says 완료`<br>`crates/dduck-core/src/task_lifecycle.rs::many_pending_cards_are_capped_but_the_total_is_not` |
| TC-TSK-22-02 | 리뷰했어요 | WHEN 사용자가 리뷰 대기 카드의 보고서에서 "리뷰했어요"를 누른다 / THEN 카드가 정리 칸으로 옮겨 가고 리뷰 배지와 리뷰 대기 칸 수가 함께 1 준다 | 자동 | `src/components/TaskLifecycle.test.tsx::리뷰했어요 from the report moves the card to 정리 and lowers the badge and the column together`<br>`crates/dduck-core/src/task_lifecycle.rs::reviewing_moves_a_card_to_wrapup_and_wrapping_takes_it_off` |

### F-TSK-23

**정리 칸에 들어가는 작업** · 구현됨 — lane_of: 기준 시각 전·외부 세션은 정리(리뷰 없이 끝났어요), 실패·무보고서는 확인 필요 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-23-01 | 이전 작업 | WHEN 업데이트 전에 끝난 보고서 작업 30개가 있고 앱을 처음 연다 / THEN 리뷰 대기 칸은 비어 있고, 그중 24시간 안에 끝난 작업만 정리 칸에 "리뷰 없이 끝났어요"로 보인다 | 부분 자동 | `crates/dduck-core/src/task_lifecycle.rs::lane_rule_in_order`<br>`crates/dduck-core/src/task_lifecycle.rs::review_column_equals_the_badge_with_failed_and_stopped_tasks`<br>`crates/dduck-core/src/review_inbox.rs::badge_matches_the_list_and_a_first_run_starts_at_zero`<br>수동: review_since 이전에 끝난 보고서 작업이 있는 DB로 업데이트 후 첫 실행 → 리뷰 대기 칸이 비고, 24시간 안에 끝난 것만 정리 칸에 '리뷰 없이 끝났어요'로 보인다 |

### F-TSK-24

**정리 점검표** · 구현됨 — check(): worktree·변경 수·PR·전달 범위, fsmonitor·필터 비실행 git · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-24-01 | worktree가 남은 작업 | WHEN worktree에서 돈 작업을 리뷰했고, 커밋 안 된 변경이 없으며, 보고서에 PR 주소가 있다 / THEN 정리 카드에 "worktree가 남아 있어요 · fix-auth · dduck/fix-auth", "커밋 안 된 변경 없어요", "보고서에 적힌 PR · #42"가 한 줄씩 보인다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::checklist_reads_worktree_branch_changes_pr_and_never_runs_fsmonitor`<br>`src/components/TaskLifecycle.test.tsx::an opened 정리 card shows local facts; worktree 정리 is off with uncommitted changes` |
| TC-TSK-24-02 | fsmonitor가 설정된 저장소 | WHEN 저장소 설정의 `core.fsmonitor`가 임의 명령을 가리킨다 / THEN 점검표를 펼쳐도 그 명령은 실행되지 않는다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::checklist_reads_worktree_branch_changes_pr_and_never_runs_fsmonitor`<br>`crates/dduck-core/src/task_lifecycle.rs::status_never_runs_a_planted_clean_filter` |

### F-TSK-25

**worktree 정리** · 구현됨 — remove_plan 조건·확인 문구(무시된 파일 수, 브랜치 남김), 링크·바뀐 폴더 거부 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-25-01 | 변경이 남은 worktree | WHEN 정리 카드의 worktree에 커밋 안 된 변경이 2개 있다 / THEN "worktree 정리"는 비활성이고 "커밋 안 된 변경이 있어 정리할 수 없어요"가 보인다 | 자동 | `src/components/TaskLifecycle.test.tsx::an opened 정리 card shows local facts; worktree 정리 is off with uncommitted changes`<br>`crates/dduck-core/src/task_lifecycle.rs::checklist_reads_worktree_branch_changes_pr_and_never_runs_fsmonitor` |
| TC-TSK-25-02 | 무시된 파일 | WHEN 변경은 없고 무시된 빌드 파일이 120개 있는 worktree를 정리한다 / THEN 확인 창에 "무시된 파일 120개(빌드 결과 등)도 함께 지워져요"가 보이고, 확인하면 worktree만 지워지고 브랜치는 남는다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::removing_a_clean_worktree_counts_ignored_files_and_keeps_the_branch` |

### F-TSK-26

**정리 끝** · 구현됨 — wrap/unwrap 5초 되돌리기, 리뷰 되돌리면 정리 끝도 해제 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-26-01 | 정리 끝 | WHEN 사용자가 정리 카드의 "정리 끝"을 누른다 / THEN 카드가 보드에서 빠지고 "정리 끝 · 되돌리기"가 5초 보이며, 리뷰 탭 전체에서 그 작업 카드에 "완료" 칩이 있다 | 부분 자동 | `src/components/TaskLifecycle.test.tsx::정리 끝 takes the card off with a 5-second undo, and the review tab shows 완료`<br>`crates/dduck-core/src/task_lifecycle.rs::reviewing_moves_a_card_to_wrapup_and_wrapping_takes_it_off`<br>수동: 정리 카드에서 '정리 끝' → 5초 '정리 끝 · 되돌리기' 확인 → 리뷰 탭 '전체'에서 그 작업 카드에 '완료' 칩이 있는지 본다(테스트 이름과 달리 이 부분 단언 없음) |
| TC-TSK-26-02 | 리뷰 되돌리기 | WHEN 정리 끝인 작업의 리뷰했어요를 리뷰 탭에서 되돌린다 / THEN 그 작업은 "완료"가 아니고 리뷰 대기 칸에 다시 나온다 | 부분 자동 | `crates/dduck-core/src/task_lifecycle.rs::reviewing_moves_a_card_to_wrapup_and_wrapping_takes_it_off`<br>수동: 정리 끝인 작업을 리뷰 탭에서 '리뷰했어요' 되돌리기 → 완료 칩이 사라지고 작업 보드 리뷰 대기 칸에 다시 나온다 |

### F-TSK-27

**트레이와 설정 문구** · 구현됨 — tooltip(확인 필요, 리뷰 대기), 리뷰 수만 바뀌어도 갱신 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-27-01 | 툴팁 | WHEN 확인 필요 1개와 리뷰 대기 2개가 있다가 리뷰 대기 하나를 리뷰한다 / THEN 트레이 툴팁이 "D.Duck — 확인 필요 1 · 리뷰 대기 2"에서 "D.Duck — 확인 필요 1 · 리뷰 대기 1"로 바뀐다 | 자동 | `crates/dduck-core/src/task_lifecycle.rs::tooltip_and_notification_copy`<br>`crates/dduck-core/src/task_lifecycle.rs::review_column_equals_the_badge_with_failed_and_stopped_tasks` |

### F-TSK-28

**끝난 개발 작업의 후속** · 구현됨 — canFollowUp: 끝난 개발 작업(실패·리뷰 대기·정리)에 QA·리뷰 맡기기 · spec: `openspec/changes/task-lifecycle/specs/task-lifecycle/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TSK-28-01 | 리뷰 대기 칸의 개발 작업 | WHEN 끝난 개발 작업 카드가 리뷰 대기 칸에 있다 / THEN 카드에 "QA 맡기기"·"리뷰 맡기기"가 보인다 | 자동 | `src/components/TaskLifecycle.test.tsx::리뷰 대기 holds the review badge's rows, failed and stopped report tasks included, and never says 완료`<br>`src/lib/brief.test.ts::follow-ups open a fresh task in the dev session's own folder with a cleaned name` |

## BRF · 작업 맡기기

### F-BRF-01

**다듬기 시작** · 구현됨 — BriefRefiner 질문 받기/다시 쓰기만, CLI 없으면 비활성, 동의 창 비용 상한 · spec: `openspec/specs/brief-grilling/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-01-01 | 질문 받기 시작 | WHEN 사용자가 목표에 "로그인 가끔 실패 고치기"만 적고 "질문 받기"를 누른 뒤 확인한다 / THEN 질문 카드 3~8개(팀 설정, 기본 5)가 나타나고 각 카드에 선택지와 미리 선택된 추천이 있다 | 부분 자동 | `src/components/BriefRefiner.test.tsx::two rounds: recommended answers preselected, keyboard submit, then 다듬기 완료`<br>`crates/dduck-core/src/brief_refine.rs::consent_follows_rounds_with_a_hard_ceiling`<br>`crates/dduck-core/src/brief_refine.rs::schema_violations_are_rejected`<br>수동: 목표만 적고 '질문 받기' → 네이티브 확인 창에 비용 상한과 전송 안내가 보이는지 확인 후 승인 → 질문 카드 3~8개(기본 5)에 추천이 미리 선택되어 있다 |

### F-BRF-02

**선택형 답** · 구현됨 — 추천 기본 선택·직접 입력 200자·건너뛰기·여러 개·키보드 제출, 라운드 상한 · spec: `openspec/specs/brief-grilling/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-02-01 | 추천 그대로 다음 라운드 | WHEN 사용자가 아무것도 바꾸지 않고 "답하고 계속"을 누른다 / THEN 추천 답들로 다음 라운드 질문과 갱신된 브리프 제안이 온다 | 자동 | `src/lib/refine.test.ts::defaults to the recommendation, so an unchanged submit accepts it`<br>`src/components/BriefRefiner.test.tsx::two rounds: recommended answers preselected, keyboard submit, then 다듬기 완료` |
| TC-BRF-02-02 | 라운드 상한 | WHEN 3라운드를 마쳤거나 Claude가 더 물을 것이 없다고 답한다 / THEN 질문 대신 "다듬기 완료"와 최종 브리프 제안만 보인다 | 자동 | `crates/dduck-core/src/brief_refine.rs::finishing_rules`<br>`crates/dduck-core/src/brief_refine.rs::input_limits`<br>`src/components/BriefRefiner.test.tsx::two rounds: recommended answers preselected, keyboard submit, then 다듬기 완료` |

### F-BRF-03

**다시 쓴 브리프 적용** · 구현됨 — 필드별 전/후 비교·적용·되돌리기, 적용 전 폼 불변 · spec: `openspec/specs/brief-grilling/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-03-01 | 완료 기준만 적용 | WHEN 사용자가 완료 기준 제안만 "적용"한다 / THEN 완료 기준만 바뀌고 나머지 필드는 그대로이며 "되돌리기"로 이전 값이 돌아온다 | 자동 | `src/components/BriefRefiner.test.tsx::applies one field, then reverts it`<br>`src/lib/refine.test.ts::compares field by field and flags lost constraint lines` |

### F-BRF-04

**안전한 실행** · 구현됨 — --tools= 비움·setting-sources 비움·strict-mcp·no-session-persistence·예산 상한, 스키마 재검증 · spec: `openspec/specs/brief-grilling/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-04-01 | 형식이 틀린 응답 | WHEN CLI 결과가 스키마에 맞지 않는다 / THEN "Claude 응답을 읽지 못했습니다. 다시 시도하세요"가 보이고 폼 값은 바뀌지 않는다 | 자동 | `src/components/BriefRefiner.test.tsx::a malformed answer shows the fixed message and leaves the form alone`<br>`crates/dduck-core/src/brief_refine.rs::schema_violations_are_rejected`<br>`crates/dduck-core/src/brief_refine.rs::one_retry_on_a_malformed_answer` |
| TC-BRF-04-02 | 비용 표시 | WHEN 한 라운드가 끝난다 / THEN 이번 라운드 비용과 누적 비용(API 환산 추정)이 보인다 | 자동 | `src/components/BriefRefiner.test.tsx::two rounds: recommended answers preselected, keyboard submit, then 다듬기 완료`<br>`src/components/BriefRefiner.test.tsx::cancel and timeout copy: the cost chip says 취소됨, the message gives the cost` |

### F-BRF-05

**개선 제안에서 작업 시작** · 구현됨 — draftFromFinding/draftFromAdvice로 초안 브리프·출처 배지·refineHint · spec: `openspec/specs/brief-grilling/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-05-01 | 서브에이전트 모델 제안을 작업으로 | WHEN 사용자가 "서브에이전트가 대부분 Opus를 상속" 점검 결과의 "작업으로 만들기"를 누른다 / THEN 새 작업 폼이 목표 "리뷰·QA 에이전트 정의에 model 지정"과 근거·완료 기준이 채워진 채 열리고, "질문 받기로 다듬기" 안내가 보이며, 시작 전에는 아무것도 실행되지 않는다 | 부분 자동 | `src/lib/refine.test.ts::subagent check: a model category and a measurable done`<br>`src/lib/refine.test.ts::local check: action as goal, evidence and diff-first as constraints, a metric as done, nothing auto-started`<br>수동: 사용 개선 분석에서 '서브에이전트가 대부분 Opus를 상속' 결과의 '작업으로 만들기' → 목표·근거·완료 기준이 채워진 새 작업 폼과 '질문 받기로 다듬기' 안내, 실행 없음 확인 |
| TC-BRF-05-02 | Claude 권고 항목을 작업으로 | WHEN Claude 권고의 '비용 줄이기' 섹션 두 번째 항목에서 "작업으로 만들기"를 누른다 / THEN 그 항목의 문장이 목표·근거로 들어간 초안이 열린다 | 부분 자동 | `src/lib/refine.test.ts::advice draft keeps the goal empty and quotes the item as reference, plan only`<br>`src/lib/refine.test.ts::top-level items under ## sections only, 출처 excluded`<br>수동: Claude 권고 '비용 줄이기' 둘째 항목의 '작업으로 만들기' → 초안이 열리는지 본다(구현은 목표를 비우고 항목을 참고 인용으로 넣음: 스펙의 '목표로 들어감'과 다름) |

### F-BRF-06

**전역 설정** · 구현됨 — task_report::wanted가 전역 설정·작업 체크를 따르고 꺼지면 --add-dir·지시 없음 · spec: `openspec/specs/decision-report/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-06-01 | 끈 상태 | WHEN 사용자가 설정에서 작업 보고서를 끄고 새 작업을 시작한다 / THEN 실행 인자에 보고서 지시와 보고서 폴더가 없고 작업 상세에 보고서가 나오지 않는다 | 부분 자동 | `crates/dduck-core/src/task_report.rs::report_is_wanted_only_for_local_tasks_with_the_setting_on`<br>`crates/dduck-core/src/task_report.rs::spec_gets_add_dir_and_instruction_after_team_rules`<br>수동: 설정에서 작업 보고서를 끄고 새 작업 시작 → 작업 상세에 보고서가 나오지 않는다 |

### F-BRF-07

**보고서 작성 지시** · 구현됨 — instruction(): report.json 경로·in_progress 갱신·시크릿 금지, 앱 데이터 폴더 · spec: `openspec/specs/decision-report/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-07-01 | 중간에 멈춘 작업 | WHEN 작업이 중간에 중단되었다 / THEN 그때까지 갱신된 보고서가 "진행 중 상태로 멈춤"과 함께 보인다 | 자동 | `src/lib/report.test.ts::puts a one-line verdict first, with the pose`<br>`crates/dduck-core/src/task_report.rs::instruction_names_the_path_and_rules` |

### F-BRF-08

**보고서 보기** · 구현됨 — ReportBody 순서(맡긴 내용→대조→주의→흐름→기록), 형식 오류·보고서 없음 · spec: `openspec/specs/decision-report/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-08-01 | 합의 확인 | WHEN 사용자가 끝난 작업의 보고서를 연다 / THEN 맨 위에 시작할 때의 목표·범위 밖·제약·완료 기준이 보이고, 그 아래 완료 기준마다 ✓/△/✕와 근거가 보인다 | 자동 | `src/components/TaskReport.test.tsx::shows the agreed brief first, then criteria, attention, flow and tables in order`<br>`crates/dduck-core/src/task_report.rs::view_uses_the_db_id_and_the_apps_brief`<br>`src/lib/report.test.ts::matches agreed criteria line by line and keeps the agent's extras` |
| TC-BRF-08-02 | 주의 먼저 | WHEN 에이전트가 묻지 않고 가정한 결정 1개와 사용자가 필요한 blocked 1개를 기록했다 / THEN 두 항목이 "주의" 칩으로 보고서 위쪽에 먼저 보인다 | 자동 | `src/components/TaskReport.test.tsx::orders the chips: a block that needs the user, unmet criteria, then flagged decisions`<br>`src/lib/report.test.ts::orders caution chips like the verdict line: needs-user, ✕, deviates, △, hard, assumed (one chip per decision)` |

### F-BRF-09

**로컬 출처로 만드는 모델 목록** · 일부 구현 — core model_select.rs만 있고 셸 명령·TaskSpec·UI 미연결(D-154 (9), tasks 2.4~6.1 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-09-01 | 세 출처를 합친다 | WHEN CLI 도움말에 `'fable'`·`'opus'`·`'sonnet'`이 있고, 최근 30일 기록에 `claude-opus-5-5`·`claude-haiku-5-5`가 있고, 설정 `model`이 `sonnet`이다 / THEN 목록에 `fable`(CLI), `opus`(CLI), `sonnet`(CLI·설정), `claude-opus-5-5`(최근 사용), `claude-haiku-5-5`(최근 사용)가 각각 한 번씩 나온다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::sources_merge_and_sort`<br>`crates/dduck-core/src/model_select.rs::help_parser_reads_the_model_paragraph_only`<br>수동: (UI 연결 후) 모델 고르기 목록을 열어 fable·opus(CLI), sonnet(CLI·설정), 최근 사용 id 2개가 한 번씩 보이는지 본다 — 현재 화면 없음 |
| TC-BRF-09-02 | CLI 도움말 형식이 바뀌었다 | WHEN `claude --help` 출력에서 `--model` 설명을 찾지 못하거나 따옴표 친 별칭이 없다 / THEN CLI 출처 항목만 빠진다. 다른 출처와 직접 입력은 그대로 쓸 수 있다. 목록 아래에 "CLI에서 별칭을 못 읽었어요" 한 줄이 보인다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::help_parser_reads_the_model_paragraph_only`<br>`crates/dduck-core/src/model_select.rs::sources_merge_and_sort`<br>수동: (UI 연결 후) 도움말에 --model 별칭이 없는 가짜 claude로 목록을 연다 → CLI 항목만 빠지고 'CLI에서 별칭을 못 읽었어요' 줄이 보인다 |
| TC-BRF-09-03 | 새로 고침 | WHEN 사용자가 목록의 새로 고침을 누른다 / THEN 캐시를 버리고 도움말·설정·기록을 다시 읽는다. 모델 호출이나 네트워크 요청은 없다 | 수동 | 수동: (model_options 명령 미구현) 목록의 새로 고침을 누르고 네트워크 모니터로 → 도움말·설정·기록만 다시 읽고 네트워크·모델 호출이 없는지 본다 |
| TC-BRF-09-04 | 형식이 이상한 값은 버린다 | WHEN 설정이나 기록에 `<synthetic>`, 공백이 든 값, 100자가 넘는 값이 있다 / THEN 그 값은 목록에 나오지 않는다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::sources_merge_and_sort`<br>`crates/dduck-core/src/model_select.rs::model_value_table`<br>수동: (UI 연결 후) 설정·기록에 <synthetic>·공백 값·101자 값을 넣고 목록을 연다 → 그 값이 안 보인다 |

### F-BRF-10

**설정은 모델 키만 읽는다** · 일부 구현 — core model_select.rs만 있고 셸 명령·TaskSpec·UI 미연결(D-154 (9), tasks 2.4~6.1 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-10-01 | env에 토큰이 같이 있다 | WHEN 사용자 settings `env`에 `ANTHROPIC_DEFAULT_OPUS_MODEL`과 `ANTHROPIC_AUTH_TOKEN`이 함께 있다 / THEN 목록 응답·로그·DB 어디에도 토큰 값이나 토큰 키 이름이 없다. opus 별칭의 실제 id는 그 변수의 값으로 표시된다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::settings_keep_only_model_keys_and_named_env`<br>수동: (UI 연결 후) settings env에 ANTHROPIC_DEFAULT_OPUS_MODEL과 가짜 ANTHROPIC_AUTH_TOKEN을 넣고 목록을 연다 → opus 실제 id만 보이고 로그·DB를 grep해 토큰 값·키 이름이 없다 |

### F-BRF-11

**기본값 항목** · 일부 구현 — core model_select.rs만 있고 셸 명령·TaskSpec·UI 미연결(D-154 (9), tasks 2.4~6.1 미완); 기본값 줄 문구 없음 · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-11-01 | 아무 설정도 없다 | WHEN 모델 관련 설정이 하나도 없고 최근 메인 세션 기록이 `claude-opus-5-5`다 / THEN 기본값 항목은 "기본값 (지금: claude-opus-5-5 · 최근 기록)"이다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::sources_merge_and_sort`<br>`crates/dduck-core/src/model_select.rs::recent_queries_skip_synthetic_and_dduck_model_sessions`<br>수동: (UI 연결 후) 모델 설정 없이 최근 메인 기록이 claude-opus-5-5인 상태 → 기본값 항목이 '기본값 (지금: claude-opus-5-5 · 최근 기록)'이다 |
| TC-BRF-11-02 | Composer에서 유형 기본값이 있다 | WHEN 리뷰 유형의 기본 모델이 `sonnet`이고 사용자가 Composer에서 리뷰 유형을 고른다 / THEN 모델 고르기의 기본값 항목은 "기본값 (지금: sonnet · 유형 설정)"이다 | 수동 | 수동: (Composer 미연결) 리뷰 유형 기본 모델을 sonnet으로 두고 Composer에서 리뷰 유형 선택 → 기본값 항목이 '기본값 (지금: sonnet · 유형 설정)'이다 |

### F-BRF-12

**직접 입력 검증** · 일부 구현 — Rust valid_model만 있음; 웹뷰 model.ts·시작 인자 재검사 미연결 · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-12-01 | 허용되는 입력 | WHEN 사용자가 `claude-opus-4-6[1m]`, `us.anthropic.claude-opus-4-8`, `claude-sonnet-4-5@20250929` 중 하나를 입력한다 / THEN 받아들여지고 "직접 입력" 출처로 표시된다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::model_value_table`<br>수동: (UI 연결 후) 직접 입력에 claude-opus-4-6[1m] 등을 넣는다 → 받아들여지고 '직접 입력' 출처로 보인다 |
| TC-BRF-12-02 | 거부되는 입력 | WHEN 입력이 `-p`, `opus --dangerously-skip-permissions`, `Opus`, `a/b`, `inherit`, `null`, `true`, `123`, 101자 문자열 중 하나다 / THEN 저장·시작을 거부하고 "모델 이름 형식이 아니에요" 한 줄을 보인다. 웹뷰를 거치지 않은 요청도 Rust가 같은 이유로 거부한다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::model_value_table`<br>`crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (UI·start_task 연결 후) -p·Opus·a/b·inherit·null·101자 입력 → '모델 이름 형식이 아니에요'로 거부, 콘솔에서 start_task 직접 호출해도 Rust가 거부 |

### F-BRF-13

**에이전트별 기본 모델** · 미구현 — TypeDef.model·agents_catalog with_model 없음(tasks 2.5·2.6 미완), file_model 읽기만 있음 · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-13-01 | 유형 기본 모델 | WHEN 사용자가 리뷰 유형의 기본 모델을 `sonnet`으로 저장하고, 그 유형으로 작업을 시작한다 / THEN 시작 인자의 `--agents` JSON에서 그 역할의 `model`이 `"sonnet"`이다 | 수동 | 수동: (미구현) 리뷰 유형 기본 모델을 sonnet으로 저장 후 그 유형으로 시작 → --agents JSON의 역할 model이 "sonnet"인지 가짜 claude argv로 본다 |
| TC-BRF-13-02 | 파일 에이전트 줄 편집 | WHEN CRLF 줄끝을 쓰는 `design-critic.md`에 `model:` 줄이 없고, 사용자가 `opus`를 저장한다 / THEN frontmatter에 `model: opus` 한 줄이 CRLF로 더해지고 나머지 바이트는 같다. 다시 기본값으로 저장하면 원래 파일과 바이트가 같다 | 수동 | 수동: (미구현) CRLF design-critic.md에 opus 저장 → model: opus 한 줄만 CRLF로 추가, 기본값으로 되돌리면 cmp로 원본과 바이트가 같다 |
| TC-BRF-13-03 | 여러 줄로 쓴 값 | WHEN 파일의 `model` 키가 두 번 나오거나 값이 여러 줄이다 / THEN 저장을 거부하고 "정의 편집에서 고쳐 주세요"를 보인다 | 수동 | 수동: (미구현) model 키가 둘이거나 여러 줄인 에이전트 파일에 저장 → 거부되고 '정의 편집에서 고쳐 주세요'가 보인다 |
| TC-BRF-13-04 | 파일에 이미 있는 inherit | WHEN 파일 에이전트의 `model:` 값이 `inherit`다 / THEN 고르기는 "기본값 (메인 대화 모델을 따라요)"로 보이고, 사용자가 바꾸지 않으면 그 줄은 그대로다 | 수동 | 수동: (미구현) model: inherit인 파일 에이전트를 연다 → '기본값 (메인 대화 모델을 따라요)'로 보이고, 저장 안 하면 줄이 그대로다 |

### F-BRF-14

**작업별 모델** · 미구현 — TaskSpec에 model 필드 없음, --model 인자 생성 안 함(tasks 2.7·4.4 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-14-01 | 작업에서 고른다 | WHEN 사용자가 Composer에서 `haiku`를 고르고 시작한다 / THEN `claude --bg` 인자에 `--model=haiku`가 정확히 한 번 있고, 프롬프트 구분자 `--`보다 앞에 있다 | 수동 | 수동: (미구현) Composer에서 haiku 선택 후 시작 → 가짜 claude argv에 --model=haiku가 한 번, -- 앞에 있다 |
| TC-BRF-14-02 | 고르지 않는다 | WHEN 유형·에이전트·작업 어디에도 모델이 없다 / THEN 시작 인자에 `--model`이 없고 역할 JSON에도 `model`이 없다 | 수동 | 수동: (미구현) 유형·에이전트·작업 모두 모델 없이 시작 → argv에 --model이 없고 역할 JSON에도 model이 없다 |

### F-BRF-15

**모델 우선순위** · 일부 구현 — resolve()/check_pin 우선순위·잠금 core만, 미리보기·인자 미연결, 스파이크 1.1 대기 · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-15-01 | 작업 값이 이긴다 | WHEN 유형 기본값이 `sonnet`이고 Composer에서 `opus`를 고른다 / THEN 역할 JSON `model`과 `--model`이 모두 `opus`이고, 미리보기에 "모델: opus · 이번 작업"이 보인다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (UI 연결 후) 유형 기본 sonnet, Composer opus → 미리보기 '모델: opus · 이번 작업', argv --model=opus·역할 model opus |
| TC-BRF-15-02 | 에이전트 파일이 유형보다 앞선다 | WHEN 유형이 "내 에이전트" `design-critic`을 쓰고, 그 파일의 `model`이 `opus`이고, 유형 기본값이 `haiku`다 / THEN 작업 모델은 `opus`(에이전트 설정)이고, 시작 인자에 `--model=haiku`가 없다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (연결 후) design-critic(model: opus)을 쓰는 유형, 유형 기본 haiku로 시작 → 작업 모델 opus(에이전트 설정), argv에 --model=haiku 없음 |
| TC-BRF-15-03 | 파일 에이전트 위에 작업 값을 고른다 | WHEN 파일 에이전트(`model: opus`)를 페르소나로 고르고 Composer에서 `sonnet`을 고른다 / THEN 확인 실행(design 1.1)에서 `--model`이 `--agent`의 모델보다 앞선다고 기록되기 전에는 Composer 모델 고르기가 잠긴다. 그때 "이 에이전트는 파일에 정한 모델(opus)로 돌아요" 한 줄이 보인다. 기록된 뒤에는 `--model=sonnet`으로 시작한다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (연결 후) 파일 에이전트(model: opus) 페르소나 선택 → Composer 모델 고르기가 잠기고 '이 에이전트는 파일에 정한 모델(opus)로 돌아요'가 보인다 |

### F-BRF-16

**관리 설정을 읽지 못하면 막는다** · 일부 구현 — core model_select.rs만 있고 셸 명령·TaskSpec·UI 미연결(D-154 (9), tasks 2.4~6.1 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-16-01 | 관리 파일이 깨졌다 | WHEN 관리 settings 파일이 있고 JSON으로 읽히지 않는다 / THEN 모든 항목이 "허용 목록을 확인할 수 없어 막았어요"로 비활성이다. 목록 아래에 "관리 설정을 읽지 못해 기본값만 쓸 수 있어요"가 보인다. 기본값으로는 시작할 수 있다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::broken_managed_file_or_plist_blocks_everything_but_the_default`<br>`crates/dduck-core/src/model_select.rs::allowlist_table`<br>수동: (UI 연결 후) 관리 settings 파일을 깨진 JSON으로 둔다 → 모든 항목이 비활성·이유 문구, 목록 아래 안내, 기본값으로는 시작된다 |

### F-BRF-17

**허용 목록 검사** · 일부 구현 — allowlist/check_allowed core만, 시작·재개·저장 경로 미연결(tasks 3.2 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-17-01 | 사용자 설정이 막는다 | WHEN 관리 설정이 없고 사용자 settings `availableModels`가 `["sonnet"]`이다 / THEN `opus` 항목은 비활성이고 "설정의 허용 목록에 없는 모델이에요"가 붙는다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::allowlist_table`<br>수동: (UI 연결 후) 사용자 settings availableModels=["sonnet"] → opus 항목 비활성과 '설정의 허용 목록에 없는 모델이에요' |
| TC-BRF-17-02 | 관리 설정이 막는다 | WHEN 관리 설정 `availableModels`가 `["sonnet", "haiku"]`이고 사용자가 `claude-opus-5-5`를 직접 입력한다 / THEN 시작을 거부하고 "관리 설정이 이 모델을 막아요" 한 줄을 보인다. CLI는 실행하지 않는다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::allowlist_table`<br>수동: (연결 후) 관리 availableModels=[sonnet,haiku]에서 claude-opus-5-5 직접 입력 후 시작 → '관리 설정이 이 모델을 막아요', 가짜 claude 미실행 |
| TC-BRF-17-03 | 저장된 값이 손상됐다 | WHEN 팀 설정 DB의 유형 모델 값이 손으로 `opus x`로 바뀌었다 / THEN 그 유형으로 시작을 거부하고 "저장된 모델 이름이 올바르지 않아요"를 보인다. `--model` 없이 조용히 시작하지 않는다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (연결 후) 팀 설정 DB의 유형 모델을 'opus x'로 고친 뒤 그 유형으로 시작 → '저장된 모델 이름이 올바르지 않아요'로 거부 |
| TC-BRF-17-04 | 빈 목록 | WHEN 관리 설정 `availableModels`가 `[]`다 / THEN 기본값만 고를 수 있다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::allowlist_table`<br>수동: (UI 연결 후) 관리 availableModels=[] → 기본값만 고를 수 있다 |

### F-BRF-18

**비용 안내** · 일부 구현 — pricing::compare·cost_line core만, 고르기 화면 미연결(tasks 2.4 미체크) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-18-01 | 더 비싼 모델 | WHEN 기본값이 `claude-sonnet-5-5`이고 사용자가 실제 id가 `claude-opus-5-5`로 알려진 `opus`를 고른다 / THEN "기본값보다 토큰당 약 N배 비싸요 · 앱 단가표 기준" 한 줄이 보인다. N은 두 출력 단가의 비율을 소수 첫째 자리에서 반올림한 값이다 | 부분 자동 | `crates/dduck-core/src/pricing.rs::compare_strips_provider_parts_and_needs_both_prices`<br>수동: (UI 연결 후) 기본값 claude-sonnet-5-5에서 opus 선택 → '기본값보다 토큰당 약 N배 비싸요 · 앱 단가표 기준' |
| TC-BRF-18-02 | 유형 기본값이 기준이다 | WHEN Composer에서 유형 기본값이 `claude-opus-5-5`이고 사용자가 `claude-opus-5-5`를 고른다 / THEN 비용 줄이 없다 | 부분 자동 | `crates/dduck-core/src/pricing.rs::compare_strips_provider_parts_and_needs_both_prices`<br>수동: (연결 후) 유형 기본값 claude-opus-5-5에서 같은 값 선택 → 비용 줄 없음 |
| TC-BRF-18-03 | 제공자 앞부분 | WHEN 사용자가 `us.anthropic.claude-opus-5-5`를 직접 입력하고 기본값이 `claude-sonnet-5-5`다 / THEN 단가는 `claude-opus-5-5`로 찾아 비용 줄을 보인다 | 부분 자동 | `crates/dduck-core/src/pricing.rs::compare_strips_provider_parts_and_needs_both_prices`<br>수동: (연결 후) us.anthropic.claude-opus-5-5 직접 입력 → 비용 줄이 보인다 |
| TC-BRF-18-04 | 단가를 모르는 모델 | WHEN 사용자가 단가표에 없는 id를 직접 입력한다 / THEN 비용 줄이 없다 | 부분 자동 | `crates/dduck-core/src/pricing.rs::compare_strips_provider_parts_and_needs_both_prices`<br>`crates/dduck-core/src/pricing.rs::unknown_model_has_no_price`<br>수동: (연결 후) 단가표에 없는 id 직접 입력 → 비용 줄 없음 |

### F-BRF-19

**이어 하기와 재작업은 원래 모델을 쓴다** · 일부 구현 — DB v13 열·ResumeSpec.model_args 자리·check_pin만, 기록·재현 미연결(tasks 2.8·5.1 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-19-01 | 한도 뒤 재개 | WHEN `--model=haiku`로 시작한 작업이 한도에 닿았다가 재개된다 / THEN 재개 인자에 `--model=haiku`가 있다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::resume_args_carry_the_start_values_and_nothing_else`<br>수동: (연결 후) --model=haiku로 시작한 작업을 한도에 걸리게 한 뒤 재개 → 재개 argv에 --model=haiku |
| TC-BRF-19-02 | 유형 기본값으로 돈 작업의 재개 | WHEN 유형 기본값 `sonnet`으로 시작한 작업이 한도에 닿았고, 그 사이 유형 기본값이 `opus`로 바뀌었다 / THEN 재개 인자와 역할 JSON `model`은 시작 때와 같은 `sonnet`이다. 파일 에이전트 잠금으로 재개가 막히지 않는다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::precedence_and_lock`<br>수동: (연결 후) 유형 기본 sonnet으로 시작→한도→유형 기본을 opus로 변경→재개 → argv·역할 model이 sonnet, 잠금 안 걸림 |
| TC-BRF-19-03 | 에이전트 층 작업의 새 작업 재작업 | WHEN 파일 에이전트 `model: opus`로 돈 작업(층 = 에이전트)에서 "새 작업으로" 재작업을 연다 / THEN Composer 모델 칸은 기본값이고 잠기지 않는다 | 수동 | 수동: (미구현) 파일 에이전트 model: opus로 돈 작업에서 '새 작업으로' 재작업 → Composer 모델 칸이 기본값이고 잠기지 않는다 |
| TC-BRF-19-04 | 그 사이 허용 목록이 바뀌었다 | WHEN 재개 전에 관리 설정이 `haiku`를 막게 바뀌었다 / THEN 자동 재개를 하지 않고, 버튼으로만 잇게 한다. 이유는 "관리 설정이 이 작업의 모델을 막아요"다 | 수동 | 수동: (미구현) 재개 전 관리 설정이 haiku를 막게 바꾼다 → 자동 재개 없이 버튼만, 이유 '관리 설정이 이 작업의 모델을 막아요' |

### F-BRF-20

**요청과 다른 모델 알림** · 일부 구현 — mismatch_line·first_main_model core만, 작업 상세 미연결(tasks 4.5 미완) · spec: `openspec/changes/model-select/specs/model-select/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-20-01 | CLI가 기본 모델로 바꿨다 | WHEN `--model=opus`로 시작했는데 그 세션 메인 대화의 첫 기록 모델이 `claude-sonnet-5-5`다 / THEN 작업 상세에 "요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요" 한 줄이 보인다 | 부분 자동 | `crates/dduck-core/src/model_select.rs::mismatch_compares_family_or_prefix`<br>`crates/dduck-core/src/model_select.rs::recent_queries_skip_synthetic_and_dduck_model_sessions`<br>수동: (연결 후) --model=opus로 시작했는데 첫 기록이 claude-sonnet-5-5인 작업 상세 → '요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요' |

### F-BRF-21

**번들 팀 역할** · 구현됨 — prepare가 유형→번들 역할 --agents/--agent 주입, 페르소나 우선, 백엔드 전용 필드 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-21-01 | 검토 유형 작업 | WHEN 사용자가 페르소나 없이 "검토" 유형으로 작업을 시작한다 / THEN `--agents`에 dduck-reviewer 정의(도구 Read, Glob, Grep과 git 읽기 명령 4개, AG-15)가 들어가고 `--agent=dduck-reviewer`, `--allowedTools`에 git 읽기 명령, 권한 manual로 시작된다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/src/team_config.rs::bundle_is_consistent` |
| TC-BRF-21-02 | 사용자 페르소나 우선 | WHEN 사용자가 "검토" 유형에 페르소나 design-critic을 고른다 / THEN `--agent=design-critic`으로 시작되고 번들 역할은 주입되지 않는다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args` |

### F-BRF-22

**작업 유형 프리셋** · 구현됨 — team_defaults.json 유형 프리셋(dev=plan·high), typed 작업 위험 모드 거부 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-22-01 | 개발 유형 기본값 | WHEN 사용자가 "개발"을 고른다 / THEN 권한이 "계획만 세우기", effort가 high로 채워지고, 안전한 git 저장소면 worktree가 켜진다 | 수동 | 수동: 안전한 git 저장소 폴더에서 새 작업에 '개발' 유형 선택 → 권한 '계획만 세우기', effort high, worktree 켜짐으로 채워진다 |
| TC-BRF-22-02 | 배포 유형의 위험 모드 | WHEN 웹뷰가 "배포" 유형에 bypassPermissions로 시작을 요청한다 / THEN 작업을 시작하지 않고 오류를 표시한다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/src/team_config.rs::untouched_items_follow_new_defaults_and_bad_items_fall_back` |

### F-BRF-23

**구조화 브리프** · 구현됨 — compose_brief 고정 제목, Composer 미리보기 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-23-01 | 브리프 조립 | WHEN 사용자가 목표와 완료 기준만 적는다 / THEN 지시는 "## 목표"와 "## 완료 기준" 두 부분으로 만들어지고 미리보기에 보인다 | 부분 자동 | `crates/dduck-core/src/team.rs::brief_is_assembled_with_fixed_headings`<br>`src/lib/brief.test.ts::assembles and parses back with the backend's headings`<br>수동: 목표와 완료 기준만 적는다 → 시작 전 미리보기에 '## 목표'와 '## 완료 기준' 두 부분만 보인다 |

### F-BRF-24

**팀 규칙** · 구현됨 — policy_text를 --append-system-prompt 한 인자로 -- 앞에 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-24-01 | 규칙 부착 | WHEN 어떤 유형이든 작업을 시작한다 / THEN 인자에 `--append-system-prompt=`로 시작하는 팀 규칙 한 인자가 `--` 앞에 들어간다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/src/task_report.rs::spec_gets_add_dir_and_instruction_after_team_rules`<br>`crates/dduck-core/src/team_config.rs::bundle_is_consistent` |

### F-BRF-25

**worktree 격리** · 구현됨 — repo_status(git 미실행)·worktree_name 정리, Composer 비활성 사유 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-25-01 | 홈 폴더 저장소 | WHEN 작업 폴더의 git 루트가 홈 폴더다 / THEN worktree 선택이 꺼지고 "홈 폴더 저장소라 격리할 수 없습니다"가 보인다 | 부분 자동 | `crates/dduck-core/src/team.rs::repo_status_without_git`<br>`crates/dduck-core/src/team.rs::worktree_names_cannot_escape`<br>수동: git 루트가 홈 폴더인 하위 폴더를 작업 폴더로 고른다 → worktree 선택이 꺼지고 '홈 폴더 저장소라 격리할 수 없어요'가 보인다 |

### F-BRF-26

**민감 영역** · 구현됨 — sensitive면 plan/manual만, effort≥xhigh, 위험 페르소나 거부(백엔드 재확인) · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-26-01 | 민감 영역 권한 상한 | WHEN 민감 영역을 켠 채 웹뷰가 acceptEdits로 시작을 요청한다 / THEN 작업을 시작하지 않고 오류를 표시한다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>수동: 작업 시키기 창에서 시작을 막는 조건을 만든다 → 하단 문구와 시작 버튼 상태를 본다 → 막힌 이유가 보이고 시작되지 않는다 |

### F-BRF-27

**새 눈 후속 검증** · 구현됨 — followUpDraft(QA·리뷰 맡기기, 실제 폴더·정리된 이름), 다시 맡기기 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-27-01 | 리뷰 맡기기 | WHEN 사용자가 완료된 개발 작업 "fix-flaky-test"에서 "리뷰 맡기기"를 누른다 / THEN 작업 창이 "검토" 유형, 같은 폴더, 목표 "'fix-flaky-test' 작업의 변경을 리뷰"로 채워져 열린다 | 자동 | `src/lib/brief.test.ts::follow-ups open a fresh task in the dev session's own folder with a cleaned name` |

### F-BRF-28

**막힌 것부터** · 구현됨 — AgentsPage 머리말 '막힌 작업 N개 — 먼저 풀기' → 작업 페이지 확인 필요 칸 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-28-01 | 막힌 작업 바로가기 | WHEN 검토 필요 작업이 2개일 때 사용자가 "막힌 작업 2개 — 먼저 풀기"를 누른다 / THEN 작업 페이지가 열리고 검토 필요 열로 스크롤된다 | 수동 | 수동: 확인 필요 작업 2개를 만든다 → 에이전트 페이지에서 '막힌 작업 2개 — 먼저 풀기' → 작업 페이지가 열리고 확인 필요 칸으로 스크롤된다 |

### F-BRF-29

**검토 용량** · 구현됨 — reviewLoad·overCapacity·blockedCount, review_capacity 설정(제한 없음 포함) · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-29-01 | 용량 초과 | WHEN 용량이 3이고 진행 중인 개발 작업이 3개일 때 새 개발 작업을 시작한다 / THEN "검토 용량을 넘깁니다" 경고와 "그래도 시작" 버튼이 보이고, 누르기 전에는 시작하지 않는다 | 부분 자동 | `src/lib/brief.test.ts::review capacity: a number warns at the limit, 제한 없음 never warns`<br>`src/lib/brief.test.ts::counts unreviewed code tasks, including finished ones`<br>수동: 용량 3에서 리뷰 안 한 개발 작업 3개 → 새 개발 작업 시작 → '검토 용량을 넘깁니다' 경고와 '그래도 시작'이 보이고 누르기 전엔 시작 안 됨 |
| TC-BRF-29-02 | 검토함 표시 | WHEN 완료된 개발 작업 3개가 있어 용량 3/3일 때 하나에 "검토함"을 누른다 / THEN 검토 용량이 2/3이 되고 새 개발 작업이 경고 없이 시작된다 | 부분 자동 | `src/lib/brief.test.ts::counts unreviewed code tasks, including finished ones`<br>수동: 용량 3/3에서 끝난 개발 작업 하나를 '리뷰했어요' 표시 → 머리말 2/3, 새 개발 작업이 경고 없이 시작 |
| TC-BRF-29-03 | 막힘 보고 | WHEN 작업이 "막힘:"으로 보고하고 멈춰 CLI가 그 세션을 blocked로 표시한다 / THEN 작업 보드의 검토 필요 열에 나타나고 막힌 작업 수에 포함된다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::status_normalization`<br>`src/lib/brief.test.ts::counts unreviewed code tasks, including finished ones`<br>수동: 작업이 '막힘:'으로 멈춰 CLI가 blocked로 표시 → 작업 보드 확인 필요 칸에 나타나고 막힌 작업 수에 포함된다 |

### F-BRF-30

**휴대폰 알림** · 구현됨 — 설정 켜기 네이티브 확인, --remote-control=dduck-…, 위험·민감·배포·보안 거부 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-30-01 | 위험한 작업의 원격 조작 (1) | WHEN 휴대폰 알림이 켜진 상태에서 민감 영역 작업을 시작한다 / THEN 그 작업은 원격 조작 없이 시작된다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>수동: 휴대폰 알림을 켠 상태에서 '돈·인증·데이터' 민감 영역을 켜고 시작 → 휴대폰 알림 체크가 꺼져 원격 조작 없이 시작된다(백엔드는 요청 시 거부) |

### F-BRF-31

**작업 알림** · 구현됨 — 원격 작업에만 PushNotification 규칙, 끝남·실패 macOS 알림(finished_body) · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-31-01 | 자리를 비운 사이 작업 완료 | WHEN 휴대폰 알림을 켠 작업이 사용자가 자리를 비운 동안 끝난다 / THEN 작업이 Claude 앱으로 "한 일" 한 줄 알림을 보내고, D.Duck은 macOS 알림으로 완료를 알린다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/src/task_lifecycle.rs::tooltip_and_notification_copy`<br>수동: 휴대폰 알림을 켠 작업을 시작하고 자리를 비운다 → 휴대폰 Claude 앱에 한 줄 알림, Mac에 끝남 알림이 온다 |
| TC-BRF-31-02 | 위험한 작업의 원격 조작 | WHEN 웹뷰가 배포 유형 작업에 휴대폰 알림을 켜서 시작을 요청한다 / THEN 백엔드가 거부하고 오류를 표시한다 | 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args` |
| TC-BRF-31-03 | 처음 켜기 | WHEN 사용자가 설정에서 휴대폰 알림을 켠다 / THEN 네이티브 확인 창이 원격 조작 범위를 설명하고, 취소하면 꺼진 상태로 남는다 | 수동 | 수동: 설정에서 '휴대폰으로 알림' 켜기 → 원격 조작 범위를 설명하는 네이티브 창 → 취소하면 꺼진 채로 남는다 |

### F-BRF-32

**작업 지시 기록** · 구현됨 — task_meta 지시 기록(90일), 지난 작업 목록·지우기+scrub, store_task_prompts 설정 · spec: `openspec/specs/tech-lead-workflow/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-BRF-32-01 | 끝난 작업의 지시 보기 | WHEN 이틀 전에 D.Duck으로 시작한 작업이 `claude agents` 목록에서 사라졌다 / THEN 작업 페이지의 "지난 작업"에서 그 작업의 지시, 유형, 폴더, 시작 시각을 볼 수 있다 | 부분 자동 | `crates/dduck-core/src/task_meta.rs::history_records_prunes_and_scrubs`<br>`crates/dduck-core/src/task_meta.rs::tags_prefer_own_record_then_transcript`<br>수동: D.Duck으로 시작한 작업이 claude agents 목록에서 사라진 뒤 → 작업 페이지 '지난 작업'에서 지시·유형·폴더·시작 시각이 보인다 |
| TC-BRF-32-02 | 지시 지우기 | WHEN 사용자가 지난 작업 하나를 지운다 / THEN 목록에서 사라지고 그 지시문이 DB·WAL 파일 바이트에 남지 않는다 | 자동 | `crates/dduck-core/src/task_meta.rs::history_records_prunes_and_scrubs` |
| TC-BRF-32-03 | 저장 끄기 | WHEN 설정에서 "작업 지시 저장"을 끄고 새 작업을 시작한다 / THEN 그 작업의 지시는 저장되지 않고 카드에는 transcript에서 읽은 처음 지시가 보인다 | 수동 | 수동: 설정에서 '작업 지시 저장'을 끄고 새 작업 시작 → 지난 작업에 지시가 저장되지 않고 카드에는 transcript의 처음 지시가 보인다 |

## RPT · 보고서·리뷰

### F-RPT-01

**완료 기준마다 근거 항목** · 구현됨 — task_report v2 evidence[] 스키마와 ReportEvidence 근거 칩 구현(tasks 2.1·4.3 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-01-01 | 근거가 있는 기준 | WHEN 기준 "목록 화면이 다크 모드에서 읽힌다"가 충족이고 근거로 스크린샷 1개와 vitest JUnit 1개가 있다 / THEN 보고서 기준 줄에 ✓와 근거 칩 2개("스크린샷 list-dark.png", "테스트 vitest.xml")가 보인다 | 자동 | `src/components/ReportEvidence.test.tsx::draws chips per criterion, loads a preview only when pressed, and marks missing evidence`<br>`src/components/ReportEvidence.test.tsx::downgrades unbacked ✓ marks and dashes a kind the role does not accept`<br>`crates/dduck-core/src/task_report.rs::version_two_evidence_and_version_one_compat` |

### F-RPT-02

**근거 파일의 위치** · 구현됨 — evidence::valid_name·Item::valid가 ../·절대·하위 폴더 거부, parse가 Invalid 반환 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-02-01 | 폴더 밖 경로 | WHEN 근거 경로가 `../other/shot.png`다 / THEN 그 보고서는 형식 오류로 보인다 | 자동 | `crates/dduck-core/src/task_report.rs::version_two_evidence_and_version_one_compat`<br>`crates/dduck-core/src/evidence.rs::names_and_items_follow_the_shape`<br>`src/components/TaskReport.test.tsx::shows a format error without any report content` |

### F-RPT-03

**근거 파일의 안전 읽기** · 구현됨 — evidence.rs 링크·하드링크·FIFO 거부, PNG/JPEG 구조 걷기, 크기·항목 상한 구현(2.2 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-03-01 | 링크로 바꿔치기 | WHEN `evidence/shot.png`가 `~/.ssh/id_rsa`를 가리키는 링크다 / THEN 그 근거는 "읽지 않았어요(안전하지 않은 파일)"로 보이고 파일 내용은 읽히지 않는다 | 자동 | `crates/dduck-core/src/evidence.rs::links_and_special_files_are_refused`<br>`src/components/ReportEvidence.test.tsx::draws chips per criterion, loads a preview only when pressed, and marks missing evidence` |
| TC-RPT-03-02 | 확장자만 png | WHEN `evidence/x.png`의 실제 내용이 HTML이다 / THEN 그 근거는 "형식이 맞지 않아요"로 보이고 유효한 근거로 세지 않는다 | 자동 | `crates/dduck-core/src/evidence.rs::items_are_checked_in_place`<br>`crates/dduck-core/src/evidence.rs::images_are_walked_not_trusted`<br>`src/lib/evidence.test.ts::못 했어요 needs no evidence, and an invalid item does not count` |
| TC-RPT-03-03 | 잘린 JPEG | WHEN JPEG의 세그먼트 길이가 파일 끝을 넘는다 / THEN 그 근거는 "형식이 맞지 않아요"다 | 자동 | `crates/dduck-core/src/evidence.rs::images_are_walked_not_trusted` |

### F-RPT-04

**근거 없음 판정** · 구현됨 — report.ts 근거 없음·테스트 실패 판정과 카드 집계 구현(4.2 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-04-01 | 개발 작업의 근거 없음 | WHEN 끝난 개발 작업의 기준 3개가 모두 충족인데 그중 1개에 근거가 없다 / THEN 판정 줄은 주의 색 "근거 없음 1 → 확인 필요", 카드 기준 줄은 "충족 2 · 근거 없음 1"이다 | 자동 | `src/lib/evidence.test.ts::a dev task with one criterion without evidence: warn, not '합의대로 마쳤어요'; the card counts it apart` |
| TC-RPT-04-02 | 테스트 실패와 충족이 어긋남 | WHEN 충족으로 적힌 기준의 JUnit 근거에 실패가 2개 있다 / THEN 먼저 확인할 것에 "테스트 실패 2" 칩이 그 기준을 가리킨다 | 자동 | `src/lib/evidence.test.ts::test failures under a met criterion become a chip pointing at it`<br>`crates/dduck-core/src/evidence.rs::junit_counts_and_failure_names` |

### F-RPT-05

**리뷰·QA의 엄격한 판정** · 구현됨 — isEvidenceStrictType·validKinds로 리뷰=file, QA=file 제외, 엄격 빨강 판정 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-05-01 | QA 작업의 파일 근거 | WHEN 끝난 QA 작업의 기준 2개가 충족이고 그중 1개는 파일 근거만 있다 / THEN 판정 줄은 빨간 "근거 없음 1 → 확인 필요"로 시작한다 | 자동 | `src/lib/evidence.test.ts::QA: a file-only criterion has no evidence (danger, right after 못 했어요) and no test report is danger` |
| TC-RPT-05-02 | 리뷰 작업의 파일 근거 | WHEN 끝난 리뷰 작업의 기준 2개가 충족이고 둘 다 파일:줄 근거가 있다 / THEN 근거 없음이 없고 각 기준에 "파일 src/a.ts:10–42" 칩이 있다 | 자동 | `src/lib/evidence.test.ts::review: file:line evidence counts, nothing missing`<br>`src/components/ReportEvidence.test.tsx::review file evidence shows the path and line range only` |
| TC-RPT-05-03 | QA 테스트 리포트 없음 | WHEN 끝난 QA 작업 보고서의 모든 근거가 스크린샷이다 / THEN 판정 줄에 빨간 "테스트 리포트 없음"이 있고 "합의대로 마쳤어요"가 아니다 | 자동 | `src/lib/evidence.test.ts::QA: a file-only criterion has no evidence (danger, right after 못 했어요) and no test report is danger`<br>`src/components/ReviewEvidence.test.tsx::a card shows 근거 N and 근거 없음 N, red for QA and amber for other types` |

### F-RPT-06

**지울 수 없는 근거 규칙** · 구현됨 — team_config 엄격 유형 [필수] 줄·TYPE_LOCKED 구현(3.1 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-06-01 | 역할 지시를 바꿔도 | WHEN 사용자가 QA 유형의 역할 지시를 통째로 바꾸고 QA 작업을 시작한다 / THEN 시스템 지시에 "[필수]" 근거 규칙 줄이 그대로 있다 | 자동 | `crates/dduck-core/src/team_config.rs::review_and_qa_get_the_locked_evidence_rule`<br>`crates/dduck-core/src/team_config.rs::strict_types_and_my_agents` |

### F-RPT-07

**보고서 설정과 리뷰·QA** · 구현됨 — Composer 보고서 줄·want_report(리뷰·QA만 설정 꺼짐 무시) 구현(4.4 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-07-01 | 보고서 설정 꺼짐 | WHEN 전역 작업 보고서 설정이 꺼져 있고 사용자가 Composer에서 QA 작업을 고른다 / THEN "근거를 남기려면 보고서가 필요해요"와 켜진 체크가 보이고, 그대로 시작하면 그 작업에만 보고서가 붙는다 | 자동 | `src/components/ReviewEvidence.test.tsx::QA with the report setting off: the report line asks for a report and stays checked`<br>`crates/dduck-core/src/task_report.rs::report_is_wanted_only_for_local_tasks_with_the_setting_on` |

### F-RPT-08

**보고서 폴더에만 쓰는 읽기 전용 역할** · 보류 — report_guard 구현·테스트 있으나 MEASURED=false로 꺼짐, 스파이크 1.1·태스크 6.1 미완 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-08-01 | 리뷰어가 저장소를 고치려 함 | WHEN 보고서가 붙은 리뷰 작업(매번 확인 모드)이 저장소의 `src/a.ts`를 쓰려 한다 / THEN 쓰기가 막히고, 같은 작업이 보고서 폴더의 `report.json`은 확인 창 없이 쓴다 | 부분 자동 | `crates/dduck-core/src/report_guard.rs::writes_only_inside_the_report_folder`<br>`crates/dduck-core/src/report_guard.rs::grant_adds_write_only_when_measured_and_hooks_work`<br>수동: MEASURED 켠 빌드에서 보고서 붙은 리뷰 작업(매번 확인) 시작 → src/a.ts 쓰기 시도·report.json 쓰기 → 저장소 쓰기 거부, report.json은 확인 창 없이 생성 |
| TC-RPT-08-02 | git 출력 옵션 | WHEN 보고서가 붙은 리뷰 작업이 `git diff --output=/tmp/x`를 실행하려 한다 / THEN 그 명령이 막힌다 | 부분 자동 | `crates/dduck-core/src/report_guard.rs::shell_output_forms_are_denied_for_roles_without_bash`<br>수동: MEASURED 켠 빌드에서 보고서 붙은 리뷰 작업 → git diff --output=/tmp/x 실행 유도 → 명령이 훅에서 거부되고 /tmp/x 생기지 않음 |

### F-RPT-09

**근거를 남기라는 보고서 지시** · 구현됨 — task_report::instruction v2(evidence/·Playwright·JUnit·2,000자) 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-09-01 | 지시 내용 | WHEN 보고서가 붙은 개발 작업을 시작한다 / THEN 시스템 지시에 `evidence/` 경로, Playwright 안내, JUnit 명령 예시, "근거 없이 met yes를 쓰지 않는다"가 있고 `screencapture`는 없으며 지시는 2,000자 이하다 | 자동 | `crates/dduck-core/src/task_report.rs::instruction_v2_rules_and_length`<br>`crates/dduck-core/src/task_report.rs::spec_gets_add_dir_and_instruction_after_team_rules` |

### F-RPT-10

**이전 형식 보고서** · 구현됨 — v1 보고서 legacy 플래그와 "근거 형식 이전 보고서" 칩 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-10-01 | 업데이트 전 보고서 | WHEN 업데이트 전에 끝난 작업의 보고서를 연다 / THEN 이전과 같은 판정 줄과 "근거 형식 이전 보고서" 칩이 보이고 근거 없음 표시는 없다 | 자동 | `src/components/ReportEvidence.test.tsx::shows the legacy chip for a version 1 report and no missing-evidence marks`<br>`src/lib/evidence.test.ts::a version 1 report keeps the old verdict and counts no missing evidence`<br>`crates/dduck-core/src/task_report.rs::version_two_evidence_and_version_one_compat` |

### F-RPT-11

**근거 칩과 미리보기** · 구현됨 — ReportEvidence 칩·인라인 미리보기·라이트박스, evidence::preview 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-11-01 | 스크린샷 보기 | WHEN 사용자가 "스크린샷 list-dark.png" 칩을 누른다 / THEN 그 기준 아래에 그림이 크게 열리고, Esc를 누르면 닫히며 포커스가 칩으로 돌아간다 | 자동 | `src/components/ReportEvidence.test.tsx::opens a screenshot large; Escape closes it and focus goes back to the chip`<br>`src/components/ReportEvidence.test.tsx::the lightbox names the image and its size and says Esc closes it` |
| TC-RPT-11-02 | 테스트 요약 | WHEN 사용자가 실패 2개가 있는 JUnit 칩을 누른다 / THEN "통과 48 · 실패 2 · 건너뜀 1" 표와 실패한 테스트 이름 2줄이 보인다 | 자동 | `src/components/ReportEvidence.test.tsx::draws chips per criterion, loads a preview only when pressed, and marks missing evidence`<br>`crates/dduck-core/src/evidence.rs::junit_counts_and_failure_names`<br>`crates/dduck-core/src/evidence.rs::previews` |

### F-RPT-12

**테스트 리포트의 안전한 해석** · 구현됨 — quick-xml 파서가 DocType·PI·알 수 없는 엔티티·깊이 초과 거부(2.3 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-12-01 | 엔티티 폭탄 | WHEN JUnit 파일에 문서 형식 선언과 엔티티 정의가 있다 / THEN 그 근거는 "형식이 맞지 않아요"이고 엔티티는 펼쳐지지 않는다 | 자동 | `crates/dduck-core/src/evidence.rs::hostile_xml_is_refused` |

### F-RPT-13

**리뷰 카드의 근거 수** · 구현됨 — review_inbox 기준별 evidenceCount·verified, 카드 근거 N/근거 없음 N 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-13-01 | 카드 | WHEN 리뷰 전 QA 작업의 보고서에 유효 근거 5개와 근거 없음 기준 1개가 있다 / THEN 카드에 "근거 5"와 빨간 "근거 없음 1"이 보인다 | 자동 | `src/components/ReviewEvidence.test.tsx::a card shows 근거 N and 근거 없음 N, red for QA and amber for other types`<br>`crates/dduck-core/src/review_inbox.rs::cards_count_evidence_without_names_and_show_the_first_attachment` |

### F-RPT-14

**근거 파일의 보관과 프라이버시** · 구현됨 — task_meta::delete_record_and_report가 보고서 폴더 즉시 삭제(2.5 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-14-01 | 기록 삭제 | WHEN 사용자가 지난 작업에서 근거가 있는 작업 기록을 지운다 / THEN 그 보고서 폴더가 `evidence/`째 곧바로 지워진다 | 자동 | `crates/dduck-core/src/task_meta.rs::deleting_a_record_removes_its_report_folder_now` |

### F-RPT-15

**첨부 보고서** · 구현됨 — attachment.rs attachments/ 안전 읽기·상한·엄격 UTF-8, 보고서 첨부 탭 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-15-01 | 리서치 첨부 | WHEN 리서치 작업 보고서에 `attachments/compare.html`("DB 후보 비교")이 적혀 있다 / THEN 보고서의 "첨부" 칸에 "DB 후보 비교" 탭이 있고 누르면 그 문서가 보인다 | 자동 | `src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab`<br>`crates/dduck-core/src/attachment.rs::attachments_are_read_safely`<br>`crates/dduck-core/src/task_report.rs::version_two_evidence_and_version_one_compat` |
| TC-RPT-15-02 | 잘못된 인코딩 | WHEN 첨부 파일에 UTF-8이 아닌 바이트가 하나 있다 / THEN 그 탭은 흐리게 "형식이 맞지 않아요"로 보이고 내용은 그려지지 않는다 | 자동 | `crates/dduck-core/src/attachment.rs::attachments_are_read_safely`<br>`src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab` |

### F-RPT-16

**HTML 첨부의 안전한 그리기** · 구현됨 — ammonia 정리·filter_css·그림 예산·CSP srcdoc, sandbox iframe 구현(4A.2 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-16-01 | 스크립트가 든 첨부 | WHEN HTML 첨부에 `<script>`, `<img src=x onerror=…>`, `<meta http-equiv="refresh">`, 원격 그림, `<style>@import url(https://…)</style>`가 있다 / THEN 첨부는 글·표·색만 보이고, 스크립트가 실행되지 않으며, 네트워크 요청과 페이지 이동이 없다 | 부분 자동 | `crates/dduck-core/src/attachment.rs::scripts_frames_forms_and_events_are_removed`<br>`crates/dduck-core/src/attachment.rs::css_loses_every_remote_or_font_rule`<br>`crates/dduck-core/src/attachment.rs::images_are_checked_and_budgeted`<br>`src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab`<br>수동: 앱(실제 WebView)에서 script·onerror·meta refresh·원격 img·@import가 든 HTML 첨부 열기 → 웹 인스펙터 Network에 요청 0, 알림·이동 없음, 글·표만 보임 |
| TC-RPT-16-02 | 출처 링크 | WHEN 리서치 첨부에 `<a href="https://docs.example.com/x">공식 문서</a>`가 있다 / THEN 문서에 "공식 문서 (docs.example.com)"이 글로 보이고, 눌러도 이동하지 않는다 | 자동 | `crates/dduck-core/src/attachment.rs::links_become_text_with_the_host` |
| TC-RPT-16-03 | 같은 그림 반복 | WHEN 첨부가 3MB 그림 하나를 두 번 참조한다 / THEN 두 번째 그림은 빠지고 첫 번째만 보인다 | 자동 | `crates/dduck-core/src/attachment.rs::images_are_checked_and_budgeted` |
| TC-RPT-16-04 | 스크립트만 있는 문서 | WHEN HTML 첨부의 내용이 `<script>`뿐이다 / THEN 그 탭은 "보여 드릴 내용이 없어요"를 보인다 | 자동 | `crates/dduck-core/src/attachment.rs::script_only_documents_are_empty`<br>`crates/dduck-core/src/attachment.rs::attachments_are_read_safely`<br>`src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab` |

### F-RPT-17

**Markdown 첨부** · 구현됨 — SafeMarkdown 64KB·파이프 표·같은 폴더 그림 구현(4A.3 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-17-01 | 비교 표 | WHEN 설계 첨부 Markdown에 3열 5행 파이프 표가 있다 / THEN 첨부 칸에 3열 5행 표가 보인다 | 자동 | `src/lib/safeMarkdown.test.ts::draws pipe tables with alignment and inline cells`<br>`src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab` |
| TC-RPT-17-02 | 큰 Markdown | WHEN Markdown 첨부가 50KB다 / THEN 끝까지 그려지고 잘리지 않는다 | 자동 | `src/lib/safeMarkdown.test.ts::keeps a 50 KB document to the end and numbers image lines`<br>`crates/dduck-core/src/attachment.rs::attachments_are_read_safely` |

### F-RPT-18

**첨부 보기 머리와 닫기** · 구현됨 — 첨부 머리·크게 보기·닫기 UI 구현(4A.5 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-18-01 | 문서 안에 포커스 | WHEN 사용자가 크게 보기 안 문서를 눌러 포커스가 문서 안에 있고 Esc를 누른다 / THEN 문서는 그대로이고, 머리의 "닫기"가 보이며 누르면 닫히고 포커스가 "크게 보기" 버튼으로 돌아간다 | 부분 자동 | `src/components/ReportEvidence.test.tsx::attachments: tabs, the app header outside a sandbox="" frame, close returns focus to the tab`<br>수동: 보고서 첨부 탭 → 크게 보기 → 문서 안을 클릭하고 Esc → 문서 유지·머리 "닫기" 보임, 닫기 클릭 → 닫히고 포커스가 "크게 보기" 버튼, Shift+Tab으로 머리 버튼 복귀 |

### F-RPT-19

**유형별 보고서 형태** · 구현됨 — team_config::report_shape와 report.ts "첨부 보고서 N개" 판정 구현(4A.4 체크) · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-19-01 | 첨부만 낸 설계 작업 | WHEN 완료 기준 없는 브리프로 맡긴 설계 작업이 기준 없이 첨부 2개를 냈다 / THEN 판정 줄은 "첨부 보고서 2개"이고 근거 없음 표시는 없다 | 자동 | `src/lib/evidence.test.ts::attachments without criteria: free types say how many, criteria types ask for a check`<br>`src/lib/evidence.test.ts::isFreeShape matches team_config.rs report_shape` |
| TC-RPT-19-02 | 첨부만 낸 디자인 작업 | WHEN 디자인 작업이 기준 없이 첨부 1개만 냈다 / THEN 판정 줄은 주의 색 "기준 대조가 없어요 · 첨부 보고서 1개 → 확인이 필요해요"다 | 자동 | `src/lib/evidence.test.ts::attachments without criteria: free types say how many, criteria types ask for a check` |

### F-RPT-20

**첨부를 남기라는 지시와 리뷰 카드** · 구현됨 — 지시 첨부 줄·attachment::first_line(16KB·캐시)·리뷰 카드 첨부 줄 구현 · spec: `openspec/specs/report-evidence/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-20-01 | 첨부 카드 | WHEN 기준 없이 첨부 1개("DB 후보 비교", 첫 문단 "세 후보를 비용·운영으로 비교했어요")를 낸 리서치 작업이 리뷰 전이다 / THEN 카드에 "첨부 보고서 1개", "DB 후보 비교", "세 후보를 비용·운영으로 비교했어요"가 한 줄씩 보인다 | 부분 자동 | `crates/dduck-core/src/review_inbox.rs::cards_count_evidence_without_names_and_show_the_first_attachment`<br>`crates/dduck-core/src/attachment.rs::first_line_reads_16_kb_once_per_signature`<br>수동: 기준 없이 첨부 1개 낸 리서치 작업(mock ?evidence=1 e2e2e2e2) → 리뷰 탭 카드 확인 → "첨부 보고서 1개"·제목·첫 문단이 한 줄씩 보임 |

### F-RPT-21

**사이드바 리뷰 탭과 배지** · 구현됨 — Sidebar "리뷰" NavItem(작업 다음)·badgeLabel, 0이면 배지 미표시 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-21-01 | 모두 리뷰함 | WHEN 리뷰 전 작업이 0개다 / THEN "리뷰" 항목에 배지가 없다 | 부분 자동 | `crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks`<br>`src/components/ReviewPage.test.tsx::before a good poll, cards say 실행 여부 확인 중, cannot be marked, and the badge hides`<br>수동: 리뷰 전 카드를 모두 리뷰했어요로 표시 → 사이드바 "리뷰" 항목 확인 → 숫자 배지가 없음 |

### F-RPT-22

**리뷰 전 작업의 정의** · 구현됨 — review_inbox pending_count·running_of와 settings review_since(최초 1회 기록) 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-22-01 | 배지 개수 | WHEN 기준 시각 이후 보고서가 붙은 끝난 작업 3개 중 1개가 리뷰했어요 표시되어 있고, 보고서가 붙은 작업 1개가 아직 실행 중이다 / THEN 사이드바 "리뷰" 배지는 2이고 접근 이름은 "리뷰 전 2개"다 | 자동 | `crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks`<br>`crates/dduck-core/src/review_inbox.rs::running_follows_the_report_screen_rule`<br>`src/components/ReviewPage.test.tsx::sits after 작업 with its own badge label, and groups 리뷰 전 cards by role in team order` |
| TC-RPT-22-02 | 처음 켰을 때 쏟아지지 않음 | WHEN 이 기능이 생기기 전에 보고서가 붙은 끝난 작업이 30개 있다 / THEN 업데이트 후 첫 실행에서 배지가 없고 리뷰 전 필터는 "모두 리뷰했어요"이며, 그 30개는 전체 필터에서 보인다 | 자동 | `crates/dduck-core/src/review_inbox.rs::badge_matches_the_list_and_a_first_run_starts_at_zero`<br>`crates/dduck-core/src/settings.rs::review_since_is_written_once`<br>`crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks` |

### F-RPT-23

**역할별로 묶인 결과 카드** · 구현됨 — ReviewPage 역할 묶음(groupByRole 팀 순서·DuckAvatar roleColor) 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-23-01 | 두 역할 | WHEN 개발 작업 2개와 QA 작업 1개에 보고서가 있다 / THEN 개발 묶음(카드 2개, 최신 먼저)과 QA 묶음(카드 1개)이 팀 설정 순서대로 보이고, 각 묶음 머리 오리는 보고서 화면과 같은 역할 색이다 | 부분 자동 | `src/lib/review.test.ts::follows the team type order, then deleted types, then no type`<br>`src/components/ReviewPage.test.tsx::sits after 작업 with its own badge label, and groups 리뷰 전 cards by role in team order`<br>`crates/dduck-core/src/review_inbox.rs::pages_hold_ten`<br>수동: 개발 2·QA 1 보고서 작업 준비 → 리뷰 탭 → 묶음 머리 오리 색이 보고서 화면 오리 색과 같은지 눈으로 비교 |

### F-RPT-24

**카드 내용** · 구현됨 — review.ts cardView(판정 줄·집계·첫 확인 칩·실행 상태) 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-24-01 | 보고서가 있는 개발 작업 | WHEN 끝난 개발 작업의 보고서에 기준 3개(충족 2, 일부 충족 1)와 묻지 않고 정한 결정 1개가 있다 / THEN 카드에 목표 한 줄, 주의 색 "일부 충족 1 · 묻지 않고 정함 1 → 확인이 필요해요", "충족 2 · 일부 충족 1", 첫 확인 칩 "일부 충족 …"이 보인다 | 자동 | `src/lib/review.test.ts::uses the report screen's verdict, counts and first caution`<br>`src/lib/review.test.ts::matches the full report screen for the same report` |
| TC-RPT-24-02 | 못 한 기준 | WHEN 기준 3개 중 충족 2, 못 함 1이고 다른 확인할 것이 없다 / THEN 카드에 빨간 "못 했어요 1 → 확인이 필요해요"와 "충족 2 · 못 했어요 1"이 보이고, 같은 말을 되풀이하는 확인 칩은 없다 | 자동 | `src/lib/review.test.ts::counts leave out zero parts and show 못 했어요; red only when a criterion was not met` |
| TC-RPT-24-03 | 보고서 없이 끝남 | WHEN 보고서가 붙어 시작했지만 report.json이 없이 끝난 작업이다 / THEN 카드는 "보고서가 아직 없어요"와 맡긴 내용을 보이고, 리뷰 전 필터와 배지에 포함된다 | 자동 | `src/lib/review.test.ts::an ended task without report.json still shows and can be marked`<br>`crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks` |
| TC-RPT-24-04 | 실행 중 | WHEN 보고서가 붙은 작업이 아직 일하는 중이다 / THEN 리뷰 전 필터와 배지에 들어가지 않고, 전체 필터에서 "일하는 중이에요"와 함께 보이며 리뷰했어요 버튼이 없다 | 자동 | `crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks`<br>`src/components/ReviewPage.test.tsx::전체 shows running, waiting, reviewed and older tasks; running ones have no mark button`<br>`src/lib/review.test.ts::running and waiting tasks show their state and cannot be marked` |
| TC-RPT-24-05 | 답을 기다리는 작업 | WHEN 보고서가 붙은 작업이 사용자 입력을 기다리고 있다 / THEN 리뷰 전 필터와 배지에 들어가지 않고, 전체 필터에서 "답을 기다려요"와 함께 보이며 리뷰했어요 버튼이 없다 | 자동 | `crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks`<br>`src/components/ReviewPage.test.tsx::전체 shows running, waiting, reviewed and older tasks; running ones have no mark button`<br>`src/lib/review.test.ts::running and waiting tasks show their state and cannot be marked` |
| TC-RPT-24-06 | 실행 여부를 아직 모름 | WHEN 앱을 켠 뒤 작업 목록을 아직 한 번도 제대로 읽지 못했다(또는 마지막 읽기가 실패했다) / THEN 리뷰했어요 표시가 없는 카드는 "실행 여부 확인 중"으로 보이고 리뷰했어요 버튼이 없으며, 앱은 표시 요청을 거절하고 배지를 숨긴다 | 자동 | `src/components/ReviewPage.test.tsx::before a good poll, cards say 실행 여부 확인 중, cannot be marked, and the badge hides`<br>`crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks` |
| TC-RPT-24-07 | QA 작업이 아직 실행 중 | WHEN 개발 작업은 끝났고 그 결과를 보는 QA 작업은 아직 실행 중이다 / THEN 개발 카드는 리뷰 전에 보이고, QA 카드는 QA 묶음에서 전체 필터에만 "일하는 중이에요"로 보인다 | 부분 자동 | `crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks`<br>수동: 개발 작업 완료 후 그 결과를 보는 QA 작업 실행 중 → 리뷰 탭 리뷰 전/전체 전환 → 개발 카드는 리뷰 전에, QA 카드는 전체에서만 "일하는 중이에요"로 QA 묶음에 보임 |

### F-RPT-25

**카드 색과 묶음 머리** · 구현됨 — 판정 tone(warn/danger)·묶음 머리 개수(groups total) 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-25-01 | 형식 오류 색 | WHEN 끝난 작업의 보고서 형식이 맞지 않는다 / THEN 카드 판정 줄이 빨강이 아닌 주의 색으로 "보고서 형식이 맞지 않아 보여 드릴 수 없어요"를 보인다 | 자동 | `src/components/ReviewPage.test.tsx::전체 shows running, waiting, reviewed and older tasks; running ones have no mark button`<br>`src/components/TaskReport.test.tsx::shows a format error without any report content` |

### F-RPT-26

**쪽 나누기와 카드 미리보기** · 구현됨 — ReviewPage 10개 쪽 넘김·기준 3줄 미리보기 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-26-01 | 두 쪽 | WHEN 리뷰 전 작업이 20개다 / THEN 1쪽에 카드 10개와 쪽 넘김이 보이고, "2쪽"을 누르면 다음 10개가 보이며, "전체"로 바꾸면 1쪽부터 다시 보인다 | 자동 | `src/components/ReviewPage.test.tsx::pages hold ten cards like the 아티팩트 tab; a new filter goes back to page 1`<br>`crates/dduck-core/src/review_inbox.rs::pages_hold_ten` |
| TC-RPT-26-02 | 기준 미리보기 | WHEN 끝난 작업의 보고서에 완료 기준이 4개 있다 / THEN 카드에 처음 3개 기준이 표시와 근거 한 줄과 함께 보이고 "+1개 더"가 붙으며, 카드를 누르면 전체 보고서가 열린다 | 자동 | `src/components/ReviewPage.test.tsx::cards preview up to three criteria with one line of evidence`<br>`src/lib/review.test.ts::shows up to three criteria rows with one line of evidence, then how many more`<br>`src/components/ReviewPage.test.tsx::opening a card shows the full report screen` |

### F-RPT-27

**리뷰했어요 모양과 차례 흐름** · 구현됨 — 리뷰했어요 토스트·되돌리기·"리뷰했어요 · 다음" 구현, 배지 neutral · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-27-01 | 되돌리기 알림 | WHEN 사용자가 리뷰 전 카드의 "리뷰했어요"를 누른다 / THEN 5초 동안 "리뷰했어요 · 되돌리기"가 보이고, 되돌리기를 누르면 카드가 다시 리뷰 전에 나온다 | 부분 자동 | `src/components/ReviewPage.test.tsx::marking shows a five-second undo that brings the card back`<br>수동: 리뷰 전 카드에서 리뷰했어요 클릭 → 알림이 약 5초 뒤 사라지는지 시계로 확인(테스트는 시간 미검증) |

### F-RPT-28

**필터와 프로젝트** · 구현됨 — ScopeBar 전역 시작·프로젝트 필터(구성요소 경계)·pendingOther 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-28-01 | 처음 열기 | WHEN 사용자가 리뷰 탭을 처음 연다 / THEN ScopeBar는 "모든 프로젝트" 쪽이 골라져 있고 모든 프로젝트의 카드가 보인다 | 자동 | `src/components/ReviewPage.test.tsx::sits after 작업 with its own badge label, and groups 리뷰 전 cards by role in team order` |
| TC-RPT-28-02 | 프로젝트 좁히기 | WHEN 리뷰 전 작업이 프로젝트 A에 2개, B에 1개 있고 사용자가 A를 고른다 / THEN A의 카드 2개와 "다른 프로젝트에 1개 더"가 보이고, 다른 탭의 프로젝트 선택과 사이드바 배지(3)는 그대로다 | 부분 자동 | `src/components/ReviewPage.test.tsx::picking a project narrows the list and says how many are elsewhere`<br>`crates/dduck-core/src/review_inbox.rs::project_scope_resolves_folders_by_component`<br>`crates/dduck-core/src/settings.rs::review_tab_picks_a_project_but_adds_nothing_to_catalogs`<br>수동: 리뷰 탭에서 프로젝트 A 선택 → 작업·아티팩트 탭으로 이동 → 그 탭의 프로젝트 선택이 바뀌지 않음 |
| TC-RPT-28-03 | 전체 필터 | WHEN 사용자가 "전체"를 고른다 / THEN 리뷰했어요 표시된 카드도 "리뷰했어요" 표시와 함께 보이고, 실행 중인 카드와 기준 시각 전 작업도 보인다 | 자동 | `src/components/ReviewPage.test.tsx::전체 shows running, waiting, reviewed and older tasks; running ones have no mark button`<br>`crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks` |

### F-RPT-29

**보고서 열기와 리뷰했어요** · 구현됨 — inbox_mark_reviewed·unmark_reviewed와 카드·다이얼로그 흐름 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-29-01 | 차례로 리뷰 | WHEN 리뷰 전 필터에서 사용자가 첫 카드의 "리뷰했어요"를 누른다 / THEN 그 카드가 목록에서 빠지고 다음 카드에 포커스가 가며 배지가 1 줄고, 작업 탭의 같은 작업에도 "리뷰했어요"가 보인다 | 부분 자동 | `src/components/ReviewPage.test.tsx::리뷰했어요 takes the card out, moves focus to the next one and lowers the badge; 되돌리기 brings it back`<br>수동: 리뷰 탭에서 첫 카드 리뷰했어요 → 작업 탭에서 같은 작업 열기 → "리뷰했어요" 표시가 보임 |
| TC-RPT-29-02 | 보고서에서 차례로 | WHEN 사용자가 첫 카드 보고서에서 "리뷰했어요 · 다음"을 누른다 / THEN 그 작업이 표시되고 다음 리뷰 전 카드의 보고서가 열린다 | 자동 | `src/components/ReviewPage.test.tsx::the report dialog walks the queue: 리뷰했어요 · 다음 marks and opens the next card`<br>`src/components/ReviewPage.test.tsx::리뷰했어요 · 다음 on the last card of a page goes on to the next page's card` |
| TC-RPT-29-03 | 실수로 표시 | WHEN 사용자가 전체 필터에서 리뷰했어요 카드의 "되돌리기"를 누른다 / THEN 표시가 지워지고 그 카드가 다시 리뷰 전에 포함되며 배지가 1 는다 | 자동 | `src/components/ReviewPage.test.tsx::리뷰했어요 takes the card out, moves focus to the next one and lowers the badge; 되돌리기 brings it back`<br>`crates/dduck-core/src/review_inbox.rs::pending_leaves_out_running_waiting_reviewed_and_old_tasks` |

### F-RPT-30

**문구 정리** · 구현됨 — 보드 칸 "확인 필요", "리뷰함" 문구 제거(2.3 체크) · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-30-01 | 보드 칸 이름 | WHEN 사용자가 작업 탭 보드를 본다 / THEN 답을 기다리거나 실패한 작업 칸의 이름이 "확인 필요"이고, 화면 어디에도 "리뷰함"이 없다 | 부분 자동 | `src/components/TaskLifecycle.test.tsx::has 대기 중 → 진행 중 → 확인 필요 → 리뷰 대기 → 정리 and no 완료 column`<br>`src/components/ReviewPage.test.tsx::sits after 작업 with its own badge label, and groups 리뷰 전 cards by role in team order`<br>수동: 작업 탭 보드·설정·리뷰 탭을 차례로 열기 → 어느 화면에도 "리뷰함" 문구가 없음(rg "리뷰함" src 0건도 확인) |

### F-RPT-31

**빈 화면** · 구현됨 — ReviewPage 빈 화면 3종(쉬는 오리) 구현 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-31-01 | 설정 꺼짐 | WHEN 작업 보고서 설정이 꺼져 있고 보고서가 붙은 기록이 없다 / THEN "보고서를 켜면 오리들이 결과를 모아 와요"와 "설정 열기" 버튼만 보이고(범위·필터 없음), 누르면 설정 탭이 열린다 | 자동 | `src/components/ReviewPage.test.tsx::no records with the report setting off: one line and a way to settings, no scope bar or filters` |

### F-RPT-32

**비용과 프라이버시** · 구현됨 — review_inbox 요약만(120자 상한, 경로·id 없음) 응답 · spec: `openspec/specs/review-inbox/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-32-01 | 목록에 전체 지시 없음 | WHEN 작업 지시가 120자를 넘고 고유한 문자열을 담고 있다 / THEN 목록 응답에는 그 문자열의 앞 120자 이내만 있고 뒷부분은 없다 | 자동 | `crates/dduck-core/src/review_inbox.rs::list_carries_a_summary_only` |
| TC-RPT-32-02 | 오류에도 경로 없음 | WHEN 보고서 폴더가 링크로 바뀌어 읽기가 거부된다 / THEN 카드는 "보고서 파일이 안전하지 않아 읽지 않았어요"를 보이고, 응답·로그 어디에도 경로나 id가 없다 | 부분 자동 | `src/lib/review.test.ts::a refused report reads as unsafe, without a path`<br>`crates/dduck-core/src/task_report.rs::folder_is_private_and_links_are_refused`<br>수동: 보고서 폴더를 심볼릭 링크로 바꾼 작업 → 리뷰 탭 열기 → 카드에 "보고서 파일이 안전하지 않아 읽지 않았어요", 앱 로그·응답에 경로·id 없음 |

### F-RPT-33

**재작업 요청 진입점** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-33-01 | 리뷰 카드에서 | WHEN 리뷰 전 카드가 끝난 개발 작업이고 보고서가 준비되어 있다 / THEN 카드 오른쪽에 "재작업 요청"(일반 버튼)과 "리뷰했어요"가 나란히 보인다 | 수동 | 수동: 끝난 개발 작업(보고서 준비) 리뷰 카드 → 카드 오른쪽 확인 → 일반 버튼 "재작업 요청"이 "리뷰했어요" 왼쪽에 보임 |
| TC-RPT-33-02 | 실행 중 | WHEN 작업이 아직 일하는 중이다 / THEN 카드와 보고서 화면 어디에도 "재작업 요청"이 없다 | 수동 | 수동: 실행 중인 보고서 작업 → 카드와 보고서 화면 확인 → "재작업 요청" 버튼 없음 |

### F-RPT-34

**고르기 위주의 재작업 시트** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-34-01 | 미리 채워진 시트 | WHEN 기준 3개 중 충족 1(근거 있음), 충족 1(근거 없음), 못 했어요 1인 보고서에서 "재작업 요청"을 누른다 / THEN 근거 없는 충족 기준과 못 했어요 기준이 체크되어 있고, 이유 칩 "근거 부족"·"기준 미충족"이 골라져 있으며, 아무것도 입력하지 않고 주 버튼을 누를 수 있다 | 수동 | 수동: 충족(근거)·충족(근거 없음)·못 했어요 기준 보고서 → 재작업 요청 → 근거 없는·못 한 기준 체크, "근거 부족"·"기준 미충족" 선택, 주 버튼 활성 |
| TC-RPT-34-02 | 기타만 고름 | WHEN 사용자가 다른 이유를 모두 끄고 "기타"만 고르고 한 줄을 비워 둔다 / THEN 주 버튼이 비활성이고 "기타를 고르면 한 줄을 적어 주세요"가 보인다 | 수동 | 수동: 재작업 시트에서 "기타"만 고르고 한 줄 비움 → 주 버튼 비활성, "기타를 고르면 한 줄을 적어 주세요" 보임 |

### F-RPT-35

**다시 하는 방법** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-35-01 | 새 작업으로 | WHEN 사용자가 "새 작업으로"를 고르고 "새 작업으로 열기"를 누른다 / THEN Composer가 원래 브리프로 열리고 재작업 요청 블록이 보이며, 사용자가 "시작"을 누르기 전에는 아무 작업도 시작되지 않는다 | 수동 | 수동: 재작업 시트 "새 작업으로" → "새 작업으로 열기" → Composer에 원래 브리프와 재작업 블록, 시작 누르기 전 작업 없음 |
| TC-RPT-35-02 | 이어서 할 수 없음 | WHEN 원래 작업의 세션을 알 수 없다 / THEN "같은 세션에서 이어서"는 비활성이고 "이전 대화를 찾을 수 없어요"가 보이며 "새 작업으로"가 골라져 있다 | 수동 | 수동: 세션 id 없는 원래 작업 → 재작업 시트 → "같은 세션에서 이어서" 비활성·"이전 대화를 찾을 수 없어요"·"새 작업으로" 선택 |
| TC-RPT-35-03 | 근거만 부족 | WHEN 이어서 하기가 가능하고 이유가 "근거 부족"뿐이다 / THEN "같은 세션에서 이어서"가 추천 배지와 함께 골라져 있고, 주 버튼은 "이어서 시작"이다 | 수동 | 수동: 이어서 가능 작업에서 이유 "근거 부족"만 → 시트 확인 → "같은 세션에서 이어서" 추천 배지와 선택, 주 버튼 "이어서 시작" |

### F-RPT-36

**같은 실행 조건** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-36-01 | 모델 유지 | WHEN 원래 작업이 특정 모델로 돌았고 사용자가 "같은 세션에서 이어서"로 재작업을 시작한다 / THEN 재작업은 같은 모델·유형·권한 모드로 시작된다 | 수동 | 수동: 특정 모델로 돈 작업 → 같은 세션에서 이어서 재작업 → 시작 인자(ps/로그)에 같은 --model·유형·권한 모드 |

### F-RPT-37

**앱이 만드는 피드백** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-37-01 | 조작된 기준 | WHEN 웹뷰가 원래 보고서·브리프에 없는 기준 문구를 담아 재작업을 시작하려 한다 / THEN 앱은 시작을 거부하고 아무 작업도 시작되지 않는다 | 수동 | 수동: 개발자 도구에서 원래 보고서에 없는 기준 문구로 재작업 시작 명령 호출 → 오류로 거부되고 작업 목록에 새 작업 없음 |

### F-RPT-38

**원래 작업과 이어진 새 작업** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-38-01 | 원래 카드 | WHEN 리뷰 전이던 작업 A에서 재작업을 새 작업 B로 시작한다 / THEN A는 리뷰 전 필터·배지·리뷰 대기 칸에서 빠지고, 작업 보드 정리 칸과 리뷰 탭 전체 필터에서 "재작업 → 새 작업"과 함께 보이며, B는 같은 역할 묶음에 "재작업" 칩과 함께 일하는 중으로 보인다 | 수동 | 수동: 리뷰 전 작업 A에서 새 작업 B로 재작업 시작 → 리뷰 탭·보드 확인 → A는 리뷰 전·배지에서 빠지고 정리 칸·전체에 "재작업 → 새 작업", B는 "재작업" 칩·일하는 중 |
| TC-RPT-38-02 | 취소 | WHEN 사용자가 재작업 시트에서 "새 작업으로 열기"를 누른 뒤 Composer를 닫는다 / THEN A는 여전히 리뷰 전이고 연결이 기록되지 않는다 | 수동 | 수동: 재작업 시트 "새 작업으로 열기" 후 Composer 닫기 → A가 여전히 리뷰 전이고 연결 기록 없음 |

### F-RPT-39

**새 보고서의 재작업 블록** · 미구현 — review-rework tasks.md 1.x~4.x 모두 미체크, 코드에 rework 모듈·ReworkSheet 없음(스펙만 존재) · spec: `openspec/changes/review-rework/specs/review-rework/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-39-01 | 재작업 보고서 | WHEN 사용자가 재작업 B의 보고서를 연다 / THEN 맡긴 내용 아래에 "재작업 요청 · 근거 부족 · 기준 미충족"과 다시 볼 기준 2줄, 한 줄, "새 작업으로"가 보이고, "원래 작업 보고서 보기"를 누르면 A의 보고서가 열린다 | 수동 | 수동: 재작업 B 보고서 열기 → 맡긴 내용 아래 "재작업 요청 · 근거 부족 · 기준 미충족"·기준 2줄·한 줄·"새 작업으로", "원래 작업 보고서 보기" 클릭 → A 보고서 열림 |

### F-RPT-40

**보고서 내용 선택** · 구현됨 — .selectable 내용 영역, 리뷰 카드 article+"보고서 열기" · spec: `openspec/changes/report-copy-save/specs/report-copy-save/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-40-01 | 완료 기준 복사 | WHEN 보고서에서 완료 기준 두 줄을 끌어 선택하고 ⌘C를 누른다 / THEN 클립보드에 두 줄의 글이 들어 있다 | 부분 자동 | `src/components/ReportCopySave.test.tsx::report sections are selectable; their headings, chips and marks are not`<br>수동: 보고서에서 완료 기준 두 줄을 끌어 ⌘C → 클립보드에 두 줄 |
| TC-RPT-40-02 | 리뷰 카드에서 끌기 | WHEN 리뷰 탭 카드의 목표 줄을 끌어 선택한다 / THEN 글이 선택되고 카드가 열리거나 리뷰했어요 표시가 바뀌지 않는다 | 자동 | `src/components/ReportCopySave.test.tsx::a card body click opens only without a selection and away from controls`<br>`src/components/ReviewI18n.test.tsx::a double or triple click selects words and does not open the report; a single click still does` |
| TC-RPT-40-03 | 키보드로 열기 | WHEN Tab으로 리뷰 카드의 "보고서 열기"에 가서 Enter를 누른다 / THEN 그 작업의 보고서가 열린다 | 부분 자동 | `src/components/ReviewPage.test.tsx::report-copy-save: the card is an article with selectable text; a body click opens, a selection does not`<br>수동: Tab으로 "보고서 열기"에 가서 Enter → 보고서 열림 |
| TC-RPT-40-04 | 조작 요소는 그대로 | WHEN 사이드바 항목이나 버튼 위에서 끈다 / THEN 글이 선택되지 않는다 | 자동 | `src/components/ReviewI18n.test.tsx::card chips are not selectable` |

### F-RPT-41

**첨부 저장** · 구현됨 — save_report_attachment(id, index): 원본 다시 읽기(열기 규칙), 저장 창, 고정 오류 코드 · spec: `openspec/changes/report-copy-save/specs/report-copy-save/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-41-01 | Markdown 저장 | WHEN 리뷰 탭에서 Markdown 첨부를 열고 "저장"을 눌러 다운로드 폴더에 저장한다 / THEN 다운로드 폴더에 원본과 같은 내용의 `.md` 파일이 생기고 "저장했어요"가 보인다 | 부분 자동 | `src/components/ReportCopySave.test.tsx::cancel shows nothing; fixed codes become one line`<br>`crates/dduck-core/src/attachment.rs::originals_are_read_with_the_open_rules`<br>수동: 리뷰 탭 Markdown 첨부 "저장" → 다운로드에 원본과 같은 .md, "저장했어요" |
| TC-RPT-41-02 | HTML 저장 | WHEN HTML 첨부에서 "저장"을 누르고 저장한다 / THEN 원본 `.html` 파일이 그대로 저장된다(화면 표시용으로 정리한 버전이 아님) | 부분 자동 | `src/components/ReportCopySave.test.tsx::HTML: 저장 sends the id and index only and says 저장했어요; no 복사`<br>수동: HTML 첨부 저장 → 원본 .html 그대로 |
| TC-RPT-41-03 | 원본이 바뀜 | WHEN 첨부를 연 뒤 저장하기 전에 원본 파일이 링크로 바뀌었거나 크기 상한을 넘었다 / THEN 저장하지 않고 이유를 보여 준다 | 자동 | `crates/dduck-core/src/attachment.rs::originals_behind_links_are_refused`<br>`src-tauri/src/report_save_cmds.rs::a_missing_original_has_its_own_code` |
| TC-RPT-41-04 | 취소 | WHEN 저장 창에서 취소를 누른다 / THEN 파일이 생기지 않고 메시지도 뜨지 않는다 | 자동 | `src/components/ReportCopySave.test.tsx::cancel shows nothing; fixed codes become one line` |

### F-RPT-42

**Markdown 원문 복사** · 구현됨 — 원문에서 bidi·제어 문자만 제거, 1.5초 "복사했어요" · spec: `openspec/changes/report-copy-save/specs/report-copy-save/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-RPT-42-01 | 원문 복사 | WHEN Markdown 첨부의 "복사"를 누른다 / THEN 클립보드에 `#`·`-` 같은 Markdown 기호가 그대로 있는 원문이 들어가고 버튼이 "복사했어요"로 바뀐다 | 자동 | `src/components/ReportCopySave.test.tsx::Markdown: 복사 puts the text with its marks, minus bidi and invisible characters, then 복사했어요 for 1.5 s` |

## AGT · 에이전트·팀

### F-AGT-01

**역할별 기본 장비** · 구현됨 — loadout.rs·team_config 유형 skills/mcpAllow, LoadoutSection UI, tasks 전부 체크 · spec: `openspec/specs/agent-loadout/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-01-01 | 디자인 역할 장비 | WHEN 사용자가 디자인 유형에 frontend-design 스킬과 figma MCP를 고르고 저장한다 / THEN 다음에 디자인 유형으로 시작한 작업의 에이전트 정의에 `skills: [frontend-design]`과 figma를 뺀 서버마다 `disallowedTools: mcp__<서버>__*`가 들어가고 `mcpServers`는 들어가지 않는다 | 자동 | `crates/dduck-core/src/claude_cli.rs::loadout_goes_into_the_inline_role`<br>`crates/dduck-core/src/loadout.rs::mcp_only_chosen_servers` |
| TC-AGT-01-02 | 플러그인 묶음 | WHEN 사용자가 장비에서 플러그인 하나를 고른다 / THEN 그 플러그인이 주는 스킬이 스킬 목록에 추가되고 MCP 쪽에는 "플러그인 MCP는 플러그인을 켜면 항상 켜져요" 한 줄이 보인다 | 부분 자동 | `src/components/LoadoutSection.test.tsx::adds a plugin bundle (skills only) and shows the preload size`<br>`src/lib/loadout.test.ts::bundles plugin skills and caps the list`<br>수동: 에이전트 설정 시트 열기 → 기본 장비에서 플러그인 묶음 담기 → 그 플러그인 스킬이 목록에 추가되고 MCP 쪽에 플러그인 MCP 안내 한 줄(현재 문구는 '목록 밖 서버(claude.ai 커넥터·플러그인 MCP)는 막지 못해요')이 보임 |

### F-AGT-02

**안전한 저장** · 구현됨 — agents_catalog with_loadout가 skills·disallowedTools 줄만 수정, 플러그인 잠금 문구 loadout.ts · spec: `openspec/specs/agent-loadout/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-02-01 | 플러그인 에이전트 | WHEN 사용자가 플러그인이 준 에이전트의 장비를 열었다 / THEN 읽기 전용으로 보이고 "플러그인 에이전트는 MCP 지정이 무시돼요 · 복사해서 내 것으로" 안내가 있다 | 부분 자동 | `src/lib/loadout.test.ts::labels and chips`<br>수동: 플러그인 에이전트(예: cloudflare:workers-reviewer) 장비 열기 → 읽기 전용이고 'MCP 지정이 무시돼요 · 복사해서 내 것으로' 안내가 보임 |

### F-AGT-03

**Agent 목록** · 구현됨 — agents_catalog discover(출처별·형식 오류·대표 선택), AgentsPage 기본 Claude 카드 · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-03-01 | 프로젝트 agent 표시 | WHEN 기본 작업 폴더에 `.claude/agents/design-critic.md`가 있다 / THEN "design-critic" 카드가 출처 "프로젝트 · dev_team"과 frontmatter 설명으로 보인다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::discovery_and_resolution`<br>수동: 기본 작업 폴더에 .claude/agents/design-critic.md 둠 → 에이전트 페이지 열기 → 'design-critic' 카드에 출처 '프로젝트 · dev_team'과 설명이 보임 |
| TC-AGT-03-02 | 깨진 정의 | WHEN `~/.claude/agents/x.md`에 frontmatter가 없다 / THEN "x" 카드가 "형식 오류" 표시와 함께 보이고 편집할 수 있다 | 자동 | `crates/dduck-core/src/agents_catalog.rs::discovery_and_resolution`<br>`crates/dduck-core/src/agents_catalog.rs::frontmatter_reader` |

### F-AGT-04

**첫 페이지** · 구현됨 — nav.ts PAGES 첫 항목 agents, 사이드바 맨 위 '에이전트' · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-04-01 | 처음 실행 | WHEN 사용자가 앱을 처음 연다 / THEN 에이전트 페이지가 보이고 사이드바의 "에이전트"가 선택되어 있다 | 부분 자동 | `src/components/NavIa.test.tsx::shows 작업 공간 and Claude 구성 in order, then usage and settings, and no project picker`<br>수동: localStorage의 dduck.page 비우고 앱 실행 → 에이전트 페이지가 열리고 사이드바 '에이전트'가 선택 상태 |

### F-AGT-05

**Duck 캐릭터와 상태 문장** · 구현됨 — DuckAvatar 5자세, index.css prefers-reduced-motion에서 애니메이션 정지 · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-05-01 | 움직임 줄이기 | WHEN macOS에서 동작 줄이기가 켜져 있다 / THEN Duck은 자세만 바뀌고 움직이지 않으며 상태 문장은 그대로 보인다 | 수동 | 수동: macOS 손쉬운 사용 '동작 줄이기' 켜기 → 일하는 agent가 있는 에이전트 페이지 열기 → Duck이 정지 자세이고 상태 문장은 그대로 보임 |

### F-AGT-06

**일하는 상태 판정** · 구현됨 — live_agents 판정(finished 없음+5분, 10분 끝남, 우선순위·건수), statusLine · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-06-01 | 서브에이전트가 일하는 중 | WHEN 실행 중인 세션에 `agentType: "design-critic"`이고 `finished` 키가 없으며 1분 전에 기록된 서브에이전트가 있다 / THEN design-critic 카드의 Duck이 자판을 두드리고, 아래에 "일하는 중 · <작업 설명>"이 보인다 | 자동 | `crates/dduck-core/src/live_agents.rs::subagent_states_follow_measured_format`<br>`src/lib/agents.test.ts::writes the line under the duck` |
| TC-AGT-06-02 | 페르소나 작업이 답을 기다림 | WHEN D.Duck이 `--agent qa-verifier`로 시작한 작업이 답 기다림 상태다 / THEN qa-verifier 카드의 Duck이 손을 들고 "답을 기다려요 · <작업 이름>"이 보이며, 카드를 누르면 작업 상세로 이동한다 | 부분 자동 | `crates/dduck-core/src/live_agents.rs::persona_sessions_and_priority`<br>`src/lib/agents.test.ts::writes the line under the duck`<br>수동: --agent qa-verifier 작업을 답 기다림 상태로 둠 → qa-verifier 카드가 손 든 자세·'답을 기다려요 · 작업 이름' 표시 → 카드 클릭 시 작업 상세로 이동 |
| TC-AGT-06-03 | 중단된 흔적 | WHEN `finished` 키가 없는 서브에이전트 기록이 30분째 바뀌지 않았다 / THEN 그 agent 카드는 일하는 중으로 보이지 않는다 | 자동 | `crates/dduck-core/src/live_agents.rs::subagent_states_follow_measured_format` |
| TC-AGT-06-04 | 아무 일도 없음 | WHEN 어떤 세션도 그 agent를 쓰지 않는다 / THEN Duck이 졸고 "쉬는 중"이 보인다 | 자동 | `src/lib/agents.test.ts::writes the line under the duck` |

### F-AGT-07

**정의 저장 확인** · 구현됨 — agents_catalog plan_save가 변경 줄 수·frontmatter 미리보기·권한 경고 판정, commit_save 재확인 · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-07-01 | 들여쓴 hooks 변경 | WHEN 저장 내용에서 `hooks:` 아래 들여쓴 줄만 바뀌었다 / THEN 확인 창에 권한 경고와 새 frontmatter가 보인다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::risk_and_warning_rules`<br>수동: hooks: 블록이 있는 agent 파일에서 들여쓴 하위 줄만 고쳐 저장 → 네이티브 확인 창에 권한 경고와 새 frontmatter가 보임 |

### F-AGT-08

**정의 파일 편집** · 구현됨 — plan_save/commit_save(ID 지정·크기·충돌·임시파일 rename), 정의 편집기 · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-08-01 | 설명 고치기 | WHEN 사용자가 design-critic의 설명을 고치고 저장을 눌러 확인 창에서 승인한다 / THEN 파일이 바뀌고 카드의 설명이 새 값으로 보인다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::guarded_save`<br>수동: design-critic 편집기에서 description 수정 → 저장·확인 창 승인 → 파일이 바뀌고 카드 설명이 새 값으로 보임 |
| TC-AGT-08-02 | 권한 필드 변경 경고 | WHEN 사용자가 `tools:` 줄을 바꿔 저장한다 / THEN 확인 창에 권한 변경 경고가 보이고, 취소하면 파일이 바뀌지 않는다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::risk_and_warning_rules`<br>`crates/dduck-core/src/agents_catalog.rs::guarded_save`<br>수동: 편집기에서 tools: 줄 변경 후 저장 → 확인 창에 권한 변경 경고 확인 → 취소 → 파일 내용 불변 |
| TC-AGT-08-03 | 밖에서 바뀐 파일 | WHEN 편집기를 연 뒤 다른 편집기에서 같은 파일을 고쳤다 / THEN 저장하지 않고 "파일이 밖에서 바뀌었습니다. 다시 불러오세요"를 보인다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::guarded_save`<br>수동: 편집기 연 채로 다른 편집기에서 같은 파일 수정 → 앱에서 저장 → 저장되지 않고 '파일이 밖에서 바뀌었…다시 불러오세요' 안내 |
| TC-AGT-08-04 | 웹뷰가 임의 경로를 요청 | WHEN 웹뷰가 목록에 없는 ID나 경로 문자열로 저장을 요청한다 / THEN 백엔드가 거부하고 아무 파일도 쓰지 않는다 | 자동 | `crates/dduck-core/src/agents_catalog.rs::guarded_save` |

### F-AGT-09

**작업 시작 옵션** · 구현됨 — TaskSpec.agent --agent=, resolve_for_cwd·persona_risk, task_meta 저장, Composer 선택 · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-09-01 | 페르소나로 작업 시작 | WHEN 사용자가 페르소나 "qa-verifier", 유형 "QA"를 고르고 작업을 시작한다 / THEN `claude --bg ... --agent=qa-verifier`로 시작되고, 카드에 "QA" 태그와 "qa-verifier"가 보인다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::task_validation_and_args`<br>`crates/dduck-core/src/task_meta.rs::tags_prefer_own_record_then_transcript`<br>수동: Composer에서 페르소나 qa-verifier·유형 QA 고르고 시작 → 작업 카드에 'QA' 태그와 'qa-verifier'가 보임 |
| TC-AGT-09-02 | 목록에 없는 페르소나 | WHEN 웹뷰가 목록에 없는 agent 이름으로 시작을 요청한다 / THEN 작업을 시작하지 않고 오류를 표시한다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::discovery_and_resolution`<br>수동: 개발자 도구로 start_task에 목록 없는 agent 이름 전달 → 작업이 시작되지 않고 오류 표시 |
| TC-AGT-09-03 | 다른 폴더의 프로젝트 agent | WHEN 작업 폴더가 side-project인데 dev_team 프로젝트에만 있는 design-critic을 페르소나로 요청한다 / THEN 작업을 시작하지 않고 "이 폴더에서 쓸 수 없는 agent입니다"를 표시하며, Composer 목록에도 보이지 않는다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::discovery_and_resolution`<br>`src/lib/agents.test.ts::offers only personas usable in the folder`<br>수동: 작업 폴더를 side-project로 두고 Composer 페르소나 목록 확인 → dev_team 전용 design-critic이 없음, 강제 요청 시 '이 폴더에서 쓸 수 없는 agent' 오류 |
| TC-AGT-09-04 | 위험한 설정을 가진 페르소나 | WHEN 고른 agent 정의에 `permissionMode: bypassPermissions`나 `hooks`가 있다 / THEN 앱이 네이티브 확인 창에 그 값을 보여 주고, 승인해야만 시작한다 | 부분 자동 | `crates/dduck-core/src/agents_catalog.rs::persona_risk_scans_what_discovery_skips`<br>`crates/dduck-core/src/agents_catalog.rs::risk_and_warning_rules`<br>수동: permissionMode: bypassPermissions가 있는 agent를 페르소나로 시작 → 네이티브 확인 창에 그 값이 보이고 승인해야만 시작 |

### F-AGT-10

**역할 색과 배치 순서** · 구현됨 — roles.ts roleOf 역할 색, orderCards(일하는 agent 앞, abc/custom) · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-10-01 | 일하는 agent 앞으로 | WHEN 이름순 배치에서 qa-verifier가 일하기 시작한다 / THEN qa-verifier 카드가 기본 Claude 앞쪽 그룹으로 옮겨지고, 끝나면 이름 순서 자리로 돌아간다 | 자동 | `src/lib/roles.test.ts::puts active agents first, then ABC or custom order` |

### F-AGT-11

**테마 전환** · 구현됨 — theme.ts 시스템→밝음→어두움 순환, localStorage 저장, Sidebar ThemeButton · spec: `openspec/specs/agent-profiles/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-11-01 | 어두운 테마 | WHEN 사용자가 테마 버튼을 두 번 누른다 / THEN 앱 전체가 어두운 테마가 되고, 앱을 다시 열어도 유지된다 | 수동 | 수동: 사이드바 테마 버튼 두 번 클릭 → 앱 전체 어두운 테마 → 앱 재시작 후에도 첫 화면부터 어두운 테마 유지 |

### F-AGT-12

**팀 규칙 편집** · 구현됨 — team_config 규칙 diff/merge, 안전 규칙 잠금, policy_text로 작업에 주입 · spec: `openspec/specs/team-presets-editable/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-12-01 | 규칙 추가 | WHEN 사용자가 "커밋 메시지는 한국어로 쓴다" 규칙을 추가하고 저장한다 / THEN 다음에 시작하는 작업의 시스템 지시에 그 규칙이 포함된다 | 자동 | `crates/dduck-core/src/team_config.rs::diff_stores_only_changes_and_round_trips` |
| TC-AGT-12-02 | 안전 규칙 삭제 시도 | WHEN 사용자가 "기본 브랜치에 직접 커밋하지 않는다" 규칙을 지우려 한다 / THEN 삭제 버튼이 비활성이고 "안전 규칙은 지울 수 없습니다"가 보인다 | 부분 자동 | `crates/dduck-core/src/team_config.rs::locked_rules_keep_bundled_text`<br>수동: 팀 설정 시트에서 '기본 브랜치에 직접 커밋하지 않는다' 규칙 확인 → 삭제 버튼 비활성·'안전 규칙은 지울 수 없…' 안내 |

### F-AGT-13

**작업 유형 편집** · 구현됨 — TypeDef 편집·추가·숨기기, stored config가 Composer·인자 결정 · spec: `openspec/specs/team-presets-editable/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-13-01 | 학습 유형 effort 바꾸기 | WHEN 사용자가 "학습" 유형의 기본 effort를 높음으로 바꾸고 저장한다 / THEN Composer에서 학습을 고르면 effort가 높음으로 채워지고 시작한 작업에 `--effort high`가 붙는다 | 부분 자동 | `crates/dduck-core/tests/ipc_contract.rs::task_type_keys_and_team_config_payloads`<br>`crates/dduck-core/src/team_config.rs::diff_stores_only_changes_and_round_trips`<br>수동: 팀 설정에서 학습 유형 effort를 높음으로 저장 → Composer에서 학습 고르면 effort가 높음으로 채워짐 → 시작한 작업 인자에 --effort=high |

### F-AGT-14

**안전 불변식** · 구현됨 — safety_invariants·merge 시 위험 모드 제거·risky_changes 확인 창 · spec: `openspec/specs/team-presets-editable/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-14-01 | 위험 권한 추가 시도 | WHEN 저장된 설정에 어떤 유형의 허용 권한으로 위험 모드가 들어 있다(직접 편집 등) / THEN 앱은 그 설정을 거부하고 기본값으로 작업을 시작하며 설정 화면에 오류를 보인다 | 부분 자동 | `crates/dduck-core/src/team_config.rs::untouched_items_follow_new_defaults_and_bad_items_fall_back`<br>`crates/dduck-core/src/team_config.rs::safety_invariants_reject`<br>수동: DB의 팀 설정에 유형 허용 권한으로 bypassPermissions를 직접 넣음 → 작업 시작은 기본값으로 진행되고 팀 설정 화면에 오류가 보임 |

### F-AGT-15

**기본값 복원** · 구현됨 — 차이 저장 + 기본값 병합, reset_one_item · spec: `openspec/specs/team-presets-editable/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-15-01 | 일부만 고친 경우 | WHEN 사용자가 "개발" 유형만 고쳤고 앱 업데이트로 "QA" 기본 프롬프트가 바뀌었다 / THEN 개발은 사용자 값, QA는 새 기본값이 쓰인다 | 자동 | `crates/dduck-core/src/team_config.rs::untouched_items_follow_new_defaults_and_bad_items_fall_back`<br>`crates/dduck-core/src/team_config.rs::reset_one_item` |

### F-AGT-16

**에이전트 켜기·끄기·숨기기·삭제** · 구현됨 — agent_controls(Agent() deny·휴지통), settings hidden_agents, AgentSettings 관리 표 · spec: `openspec/specs/team-presets-editable/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-16-01 | 끄기 | WHEN 사용자가 사용자 에이전트 "code-reviewer"를 끈다 / THEN 전역 settings의 permissions.deny에 `Agent(code-reviewer)`가 추가되고, 카드에 "꺼짐"이 보이며 Composer 역할 목록에서 빠진다. 다시 켜면 그 규칙만 빠진다 | 부분 자동 | `crates/dduck-core/src/agent_controls.rs::deny_rules_are_exact`<br>`crates/dduck-core/src/agent_controls.rs::off_then_on_restores_the_file`<br>`src/lib/team.test.ts::knows which file turned an agent off`<br>수동: code-reviewer 끄기 → 카드에 '꺼짐', Composer 역할 목록에서 빠짐 확인 → 다시 켜기 → settings.json에서 그 규칙만 빠짐 |
| TC-AGT-16-02 | 숨기기 | WHEN 사용자가 내장 에이전트 "statusline-setup"을 숨긴다 / THEN 카드 목록에서 사라지고 settings 파일은 바뀌지 않으며, "숨긴 에이전트 보기"에서 다시 보이게 할 수 있다 | 부분 자동 | `crates/dduck-core/src/settings.rs::hidden_agents_and_team_config_have_own_setters`<br>수동: statusline-setup 숨기기 → 카드 목록에서 사라지고 settings 파일 불변 → '숨긴 에이전트 보기'에서 다시 표시 |
| TC-AGT-16-03 | 삭제 | WHEN 사용자가 프로젝트 에이전트 "old-helper"를 삭제하고 확인한다 / THEN 정의 파일이 휴지통으로 옮겨지고 목록에서 사라진다. 플러그인 에이전트에는 삭제 버튼이 없다 | 부분 자동 | `crates/dduck-core/src/agent_controls.rs::trash_only_plain_user_and_project_files`<br>수동: 프로젝트 에이전트 old-helper 삭제·확인 → Finder 휴지통에 파일, 목록에서 사라짐 → 플러그인 에이전트 행에는 삭제 버튼 없음 |

### F-AGT-17

**범위별 규칙 보기** · 구현됨 — permissions read_scope(permissions만), managed 읽기 전용, PermissionsPage 범위 선택 · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-17-01 | 프로젝트 고르기 | WHEN 사용자가 프로젝트 dev_team을 고른다 / THEN 그 프로젝트의 공유·개인 규칙이 출처 파일과 함께 보이고, 파일이 없으면 "없음(저장하면 만듭니다)"이 보인다 | 부분 자동 | `crates/dduck-core/src/permissions.rs::reads_only_permissions`<br>`crates/dduck-core/src/permissions.rs::managed_sources`<br>수동: 권한 탭에서 프로젝트 dev_team 선택 → 공유·개인 규칙이 출처 파일 경로와 함께 보이고 없는 파일은 '없음(저장하면 만듭니다)' |

### F-AGT-18

**규칙 편집** · 구현됨 — splice(다른 키 보존)·plan_save/commit_save(충돌·백업) · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-18-01 | 다른 키 보존 | WHEN 전역 settings에 hooks·env가 있고 사용자가 deny 규칙 하나를 추가해 저장한다 / THEN 파일의 hooks·env는 그대로이고 permissions.deny에만 규칙이 더해진다 | 자동 | `crates/dduck-core/src/permissions.rs::splice_preserves_other_bytes`<br>`crates/dduck-core/src/permissions.rs::guarded_save` |
| TC-AGT-18-02 | 충돌 | WHEN 편집하는 동안 Claude Code가 같은 파일을 고쳤다 / THEN 저장이 거부되고 "파일이 바뀌었습니다. 다시 불러오세요"가 보인다 | 부분 자동 | `crates/dduck-core/src/permissions.rs::guarded_save`<br>수동: 권한 탭에서 편집 중 다른 편집기로 같은 settings 수정 → 저장 → 거부되고 '파일이 바뀌었…다시 불러오세요' 안내 |

### F-AGT-19

**에이전트 도구 제한** · 구현됨 — with_tool_lists로 tools/disallowedTools 줄만 편집, 플러그인 읽기 전용 · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-19-01 | 리뷰 에이전트에서 쓰기 막기 | WHEN 사용자가 reviewer 에이전트의 disallowedTools에 Edit, Write를 넣고 저장한다 / THEN 정의 파일 frontmatter에만 반영되고 본문은 그대로다 | 자동 | `crates/dduck-core/src/agents_catalog.rs::tool_lists_edit_only_their_lines`<br>`crates/dduck-core/src/agents_catalog.rs::line_edits_keep_bom_and_crlf` |

### F-AGT-20

**실효 확인** · 구현됨 — explain: 전 범위 deny→ask→allow, managed-only · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-20-01 | 거부 규칙 우선 | WHEN 전역 allow에 `Bash(git:*)`, 프로젝트 deny에 `Bash(git push:*)`가 있고 `Bash(git push origin main)`을 확인한다 / THEN "거부 — 프로젝트(공유) deny `Bash(git push:*)`"가 보인다 | 자동 | `crates/dduck-core/src/permissions.rs::explain_deny_wins_across_scopes` |

### F-AGT-21

**권한 카탈로그와 위험도** · 구현됨 — 권한 카탈로그 데이터·위험도·토글·권장값 차이 · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-21-01 | 위험한 항목 허용 | WHEN 사용자가 위험도 "높음"인 `Bash(git push:*)`를 전역 "허용"으로 토글한다 / THEN JSON 미리보기의 allow에 규칙이 생기고, 저장 확인 창에 "위험도 높음 규칙을 허용합니다" 경고가 함께 보인다 | 부분 자동 | `src/lib/permissions.test.ts::reads exact matches and moves only that rule`<br>`crates/dduck-core/src/permissions.rs::guarded_save`<br>`crates/dduck-core/src/permissions.rs::catalog_loads_and_rates`<br>수동: Bash(git push:*)를 전역 '허용'으로 토글 → JSON 미리보기 allow에 생김 → 저장 시 확인 창에 '위험도 높음 규칙을 허용합니다' 경고 |
| TC-AGT-21-02 | 권장값 적용 | WHEN 사용자가 프로젝트 범위에서 "권장값 적용"을 누른다 / THEN 추가·변경·제거될 규칙 목록이 보이고, 확인하면 그대로 미리보기에 반영되며 저장 전까지 파일은 바뀌지 않는다 | 자동 | `src/lib/permissions.test.ts::diffs against scope-specific recommendations` |

### F-AGT-22

**permissions JSON 직접 편집** · 구현됨 — parseJson(줄·이유), permissions 밖 키 읽기 전용 · spec: `openspec/specs/tool-permissions/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-AGT-22-01 | 잘못된 JSON | WHEN 사용자가 쉼표를 빠뜨린 JSON으로 저장을 누른다 / THEN 저장되지 않고 오류 줄·이유가 보이며 토글 화면은 마지막 올바른 상태를 유지한다 | 부분 자동 | `src/lib/permissions.test.ts::reports syntax errors with a line and keeps other keys read-only`<br>`src/lib/permissions.test.ts::counts changes and reports JSON errors in Korean`<br>수동: JSON 편집기에서 쉼표 하나 지우고 저장 → 저장 안 되고 오류 줄·이유 표시 → 토글 화면은 직전 올바른 상태 유지 |

## EXT · 확장(스킬·MCP·플러그인·규칙·메모리)

### F-EXT-01

**사용 기록 표시** · 구현됨 — Uses '기록 없음'·툴팁, copyRules가 '안 씀' 금지 · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-01-01 | 한 번도 안 부른 내 스킬 | WHEN 사용 기록이 0회인 내 스킬 `standup`이 확장 탭에 보인다 / THEN 줄 끝에 "기록 없음"이 보이고, 가리키면 "수집한 기록에서 부른 적이 없어요 · 켜져 있는지와는 달라요"가 뜬다 | 자동 | `src/components/ExtensionsDelete.test.tsx::shows 기록 없음 and the buttons per row`<br>`src/components/ExtensionsDelete.test.tsx::PluginsPage ItemList says 기록 없음 with the tooltip, or N회`<br>`src/lib/copyRules.test.ts::says 기록 없음, not 안 씀, for usage on the extensions screens (D-147)` |
| TC-EXT-01-02 | 부른 적 있는 스킬 | WHEN 사용 기록이 12회인 스킬이 보인다 / THEN "12회"가 보이고 툴팁에는 마지막으로 부른 시각이 뜬다 | 자동 | `src/components/ExtensionsDelete.test.tsx::PluginsPage ItemList says 기록 없음 with the tooltip, or N회` |

### F-EXT-02

**꺼짐 상태 칩** · 구현됨 — InnerLine '꺼짐' 칩(사용 표시와 별도) · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-02-01 | 꺼진 플러그인의 스킬 | WHEN 꺼진 플러그인을 펼친다 / THEN 그 안의 각 스킬·커맨드 줄에 "꺼짐" 칩이 보이고, 사용 표시는 그대로 "기록 없음" 또는 "N회"다 | 자동 | `src/components/ExtensionsDelete.test.tsx::an item of an off plugin shows 꺼짐 before the usage; an on plugin's does not` |
| TC-EXT-02-02 | 켜진 플러그인 | WHEN 켜진 플러그인을 펼친다 / THEN "꺼짐" 칩은 보이지 않는다 | 자동 | `src/components/ExtensionsDelete.test.tsx::an item of an off plugin shows 꺼짐 before the usage; an on plugin's does not` |

### F-EXT-03

**수정·보기 버튼** · 구현됨 — rowActions 수정/보기/복사 규칙 · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-03-01 | 내 커맨드 수정 | WHEN 사용자가 내 커맨드 `/standup` 줄의 "수정"을 누른다 / THEN 편집기가 그 파일로 열리고 저장할 수 있다 | 부분 자동 | `src/components/ExtensionsDelete.test.tsx::own items edit and delete, copies included; read-only items view (+ copy); usage-only rows have none`<br>`src/components/ExtensionsDelete.test.tsx::deletes from the editor and closes it; unsaved changes are passed to the dialog`<br>수동: 확장 탭에서 내 커맨드 /standup의 '수정' 클릭 → 그 파일 편집기가 열림 → 내용 고쳐 저장 → 파일에 반영 |
| TC-EXT-03-02 | 복사본도 수정 | WHEN "frontend-design에서 복사" 배지가 붙은 `frontend-design-custom` 줄을 본다 / THEN "수정"과 "삭제"가 보이고 "복사해서 내 것으로"는 없다 | 자동 | `src/components/ExtensionsDelete.test.tsx::own items edit and delete, copies included; read-only items view (+ copy); usage-only rows have none`<br>`src/components/ExtensionsDelete.test.tsx::shows 기록 없음 and the buttons per row` |
| TC-EXT-03-03 | 플러그인 스킬은 보기 | WHEN 플러그인을 펼쳐 잠긴 스킬 줄을 본다 / THEN "보기"와 "복사해서 내 것으로"가 보이고 "수정"·"삭제"는 없다 | 자동 | `src/components/ExtensionsDelete.test.tsx::an item of an off plugin shows 꺼짐 before the usage; an on plugin's does not`<br>`src/components/NavIa.test.tsx::copies a locked plugin skill to mine from inside its plugin and opens the editor` |

### F-EXT-04

**삭제 대상 제한** · 구현됨 — ext_trash plan_trash(카탈로그 id·종류별 대상·링크/하드링크·dduck-new 거부) · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-04-01 | 링크된 스킬 폴더 | WHEN `~/.claude/skills/shared`가 다른 폴더를 가리키는 링크이고 그 항목의 id로 삭제 명령이 들어온다(카탈로그는 링크 폴더를 목록에 넣지 않으므로 화면에는 버튼이 없다) / THEN 확인 창 없이 "링크라서 여기서 지우지 않아요" 또는 "목록에 없는 항목이에요. 다시 열어 주세요" 한 줄 오류로 거부하고 아무것도 옮기지 않는다 | 자동 | `crates/dduck-core/src/ext_trash.rs::refuses_links_on_the_way_and_inside` |
| TC-EXT-04-02 | 플러그인 항목 요청 | WHEN 플러그인 스킬의 id로 삭제 명령이 들어온다 / THEN 거부하고 파일은 그대로다 | 자동 | `crates/dduck-core/src/ext_trash.rs::refuses_plugins_synced_and_the_dduck_new_command` |
| TC-EXT-04-03 | 하드링크 커맨드 | WHEN 링크 수가 2인 커맨드 파일이 있다 / THEN 목록에서 그 줄은 읽기 전용("보기")이라 "삭제"가 없고, 그 id로 삭제 명령이 들어와도 거부한다 | 자동 | `crates/dduck-core/src/skills_catalog.rs::hard_linked_definitions_are_read_only`<br>`crates/dduck-core/src/ext_trash.rs::refuses_links_on_the_way_and_inside` |

### F-EXT-05

**확인 후 휴지통 이동** · 구현됨 — trash_extension_item: 네이티브 확인 → recheck → move_to_trash · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-05-01 | 내 스킬 지우기 | WHEN 사용자가 내 스킬 `standup`의 "삭제"를 누르고 확인 창에서 "휴지통으로"를 누른다 / THEN `~/.claude/skills/standup/` 폴더 전체가 휴지통으로 가고 목록에서 사라진다 | 부분 자동 | `crates/dduck-core/src/ext_trash.rs::plans_own_skills_commands_and_workflows`<br>`src/components/ExtensionsDelete.test.tsx::moves an item to the Trash; a command with usage leaves a usage-only row in the command group`<br>수동: 테스트 스킬 standup 만들고 '삭제' → 확인 창 '휴지통으로' → Finder 휴지통에 폴더째(보조 파일 포함) 있고 목록에서 사라짐 |
| TC-EXT-05-02 | 확인 사이 내용이 바뀜 | WHEN 확인 창이 떠 있는 동안 다른 프로그램이 그 SKILL.md를 고친다 / THEN "확인하는 동안 바뀌어 지우지 않았어요"가 뜨고 폴더는 그대로다 | 부분 자동 | `crates/dduck-core/src/ext_trash.rs::changes_after_the_dialog_are_caught`<br>수동: 삭제 확인 창을 띄운 채 다른 편집기로 SKILL.md 수정 → '휴지통으로' → '확인하는 동안 바뀌어 지우지 않았어요' 표시·폴더 그대로 |
| TC-EXT-05-03 | 취소 | WHEN 확인 창에서 "취소"를 누른다 / THEN 목록과 파일이 그대로이고 오류 줄이 없다 | 자동 | `src/components/ExtensionsDelete.test.tsx::a cancelled dialog changes nothing and shows no error` |
| TC-EXT-05-04 | 저장 안 한 내용이 있는 편집기에서 지우기 | WHEN 편집기에 저장하지 않은 변경이 있는 채로 "삭제"를 누른다 / THEN 확인 창에 "저장 안 한 내용도 사라져요" 줄이 함께 보이고, 옮긴 뒤에는 그 항목의 편집기 캐시가 비워진다 | 부분 자동 | `src/components/ExtensionsDelete.test.tsx::deletes from the editor and closes it; unsaved changes are passed to the dialog`<br>수동: 편집기에 저장 안 한 변경을 남기고 '삭제' → 네이티브 확인 창에 '저장 안 한 내용도 사라져요' 줄 → 옮긴 뒤 다시 열어도 이전 편집 캐시 없음 |
| TC-EXT-05-05 | 큰 스킬 폴더 | WHEN 스킬 폴더 안 항목이 2,000개를 넘거나 깊이가 8단계를 넘는다 / THEN 확인 창 없이 거부하고 아무것도 옮기지 않는다 | 자동 | `crates/dduck-core/src/ext_trash.rs::refuses_oversized_skill_folders` |

### F-EXT-06

**기본 장비 사용처 경고** · 구현됨 — loadout_users(파일 에이전트 skills·유형 SkillPin) 경고 줄 · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-06-01 | 장비로 쓰는 스킬 | WHEN 에이전트 `code-reviewer`의 `skills`에 `standup`이 있고 사용자가 `standup`을 지우려 한다 / THEN 확인 창에 "code-reviewer가 기본 장비로 써요" 줄이 함께 보인다 | 부분 자동 | `crates/dduck-core/src/ext_trash.rs::finds_loadout_users`<br>수동: code-reviewer의 skills에 standup 넣고 standup '삭제' → 확인 창에 'code-reviewer가 기본 장비로 써요' 줄 |
| TC-EXT-06-02 | 아무도 안 쓰는 스킬 | WHEN 어떤 에이전트도 그 스킬을 장비로 쓰지 않는다 / THEN 확인 창에 장비 줄이 없다 | 부분 자동 | `crates/dduck-core/src/ext_trash.rs::finds_loadout_users`<br>수동: 장비로 쓰지 않는 스킬 '삭제' → 확인 창에 장비 줄 없음 |

### F-EXT-07

**삭제 뒤 화면** · 구현됨 — 삭제 뒤 편집기 닫기·목록 재조회·기록만 줄·한 줄 오류 · spec: `openspec/specs/extension-item-management/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-07-01 | 편집기에서 지우기 | WHEN 사용자가 내 워크플로 편집기를 열어 둔 채 "삭제"를 누르고 확인한다 / THEN 편집기가 닫히고 목록에서 그 워크플로가 사라진다 | 자동 | `src/components/ExtensionsDelete.test.tsx::deletes from the editor and closes it; unsaved changes are passed to the dialog` |
| TC-EXT-07-02 | 휴지통 이동 실패 | WHEN 휴지통으로 옮기지 못한다 / THEN "휴지통으로 옮기지 못했어요" 한 줄이 보이고 항목은 목록에 남는다 | 자동 | `src/components/ExtensionsDelete.test.tsx::a failed move shows one line under that row and keeps the item` |
| TC-EXT-07-03 | 기록이 있는 스킬을 지움 | WHEN 사용 기록이 5회인 내 스킬 `openspec-apply-change`를 지운다 / THEN 정의 줄("수정"·"삭제"가 있던 줄)은 사라지고, 버튼 없는 기록만 있는 줄이 "5회"와 함께 남으며 설명 자리에 "지운 스킬이에요 · 기록만 남아 있어요"가 보인다 | 부분 자동 | `crates/dduck-core/src/skills_catalog.rs::a_deleted_skill_with_usage_stays_as_a_usage_only_row`<br>`src/components/ExtensionsDelete.test.tsx::marks only usage-only rows and gives them the deleted kind; a same-named defined item stays`<br>수동: 사용 기록 있는 내 스킬 삭제 → 버튼 없는 기록만 줄이 'N회'와 '지운 스킬이에요 · 기록만 남아 있어요'로 스킬 묶음에 남음 |
| TC-EXT-07-04 | 기록이 있는 커맨드를 지움 | WHEN 사용 기록이 2회인 내 커맨드 `/standup`을 지운다 / THEN 기록만 있는 줄은 커맨드 묶음에 남고 "지운 커맨드예요 · 기록만 남아 있어요"가 보이며 칩 숫자는 그대로다 | 자동 | `src/components/ExtensionsDelete.test.tsx::moves an item to the Trash; a command with usage leaves a usage-only row in the command group` |
| TC-EXT-07-05 | 링크로 연결된 프로젝트 폴더 | WHEN 알려진 프로젝트 경로가 링크이고 그 프로젝트의 스킬을 지우려 한다 / THEN "링크로 연결된 프로젝트 폴더는 지울 수 없어요"로 거부한다 | 자동 | `crates/dduck-core/src/ext_trash.rs::a_project_configured_through_a_link_is_refused_with_its_own_message` |

### F-EXT-08

**링크 판별과 미리보기** · 구현됨 — install_link parse_link·inspect_github(마켓플레이스·스킬 미리보기), AddSheet · spec: `openspec/specs/install-from-link/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-08-01 | 마켓플레이스 저장소 | WHEN 사용자가 플러그인 마켓플레이스 GitHub 저장소 링크를 붙여 넣는다 / THEN 마켓플레이스가 추가되고 그 안의 플러그인 목록이 보이며, 하나를 골라 설치할 수 있다 | 부분 자동 | `crates/dduck-core/src/install_link.rs::marketplace_preview_without_changing_anything`<br>`crates/dduck-core/src/install_link.rs::cli_add_then_install_never_auto_approves`<br>수동: '링크로 추가'에 공개 마켓플레이스 GitHub 링크 붙여넣기 → 플러그인 목록 미리보기 → 하나 골라 설치 → 플러그인 목록에 나타남 |
| TC-EXT-08-02 | 스킬 폴더 | WHEN 사용자가 SKILL.md가 있는 GitHub 폴더 링크를 붙여 넣는다 / THEN 스킬 이름·설명·파일 목록·SKILL.md 내용이 보이고, 스크립트 파일이 있으면 "실행 파일 포함" 표시가 붙는다 | 부분 자동 | `crates/dduck-core/src/install_link.rs::skill_is_staged_pinned_and_installed_without_overwrite`<br>수동: SKILL.md와 스크립트가 있는 GitHub 폴더 링크 붙여넣기 → 이름·설명·파일 목록·SKILL.md 내용과 '실행 파일 포함' 표시 |
| TC-EXT-08-03 | 허용하지 않는 입력 | WHEN 사용자가 `curl ... \| sh` 같은 명령어나 http 링크를 붙여 넣는다 / THEN 설치 버튼 없이 "지원하지 않는 링크입니다"와 허용 형식이 보인다 | 부분 자동 | `crates/dduck-core/src/install_link.rs::link_forms`<br>수동: 'curl ... \| sh'와 http:// 링크를 각각 붙여넣기 → 설치 버튼 없이 '지원하지 않는 링크' 문구와 허용 형식이 보임 |

### F-EXT-09

**확인과 설치** · 구현됨 — install_skill(네이티브 확인·RENAME_EXCL), NeedsApproval 시 명령 안내 · spec: `openspec/specs/install-from-link/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-09-01 | 이미 있는 스킬 이름 | WHEN 같은 이름의 스킬 폴더가 이미 있다 / THEN 덮어쓰지 않고 "이미 있는 스킬입니다"를 보인다 | 자동 | `crates/dduck-core/src/install_link.rs::skill_is_staged_pinned_and_installed_without_overwrite` |
| TC-EXT-09-02 | 추가 승인 필요 | WHEN 플러그인 설치가 마켓플레이스 선언 명령의 승인을 요구한다 / THEN 설치되지 않고 터미널에서 실행할 명령이 복사 버튼과 함께 보인다 | 부분 자동 | `crates/dduck-core/src/install_link.rs::cli_add_then_install_never_auto_approves`<br>`crates/dduck-core/src/plugins.rs::classifies_cli_results`<br>수동: 승인 명령을 요구하는 플러그인 설치 시도 → 설치되지 않고 터미널에서 실행할 명령이 복사 버튼과 함께 보임 |

### F-EXT-10

**메모리 목록** · 구현됨 — memory list(색인·고아 줄), MemoryPage 프로젝트 선택 · spec: `openspec/specs/memory-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-10-01 | 프로젝트 메모리 보기 | WHEN 사용자가 홈 프로젝트를 고른다 / THEN MEMORY.md의 각 줄에 대응하는 메모리 5개가 이름·설명·종류와 함께 보이고, 누르면 본문이 열린다 | 부분 자동 | `crates/dduck-core/src/memory.rs::lists_entries_and_orphans`<br>`crates/dduck-core/src/memory.rs::projects_hide_nothing_but_mark_empty_and_worktrees`<br>수동: 메모리 탭에서 홈 프로젝트 선택 → MEMORY.md 각 줄의 메모리가 이름·설명·종류와 함께 보임 → 누르면 본문 열림 |

### F-EXT-11

**메모리 편집** · 구현됨 — memory save(토큰 충돌·색인 설명 동기화·링크 거부) · spec: `openspec/specs/memory-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-11-01 | 충돌 | WHEN 편집하는 동안 Claude가 같은 메모리 파일을 고쳤다 / THEN 저장이 거부되고 "파일이 바뀌었습니다. 다시 불러오세요"가 보인다 | 부분 자동 | `crates/dduck-core/src/memory.rs::save_syncs_the_index_description_and_detects_conflicts`<br>`crates/dduck-core/src/memory.rs::conflicts_duplicates_and_links_are_refused`<br>수동: 메모리 편집기 연 채 다른 편집기로 같은 파일 수정 → 저장 → 거부되고 '파일이 바뀌었…다시 불러오세요' 안내 |

### F-EXT-12

**끄기와 켜기** · 구현됨 — set_enabled(.off/ 이동·색인 줄 복원·중복 없음), autoMemoryEnabled 스위치 · spec: `openspec/specs/memory-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-12-01 | 끄고 다시 켜기 | WHEN 사용자가 "Commit style" 메모리를 끄고 나중에 다시 켠다 / THEN 끄면 파일이 `memory/.off/`로 옮겨지고 해당 줄이 사라지며, 그사이 Claude가 MEMORY.md를 고쳐 써도 켜면 그 메모리의 줄이 한 번만 다시 생긴다 | 자동 | `crates/dduck-core/src/memory.rs::off_then_on_after_claude_rewrote_the_index`<br>`crates/dduck-core/src/memory.rs::on_does_not_duplicate_a_line_claude_already_added`<br>`crates/dduck-core/src/memory.rs::round_trip_restores_the_exact_line_in_place` |

### F-EXT-13

**지시 파일** · 구현됨 — instruction_files(경로·크기·git 추적), open_instruction_file · spec: `openspec/specs/memory-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-13-01 | 프로젝트 지시 보기 | WHEN 사용자가 dev_team을 고르고 지시 파일을 연다 / THEN CLAUDE.md의 경로와 내용이 읽기 전용으로 보이고 "편집기에서 열기"를 누르면 기본 편집기로 열린다 | 부분 자동 | `crates/dduck-core/src/memory.rs::instruction_files_are_listed_read_only`<br>수동: 메모리 탭 dev_team 선택 → 지시 파일 열기 → CLAUDE.md 경로·내용이 읽기 전용 → '편집기에서 열기'로 기본 편집기 실행 |

### F-EXT-14

**비밀값 경고** · 구현됨 — secrets 탐지(본문 미반환), 목록·편집기 배지 · spec: `openspec/specs/memory-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-14-01 | 토큰이 적힌 메모리 | WHEN 메모리 본문에 API 키 모양 문자열이 있다 / THEN 목록과 편집기에 "비밀값처럼 보이는 내용" 경고가 보인다 | 부분 자동 | `crates/dduck-core/src/memory.rs::secrets_are_detected_not_returned`<br>수동: 메모리 파일에 sk-ant-… 모양 가짜 키 적기 → 메모리 탭 목록과 편집기에 '비밀값처럼 보이는 내용' 경고 |

### F-EXT-15

**플러그인 목록** · 구현됨 — plugins parse_list·components·apply_usage, 확장 탭 플러그인 칩(nav-ia로 통합) · spec: `openspec/specs/plugins-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-15-01 | 플러그인 펼치기 | WHEN 사용자가 스킬 3개와 커맨드 2개가 든 플러그인을 펼친다 / THEN 다섯 항목이 종류별로 이름·설명·사용 횟수와 함께 보인다 | 부분 자동 | `crates/dduck-core/src/plugins.rs::components_from_defaults_and_manifest`<br>`crates/dduck-core/src/plugins.rs::list_keeps_allow_listed_fields_only`<br>수동: 스킬 3·커맨드 2개 플러그인 펼치기 → 다섯 항목이 종류별로 이름·설명·사용 횟수와 함께 보임 |

### F-EXT-16

**켜기와 끄기** · 구현됨 — set_plugin_enabled(CLI -s scope), judge_toggle 재조회, Locked 처리 · spec: `openspec/specs/plugins-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-16-01 | 끄기 | WHEN 사용자가 켜진 플러그인의 토글을 끈다 / THEN 플러그인이 꺼짐으로 표시되고 다시 목록을 불러와도 꺼져 있다 | 부분 자동 | `crates/dduck-core/src/plugins.rs::results_are_judged_by_the_reread_list`<br>`crates/dduck-core/src/plugins.rs::cli_wrapper_passes_scope_and_never_auto_approves`<br>수동: 켜진 플러그인 토글 끄기 → '꺼짐'과 재시작 안내 → 목록 새로고침 후에도 꺼짐 |

### F-EXT-17

**업데이트** · 구현됨 — compare 버전, update_plugin(네이티브 확인·자동 승인 금지) · spec: `openspec/specs/plugins-tab/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-17-01 | 새 버전 | WHEN 설치 1.0.2, 마켓플레이스 1.0.3이다 / THEN "1.0.3으로 업데이트"가 활성이고, 확인하면 업데이트 후 버전이 1.0.3으로 바뀐다 | 부분 자동 | `crates/dduck-core/src/plugins.rs::version_then_sha_comparison`<br>`crates/dduck-core/src/plugins.rs::local_marketplace_files`<br>`crates/dduck-core/src/plugins.rs::results_are_judged_by_the_reread_list`<br>수동: 설치 1.0.2·마켓 1.0.3 플러그인 → '1.0.3으로 업데이트' 활성 → 확인 창에 1.0.2→1.0.3 → 승인 후 버전 1.0.3 |
| TC-EXT-17-02 | 추가 승인 필요 | WHEN 업데이트가 마켓플레이스 선언 명령 승인을 요구한다 / THEN 아무것도 실행되지 않고 "터미널에서 claude plugin update로 진행하세요"가 보인다 | 부분 자동 | `crates/dduck-core/src/plugins.rs::cli_wrapper_passes_scope_and_never_auto_approves`<br>`crates/dduck-core/src/plugins.rs::classifies_cli_results`<br>수동: 승인 명령을 요구하는 플러그인 업데이트 → 아무것도 실행되지 않고 '터미널에서 claude plugin update로 진행하세요' 안내 |

### F-EXT-18

**규칙 목록** · 구현됨 — rules list_group 재귀·paths 칩, RulesPanel 범위 머리 한 줄 · spec: `openspec/specs/rules-editor/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-18-01 | 프로젝트 규칙 보기 | WHEN 사용자가 dev_team을 고르고 "규칙"을 연다 / THEN `.claude/rules/` 아래 규칙들이 "항상" 또는 패턴 칩과 함께 보이고, 규칙이 없으면 "아직 규칙이 없어요 · 새 규칙" 안내가 보인다 | 자동 | `crates/dduck-core/src/rules.rs::lists_recursively_with_scope_chips`<br>`src/components/RulesPanel.test.tsx::shows global even without a project, and the empty project hint`<br>`src/components/RulesPanel.test.tsx::labels scope, groups by folder and filters` |

### F-EXT-19

**규칙 만들기·편집** · 구현됨 — create(O_EXCL)·save(paths 외 보존·충돌)·이름·glob 경고 · spec: `openspec/specs/rules-editor/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-19-01 | 경로 한정 규칙 만들기 | WHEN 사용자가 프로젝트 범위에서 템플릿 "API 규칙"으로 만들고 적용 범위에 `src/api/**/*.ts`를 넣어 저장한다 / THEN `.claude/rules/api.md`가 `paths` 앞부분과 함께 생기고 목록에 패턴 칩이 보인다 | 자동 | `crates/dduck-core/src/rules.rs::create_edit_and_conflict`<br>`src/components/RulesPanel.test.tsx::creates a path-scoped rule from the API template` |
| TC-EXT-19-02 | 충돌 | WHEN 편집 중 Claude가 같은 규칙 파일을 고쳤다 / THEN 저장이 거부되고 "파일이 바뀌었어요. 다시 불러오세요"가 보인다 | 자동 | `crates/dduck-core/src/rules.rs::create_edit_and_conflict`<br>`src/components/RulesPanel.test.tsx::shows a conflict and offers reload` |

### F-EXT-20

**끄기·켜기·삭제** · 구현됨 — set_enabled(claudeMdExcludes 앱 항목만)·trash_rule · spec: `openspec/specs/rules-editor/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-20-01 | 규칙 잠시 끄기 | WHEN 사용자가 git이 추적하는 프로젝트 규칙 `testing.md`를 끈다 / THEN 파일은 그대로이고 `.claude/settings.local.json`의 `claudeMdExcludes`에 그 절대 경로가 추가되며 목록에 "꺼짐"으로 보이고, 켜면 그 항목만 빠진다 | 자동 | `crates/dduck-core/src/rules.rs::turning_off_and_on_edits_only_the_app_entry`<br>`src/components/RulesPanel.test.tsx::toggles with the settings token; other-pattern and linked rules are handled` |

### F-EXT-21

**안전한 쓰기** · 구현됨 — 링크·하드링크·링크 폴더 거부, 앞부분 오류 표시, git 배지 · spec: `openspec/specs/rules-editor/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-21-01 | 공유 링크 규칙 | WHEN 규칙 파일이 다른 곳을 가리키는 심볼릭 링크다 / THEN 편집·삭제가 비활성화되고(끄기는 가능) "공유 링크 · 원본에서 편집"이 보인다 | 자동 | `crates/dduck-core/src/rules.rs::links_are_read_only_but_can_be_turned_off`<br>`src/components/RulesPanel.test.tsx::toggles with the settings token; other-pattern and linked rules are handled` |

### F-EXT-22

**세션 목록 페이지** · 구현됨 — analytics sessions_page(offset·total), 세션 섹션 기간·페이지 · spec: `openspec/specs/sessions-skills-connectors/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-22-01 | 전체 세션 | WHEN 기록에 세션이 34개 있고 사용자가 사용 통계를 연다 / THEN 세션 목록에 10개가 보이고 "1–10 / 34"와 다음 버튼이 있으며, 다음을 누르면 11–20이 보인다 | 부분 자동 | `crates/dduck-core/tests/sessions_skills_contract.rs::sessions_page_counts_and_offsets`<br>수동: 세션 34개 기록에서 사용 통계 열기 → 10개와 '1–10 / 34'·다음 버튼 → 다음 누르면 11–20 |

### F-EXT-23

**세션 타임라인** · 구현됨 — timeline read(요약·가림·상한), 세션 상세 흐름·필터 · spec: `openspec/specs/sessions-skills-connectors/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-23-01 | 도구 호출 확인 | WHEN 사용자가 Bash 호출 12번이 있는 세션을 열고 "도구" 필터를 고른다 / THEN 도구 호출 항목만 시간순으로 보이고 각 항목에 명령 요약과 성공·오류가 표시된다 | 부분 자동 | `crates/dduck-core/src/timeline.rs::builds_flow_without_thinking_or_result_bodies`<br>수동: Bash 호출이 여러 번 있는 세션 열기 → '도구' 필터 → 도구 항목만 시간순으로, 각 항목에 명령 요약과 성공·오류 |
| TC-EXT-23-02 | 비밀처럼 보이는 내용 | WHEN 도구 입력에 긴 토큰 문자열이 있다 / THEN 화면에는 요약 길이까지만 보이고 D.Duck DB와 로그에는 아무것도 남지 않는다 | 자동 | `crates/dduck-core/src/timeline.rs::redacts_secret_shapes`<br>`crates/dduck-core/src/timeline.rs::builds_flow_without_thinking_or_result_bodies`<br>`crates/dduck-core/tests/sessions_skills_contract.rs::timeline_is_never_logged_or_stored` |

### F-EXT-24

**세션 제목 수정** · 구현됨 — labels set/get(검증·제목 끄기와 분리), 연필 편집 · spec: `openspec/specs/sessions-skills-connectors/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-24-01 | 제목 고치기 | WHEN 사용자가 세션 "c3000000"의 연필을 눌러 "노트북 정리"를 입력하고 Enter를 누른다 / THEN 목록과 상세에 "노트북 정리"가 보이고 앱을 다시 켜도 유지된다 | 부분 자동 | `crates/dduck-core/src/labels.rs::labels_validate_and_survive_title_toggle`<br>수동: 세션 목록 연필 클릭 → '노트북 정리' 입력 후 Enter → 목록·상세에 표시 → 앱 재시작 후 유지 |
| TC-EXT-24-02 | 제목 저장 끄기와의 관계 | WHEN 설정에서 "세션 제목 저장"을 끈다 / THEN Claude가 붙인 제목은 지워지지만 사용자가 직접 붙인 제목은 남는다 | 자동 | `crates/dduck-core/src/labels.rs::labels_validate_and_survive_title_toggle`<br>`crates/dduck-core/tests/ingest_fixture.rs::turning_titles_off_scrubs_them_from_disk_and_keeps_them_out` |

### F-EXT-25

**스킬 목록** · 구현됨 — skills_catalog(출처·사용 합산·정의 없음); 사이드바 '스킬'은 nav-ia로 확장 탭 칩으로 이동 · spec: `openspec/specs/sessions-skills-connectors/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-25-01 | 많이 쓴 스킬 | WHEN 사용자가 스킬 페이지를 연다 / THEN 스킬이 사용 횟수 순으로 보이고 각 항목에 출처와 마지막 사용 시각이 있다 | 부분 자동 | `crates/dduck-core/src/skills_catalog.rs::finds_all_sources_and_counts_once`<br>수동: 확장 탭 '스킬' 칩 열기 → 사용 횟수 순으로 정렬, 각 항목에 출처와 마지막 사용 시각 |

### F-EXT-26

**커넥터 목록** · 구현됨 — mcp_list parse(대상 문자열 폐기), MCP 탭 연결 상태 확인; 40MB 상한은 timeline WINDOW_BYTES · spec: `openspec/specs/sessions-skills-connectors/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-26-01 | 상태 확인 | WHEN 사용자가 "연결 상태 확인"을 누른다 / THEN 커넥터마다 이름과 상태가 보이고, 로컬 커넥터에는 실행 명령이 보이지 않는다 | 자동 | `crates/dduck-core/src/mcp_list.rs::keeps_name_kind_host_state_only`<br>`src/components/NavIa.test.tsx::lists every claude.ai connector in its own group on both sides and keeps the result after leaving the tab` |
| TC-EXT-26-02 | 긴 세션 | WHEN 기록 파일이 40MB인 세션을 연다 / THEN 최근 항목이 보이고 "최근 부분만 보여 줍니다"가 표시된다 | 수동 | 수동: transcript가 32MB 넘는(예: 40MB) 세션 열기 → 최근 항목이 보이고 '기록이 길어 최근 부분만 보여 줘요' 표시 |

### F-EXT-27

**직접 만들기** · 구현됨 — create_item(기본 틀·O_EXCL·이름 1–64자), AddSheet 만들기 · spec: `openspec/specs/skill-authoring/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-27-01 | 내 커맨드 만들기 | WHEN 사용자가 커맨드, 내 것, 이름 `standup`, 설명을 입력하고 저장한다 / THEN `~/.claude/commands/standup.md`가 frontmatter와 함께 생기고 커맨드 목록에 `/standup`이 보인다 | 부분 자동 | `crates/dduck-core/src/authoring.rs::create_never_overwrites_and_refuses_links`<br>`crates/dduck-core/src/authoring.rs::templates_pass_their_own_checks_and_carry_no_shell`<br>`src/lib/addItems.test.ts::fills templates with one safe line`<br>수동: 만들기 시트에서 커맨드·전역·standup·설명 입력 후 저장 → ~/.claude/commands/standup.md가 frontmatter와 함께 생기고 목록에 /standup |
| TC-EXT-27-02 | 이름 충돌 | WHEN 이미 있는 스킬 이름으로 만들려 한다 / THEN 저장되지 않고 "이미 있는 이름입니다"가 보인다 | 자동 | `crates/dduck-core/src/authoring.rs::create_never_overwrites_and_refuses_links` |

### F-EXT-28

**Claude에게 만들기** · 구현됨 — start_authoring_task(고정 도구·임시 작업 폴더) → 리뷰 후 install_produced, '새로 만듦' 표시 · spec: `openspec/specs/skill-authoring/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-28-01 | 스킬 요청 | WHEN 사용자가 "PR 설명을 우리 팀 형식으로 써 주는 스킬"을 요청한다 / THEN 작업이 시작되어 작업 보드에 보이고, 끝나면 내 스킬 목록에 새 스킬이 나타나며 바로 고칠 수 있다 | 부분 자동 | `crates/dduck-core/src/authoring.rs::job_preset_is_fixed_by_rust`<br>`crates/dduck-core/src/authoring.rs::jobs_are_reviewed_then_installed_through_create`<br>`src/lib/addItems.test.ts::marks items made in the last 7 days by kind, name and folder`<br>수동: 'Claude에게 만들기'로 PR 설명 스킬 요청 → 작업 보드에 작업 표시 → 끝난 뒤 결과 설치 → 내 스킬 목록에 '새로 만듦'으로 보이고 바로 수정 가능 |

### F-EXT-29

**고치기** · 구현됨 — save_authoring_item 안전 저장, 읽기 전용은 Rust(skills_catalog readonly) 판단 · spec: `openspec/specs/skill-authoring/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-29-01 | 플러그인 스킬 | WHEN 사용자가 플러그인에 들어 있는 스킬을 연다 / THEN 내용은 보이지만 편집기는 읽기 전용이고 "플러그인 항목은 고칠 수 없습니다"가 보인다 | 부분 자동 | `src/components/ExtensionsDelete.test.tsx::read-only items open the editor without a delete button`<br>`crates/dduck-core/src/skills_catalog.rs::finds_all_sources_and_counts_once`<br>수동: 플러그인 스킬 '보기' → 내용은 보이나 편집기 읽기 전용, '플러그인 항목은 고칠 수 없어요' 안내 |

### F-EXT-30

**만들기 지침과 터미널 커맨드** · 일부 구현 — /dduck-new 설치·제거는 구현, 지침 편집은 설계 리뷰(decisions 637행)로 제외·고정 GUIDE · spec: `openspec/specs/skill-authoring/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-EXT-30-01 | 터미널 커맨드 설치 | WHEN 사용자가 "터미널에서도 쓰기"를 켜고 확인한다 / THEN `~/.claude/commands/dduck-new.md`가 생기고, 끄면 그 파일이 휴지통으로 간다 | 부분 자동 | `crates/dduck-core/src/authoring.rs::jobs_are_reviewed_then_installed_through_create`<br>`crates/dduck-core/src/authoring.rs::templates_pass_their_own_checks_and_carry_no_shell`<br>수동: '터미널에서도 쓰기' 켜고 확인 → ~/.claude/commands/dduck-new.md 생성 → 끄기 → 그 파일이 Finder 휴지통으로 |

## ART · 아티팩트·루틴

### F-ART-01

**카드 미리보기** · 구현됨 — 원본 탐색·경로 검사·sandbox srcdoc 구현. tasks 2.2(번들 앱에서 iframe 격리 실측)만 사용자 확인 대기 · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-01-01 | 로컬 HTML 원본이 있음 | WHEN 아티팩트가 `~/proj/report.html`에서 게시되었고 파일이 남아 있다 / THEN 카드에 그 페이지가 축소되어 보이고, 페이지 안 스크립트는 실행되지 않는다 | 부분 자동 | `crates/dduck-core/src/artifact_preview.rs::reads_the_last_publish_of_the_matching_artifact`<br>`src/components/ArtifactsPage.test.tsx::renders a sandboxed srcdoc frame, markdown as text and reasons`<br>`src/components/ArtifactsPage.test.tsx::puts the CSP and dns-prefetch metas before any file content`<br>수동: ~/proj에 <script>로 제목을 바꾸는 HTML을 Artifact로 게시 → 아티팩트 탭 열기 → 카드에 축소된 페이지가 보이고 스크립트 효과·외부 요청이 없음(네트워크 탭 0건) |
| TC-ART-01-02 | 원본이 지워짐 | WHEN 원본 파일이 삭제되었다 / THEN 카드에 종류 아이콘과 "미리보기 없음 · 원본 파일이 없습니다"가 보이고 열기·링크 버튼은 그대로 동작한다 | 자동 | `crates/dduck-core/src/artifact_preview.rs::too_large_and_missing`<br>`src/components/ArtifactsPage.test.tsx::renders a sandboxed srcdoc frame, markdown as text and reasons`<br>`src/components/ArtifactsPage.test.tsx::names each card and its open button` |
| TC-ART-01-03 | 위험한 경로 | WHEN 기록의 원본 경로가 심볼릭 링크이거나 홈 폴더 밖이거나 확장자가 다르다 / THEN 파일을 읽지 않고 "미리보기 없음"이 보인다 | 자동 | `crates/dduck-core/src/artifact_preview.rs::symlinks_are_refused`<br>`crates/dduck-core/src/artifact_preview.rs::outside_home_and_denied_folders_are_refused`<br>`crates/dduck-core/src/artifact_preview.rs::extension_must_be_allowed`<br>`crates/dduck-core/src/artifact_preview.rs::hard_links_are_refused` |

### F-ART-02

**아티팩트 수집** · 구현됨 — ingest가 버전 표식·링크 형식 검사 후 artifacts 테이블에 upsert(seq MAX, last_ts MAX) · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-02-01 | 여러 번 게시한 아티팩트 | WHEN 한 아티팩트가 순번 2, 3, 4로 세 번 게시되었다 / THEN 목록에는 그 아티팩트가 한 번만 나오고 버전 4와 마지막 게시 시각이 표시된다 | 자동 | `crates/dduck-core/tests/artifacts.rs::artifacts_are_merged_by_link_id_and_filtered` |
| TC-ART-02-02 | 다른 도구의 링크 | WHEN 웹 페이지 가져오기 결과에 claude.ai 아티팩트 링크가 들어 있다 / THEN 그 결과는 아티팩트로 저장되지 않는다 | 자동 | `crates/dduck-core/tests/artifacts.rs::artifacts_are_merged_by_link_id_and_filtered` |
| TC-ART-02-03 | 결과의 다른 필드 | WHEN 게시 결과에 로컬 파일 경로와 지침 텍스트가 함께 들어 있다 / THEN 앱 DB와 로그 어디에도 그 경로와 텍스트가 남지 않는다 | 자동 | `crates/dduck-core/tests/artifacts.rs::no_other_result_fields_are_stored`<br>`crates/dduck-core/tests/no_content_logging.rs::no_logging_macros_in_app_sources` |
| TC-ART-02-04 | 형식이 다른 링크 | WHEN 게시 결과의 링크가 claude.ai 아티팩트 형식이 아니다 / THEN 그 결과는 저장되지 않는다 | 자동 | `crates/dduck-core/tests/artifacts.rs::artifacts_are_merged_by_link_id_and_filtered`<br>`crates/dduck-core/src/artifacts.rs::only_collected_valid_links_are_returned` |

### F-ART-03

**아티팩트 목록** · 구현됨 — ArtifactsPage 목록·제목 검색·빈 상태 구현(문구는 해요체 '아직 없어요'로 바뀜) · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-03-01 | 제목 검색 | WHEN 사용자가 3쪽을 보다가 검색창에 "대시보드"를 입력한다 / THEN 제목에 "대시보드"가 들어간 아티팩트만 1쪽부터 보인다 | 자동 | `src/components/ArtifactsPage.test.tsx::pages 10 at a time and resets to page 1 on search`<br>`crates/dduck-core/src/artifacts.rs::only_collected_valid_links_are_returned`<br>`src/components/ArtifactsPage.test.tsx::resets to page 1 on search even when the results still span pages` |
| TC-ART-03-02 | 페이지 넘기기 | WHEN 아티팩트가 23개 있다 / THEN 1쪽에 카드 10개, 페이지 표시 "1 / 3"이 보이고, 다음을 누르면 11~20번째가, 3쪽에는 3개가 보인다 | 자동 | `src/components/ArtifactsPage.test.tsx::splits 23 rows into 10 / 10 / 3 and clamps out-of-range pages`<br>`src/components/ArtifactsPage.test.tsx::pages 10 at a time and resets to page 1 on search` |
| TC-ART-03-03 | 아티팩트 없음 | WHEN 이 기기에서 만든 아티팩트가 없다 / THEN "이 기기에서 만든 아티팩트가 아직 없습니다" 안내가 보인다 | 수동 | 수동: 아티팩트 기록이 없는 설정 폴더로 앱 실행 → 아티팩트 탭 열기 → '이 기기에서 만든 아티팩트가 아직 없어요' 안내가 보임 |

### F-ART-04

**아티팩트 열기** · 구현됨 — open_artifact 명령이 DB에 수집된 id의 검증된 URL만 연다(safe_url) · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-04-01 | 브라우저로 열기 | WHEN 사용자가 아티팩트의 "열기"를 누른다 / THEN 기본 브라우저에서 그 아티팩트 링크가 열린다 | 수동 | 수동: 아티팩트 1개 이상 수집된 상태 → 카드의 '열기' 클릭 → 기본 브라우저에 claude.ai 아티팩트 링크가 열림 |
| TC-ART-04-02 | 임의 URL 거부 | WHEN 웹뷰가 수집되지 않은 ID로 열기를 요청한다 / THEN 아무것도 열지 않고 오류를 돌려준다 | 부분 자동 | `crates/dduck-core/src/artifacts.rs::only_collected_valid_links_are_returned`<br>`crates/dduck-core/src/artifact_preview.rs::lookup_validates_ids`<br>수동: 개발자 도구 콘솔에서 open_artifact를 없는 id로 invoke → 오류가 돌아오고 브라우저가 열리지 않음 |

### F-ART-05

**제목 저장 설정 연동** · 구현됨 — 제목 저장 끄기 시 artifacts.title을 NULL로 지우고, 화면은 artifactId 앞 8자로 대체 · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-05-01 | 제목 저장 끄기 | WHEN 사용자가 세션 제목 저장을 끈다 / THEN 아티팩트 목록의 제목이 아티팩트 ID 앞부분으로 바뀐다 | 부분 자동 | `crates/dduck-core/tests/artifacts.rs::titles_off_keeps_artifact_titles_out`<br>수동: 아티팩트가 있는 상태 → 설정에서 세션 제목 저장 끄기 → 아티팩트 탭의 카드 제목이 ID 앞 8자로 바뀜 |

### F-ART-06

**사이드바 메뉴** · 구현됨 — 사이드바 '작업 공간' 그룹에 아티팩트 메뉴 존재. nav-ia 개편으로 순서가 작업·리뷰·루틴·아티팩트로 바뀜(스펙 위치와 다름) · spec: `openspec/specs/artifacts/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-06-01 | 메뉴 이동 | WHEN 사용자가 사이드바의 "아티팩트"를 누른다 / THEN 아티팩트 페이지가 열린다 | 부분 자동 | `src/components/NavIa.test.tsx::shows 작업 공간 and Claude 구성 in order, then usage and settings, and no project picker`<br>수동: 앱 실행 → 사이드바 '아티팩트' 클릭 → 아티팩트 페이지가 열림 |

### F-ART-07

**루틴 정의** · 구현됨 — RoutineInput.validate가 이름·폴더·일정·위험 모드를 검사, 기본 plan · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-07-01 | 매일 루틴 만들기 | WHEN 사용자가 이름 "아침 점검", 폴더, 지시, 일정 "매일 09:00"으로 루틴을 저장한다 / THEN 루틴 목록에 나타나고 다음 실행 시각이 다음 09:00으로 표시된다 | 부분 자동 | `crates/dduck-core/src/routines.rs::daily_weekly_interval`<br>`crates/dduck-core/src/routines.rs::validation_rules`<br>`src/lib/routines.test.ts::describes the next run relative to now`<br>수동: 루틴 탭 → 이름 '아침 점검'·폴더·지시·매일 09:00으로 저장 → 목록에 나타나고 다음 실행이 다음 09:00으로 표시 |
| TC-ART-07-02 | 위험 모드 차단 | WHEN 웹뷰가 bypassPermissions 모드로 루틴 저장을 요청한다 / THEN 저장이 거부되고 오류가 표시된다 | 자동 | `crates/dduck-core/src/routines.rs::validation_rules` |
| TC-ART-07-03 | 입력 검증 | WHEN 이름이 비었거나 폴더가 없거나 N시간 값이 25다 / THEN 저장하지 않고 무엇이 잘못되었는지 표시한다 | 자동 | `crates/dduck-core/src/routines.rs::validation_rules`<br>`src/lib/routines.test.ts::validates the form before the native dialog` |

### F-ART-08

**루틴 승인** · 구현됨 — save_routine이 네이티브 확인 창을 띄우고 승인 해시를 저장, tick이 변조된 정의를 notApproved로 거부 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-08-01 | 웹뷰가 지시를 바꿔 저장 요청 | WHEN 웹뷰가 루틴의 지시를 바꿔 저장을 요청한다 / THEN 네이티브 확인 창이 바뀐 지시 앞부분을 보여 주고, 취소하면 DB가 바뀌지 않는다 | 수동 | 수동: 기존 루틴의 지시를 고쳐 저장 → 네이티브 확인 창에 바뀐 지시 앞부분·'사용자가 없을 때도 실행' 안내가 보임 → 취소 → 목록·DB의 지시가 그대로 |
| TC-ART-08-02 | 승인 없이 바뀐 내용 | WHEN 저장된 루틴의 지시가 승인 절차 없이 DB에서 바뀌었다 / THEN 예약 실행은 시작하지 않고 "시작 실패(승인되지 않음)"로 기록한다 | 자동 | `crates/dduck-core/src/routines.rs::tampered_definition_is_not_run`<br>`src/lib/routines.test.ts::names run statuses with reasons` |

### F-ART-09

**신뢰되지 않은 폴더 경고** · 구현됨 — RoutinesPage에 신뢰 경고와 '터미널에서 신뢰하기' 버튼 구현(trust.rs로 판정) · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-09-01 | 신뢰되지 않은 폴더 | WHEN 사용자가 신뢰되지 않은 폴더로 루틴을 저장한다 / THEN "이 폴더는 아직 신뢰되지 않아 실행이 실패합니다" 경고와 "터미널에서 신뢰하기" 버튼이 보인다 | 수동 | 수동: Claude가 신뢰하지 않은 새 폴더 → 그 폴더로 루틴 저장 → '이 폴더는 아직 신뢰되지 않아 실행이 실패해요' 경고와 '터미널에서 신뢰하기' 버튼이 보임 |

### F-ART-10

**예약 실행** · 구현됨 — routines::tick(점유·일정 선이동·busy/동시성 건너뜀)과 셸 스케줄러 구현. tasks 3.1(가짜 CLI 통합 확인) 미체크 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-10-01 | 정시 실행 | WHEN "매일 09:00" 루틴이 켜져 있고 앱이 실행 중일 때 09:00이 된다 / THEN 09:00 이후 첫 검사에서 루틴 이름으로 백그라운드 작업이 한 번 시작되고, 다음 실행 시각은 다음 날 09:00이 된다 | 자동 | `crates/dduck-core/src/routines.rs::tick_claims_once_and_moves_schedule_first`<br>`crates/dduck-core/src/routines.rs::decisions` |
| TC-ART-10-02 | 신뢰되지 않은 폴더의 예약 실행 | WHEN 신뢰되지 않은 폴더의 루틴 시각이 된다 / THEN "시작 실패(신뢰되지 않은 폴더)"로 기록되고 창이나 터미널은 뜨지 않는다 | 부분 자동 | `crates/dduck-core/tests/scheduler_isolation.rs::scheduler_paths_have_no_interactive_calls`<br>`src/lib/routines.test.ts::names run statuses with reasons`<br>수동: 신뢰되지 않은 폴더의 루틴을 1~2분 뒤로 저장 → 시각까지 대기 → 이력에 '시작 실패(신뢰되지 않은 폴더)', 창·터미널 안 뜸 |
| TC-ART-10-03 | 겹치는 실행 | WHEN "1시간마다" 루틴의 이전 실행이 아직 진행 중인데 다음 시각이 된다 / THEN 새 실행을 시작하지 않고 "건너뜀(이전 실행 중)"으로 기록한다 | 자동 | `crates/dduck-core/src/routines.rs::decisions`<br>`crates/dduck-core/src/routines.rs::tick_claims_once_and_moves_schedule_first`<br>`src/lib/routines.test.ts::names run statuses with reasons` |

### F-ART-11

**앱이 꺼져 있던 동안의 실행** · 구현됨 — decide가 5분 넘게 지난 회차를 Missed로 기록하고 다음 시각을 현재 이후로 계산 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-11-01 | 밤새 꺼져 있던 앱 | WHEN "매일 09:00" 루틴이 있고 앱이 어제 저녁부터 오늘 10:00까지 꺼져 있었다 / THEN 앱을 켜면 오늘 09:00 실행이 "놓침"으로 기록되고, 다음 실행 시각은 내일 09:00이 된다 | 부분 자동 | `crates/dduck-core/src/routines.rs::missed_while_asleep_and_no_missed_after_enabling`<br>`crates/dduck-core/src/routines.rs::decisions`<br>수동: 매일 09:00 루틴 → 앱을 09:00 전에 끄고 10:00 뒤 켬 → 이력에 오늘 09:00 '놓침' 1건, 다음 실행이 내일 09:00 |

### F-ART-12

**지금 실행과 켜고 끄기** · 구현됨 — claim_now(Busy 거부)·set_enabled(다음 시각 재계산, 놓침 없음) 구현 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-12-01 | 진행 중인 루틴의 지금 실행 | WHEN 루틴이 진행 중인데 사용자가 "지금 실행"을 누른다 / THEN 새 실행을 시작하지 않고 "이미 실행 중입니다"를 표시한다 | 부분 자동 | `crates/dduck-core/src/routines.rs::tick_claims_once_and_moves_schedule_first`<br>수동: 루틴 '지금 실행'으로 진행 중 상태 만들기 → 다시 '지금 실행' → 새 실행이 없고 '이미 실행 중이에요' 표시 |
| TC-ART-12-02 | 지금 실행 | WHEN 사용자가 꺼진 루틴에서 "지금 실행"을 누른다 / THEN 그 루틴이 한 번 실행되고 이력에 남으며, 루틴은 계속 꺼진 상태다 | 자동 | `crates/dduck-core/src/routines.rs::run_now_keeps_disabled_state` |

### F-ART-13

**실행 이력** · 구현됨 — routine_runs 이력·track(폴링으로 상태 갱신)·'작업 보기' 이동 구현 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-13-01 | 이력 확인 | WHEN 사용자가 3번 실행된 루틴을 고른다 / THEN 실행 3건이 최신순으로 상태와 함께 보인다 | 부분 자동 | `crates/dduck-core/src/routines.rs::tracking_and_lost_runs`<br>`src/lib/routines.test.ts::names run statuses with reasons`<br>수동: 루틴을 '지금 실행'으로 3번 실행 → 루틴 선택 → 실행 3건이 최신순으로 상태와 함께 보임 |
| TC-ART-13-02 | 실행 중인 작업으로 이동 | WHEN 사용자가 진행 중인 실행의 "작업 보기"를 누른다 / THEN 작업 페이지가 열리고 그 작업의 상세가 선택된다 | 수동 | 수동: 진행 중인 루틴 실행 → 이력의 '작업 보기' 클릭 → 작업 페이지가 열리고 해당 작업 상세가 선택됨 |

### F-ART-14

**루틴 지시문 저장** · 구현됨 — delete 후 scrub(VACUUM 등)로 지시문 바이트 제거 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-14-01 | 루틴 삭제 | WHEN 사용자가 루틴을 지운다 / THEN 그 루틴의 지시문과 실행 이력이 DB에서 사라지고, DB·WAL 파일 바이트에도 지시문이 남지 않는다 | 부분 자동 | `crates/dduck-core/src/routines.rs::deleting_scrubs_prompt_from_disk`<br>수동: 고유 문자열을 지시로 둔 루틴을 만들고 1회 실행 → 루틴 삭제 → 이력이 사라지고 app 데이터 폴더의 dduck.db·-wal을 grep해도 그 문자열이 없음 |

### F-ART-15

**클라우드 루틴 목록** · 구현됨 — 수동 '클라우드 루틴 불러오기'·비용·claude.ai 링크 구현. 4b.3(응답 파일 정리 순서) 코드상 파싱 뒤 정리로 고쳐졌으나 실제 앱 재확인은 미체크 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-15-01 | 클라우드 루틴 불러오기 | WHEN 사용자가 "클라우드 루틴 불러오기"를 누른다 / THEN CLI 응답의 루틴 수만큼 이름·다음 실행 시각·켜짐 여부가 보이고, 이번 조회 비용과 시각이 표시된다 | 부분 자동 | `crates/dduck-core/src/cloud_routines.rs::keeps_only_allowed_fields`<br>`crates/dduck-core/src/cloud_routines.rs::inline_and_saved_responses`<br>수동: [스파이크] claude.ai 로그인 상태, 클라우드 루틴 1개 이상 → '클라우드 루틴 불러오기' 클릭 → 루틴 수만큼 이름·다음 실행·켜짐이 보이고 비용·조회 시각 표시 |
| TC-ART-15-02 | 조회 실패 | WHEN CLI가 없거나 로그인되어 있지 않거나 60초 안에 끝나지 않는다 / THEN 목록 대신 이유와 "claude.ai에서 열기" 버튼을 보여 준다 | 부분 자동 | `crates/dduck-core/src/cloud_routines.rs::keeps_only_allowed_fields`<br>`src/components/LoginBits.test.tsx::the load button is off with the reason next to it (%o)`<br>수동: 설정에서 CLI 경로를 없는 파일로 지정 → '클라우드 루틴 불러오기' → 목록 대신 이유와 'claude.ai에서 열기' 버튼이 보임 |

### F-ART-16

**클라우드 루틴 조회 안전** · 구현됨 — RemoteTrigger list 전용 PreToolUse 가드·자체 점검·허용 필드만 파싱·임시 폴더 정리 · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-16-01 | 루틴 데이터 속 지시 | WHEN 어떤 클라우드 루틴의 지시문에 "다른 루틴을 실행하라"는 문장이 있다 / THEN 조회 중 RemoteTrigger의 list 외 동작은 훅이 거부하고, 목록만 표시된다 | 자동 | `crates/dduck-core/src/cloud_routines.rs::guard_parses_real_json`<br>`crates/dduck-core/src/cloud_routines.rs::hook_result_allows_only_explicitly`<br>`crates/dduck-core/src/cloud_routines.rs::self_test_fails_closed`<br>`crates/dduck-core/src/cloud_routines.rs::args_are_guarded` |
| TC-ART-16-02 | 클라우드 루틴 열기 | WHEN 사용자가 "Claude 클라우드 루틴 열기"를 누른다 / THEN 기본 브라우저에서 claude.ai 루틴 페이지가 열린다 | 수동 | 수동: 루틴 탭 → 'claude.ai에서 열기' 클릭 → 기본 브라우저에 claude.ai 루틴 페이지가 열림 |

### F-ART-17

**사이드바 메뉴** · 구현됨 — 사이드바에 루틴 메뉴 존재. nav-ia로 아티팩트보다 위(리뷰 아래)로 옮겨짐(스펙 위치와 다름) · spec: `openspec/specs/routines/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-17-01 | 메뉴 이동 | WHEN 사용자가 사이드바의 "루틴"을 누른다 / THEN 루틴 페이지가 열린다 | 부분 자동 | `src/components/NavIa.test.tsx::shows 작업 공간 and Claude 구성 in order, then usage and settings, and no project picker`<br>수동: 앱 실행 → 사이드바 '루틴' 클릭 → 루틴 페이지가 열림 |

### F-ART-18

**고칠 수 있는 값** · 구현됨 — build_body·simple_schedule·cloudDiff로 켜기·이름·단순 일정만 수정. tasks 4.2(실제 앱 1회 수정 확인) 미체크 · spec: `openspec/specs/cloud-routine-edit/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-18-01 | 한 번 실행 루틴의 날짜 옮기기 | WHEN 사용자가 "한 번" 루틴의 날짜·시각을 미래 시각으로 바꾸고 저장한다 / THEN 확인 창에 전/후 시각이 보이고, 확인하면 그 루틴의 일정만 바뀌며 목록의 다음 실행 시각이 갱신된다 | 부분 자동 | `crates/dduck-core/src/cloud_routines/edit.rs::body_has_changed_fields_only`<br>`src/lib/cloudRoutines.test.ts::sends only changed fields`<br>`src/lib/cloudRoutines.test.ts::round-trips the datetime input`<br>수동: [스파이크] '한 번' 클라우드 루틴 → 고치기에서 미래 날짜·시각 저장 → 확인 창에 전/후 시각 → 확인 → 목록 다음 실행 시각이 갱신 |
| TC-ART-18-02 | 복잡한 cron | WHEN 루틴 일정이 매일/매주로 표현되지 않는 cron이다 / THEN 일정 칸은 읽기 전용이고 "claude.ai에서 고치기" 링크만 있다 | 부분 자동 | `crates/dduck-core/src/cloud_routines/edit.rs::cron_round_trip_with_day_wrap`<br>`src/lib/cloudRoutines.test.ts::describes schedules in local time`<br>수동: 매월 1일 같은 복잡한 cron 루틴 → 고치기 열기 → 일정 칸이 읽기 전용이고 'claude.ai에서 고치기' 링크만 있음 |
| TC-ART-18-03 | 과거 시각 | WHEN "한 번" 일정에 지금보다 이른 시각을 넣는다 / THEN 저장되지 않고 입력 옆에 이유가 보인다 | 부분 자동 | `crates/dduck-core/src/cloud_routines/edit.rs::body_has_changed_fields_only`<br>`src/lib/cloudRoutines.test.ts::refuses to enable a past one-time routine and empty weekdays`<br>수동: '한 번' 루틴 고치기 → 지금보다 이른 시각 입력 → 저장 안 되고 입력 옆에 이유가 보임 |

### F-ART-19

**정확히 한 요청만 보낸다** · 구현됨 — 기대 요청(base64url)과 정확히 같은 update만 허용하는 1회용 가드 · spec: `openspec/specs/cloud-routine-edit/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-19-01 | 모델이 다른 입력을 만든 경우 | WHEN 실행 중 도구 입력이 앱의 기대 요청과 하나라도 다르다 / THEN 호출은 차단되고 사용자에게 "바꾸지 못했습니다"가 보이며 루틴은 그대로다 | 부분 자동 | `crates/dduck-core/src/cloud_routines/edit.rs::exact_match_only`<br>`crates/dduck-core/src/cloud_routines/edit.rs::guard_is_one_time_and_fails_closed`<br>`crates/dduck-core/src/cloud_routines/edit.rs::expected_is_allow_listed`<br>수동: [스파이크] 실제 CLI 수정 중 가드가 거부하는 상황(예: 다른 필드 입력) 재현 → 화면에 '바꾸지 못했어요' 표시, claude.ai에서 루틴이 그대로 |

### F-ART-20

**확인과 흔적** · 구현됨 — 수정 전 네이티브 확인, 성공·실패와 무관하게 cloud-routines 작업 폴더 정리 · spec: `openspec/specs/cloud-routine-edit/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-ART-20-01 | 확인 취소 | WHEN 사용자가 확인 창에서 취소한다 / THEN Claude CLI는 실행되지 않는다 | 수동 | 수동: 클라우드 루틴 고치기 → 저장 → 네이티브 확인 창에서 취소 → claude 프로세스가 실행되지 않음(ps로 확인), 루틴 그대로 |
| TC-ART-20-02 | 실패해도 정리 | WHEN 실행이 시간 초과·로그인 만료로 실패한다 / THEN 오류가 보이고 앱 작업 폴더의 Claude 기록은 남지 않는다 | 부분 자동 | `crates/dduck-core/src/cloud_routines.rs::cleans_only_the_private_folder`<br>`crates/dduck-core/src/cloud_routines.rs::inline_and_saved_responses`<br>수동: CLI 경로를 sleep만 하는 가짜로 지정 → 클라우드 루틴 수정 → 시간 초과 오류가 보이고 설정 폴더 projects 아래 cloud-routines 작업 폴더 기록이 없음 |

## SAF · 계정·안전·한도

### F-SAF-01

**로그인 상태 확인** · 구현됨 — AuthCache.ensure(60초 쿨다운·단일 실행)·auth_status_raw(5초) 구현 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-01-01 | 포커스 확인 속도 제한 | WHEN 창이 30초 사이에 다섯 번 포커스를 되찾는다 / THEN `auth status`는 그동안 최대 한 번 실행된다 | 자동 | `crates/dduck-core/src/login_session.rs::focus_checks_are_rate_limited` |
| TC-SAF-01-02 | 동시 요청 | WHEN 두 화면이 동시에 상태를 요청한다 / THEN `auth status`는 한 번만 실행되고 두 화면이 같은 결과를 받는다 | 자동 | `crates/dduck-core/src/login_session.rs::concurrent_requests_share_one_run` |

### F-SAF-02

**상태 판정** · 구현됨 — judge가 종료 코드·loggedIn 일치로 3상태 판정, stderr 버림 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-02-01 | 로그아웃 판정 | WHEN `auth status`가 종료 코드 1로 `{"loggedIn": false, "authMethod": "none", ...}`을 출력한다 / THEN 상태는 로그아웃이고 로그인 배너가 보인다 | 자동 | `crates/dduck-core/src/auth_status.rs::real_logged_out_shape`<br>`src/components/LoginBits.test.tsx::%s (cli missing: %s) shows %i banner` |
| TC-SAF-02-02 | 파싱 실패는 알 수 없음 | WHEN `auth status`가 JSON이 아닌 출력을 내거나 5초 안에 끝나지 않는다 / THEN 상태는 알 수 없음이고 배너도 게이트도 없으며 설정에 "로그인 상태를 확인하지 못했어요" [다시 확인]이 보인다 | 자동 | `crates/dduck-core/src/auth_status.rs::exit_code_and_json_must_agree`<br>`crates/dduck-core/src/claude_cli.rs::auth_status_times_out_in_5s`<br>`src/components/LoginBits.test.tsx::shows %o as %s`<br>`src/components/LoginBits.test.tsx::[다시 확인] refreshes` |
| TC-SAF-02-03 | 코드와 값 불일치 | WHEN 종료 코드가 0인데 `loggedIn`이 false다 / THEN 상태는 알 수 없음이다 | 자동 | `crates/dduck-core/src/auth_status.rs::exit_code_and_json_must_agree` |

### F-SAF-03

**허용 필드만 보관** · 구현됨 — 허용 필드만 역직렬화, authMethod·planTier 허용 목록 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-03-01 | 개인 정보 canary | WHEN `email`·`orgId`·`orgName`·`projectsDirectory`·`configDirectory`에 서로 다른 CANARY 값을 넣은 출력을 처리한다 / THEN 판정 결과의 직렬화·Debug 출력·IPC 응답·앱 로그(tracing)·DB 파일 바이트 어디에도 CANARY가 없다 | 부분 자동 | `crates/dduck-core/src/auth_status.rs::personal_fields_never_survive`<br>`src-tauri/src/login_cmds.rs::gate_with_a_fake_cli_runs_nothing_but_auth_status`<br>`crates/dduck-core/tests/no_content_logging.rs::no_logging_macros_in_app_sources`<br>수동: CANARY 필드를 출력하는 가짜 CLI로 앱 실행 → 앱 데이터 폴더 DB 파일들을 grep → CANARY 없음 |
| TC-SAF-03-02 | 낯선 값 | WHEN `authMethod`가 허용 목록에 없는 문자열이고 `planTier`가 `Max 5x!`다 / THEN 연결 방식은 "기타", 요금제는 비어 있고 원래 문자열은 남지 않는다 | 자동 | `crates/dduck-core/src/auth_status.rs::methods_map_to_an_allowlist`<br>`crates/dduck-core/src/auth_status.rs::plan_values_are_kept_only_when_safe` |

### F-SAF-04

**연결 방식 표시** · 구현됨 — connection_text와 AuthSettingsRow, 로그인 버튼은 loggedOut에서만 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-04-01 | 클라우드 공급자 | WHEN `loggedIn`이 true이고 `authMethod`가 `third_party`다 / THEN 설정에 "다른 방식으로 연결돼 있어요"가 보이고 로그인 버튼과 배너는 없다 | 자동 | `crates/dduck-core/src/auth_status.rs::methods_map_to_an_allowlist`<br>`src/components/LoginBits.test.tsx::shows %o as %s`<br>`src/components/LoginBits.test.tsx::%s (cli missing: %s) shows %i banner` |
| TC-SAF-04-02 | 환경 토큰 | WHEN `loggedIn`이 true이고 `authMethod`가 `oauth_token`이다 / THEN "토큰으로 연결돼 있어요"가 보이고 모델 호출 기능은 잠기지 않는다 | 자동 | `crates/dduck-core/src/auth_status.rs::gate_by_need`<br>`src/components/LoginBits.test.tsx::shows %o as %s` |

### F-SAF-05

**모델 호출 기능의 게이트** · 구현됨 — 백엔드 명령 앞 check(Need::Model)·login_required 코드, 루틴·재개 경로 반영 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-05-01 | 로그아웃에서 작업 시작 | WHEN 로그아웃 상태에서 작업 시작을 누른다 / THEN `claude --bg`는 실행되지 않고 작성 내용은 그대로이며 한 줄 안내와 [로그인]이 보인다 | 부분 자동 | `src-tauri/src/login_cmds.rs::gate_with_a_fake_cli_runs_nothing_but_auth_status`<br>수동: 로그아웃 가짜 CLI 지정 → Composer에 목표 작성 후 시작 → claude --bg 미실행, 작성 내용 유지, 안내 한 줄과 [로그인] |
| TC-SAF-05-02 | 화면 우회 | WHEN 로그아웃 상태에서 웹뷰가 작업 시작 명령을 직접 호출한다 / THEN 백엔드가 `login_required`를 돌려주고 CLI를 실행하지 않는다 | 자동 | `src-tauri/src/login_cmds.rs::gate_with_a_fake_cli_runs_nothing_but_auth_status` |
| TC-SAF-05-03 | 확인 창보다 먼저 | WHEN 로그아웃 상태에서 루틴 "지금 실행"이나 "직접 이어서"를 누른다 / THEN 네이티브 확인 창이 뜨지 않고 루틴·재개 행은 claim되지 않는다 | 부분 자동 | `src/components/LoginBits.test.tsx::is off with the reason when logged out, back on after login`<br>수동: 로그아웃 상태 → 개발자 도구에서 run_routine_now·직접 이어서 명령 invoke → 네이티브 확인 창 없음, routine_runs·limit_resumes 상태 변화 없음 |
| TC-SAF-05-04 | 예약된 루틴 | WHEN 로그아웃 상태에서 로컬 루틴 시각이 된다 / THEN 실행하지 않고 그 회차를 `login_required`로 기록한다 | 자동 | `crates/dduck-core/src/routines.rs::logged_out_occurrence_is_recorded_as_login_required`<br>`src-tauri/src/login_cmds.rs::gate_codes_go_into_routine_records` |
| TC-SAF-05-05 | 자동 재개 보류 | WHEN 로그아웃 상태에서 한도 재개 행의 due 시각이 된다 / THEN 행은 claim되지 않고 `auto`로 남으며, 로그인 뒤 다음 확인 주기에 평소처럼 재개된다 | 자동 | `crates/dduck-core/src/limit_resume.rs::logged_out_keeps_auto_rows_until_login` |

### F-SAF-06

**claude.ai 로그인 전용 기능** · 구현됨 — Need::ClaudeAi 게이트와 클라우드 루틴 버튼 비활성 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-06-01 | 토큰 연결에서 클라우드 루틴 | WHEN `authMethod`가 `oauth_token`일 때 클라우드 루틴 탭을 연다 / THEN CLI를 실행하지 않고 "클라우드 루틴은 Claude 계정으로 로그인해야 써요"가 보인다 | 자동 | `crates/dduck-core/src/auth_status.rs::gate_by_need`<br>`src/components/LoginBits.test.tsx::cloud routines with a token connection: the claude.ai line, no login button`<br>`src/components/LoginBits.test.tsx::the load button is off with the reason next to it (%o)` |

### F-SAF-07

**로그인 없이 쓰는 기능과 배너** · 구현됨 — LoginBanner, 로그아웃 중 LimitBanner 숨김, 투어 로그인 줄 구현 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-07-01 | 읽기 기능 | WHEN 로그아웃 상태에서 세션 기록과 통계를 연다 / THEN 평소처럼 보인다 | 수동 | 수동: 로그아웃 가짜 CLI 지정 → 세션 기록·사용 통계 탭 열기 → 평소처럼 목록과 수치가 보임 |
| TC-SAF-07-02 | 한도 배너와 겹침 | WHEN 한도 대기 작업이 있고 로그아웃 상태다 / THEN 작업 화면에는 로그인 배너만 보이고, 로그인하면 한도 배너가 돌아온다 | 자동 | `src/components/LoginBits.test.tsx::hides the limit banner while logged out and brings it back after login` |
| TC-SAF-07-03 | 투어 | WHEN CLI는 찾았고 로그아웃 상태에서 투어를 연다 / THEN "Claude Code를 찾지 못했어요"가 있던 자리에 로그인 한 줄과 [로그인]이 보인다 | 자동 | `src/components/TourDeck.test.tsx::start card, cli %s and %s: the status line is %s (claude-login)` |

### F-SAF-08

**기존 로그인 오류 문구 통일** · 구현됨 — recheck_after_failure로 로그인 오류 시 재확인, 게이트 문구로 표시 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-08-01 | 늦은 실패 | WHEN 상태가 알 수 없음이라 통과한 요청 다듬기가 로그인 오류로 끝난다 / THEN "Claude에 로그인해야 이 기능을 쓸 수 있어요" [로그인]이 보이고 상태 확인이 한 번 실행된다 | 부분 자동 | `src/components/LoginBits.test.tsx::a backend refusal shows the same line even while the state is unknown`<br>수동: auth status는 JSON 아닌 출력, -p 실행은 로그인 오류를 내는 가짜 CLI → 요청 다듬기 실행 → 'Claude에 로그인해야 이 기능을 쓸 수 있어요' [로그인] 표시, 가짜 CLI 로그에 auth status 1회 추가 |

### F-SAF-09

**브라우저 로그인 실행** · 구현됨 — login_child(고정 인자·stdin null·프로세스 그룹)와 2초 폴링·5분 제한 구현. tasks 1.1(TTY 없는 실제 로그인 실측) 미완료 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-09-01 | 성공 | WHEN [로그인]을 누르고 브라우저에서 승인한다 / THEN 다음 확인에서 로그인됨이 되고 배너가 사라지며 작업 시작이 가능해진다 | 부분 자동 | `src/components/LoginBits.test.tsx::[로그인] starts the claude.ai login and opens the sheet; Console is the second choice`<br>`crates/dduck-core/src/login_session.rs::clean_exit_keeps_polling_until_the_login_shows`<br>`crates/dduck-core/src/claude_cli.rs::login_child_args_env_and_stdin`<br>수동: [스파이크] 실제 로그아웃 상태 → [로그인] → 브라우저에서 승인 → 배너가 사라지고 작업 시작 가능 |
| TC-SAF-09-02 | 시간 초과 | WHEN 5분 동안 로그인됨이 되지 않는다 / THEN 자식이 끝나고 "로그인이 끝나지 않았어요" [다시 시도] [터미널에서 로그인]이 보인다 | 자동 | `crates/dduck-core/src/login_session.rs::success_timeout_exit_and_paste_prompt`<br>`src/components/LoginBits.test.tsx::failed: [다시 시도] and [터미널에서 로그인]` |

### F-SAF-10

**로그인 프로세스 수명** · 구현됨 — LoginRunner 단일 잡·killpg·Drop 종료·출력 EOF까지 소비 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-10-01 | 취소 | WHEN 진행 중에 [취소]를 누른다 / THEN 자식 프로세스 그룹이 끝나고 상태가 다시 확인되며 로그인 버튼이 다시 보인다 | 부분 자동 | `crates/dduck-core/src/login_session.rs::link_is_kept_in_memory_and_cancel_ends_the_group`<br>`src/components/LoginBits.test.tsx::Esc asks before cancelling a running login; focus starts inside and Tab wraps`<br>수동: [로그인] 진행 중 [취소] → ps로 claude auth login 그룹이 없음, 로그인 버튼이 다시 보임 |
| TC-SAF-10-02 | 중복 실행 | WHEN 로그인 진행 중에 다른 화면의 [로그인]을 누른다 / THEN 두 번째 `claude auth login`은 실행되지 않고 진행 화면이 앞으로 온다 | 부분 자동 | `crates/dduck-core/src/login_session.rs::link_is_kept_in_memory_and_cancel_ends_the_group`<br>수동: 설정에서 [로그인] 진행 중 배너의 [로그인] 클릭 → claude auth login 프로세스가 하나뿐이고 진행 시트가 앞으로 옴 |
| TC-SAF-10-03 | 앱 종료 | WHEN 로그인 진행 중에 앱을 끝낸다 / THEN `claude auth login` 프로세스 그룹이 남지 않는다 | 부분 자동 | `crates/dduck-core/src/login_session.rs::big_output_does_not_block_and_drop_ends_the_group`<br>수동: [로그인] 진행 중 앱 종료(Cmd+Q) → ps aux \| grep 'auth login' 결과 없음 |
| TC-SAF-10-04 | 많은 출력 | WHEN 자식이 1MB를 출력한다 / THEN 자식은 막히지 않고 앱은 64KB까지만 검사한다 | 자동 | `crates/dduck-core/src/login_session.rs::big_output_does_not_block_and_drop_ends_the_group`<br>`crates/dduck-core/src/login_output.rs::reader_consumes_everything_but_scans_64kb` |

### F-SAF-11

**브라우저가 안 열릴 때의 링크** · 구현됨 — allowed_url 허용 목록, URL은 백엔드 메모리에만, has_link만 전달 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-11-01 | 허용 호스트 | WHEN CLI가 `https://claude.ai/oauth/authorize?...`를 출력한다 / THEN "브라우저에서 직접 열기"가 보이고 누르면 그 주소가 열린다 | 부분 자동 | `crates/dduck-core/src/login_output.rs::url_allowlist`<br>`crates/dduck-core/src/login_session.rs::link_is_kept_in_memory_and_cancel_ends_the_group`<br>`src/components/LoginBits.test.tsx::running: progress, [취소], and the link button only with has_link`<br>수동: https://claude.ai/oauth/authorize?... 를 출력하는 가짜 CLI → [로그인] → '브라우저에서 직접 열기' 클릭 → 그 주소가 브라우저에 열림 |
| TC-SAF-11-02 | 낯선 호스트 | WHEN CLI 출력에 `https://claude.ai.example.com/...`이나 `http://claude.ai/...`이 있다 / THEN 링크는 보이지 않는다 | 자동 | `crates/dduck-core/src/login_output.rs::url_allowlist`<br>`crates/dduck-core/src/login_output.rs::urls_inside_text`<br>`crates/dduck-core/src/login_session.rs::only_allowlisted_links_are_kept` |
| TC-SAF-11-03 | URL 비노출 | WHEN 로그인 진행 중 IPC 응답과 앱 로그를 살핀다 / THEN URL 문자열이 없다 | 자동 | `crates/dduck-core/src/login_session.rs::link_is_kept_in_memory_and_cancel_ends_the_group`<br>`crates/dduck-core/tests/no_content_logging.rs::no_logging_macros_in_app_sources` |

### F-SAF-12

**터미널 대체 경로** · 구현됨 — terminal_args(argv 전달·env -u CLAUDE_CONFIG_DIR)·네이티브 확인 후 Terminal.app · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-12-01 | 코드 붙여넣기 요구 | WHEN CLI 출력에 코드 붙여넣기 안내가 나온다 / THEN 앱은 코드를 받지 않고 [터미널에서 로그인]을 보여 준다 | 자동 | `crates/dduck-core/src/login_output.rs::paste_prompt`<br>`crates/dduck-core/src/login_session.rs::success_timeout_exit_and_paste_prompt`<br>`src/components/LoginBits.test.tsx::failed (%s) says why in one line; primary is %s` |
| TC-SAF-12-02 | 같은 프로필 | WHEN 앱 설정 폴더가 기본값이고 사용자 셸에 `CLAUDE_CONFIG_DIR`이 설정되어 있다 / THEN 터미널 명령은 `CLAUDE_CONFIG_DIR`을 지우고 실행되어 앱과 같은 프로필에 로그인한다 | 자동 | `src-tauri/src/login_cmds.rs::terminal_args_keep_values_out_of_the_script`<br>`src-tauri/src/login_cmds.rs::copy_command_matches_the_profile` |
| TC-SAF-12-03 | 확인 후 실행 | WHEN [터미널에서 로그인]을 누르고 확인 창에서 취소한다 / THEN Terminal.app이 열리지 않는다 | 수동 | 수동: 로그인 실패 시트 → [터미널에서 로그인] → 네이티브 확인 창에서 취소 → Terminal.app이 열리지 않음 |

### F-SAF-13

**만료 안내** · 구현됨 — expires_soon(3일, claude.ai만)과 ExpiryLine · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-13-01 | 2일 남음 | WHEN `loginExpiresAt`이 지금부터 2일 뒤다 / THEN 안내 한 줄이 보이고 작업 시작은 막히지 않는다 | 자동 | `crates/dduck-core/src/auth_status.rs::expiry_window`<br>`src/components/LoginBits.test.tsx::expiry notice only when the backend says it is soon (2 days yes, 10 days no)` |
| TC-SAF-13-02 | 10일 남음 | WHEN `loginExpiresAt`이 10일 뒤다 / THEN 안내가 없다 | 자동 | `crates/dduck-core/src/auth_status.rs::expiry_window`<br>`src/components/LoginBits.test.tsx::expiry notice only when the backend says it is soon (2 days yes, 10 days no)` |

### F-SAF-14

**자격증명 비접근** · 구현됨 — 로그인 셸에서 PATH만 가져오고 토큰 변수는 자식에 넘기지 않음 · spec: `openspec/specs/claude-login/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-14-01 | 환경 변수 비전달 | WHEN 로그인 셸에 `CLAUDE_CODE_OAUTH_TOKEN`이 설정되어 있다 / THEN 앱이 실행하는 `auth status`·`auth login` 자식 환경에는 앱 자체 환경에 없던 그 변수가 없다 | 자동 | `crates/dduck-core/src/claude_cli.rs::login_shell_tokens_do_not_reach_children` |

### F-SAF-15

**전달 범위 결정** · 구현됨 — guard_for/repo_info(링크 해제·NFC·대소문자·성분 비교, worktree→본 저장소). tasks 1.1(실제 CLI 거부·훅 실측) 미완료 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-15-01 | 체크하지 않은 프로젝트 | WHEN 체크 목록이 비어 있고 `/work/app`에서 작업을 시작한다 / THEN 범위는 `draft_pr`다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::storage_fails_closed_and_round_trips` |
| TC-SAF-15-02 | 체크한 프로젝트의 worktree | WHEN `/work/client`가 체크되어 있고 작업 cwd가 `/work/client/.claude/worktrees/fix-1`이다 / THEN 범위는 `local_only`다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::repo_info_worktrees_and_guard` |
| TC-SAF-15-03 | 외부 위치의 git worktree | WHEN `/work/client`가 체크되어 있고 `/tmp/wt`가 `git worktree add`로 만든 그 저장소의 worktree다 / THEN `/tmp/wt`에서 시작한 작업의 범위는 `local_only`다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::repo_info_worktrees_and_guard` |
| TC-SAF-15-04 | 대소문자·정규화가 다른 경로 | WHEN `/Work/Client`(NFD 한글 폴더명 포함)로 체크했고 `/work/client`(NFC)에서 작업을 시작한다 / THEN 범위는 `local_only`다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::path_keys_nfc_case_and_components`<br>`crates/dduck-core/src/delivery_guard.rs::repo_info_worktrees_and_guard` |
| TC-SAF-15-05 | 이름이 겹치는 이웃 폴더 | WHEN `/work/client`가 체크되어 있고 `/work/client2`에서 작업을 시작한다 / THEN 범위는 `draft_pr`다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::path_keys_nfc_case_and_components`<br>`crates/dduck-core/src/delivery_guard.rs::repo_info_worktrees_and_guard` |
| TC-SAF-15-06 | 루틴 | WHEN 체크한 프로젝트 폴더에서 도는 로컬 루틴이 실행된다 / THEN 그 실행도 `local_only` 거부 규칙·훅·규칙 문장을 받는다 | 부분 자동 | `crates/dduck-core/src/routines.rs::routine_in_a_local_only_project_gets_the_rule_and_bad_list_refuses`<br>수동: 체크한 프로젝트 폴더로 루틴 '지금 실행' → 작업 인자(ps 또는 가짜 CLI 로그)에 Bash(git push *) 거부와 --settings 훅, 팀 규칙에 로컬만 문장 |

### F-SAF-16

**범위를 모르면 시작하지 않음** · 구현됨 — 목록 파싱 실패·판정 오류 시 UNREADABLE로 거부, draft_pr로 대체 안 함 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-16-01 | 설정 손상 | WHEN 저장된 체크 목록 값이 JSON이 아니다 / THEN 작업이 시작되지 않고 "원격 push 막기 설정을 못 읽었어요"가 보인다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::start_ctx_forces_worktree_on_default_and_refuses_bad_list`<br>`crates/dduck-core/src/delivery_guard.rs::storage_fails_closed_and_round_trips`<br>수동: 작업 시키기 창에서 시작을 막는 조건을 만든다 → 하단 문구와 시작 버튼 상태를 본다 → 막힌 이유가 보이고 시작되지 않는다 |
| TC-SAF-16-02 | 재개 때 설정 손상 | WHEN 사용량 리셋 후 재개 직전에 체크 목록을 못 읽는다 / THEN 재개가 실패로 기록되고 `claude`가 실행되지 않는다 | 부분 자동 | `crates/dduck-core/src/routines.rs::routine_in_a_local_only_project_gets_the_rule_and_bad_list_refuses`<br>`src/lib/limits.test.ts::delivery-scope failures have their own lines`<br>`crates/dduck-core/src/claude_cli.rs::resume_args_carry_the_start_values_and_nothing_else`<br>수동: 한도 대기 작업이 있는 상태에서 DB의 local_only_projects 값을 '{'로 바꿈 → '지금 이어서' → 재개 실패(원격 push 막기 설정을 못 읽어 못 이었어요) 기록, claude 미실행 |

### F-SAF-17

**같은 범위 인자를 모든 실행 경로에** · 구현됨 — guard_args 하나로 시작·재개 인자 생성, 번들 데이터만 사용 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-17-01 | 재개 인자 | WHEN `local_only` 프로젝트의 작업을 사용량 리셋 후 잇는다 / THEN 재개 인자에 `Bash(git push *)`가 든 `--disallowedTools=`와 훅이 든 `--settings=`가 있다 | 자동 | `crates/dduck-core/src/claude_cli.rs::resume_args_carry_the_start_values_and_nothing_else`<br>`crates/dduck-core/tests/claude_cli_fake.rs::start_background_refuses_without_a_delivery_scope` |
| TC-SAF-17-02 | 웹뷰 필드 무시 | WHEN 웹뷰가 시작 요청에 `denyTools: []`를 넣어 보낸다 / THEN 무시되고 기본 거부 목록이 붙는다 | 수동 | 수동: 가짜 CLI(argv 기록) 지정 → 개발자 도구에서 start_task를 denyTools: [] 포함 payload로 invoke → 기록된 인자에 기본 --disallowedTools 목록이 그대로 있음 |

### F-SAF-18

**잠금 규칙 문구 "PR 초안까지"** · 구현됨 — branch 규칙은 잠금, 저장된 override text 무시 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-18-01 | 저장한 팀 설정이 branch 문구를 바꿈 | WHEN 저장한 팀 설정의 `branch` 덮어쓰기 text가 "push 마음대로"다 / THEN 규칙 문장은 번들 문구를 쓰고 거부 목록은 그대로다 | 자동 | `crates/dduck-core/src/team_config.rs::locked_rules_keep_bundled_text`<br>`crates/dduck-core/src/team_config.rs::local_only_line_is_a_safety_rule` |

### F-SAF-19

**기본 거부 목록** · 구현됨 — deny_rules 기본 목록·감지 기본 브랜치 검증. 실제 CLI 매칭은 tasks 1.1 실측 대기 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-19-01 | 머지 명령 | WHEN 작업이 `gh pr merge 12 --squash`를 실행하려 한다 / THEN 거부 규칙으로 막힌다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::base_rules_cover_merge_ready_default_and_force`<br>수동: [스파이크] 실제 CLI 작업에서 gh pr merge 12 --squash 실행 지시 → 거부 규칙으로 막힘 |
| TC-SAF-19-02 | 초안 PR | WHEN 작업 브랜치에서 `git push -u origin fix-1`과 `gh pr create --draft`를 실행한다 / THEN 둘 다 거부 규칙에 걸리지 않는다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::base_rules_cover_merge_ready_default_and_force`<br>수동: [스파이크] draft_pr 작업 브랜치에서 git push -u origin fix-1과 gh pr create --draft 실행 → 거부되지 않음 |
| TC-SAF-19-03 | 기본 브랜치 push | WHEN 작업이 `git push origin HEAD:refs/heads/main` 또는 `git push --all`을 실행하려 한다 / THEN 막힌다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::base_rules_cover_merge_ready_default_and_force`<br>수동: [스파이크] 실제 CLI 작업에서 git push origin HEAD:refs/heads/main / git push --all 지시 → 막힘 |
| TC-SAF-19-04 | 감지한 기본 브랜치 | WHEN 저장소의 `origin/HEAD`가 `origin/develop`을 가리킨다 / THEN 거부 목록에 `Bash(git push * develop)`이 들어간다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::detected_default_branch_is_validated` |
| TC-SAF-19-05 | 이상한 기본 브랜치 이름 | WHEN 감지한 이름이 `x) Edit(`이다 / THEN 그 이름은 버려지고 `main`·`master` 규칙만 들어간다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::detected_default_branch_is_validated` |

### F-SAF-20

**기본 브랜치에서 시작하면 worktree** · 구현됨 — start_ctx가 기본 브랜치에서 force_worktree, 루틴 등은 bare push 규칙 추가 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-20-01 | main에서 시작 | WHEN 사용자가 worktree를 끈 채 `main`이 체크아웃된 폴더에서 작업을 시작한다 / THEN 작업은 worktree에서 시작되고 Composer에 "기본 브랜치라 worktree에서 해요"가 보였다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::start_ctx_forces_worktree_on_default_and_refuses_bad_list`<br>`src/components/LocalOnlySection.test.tsx::Composer line: forced worktree note, nothing for a relative path` |
| TC-SAF-20-02 | main에서 도는 루틴 | WHEN 루틴이 `main`이 체크아웃된 폴더에서 실행된다 / THEN 거부 목록에 `Bash(git push)`와 `Bash(git push * HEAD*)`가 있다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::start_on_default_adds_bare_push`<br>수동: main이 체크아웃된 폴더로 루틴 '지금 실행' → 작업 인자에 Bash(git push)·Bash(git push * HEAD*) 거부 포함 |

### F-SAF-21

**프로젝트별 "로컬 작업만" 설정** · 구현됨 — 설정 '원격 push 막기' 목록, 체크 즉시 저장, 해제는 네이티브 확인, 일반 setter 차단 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-21-01 | 체크하면 새 작업에 push 거부 | WHEN 사용자가 설정에서 `client`를 체크하고 그 폴더에서 새 작업을 시작한다 / THEN 거부 목록에 `Bash(git push *)`·`Bash(gh *)`가 있고 로컬만 훅이 붙는다 / AND 팀 규칙에 "원격으로 push하거나 PR을 열지 않는다 · 로컬 작업까지만" 문장이 있다 / AND Composer 범위 줄은 "로컬 작업만 · push하지 않아요"다 | 부분 자동 | `src/components/LocalOnlySection.test.tsx::ticking saves at once; unticking only calls the backend (which asks natively)`<br>`crates/dduck-core/src/delivery_guard.rs::local_only_rules_and_hook_args`<br>`crates/dduck-core/src/team_config.rs::local_only_line_is_a_safety_rule`<br>`src/components/LocalOnlySection.test.tsx::copy`<br>수동: 설정에서 client 체크 → 그 폴더로 새 작업 → Composer 줄 '로컬 작업만 · push하지 않아요', 작업 인자에 Bash(git push *)·Bash(gh *)·훅 |
| TC-SAF-21-02 | 해제하면 초안 PR 기본값으로 | WHEN 사용자가 체크를 풀고 네이티브 확인에서 "허용"을 누른 뒤 그 폴더에서 새 작업을 시작한다 / THEN 거부 목록에 `Bash(git push *)`가 없고 훅도 없다 / AND Composer 범위 줄은 "PR 초안까지 · 머지는 사람이 해요"다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::storage_fails_closed_and_round_trips`<br>`src/components/LocalOnlySection.test.tsx::copy`<br>수동: 체크 해제 → 네이티브 확인 '허용' → 같은 폴더 새 작업 → Composer 줄 'PR 초안까지 · 머지는 사람이 해요', 인자에 Bash(git push *)·훅 없음 |
| TC-SAF-21-03 | 해제 확인에서 취소 | WHEN 사용자가 체크를 풀다가 네이티브 확인에서 취소한다 / THEN 체크가 그대로 남는다 | 부분 자동 | `src/components/LocalOnlySection.test.tsx::ticking saves at once; unticking only calls the backend (which asks natively)`<br>수동: 체크 해제 → 네이티브 확인에서 취소 → 체크가 그대로 |
| TC-SAF-21-04 | 일반 setter로 해제 시도 | WHEN 웹뷰가 일반 설정 setter로 `local_only_projects` 키를 빈 값으로 보낸다 / THEN 거부된다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::storage_fails_closed_and_round_trips`<br>`crates/dduck-core/src/settings.rs::generic_setter_rejects_security_keys_and_bad_values` |
| TC-SAF-21-05 | 새 폴더 | WHEN 목록에 없던 `/work/new-client`를 Composer에서 고른다 / THEN 범위 줄은 "PR 초안까지 · 머지는 사람이 해요"이고 "로컬만으로 바꾸기" 버튼이 있다 / AND 버튼을 누르면 그 폴더가 체크되고 줄이 "로컬 작업만 · push하지 않아요"로 바뀐다 | 자동 | `src/components/LocalOnlySection.test.tsx::Composer line: draft PR by default, the tick button switches to local-only` |
| TC-SAF-21-06 | 이미 실행 중인 작업 | WHEN 작업이 실행 중일 때 그 프로젝트를 체크한다 / THEN 실행 중인 작업의 인자는 바뀌지 않고 설정 구역에 "실행 중인 작업은 그대로예요"가 보인다 | 부분 자동 | `src/components/LocalOnlySection.test.tsx::lists projects with their ticks; worktrees fold into the parent; home cannot be ticked`<br>수동: 작업 실행 중 그 프로젝트 체크 → 실행 중 작업의 인자(ps)가 바뀌지 않고 설정 구역에 '실행 중인 작업은 그대로예요' |

### F-SAF-22

**"로컬 작업만" 강제** · 구현됨 — local_only 추가 거부 목록과 셸 파싱 PreToolUse 훅(별칭·셸·전역 옵션), 위험 모드 거부(DANGER_MODES_MEASURED=false) · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-22-01 | 전역 옵션으로 우회 | WHEN `local_only` 작업이 `git -C . push origin fix-1`을 실행하려 한다 / THEN 훅이 막는다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::hook_denies_bypass_forms`<br>`crates/dduck-core/src/delivery_guard.rs::security_probe_cases_are_denied` |
| TC-SAF-22-02 | 셸 안의 push | WHEN `local_only` 작업이 `sh -c 'cd sub && /usr/bin/git push'`를 실행하려 한다 / THEN 훅이 막는다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::hook_denies_bypass_forms`<br>`crates/dduck-core/src/delivery_guard.rs::security_probe_cases_are_denied` |
| TC-SAF-22-03 | 별칭 | WHEN 저장소에 `alias.p = push`가 있고 작업이 `git p`를 실행하려 한다 / THEN 훅이 막는다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::hook_alias_resolution` |
| TC-SAF-22-04 | 해석 불가 | WHEN 명령에 닫히지 않은 따옴표가 있다 / THEN 훅이 막는다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::hook_denies_bypass_forms` |
| TC-SAF-22-05 | 평범한 명령 | WHEN `local_only` 작업이 `git commit -m "정리"`나 `npm test`를 실행한다 / THEN 훅은 결정을 내지 않고 평소 권한 흐름을 따른다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::hook_leaves_local_work_alone` |
| TC-SAF-22-06 | 위험 모드 | WHEN `local_only` 프로젝트에서 bypassPermissions 작업을 시작하려 하고 실측 기록이 없다 / THEN 시작이 거부되고 이유가 한 줄로 보인다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::local_only_refusals`<br>수동: 작업 시키기 창에서 시작을 막는 조건을 만든다 → 하단 문구와 시작 버튼 상태를 본다 → 막힌 이유가 보이고 시작되지 않는다 |

### F-SAF-23

**로컬만 프로젝트와 클라우드 루틴** · 구현됨 — origin slug 비교로 local_only 저장소 클라우드 루틴 차단(목록 못 읽으면 차단) · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-23-01 | 로컬만 저장소의 클라우드 루틴 실행 | WHEN `client` 저장소를 쓰는 클라우드 루틴을 D.Duck에서 실행하려 한다 / THEN 거부되고 "로컬만 프로젝트는 클라우드 루틴을 쓸 수 없어요"가 보인다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::remote_slugs_and_cloud_block`<br>`crates/dduck-core/src/delivery_guard.rs::cloud_guard_fails_closed`<br>`crates/dduck-core/src/delivery_guard.rs::second_clone_matches_by_origin_slug`<br>수동: [스파이크] client 저장소를 쓰는 클라우드 루틴이 있고 client를 로컬만으로 체크 → 그 루틴 켜기 시도 → 거부되고 '로컬만 프로젝트는 클라우드 루틴을 쓸 수 없어요' |

### F-SAF-24

**범위 표시** · 구현됨 — DeliveryGuardLine(시작과 같은 preview 판정), task_meta 범위 기록·보고서 칩 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-24-01 | 보고서 칩 | WHEN `local_only`로 시작한 작업의 보고서를 연다 / THEN "맡긴 내용"에 "로컬만" 칩이 있다 | 자동 | `src/components/TaskReport.test.tsx::shows the delivery scope chip under 맡긴 내용 (delivery-guard)`<br>`crates/dduck-core/src/task_meta.rs::records_the_delivery_scope_with_checked_values` |

### F-SAF-25

**로컬 git 사후 점검** · 구현됨 — post_check(default_moved·push_detected, reflog 시각·메시지만) 후 delivery_alert 저장 · spec: `openspec/specs/delivery-guard/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-25-01 | 기본 브랜치 로컬 커밋 | WHEN 루틴이 `main`에서 시작해 `main`에 커밋했다 / THEN 보고서에 "기본 브랜치에 새 커밋이 생겼어요"가 있다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::post_check_stores_alert_bits`<br>`crates/dduck-core/src/delivery_guard.rs::post_checks_default_moved_and_push_reflog`<br>`src/components/LocalOnlySection.test.tsx::copy` |
| TC-SAF-25-02 | 로컬만 프로젝트에서 push | WHEN `local_only` 작업 시간 안에 `refs/remotes/origin/fix-1` reflog에 "update by push"가 생겼다 / THEN 보고서에 "원격으로 push한 기록이 있어요"가 있고 리뷰 탭 카드에 경고 표시가 있다 | 부분 자동 | `crates/dduck-core/src/delivery_guard.rs::post_check_stores_alert_bits`<br>`src/components/TaskReport.test.tsx::shows the delivery post-check warnings (delivery-guard)`<br>수동: local_only 작업 시간 안에 수동으로 git push origin main:fix-1 → 작업 종료 후 보고서에 '원격으로 push한 기록이 있어요', 리뷰 탭 카드에 경고 표시 |
| TC-SAF-25-03 | 초안 PR 범위의 작업 브랜치 push | WHEN `draft_pr` 작업이 `fix-1`을 push했다 / THEN 경고가 없다 | 자동 | `crates/dduck-core/src/delivery_guard.rs::post_checks_default_moved_and_push_reflog` |

### F-SAF-26

**한도 감지** · 구현됨 — parse::limit_hit(조건·resetsAt 우선순위), limit_hits uuid 중복 방지, 구조화 값만 저장 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-26-01 | 5시간 한도 | WHEN 세션 transcript에 assistant 레코드가 추가된다: `error: "rate_limit"`, `quotaLimits: {status: "rejected", resetsAt: 1794113400, rateLimitType: "five_hour"}` / THEN 그 세션에 한도 사건이 하나 생긴다(리셋 시각 1794113400초, 종류 five_hour) / AND DB·WAL·로그에 메시지 본문 카나리 문자열이 없다 | 자동 | `crates/dduck-core/src/parse.rs::limit_hits_keep_structured_fields_only`<br>`crates/dduck-core/src/limit_resume.rs::nothing_but_ids_and_codes_reach_the_db` |
| TC-SAF-26-02 | 메인과 서브에이전트 기록 | WHEN 같은 세션의 메인 파일과 서브에이전트 파일에 같은 리셋 시각의 한도 레코드가 하나씩 생긴다 / THEN 한도 사건은 하나다 | 자동 | `crates/dduck-core/src/limit_resume.rs::same_reset_across_sessions_notifies_once_with_counts` |
| TC-SAF-26-03 | 같은 레코드 다시 읽기 | WHEN 같은 파일을 처음부터 다시 수집한다 / THEN 레코드 uuid가 같아 기록이 늘지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::same_reset_across_sessions_notifies_once_with_counts` |
| TC-SAF-26-04 | 리셋 시각 없음 | WHEN 한도 레코드에 `quotaLimits`와 `apiErrorParams`가 모두 없다 / THEN "리셋 시각 모름" 사건이 생긴다 / AND 자동 재개 예약은 만들지 않는다 | 자동 | `crates/dduck-core/src/parse.rs::limit_hits_keep_structured_fields_only`<br>`crates/dduck-core/src/limit_resume.rs::interactive_sessions_and_unknown_resets_are_manual` |

### F-SAF-27

**한도 알림** · 구현됨 — sync가 (세션,리셋) 단위 Notice 묶음, 백필 무음, notice_body 문구 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-27-01 | 작업 3개가 동시에 한도 | WHEN 같은 리셋 시각으로 세 세션이 한도에 닿는다 / AND 그중 두 개는 자동 재개 대상이고 하나는 대상이 아니다 / THEN OS 알림 하나만 뜬다: "사용량 한도에 닿았어요 · 오후 1:50 리셋 · 2개는 이어서 할게요 · 1개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요" | 자동 | `crates/dduck-core/src/limit_resume.rs::same_reset_across_sessions_notifies_once_with_counts`<br>`crates/dduck-core/src/limit_resume.rs::texts` |
| TC-SAF-27-02 | 자동 재개를 꺼 둠 | WHEN 자동 재개 설정이 꺼진 상태에서 두 세션이 한도에 닿는다 / THEN 알림은 "사용량 한도에 닿았어요 · 오후 1:50 리셋 · 2개는 직접 이어야 해요"이다 / AND "Mac이 깨어 있어야 이어요"는 없다 | 자동 | `crates/dduck-core/src/limit_resume.rs::texts`<br>`crates/dduck-core/src/limit_resume.rs::flag_off_or_setting_off_makes_everything_manual` |
| TC-SAF-27-03 | 주간 한도 | WHEN 리셋 시각이 사흘 뒤다 / THEN 알림의 리셋 시각이 "11월 12일(목) 오전 9:00" 꼴이다 | 자동 | `crates/dduck-core/src/limit_resume.rs::texts` |
| TC-SAF-27-04 | 처음 켤 때 | WHEN 리셋 시각이 아직 미래인 한도 기록이 있는 transcript를 처음 수집한다 / THEN 알림이 뜨지 않는다 / AND 배너에는 보인다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::notices_need_a_future_reset_and_backfill_is_silent`<br>`crates/dduck-core/tests/limit_flow.rs::first_ingest_is_quiet_and_restarts_never_run_twice`<br>수동: 미래 리셋 한도 레코드가 있는 transcript로 새 데이터 폴더 첫 실행 → OS 알림 없음, 작업 화면 배너에는 보임 |
| TC-SAF-27-05 | 같은 사건 다시 보기 | WHEN 이미 알린 (세션, 리셋 시각)의 레코드가 또 수집된다 / THEN 알림이 다시 뜨지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::same_reset_across_sessions_notifies_once_with_counts`<br>`crates/dduck-core/tests/limit_flow.rs::stop_is_announced_once_waits_and_resumes_once` |

### F-SAF-28

**한도 배너와 카드 표시** · 구현됨 — view 배너 문구·카드 칩(limitChip)·이번엔 안 이어요/지금 이어서 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-28-01 | 섞인 배너 | WHEN 자동 재개 대상 2개와 위험 권한 작업 1개가 한도로 멈췄다 / THEN 배너에 "사용량 한도 · 오후 1:50 리셋 · 2개는 이어서 할게요 · 1개는 직접 이어야 해요 · Mac이 깨어 있어야 이어요"가 보인다 | 자동 | `crates/dduck-core/src/limit_resume.rs::view_banner_counts_and_labels`<br>`src/lib/limits.test.ts::mixed, auto off and late` |
| TC-SAF-28-02 | 자동 재개 꺼짐 배너 | WHEN 자동 재개 설정이 꺼진 상태에서 작업 2개가 한도로 멈췄다 / THEN 배너에 "사용량 한도 · 오후 1:50 리셋 · 2개는 직접 이어야 해요"가 보인다 / AND 두 카드에 직접 잇기 버튼이 있다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::view_banner_counts_and_labels`<br>`src/lib/limits.test.ts::mixed, auto off and late`<br>수동: 자동 재개 끈 상태에서 작업 2개 한도 레코드 주입 → 배너 '… 2개는 직접 이어야 해요', 두 카드에 직접 잇기 버튼 |

### F-SAF-29

**자동 재개 대상** · 보류 — eligibility 규칙 구현·테스트됨, AUTO_RESUME_SHIPPED=false라 실제 자동 재개 예약은 not_shipped로 모두 직접 잇기 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-29-01 | 사용자가 멈춘 작업 | WHEN 한도 대기 중인 작업을 사용자가 D.Duck에서 멈춘다 / THEN 그 작업의 예약이 취소되고 리셋 후에도 이어지지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::stopped_already_continued_and_interrupted` |
| TC-SAF-29-02 | 이미 직접 이어서 함 | WHEN 리셋 전에 그 세션에서 한도 시각 뒤의 정상 응답이 수집된다 / THEN 예약이 "이미 이어서 했어요"로 닫히고 재개하지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::stopped_already_continued_and_interrupted` |
| TC-SAF-29-03 | 체크 없이 시작한 위험 권한 작업 | WHEN bypassPermissions로 시작하면서 확인 창의 "리셋 후 자동으로 이어서 하기"를 체크하지 않은 작업이 한도에 닿는다 / THEN 자동 재개 예약이 없다 / AND 카드에 "지금 이어서" 버튼이 있고, 누르면 네이티브 확인 창을 거친 뒤에만 이어진다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::eligibility_rules`<br>`src/lib/limits.test.ts::waiting, manual and resumed`<br>`crates/dduck-core/src/claude_cli.rs::resume_args_carry_the_start_values_and_nothing_else`<br>수동: bypass 작업(체크 안 함) 한도 상태 → 카드 '지금 이어서' → 네이티브 확인 창 뒤에만 재개 |
| TC-SAF-29-04 | 체크하고 시작한 auto 작업 | WHEN auto 모드 확인 창에서 체크하고 시작한 작업이 한도에 닿는다 / THEN 다른 조건을 만족하면 자동 재개 예약이 생긴다 | 자동 | `crates/dduck-core/src/limit_resume.rs::eligibility_rules` |
| TC-SAF-29-05 | 시작 뒤 페르소나 위험이 생김 | WHEN 대기 중에 그 작업 페르소나의 정의에 `permissionMode: bypassPermissions`가 추가된다 / THEN 리셋 후 자동으로 잇지 않는다 / AND 카드가 직접 잇기로 바뀐다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::risk_rechecks`<br>수동: [스파이크] 대기 중 페르소나 정의에 permissionMode: bypassPermissions 추가 → 리셋 후 자동 재개 없음, 카드가 직접 잇기로 바뀜 |
| TC-SAF-29-06 | 배포 작업 | WHEN 배포 유형 작업이 한도에 닿는다 / THEN 확인 창 체크와 관계없이 자동 재개 예약이 없다 | 자동 | `crates/dduck-core/src/limit_resume.rs::eligibility_rules`<br>`src/lib/limits.test.ts::never offers to run terminal sessions or deploy tasks` |
| TC-SAF-29-07 | 터미널 세션 | WHEN VS Code나 터미널의 대화형 세션이 한도에 닿는다 / THEN 알림과 배너만 보이고 D.Duck은 그 세션을 실행하지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::interactive_sessions_and_unknown_resets_are_manual`<br>`src/lib/limits.test.ts::never offers to run terminal sessions or deploy tasks` |
| TC-SAF-29-08 | 횟수 상한 | WHEN 같은 작업이 자동 재개 뒤 세 번째로 다시 한도에 닿는다. 재개로 bg id가 바뀌었어도 마찬가지다. / THEN 더는 예약하지 않는다 / AND "자동으로 3번 이어서 멈췄어요" 알림과 카드 표시가 남는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::cap_counts_across_aliases_and_same_reset_waits`<br>`src/lib/limits.test.ts::waiting, manual and resumed` |

### F-SAF-30

**재개 실행** · 보류 — 지연·간격·동시 3개·재개 인자 구현, 자동 경로는 AUTO_RESUME_SHIPPED=false로 꺼짐(tasks 1.1·5.2 실측 대기, 2.8 --model 미완) · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-30-01 | 리셋 후 재개 | WHEN 리셋 시각에서 2분이 지났고, 대상 작업이 멈춰 있다 / THEN CLI가 한 번 실행된다. 인자는 `--bg`, `--resume=<세션 uuid>`, `--permission-mode=<원래 모드>`, 원래 팀 규칙·보고서 지시·`--allowedTools`, 고정 문구다. `--worktree=`와 `--remote-control=`은 없다. / AND 작업 카드가 같은 세션으로 다시 진행 중이 된다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::due_time_is_jittered_between_60_and_180_seconds`<br>`crates/dduck-core/src/limit_resume.rs::scheduler_claims_after_due_with_gap_and_limit`<br>`crates/dduck-core/src/claude_cli.rs::resume_args_carry_the_start_values_and_nothing_else`<br>`crates/dduck-core/tests/limit_flow.rs::stop_is_announced_once_waits_and_resumes_once`<br>수동: [스파이크] 실제 한도 도달 후 리셋 2분 뒤 → CLI 1회 재개, 카드가 같은 세션으로 다시 진행 중 |
| TC-SAF-30-02 | bg id가 바뀜 | WHEN 재개 출력의 bg id가 원래 id와 다르다 / THEN 새 id가 같은 작업의 별칭으로 기록된다 / AND 카드의 유형·보고서·누적 횟수가 그대로 보인다 | 부분 자동 | `crates/dduck-core/src/task_meta.rs::limit_fields_session_links_and_aliases`<br>`crates/dduck-core/src/limit_resume.rs::cap_counts_across_aliases_and_same_reset_waits`<br>수동: [스파이크] 재개 결과 bg id가 바뀐 경우 → 카드의 유형·보고서·누적 횟수가 그대로 |
| TC-SAF-30-03 | 사본이 시작됨 | WHEN 재개 출력이 사본 세션 시작을 알린다 / THEN 그 사본이 바로 멈춰진다 / AND 예약은 "못 이었어요"로 닫힌다 | 부분 자동 | `crates/dduck-core/src/claude_cli.rs::resume_output_parsing`<br>`crates/dduck-core/tests/claude_cli_fake.rs::resume_runs_in_the_session_folder_and_reports_copies_and_failures`<br>수동: [스파이크] 'started a copy'를 출력하는 가짜 CLI로 직접 이어서 → 사본이 곧 멈추고 예약이 '못 이었어요' |
| TC-SAF-30-04 | 재개 실패 | WHEN 재개 실행이 실패하거나 신뢰되지 않은 폴더 오류가 난다 / THEN 예약은 "못 이었어요"(고정 코드)로 닫힌다 / AND 신뢰 확인 창을 띄우거나 다시 시도하지 않는다 / AND 카드에 직접 잇기 버튼이 보인다 | 부분 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::resume_runs_in_the_session_folder_and_reports_copies_and_failures`<br>`src/lib/limits.test.ts::waiting, manual and resumed`<br>수동: untrusted를 내는 가짜 CLI로 직접 이어서 → '못 이었어요'로 닫히고 신뢰 창 없음, 카드에 직접 잇기 버튼 |

### F-SAF-31

**재개 폴더 검증** · 구현됨 — verify_cwd(정규화·worktree 위치·링크·신뢰)로 직접·자동 재개 모두 검사 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-31-01 | worktree가 아닌 곳 | WHEN worktree로 시작한 작업의 현재 cwd가 `.claude/worktrees/` 밖이다 / THEN 재개하지 않는다 / AND 예약은 고정 코드 `cwd_mismatch`로 닫힌다 | 자동 | `crates/dduck-core/src/limit_resume.rs::folder_checks` |
| TC-SAF-31-02 | 신뢰 미확인 | WHEN 폴더의 신뢰 여부를 알 수 없다 / THEN 재개하지 않는다 / AND 고정 코드 `untrusted`로 닫힌다 | 자동 | `crates/dduck-core/src/limit_resume.rs::folder_checks` |

### F-SAF-32

**세션 멈추기 조건** · 일부 구현 — 재수집 후 이미 이어서 함 처리는 있으나 attach 여부를 못 읽어(스파이크 1.1) 살아 있는 세션은 모두 Attached로 멈추지 않음 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-32-01 | 사용자가 보고 있는 세션 | WHEN 한도로 멈춘 bg 세션에 사용자가 터미널로 attach해 있다 / THEN D.Duck은 그 세션을 멈추지도 재개하지도 않는다 / AND 예약은 "확인 필요"가 된다 | 수동 | 수동: [스파이크] 한도로 멈춘 bg 세션에 터미널로 claude attach → 리셋 후 D.Duck이 멈추거나 재개하지 않고 예약이 '확인 필요' |

### F-SAF-33

**재시작·늦은 재개** · 보류 — DB 예약 유지·2시간 늦음 needs_choice·3일 만료·interrupted 처리 구현, 자동 재개 자체는 플래그 off · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-33-01 | 밤새 꺼져 있던 앱 | WHEN 오전 1시 리셋 예약이 있고 앱을 오전 9시에 켠다 / THEN 자동으로 잇지 않는다 / AND "앱이 꺼져 있어 리셋 때 못 이었어요" 배너에 두 버튼이 보인다 | 자동 | `crates/dduck-core/src/limit_resume.rs::late_wake_up_asks_and_old_rows_expire`<br>`crates/dduck-core/tests/limit_flow.rs::late_wake_up_asks_and_unshipped_flag_runs_nothing`<br>`src/lib/limits.test.ts::never offers to run terminal sessions or deploy tasks` |
| TC-SAF-33-02 | 잠깐 꺼져 있던 앱 | WHEN 리셋 30분 뒤 앱을 다시 켠다 / THEN 지연 규칙에 따라 자동으로 이어진다 | 자동 | `crates/dduck-core/src/limit_resume.rs::late_wake_up_asks_and_old_rows_expire` |

### F-SAF-34

**설정·확인 창·비용 안내** · 구현됨 — 설정 두 토글(기본 켬)·설명 문구, 끄면 auto 행을 auto_off 직접 잇기로 전환, Composer 체크 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-34-01 | 전역 끄기 | WHEN 대기 예약 2개가 있는 상태에서 자동 재개를 끈다 / THEN 리셋 후에도 실행되지 않는다 / AND 두 카드에 직접 잇기 버튼이 보인다 | 부분 자동 | `crates/dduck-core/src/limit_resume.rs::flag_off_or_setting_off_makes_everything_manual`<br>`crates/dduck-core/src/settings.rs::limit_settings_default_on_and_backfill_flag_is_not_user_set`<br>수동: 대기 예약 2개 상태에서 설정 자동 재개 끄기 → 리셋 후 실행 없음, 두 카드에 직접 잇기 버튼 |

### F-SAF-35

**단계적 출시** · 구현됨 — AUTO_RESUME_SHIPPED=false 컴파일 플래그로 자동 재개 차단, 문구도 직접 잇기만 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-35-01 | 스파이크 전 | WHEN 활성화 플래그가 꺼진 빌드에서 작업이 한도에 닿는다 / THEN 알림과 배너는 "직접 이어야 해요"로만 보인다 / AND 리셋 후에도 CLI가 실행되지 않는다 | 자동 | `crates/dduck-core/src/limit_resume.rs::flag_off_or_setting_off_makes_everything_manual`<br>`crates/dduck-core/tests/limit_flow.rs::late_wake_up_asks_and_unshipped_flag_runs_nothing`<br>`crates/dduck-core/src/limit_resume.rs::eligibility_rules` |

### F-SAF-36

**세션 id 인자 검증** · 구현됨 — check_id('-' 시작 거부)·세션 id 소문자 UUID 검사 · spec: `openspec/specs/usage-limit-resume/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-SAF-36-01 | 옵션처럼 보이는 id | WHEN bg id `-h`로 멈추기나 로그를 요청한다 / THEN 잘못된 세션 ID 오류가 나고 CLI가 실행되지 않는다 | 자동 | `crates/dduck-core/tests/claude_cli_fake.rs::ids_starting_with_a_dash_never_reach_the_cli`<br>`crates/dduck-core/src/claude_cli.rs::id_validation` |

## MOB · 모바일 연동

### F-MOB-01

**휴대폰 보기 서버** · 미구현 — mobile-companion tasks 전부 미체크, 서버·페어링·mobile 모듈 코드 없음 · spec: `openspec/changes/mobile-companion/specs/mobile-companion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-MOB-01-01 | 켜고 끄기 | WHEN 사용자가 설정에서 "휴대폰에서 보기"를 켠다 / THEN `127.0.0.1:<포트>`에서 서버가 응답하고, 끄면 같은 주소로 연결할 수 없다 | 수동 | 수동: (구현 후) 설정 '휴대폰에서 보기' 켜기 → curl 127.0.0.1:<포트> 응답 → 끄기 → 연결 거부 |
| TC-MOB-01-02 | 외부 주소 | WHEN 같은 Wi-Fi의 다른 기기가 Mac의 LAN 주소와 그 포트로 접속한다 / THEN 연결되지 않는다 | 수동 | 수동: (구현 후) 같은 Wi-Fi 다른 기기에서 Mac LAN IP:<포트> 접속 → 연결 안 됨 |

### F-MOB-02

**기기 연결** · 미구현 — mobile-companion tasks 전부 미체크, 서버·페어링·mobile 모듈 코드 없음 · spec: `openspec/changes/mobile-companion/specs/mobile-companion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-MOB-02-01 | 휴대폰 연결 | WHEN 사용자가 Mac에 표시된 코드를 휴대폰 웹앱에 입력한다 / THEN 휴대폰이 홈 화면을 보고, Mac 설정의 연결된 기기에 그 휴대폰이 나타난다 | 수동 | 수동: (구현 후) Mac 설정의 6자리 코드를 휴대폰 웹앱에 입력 → 휴대폰 홈 표시, Mac 연결 기기 목록에 등장 |
| TC-MOB-02-02 | 기기 끊기 | WHEN 사용자가 Mac에서 그 기기를 끊는다 / THEN 휴대폰의 다음 요청이 401을 받고 다시 연결 화면이 보인다 | 수동 | 수동: (구현 후) Mac에서 그 기기 끊기 → 휴대폰 새로고침 → 401과 연결 화면 |
| TC-MOB-02-03 | 코드 추측 | WHEN 틀린 코드가 5번 입력된다 / THEN 그 코드는 더 쓸 수 없고 Mac에서 새 코드를 만들어야 한다 | 수동 | 수동: (구현 후) 틀린 코드 5번 입력 → 맞는 코드도 거부, Mac에서 새 코드 필요 |

### F-MOB-03

**모바일 웹앱** · 미구현 — mobile-companion tasks 전부 미체크, 서버·페어링·mobile 모듈 코드 없음 · spec: `openspec/changes/mobile-companion/specs/mobile-companion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-MOB-03-01 | 막힌 작업 확인 | WHEN Mac에서 작업 2개가 답을 기다린다 / THEN 휴대폰 홈 맨 위에 "막힌 작업 2개"와 그 작업 이름이 보인다 | 수동 | 수동: (구현 후) Mac에서 답 대기 작업 2개 → 휴대폰 홈 맨 위 '막힌 작업 2개'와 이름 |
| TC-MOB-03-02 | Mac 꺼짐 | WHEN Mac이 잠들어 응답하지 않는다 / THEN 휴대폰은 마지막 화면을 유지하고 "Mac에 연결할 수 없습니다 · 마지막 <시각>"을 보인다 | 수동 | 수동: (구현 후) Mac 잠자기 → 휴대폰 화면 유지, 'Mac에 연결할 수 없어요 · 마지막 <시각>' 표시 |

### F-MOB-04

**응답 내용 제한** · 미구현 — mobile-companion tasks 전부 미체크, 서버·페어링·mobile 모듈 코드 없음 · spec: `openspec/changes/mobile-companion/specs/mobile-companion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-MOB-04-01 | 루틴 지시 | WHEN 휴대폰이 루틴 목록을 받는다 / THEN 응답에 루틴 지시문은 없고 이름·일정·다음 실행·마지막 결과만 있다 | 수동 | 수동: (구현 후) 토큰으로 루틴 API GET → 응답 JSON에 지시문 없이 이름·일정·다음 실행·마지막 결과만 |

### F-MOB-05

**Tailscale 연결 안내** · 미구현 — mobile-companion tasks 전부 미체크, 서버·페어링·mobile 모듈 코드 없음 · spec: `openspec/changes/mobile-companion/specs/mobile-companion/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-MOB-05-01 | Tailscale 없음 | WHEN Tailscale이 설치되지 않은 Mac에서 휴대폰 영역을 연다 / THEN 설치·로그인 단계 안내가 보이고 휴대폰 주소는 표시되지 않는다 | 수동 | 수동: (구현 후) Tailscale 없는 Mac에서 설정 휴대폰 영역 열기 → 설치·로그인 안내만, 휴대폰 주소 없음 |

## TUR · 둘러보기

### F-TUR-01

**첫 실행 안내** · 일부 구현 — tour-v2(D-133)로 카드 구성 대체: 환영 화면 없이 첫 카드가 바로 열리고 CLI 상태는 마지막 카드에 · spec: `openspec/specs/onboarding-tour/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-01-01 | 처음 켜기 | WHEN 사용자가 설치 후 처음 앱을 켠다 / THEN 환영 화면이 뜨고 "둘러보기 시작"을 누르면 첫 카드(에이전트)가 보이며, 카드를 클릭할 때마다 다음 기능 카드로 넘어간다 | 부분 자동 | `src/components/TourDeck.test.tsx::opens on a fresh install with n/5, flips by click and keys, and Esc closes as dismissed`<br>`src/components/TourDeck.test.tsx::does not open when everything was seen`<br>수동: 앱 데이터 없는 새 설치로 실행 → 투어가 자동으로 열리고 클릭마다 다음 카드(v2: 첫 카드 '세션 하나에…'), 끝낸 뒤 재실행 시 자동으로 안 뜸 |

### F-TUR-02

**카드 넘기기 튜토리얼** · 일부 구현 — 카드 넘기기·이전·건너뛰기·점은 v2에 있음. 카드별 '이 화면 열기'는 v2에서 빠지고 행동 카드 링크로 대체 · spec: `openspec/specs/onboarding-tour/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-02-01 | 건너뛰기 | WHEN 사용자가 3번째 카드에서 Esc를 누른다 / THEN 튜토리얼이 닫히고 원래 보던 화면으로 돌아오며 설정에서 다시 볼 수 있다 | 자동 | `src/components/TourDeck.test.tsx::opens on a fresh install with n/5, flips by click and keys, and Esc closes as dismissed`<br>`src/components/TourDeck.test.tsx::restores focus to the replay opener even though the shell blurs it when it turns inert`<br>`crates/dduck-core/src/settings.rs::onboarding_state_roundtrip_and_validation` |

### F-TUR-03

**시작하기 체크리스트** · 구현됨 — StartChecklist가 markDone으로 자동 체크·접기·완료 시 숨김(항목은 v2 기준) · spec: `openspec/specs/onboarding-tour/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-03-01 | 첫 작업 맡기기 | WHEN 사용자가 작업 탭에서 새 작업을 시작한다 / THEN 체크리스트의 "첫 작업 맡기기"가 체크된다 | 부분 자동 | `src/components/TourDeck.test.tsx::lists the v2 items, ticks them as they happen and disappears when all are done`<br>수동: 투어를 본 상태에서 작업 탭 → 새 작업 시작 성공 → 체크리스트 '첫 작업 맡기기'가 체크됨 |

### F-TUR-04

**다시 보기와 새 단계** · 구현됨 — 설정 '튜토리얼 다시 보기', pendingCards가 새 카드만 반환 · spec: `openspec/specs/onboarding-tour/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-04-01 | 새 탭 안내 | WHEN 둘러보기를 끝낸 사용자가 메모리 탭이 추가된 버전으로 업데이트한다 / THEN 다음 실행 때 메모리 탭 단계 하나만 안내된다 | 부분 자동 | `src/lib/tour.test.ts::fresh install shows every card; an update without new cards shows none; a new card shows only itself`<br>`crates/dduck-core/src/settings.rs::onboarding_version_bump_leaves_only_new_cards`<br>수동: 투어 끝낸 상태 DB에서 seen 목록에서 카드 하나 빼고 앱 실행 → 그 카드 하나만 안내됨 |

### F-TUR-05

**역할을 가진 오리 이야기로 된 투어** · 구현됨 — TOUR_CARDS 이야기 5장+행동 1장, 클릭·키 넘기기 · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-05-01 | 처음 실행 | WHEN 새로 설치한 사용자가 앱을 연다 / THEN 첫 카드에 "세션 하나에 다 시키고 있나요?"와 진행 "1/5"가 보이고, 넘기면 역할 → 맡기기·다듬기 → 진행 보드 → 보고서 → 행동 카드 순서로 이어진다 | 자동 | `src/components/TourDeck.test.tsx::opens on a fresh install with n/5, flips by click and keys, and Esc closes as dismissed`<br>`src/lib/tour.test.ts::is five story cards and the action card, ids matching the Rust allowlist` |
| TC-TUR-05-02 | 넘기기와 닫기 | WHEN 사용자가 카드(버튼·점·링크·선택한 글자 제외)를 누르거나 →·Space를 누른다 / THEN 다음 카드로 넘어가고, ←는 이전 카드, Esc는 덱을 닫아 '건너뜀'으로 저장한다. 건너뛰기는 행동 카드 전까지 모든 카드에 있다 | 자동 | `src/components/TourDeck.test.tsx::opens on a fresh install with n/5, flips by click and keys, and Esc closes as dismissed`<br>`src/components/TourDeck.test.tsx::button, dot and selected-text clicks do not advance`<br>`src/components/TourDeck.test.tsx::skip is there on every card but the last` |

### F-TUR-06

**진행 표시가 한 가지 기준을 쓴다** · 구현됨 — data-progress story 5개·action 1개, aria-label 기준 통일 · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-06-01 | 마지막 이야기 카드에서 행동 카드로 | WHEN 사용자가 5/5 카드에서 다음으로 넘긴다 / THEN 진행 글자는 "직접 해 보기"로 바뀌고, 다섯 점은 모두 현재 표시가 꺼지며 화살표 모양 행동 표시가 현재 표시를 받는다 | 자동 | `src/components/TourDeck.test.tsx::progress text and dots agree: five story dots n/5 and a separate marker for the action card` |

### F-TUR-07

**오리는 항상 역할 라벨과 함께** · 구현됨 — TourDemos RoleDuck+역할 칩, roleColors · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-07-01 | 카드 2 역할 오리 | WHEN 사용자가 2번째 카드를 본다 / THEN 설계·디자인·개발·리뷰·QA 오리 5마리가 깨어 있는 포즈(일하는 중, 눈 뜸, 56px 이상)로 각자 역할 칩과 일하는 방식 칩 1개를 달고 있다: 설계 "계획부터 · 깊게", 디자인 "화면 · 접근성", 개발 "격리된 곳에서 코드 고치기", 리뷰 "읽기 위주 · 다른 눈", QA "실행해서 확인" / AND 이 칩은 한곳(tour.ts `ROLE_STYLE`)에서 오고, 실제 권한 프리셋(개발=계획만 세우기 등 안전 기본값)은 바꾸지 않으며 카드에 권한·노력 값으로 보이지 않는다 | 자동 | `src/components/TourDemos.test.tsx::card 2 shows the five role ducks awake in their fixed colours, each with how that role works`<br>`src/components/TourDemos.test.tsx::card 2 ducks are awake (not idle, eyes open) and large enough for the role item to read`<br>`src/components/TourDemos.test.tsx::every duck has a role label next to it (never a mascot)` |
| TC-TUR-07-02 | 마지막 카드 오리 | WHEN 사용자가 행동 카드를 본다 / THEN 손을 흔드는 오리 한 마리가 "역할 고르기 전" 라벨과 함께 다섯 유형 칩(설계·디자인·개발·리뷰·QA) 옆에 있다 | 자동 | `src/components/TourDemos.test.tsx::the action card has a waving duck that has no role yet`<br>`src/components/TourDemos.test.tsx::cards 3 and 6 offer the same five types as card 2's roles` |

### F-TUR-08

**실제 화면 조각으로 된 데모** · 구현됨 — 표시 전용 데모 조각, inert·aria-hidden·텍스트 대안 · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-08-01 | 카드 3 질문 하나가 요청서를 채운다 | WHEN 사용자가 3번째 카드를 본다 / THEN 카드2와 같은 다섯 유형 칩 중 개발이 골라져 있고 그 아래 "개발 담당 · 격리된 곳에서 코드 고치기"(카드2와 같은 설명), 추천이 골라진 질문 카드 1장, 그 답이 채운 요청서 줄 "완료 기준: 같은 테스트 20번 연속 통과"가 보인다 | 자동 | `src/components/TourDemos.test.tsx::card 3 shows one question card; the picked answer fills the brief's 완료 기준 line, and the role line matches card 2`<br>`src/components/TourDemos.test.tsx::cards 3 and 6 offer the same five types as card 2's roles` |
| TC-TUR-08-02 | 카드 4 진행 보드 | WHEN 사용자가 4번째 카드를 본다 / THEN 개발(일하는 중)·리뷰(답을 기다려요)·QA(끝남) 오리 줄 3개가 보이고, 답을 기다리는 줄만 노랑 테두리와 왼쪽 3px 굵은 선(모서리 안쪽)으로 강조된다 | 자동 | `src/components/TourDemos.test.tsx::card 4 tells who is doing what and who waits for an answer, the waiting row with the board's yellow ring` |
| TC-TUR-08-03 | 카드 5 보고서 순서 | WHEN 사용자가 5번째 카드를 본다 / THEN 개발 오리(done)와 역할 칩, "기준 3개 중 2개 충족" 아래로 맡긴 내용(합의) 줄 → 완료 기준 대조 3줄(✓ 2개, △ 1개) → 주의 칩 "묻지 않고 가정" 1개 → 리뷰 오리 판정 줄 "다른 눈으로 본 판정: 통과 · 1개 확인 필요" 순서로 보인다 | 자동 | `src/components/TourDemos.test.tsx::card 5 reads in the report's order: 맡긴 내용, then the criteria (2 ✓ and 1 △), then the caution` |

### F-TUR-09

**첫 작업으로 이어지되 자동 실행하지 않음** · 구현됨 — 행동 카드 → tourDraft(빈 목표·예시 칩·다듬기 안내)로 Composer, 자동 실행 없음 · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-09-01 | 첫 작업 맡겨 보기 | WHEN 사용자가 "첫 작업 맡겨 보기"를 누른다 / THEN 투어가 '끝냄'으로 닫히고 목표가 비어 있는 새 작업 폼에 예시 칩과 강조된 다듬기 안내가 보이며, 작업 시작·다듬기 실행 요청은 없다 | 자동 | `src/components/TourDeck.test.tsx::the last card opens the composer and starts nothing; setup links navigate`<br>`src/lib/tour.test.ts::the action card's draft has an empty goal, example chips and the refine hint, and no origin` |
| TC-TUR-09-02 | 구성 링크 | WHEN 사용자가 행동 카드의 "권한" 링크를 누른다 / THEN 투어가 닫히고 권한 탭이 열린다 | 자동 | `src/components/TourDeck.test.tsx::the last card opens the composer and starts nothing; setup links navigate`<br>`src/components/TourDeck.test.tsx::the action card keeps the setup links to one quiet line under the text` |

### F-TUR-10

**카드 레이아웃** · 구현됨 — 고정 높이 카드·바 고정. 코드는 h-[480px](스펙 540px와 다름, 이후 조정) · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-10-01 | 1440·900px | WHEN 1440px(라이트·다크)와 900px에서 각 카드를 본다 / THEN 여섯 장 모두 아래 바와 다음 버튼의 위치가 같고, 문장·데모는 세로 가운데에 있으며 가로 스크롤이 없다 | 부분 자동 | `src/components/TourDeck.test.tsx::every card has the same fixed height with the bottom bar pinned to its foot (다음 does not move)`<br>수동: 1440px(라이트·다크)·900px 창에서 투어 6장 넘기기 → 다음 버튼 위치 동일, 문장·데모 세로 가운데, 가로 스크롤 없음 |

### F-TUR-11

**버전과 기존 사용자** · 구현됨 — TOUR_VERSION=2, v1 상태 마이그레이션과 1회 토스트 · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-11-01 | 버전 1 사용자 | WHEN 버전 1 투어를 본 사용자가 업데이트 후 앱을 연다 / THEN 덱은 열리지 않고 토스트가 한 번 보이며, "새로워진 안내 보기"를 누르면 v2 전체 덱이 열리고 다음 실행에는 토스트도 덱도 없다 | 자동 | `src/components/TourDeck.test.tsx::a version 1 user gets a toast once instead of the deck; the toast opens the full deck`<br>`src/lib/tour.test.ts::a version 1 user gets a one-time toast, never the deck by itself`<br>`crates/dduck-core/src/settings.rs::onboarding_v1_state_migrates_and_gets_one_toast` |
| TC-TUR-11-02 | 다시 보기 | WHEN 사용자가 설정의 "튜토리얼 다시 보기"를 누른다 / THEN v2 전체 덱(이야기 5장 + 행동 카드)이 열리고, 닫으면 포커스가 그 버튼으로 돌아온다 | 부분 자동 | `src/components/TourDeck.test.tsx::restores focus to the replay opener even though the shell blurs it when it turns inert`<br>수동: 설정 → '튜토리얼 다시 보기' 클릭 → 6장 덱이 열리고 Esc로 닫으면 포커스가 그 버튼으로 돌아옴 |

### F-TUR-12

**시작 체크리스트** · 구현됨 — 체크리스트 v2 세 항목(firstTask·refineUsed·reportOpened) · spec: `openspec/specs/tour-v2/spec.md`

| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |
|---|---|---|---|---|
| TC-TUR-12-01 | 체크리스트 완료 | WHEN 사용자가 세 가지를 모두 한다 / THEN 항목이 차례로 체크되고 체크리스트가 사라진다 | 자동 | `src/components/TourDeck.test.tsx::lists the v2 items, ticks them as they happen and disappears when all are done`<br>`src/lib/tour.test.ts::dismissing still counts every card as seen, keeps the checklist; the list is the v2 one` |
