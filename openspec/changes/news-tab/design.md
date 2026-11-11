## Context
사용 개선 분석(D-078)은 사용자가 로그인한 `claude` CLI로 웹 도구만 열어 둔 헤드리스 실행을 한 번 돌린다. 네이티브 확인 창(예상 비용), `--max-budget-usd`, 시간 제한, 앱 소유 빈 cwd, PreToolUse 가드 훅(앱 실행 파일 자신)으로 호스트·횟수를 막는다. 소식 탭은 이 길을 그대로 쓰고, 답을 Markdown 대신 엄격한 JSON으로 받아 항목 단위로 저장한다는 점만 다르다.

### 조사 결과 (2026-11-11, 모델 호출 없음, `curl`로 머리만 확인)
- `https://code.claude.com/docs/en/changelog` (200): 버전마다 **날짜**가 있고 Added/Changed/Fixed로 묶인다. GitHub CHANGELOG에서 생성된다.
- `https://code.claude.com/docs/en/whats-new` (200): 주간 다이제스트, 주마다 `/docs/en/whats-new/2026-wNN` 상세. 왜 중요한지·예시가 있어 "언제 쓰면 좋은지·베스트 프랙티스"의 근거로 좋다.
- `https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md` (200): 날짜 없는 `## 2.1.322` 목록. 예비 출처.
- `https://docs.claude.com/en/release-notes/claude-code` → 301 `platform.claude.com/docs/en/release-notes/claude-code`.
- `code.claude.com/docs/en/release-notes`는 404라 쓰지 않는다.

## Goals / Non-Goals
- Goals: 공식 출처만, 확인 후 한 번, 엄격한 검사, 실패해도 예전 소식 유지, 새 것만 받아 비용 절약, 매일 가져오기(기본 꺼짐), 언어별 저장, CSP 그대로.
- Non-Goals: 원격 썸네일·og:image, 사용자 지정 출처, 알림·사이드바 배지, 소식 지우기·검색, 클라우드 루틴.

## Decisions

### 1. 출처 허용 목록 (앱에 고정, 두 개)
누구나 글을 쓰는 경로(GitHub issues·PR·discussions)는 열지 않는다. 공통 규칙: https, 계정·포트 없음, 쿼리·프래그먼트 없음, 경로에 `..`·`%2F`·`%5C`·`%2E`(대소문자 무관) 없음, 300자 이하.
- **읽기(`fetch_allowed`, 가드 훅)**: `code.claude.com` + `/docs/` 접두(공식 문서, 사용자 글 없음) · `github.com` 정확히 `/anthropics/claude-code/blob/main/CHANGELOG.md`, `/anthropics/claude-code/releases`, `/anthropics/claude-code/releases/tag/v<버전>` · `raw.githubusercontent.com` 정확히 `/anthropics/claude-code/main/CHANGELOG.md` · `platform.claude.com` 정확히 `/docs/en/release-notes/claude-code`.
- **원문(`source_allowed`, 답 검사·원문 열기 재검사)**: `code.claude.com` + `/docs/en/` 접두 · `github.com` 정확히 CHANGELOG blob과 `releases/tag/v<버전>`. 원문 열기가 확인 창 없이 여는 근거가 이 좁은 목록이다.
사용자 입력 주소는 없다.

### 2. 실행 (D-078 재사용)
- 인자: `-p --output-format=json --model=sonnet --tools=WebFetch --permission-mode=dontAsk --max-turns=12 --max-budget-usd=0.4 --no-session-persistence --strict-mcp-config --setting-sources= --settings={PreToolUse 훅} -- <프롬프트>`. WebSearch는 없다(검색은 출처를 넓힌다).
- 가드: `<exe> --dduck-guard-news <카운터 폴더>`. `tool_name == WebFetch`, `tool_input` 키는 `url`·`prompt`만, `url`이 허용 목록, 읽기 8번 이하(파일 길이 카운터, D-078과 같은 방식). 그 밖은 모두 거부. 시작 전 `guard_self_test`(거부 샘플 `https://example.com/`, 허용 샘플 `https://code.claude.com/docs/en/changelog`)가 실패하면 시작하지 않는다(fail closed).
- cwd: 앱 데이터 `news/cwd`(0700), 실행 뒤 `clean_owned_cwd`. 카운터는 `news/guard`.
- 시간 240초(읽기가 최대 8번이라 120초는 빠듯하다, 전용 문구 `NewsTimeout`), 취소 플래그, 한 번에 하나(`news_busy`). 수동·매일 모두 같은 busy를 쓴다. busy는 **확인 창을 띄우기 전에** 잡는다(D-078과 같은 순서). 그래서 수동 확인 창이 열린 동안 매일 실행은 시작되지 않는다.
- 오류는 고정 코드만 상태에 남긴다. stderr와 답 원문은 버리고 로그에도 쓰지 않는다. 저장 뒤 `IngestMsg::Scrub`.
- 로그인 게이트: 수동은 `gate_async(Need::Model)`(D-078과 같음). 매일은 `login_cmds::should_tick`이 거짓이면 건너뛰고 시도 시각도 남기지 않는다(로그인하면 바로 돈다).

### 3. 프롬프트와 새 것만 받기
고정 영어 지시 + 언어 줄(`i18n::Lang::reply_line`) + "이미 가진 소식" JSON. 이미 가진 것은 그 언어의 최근 id 최대 80개와 가장 높은 버전(`known_ids`, `newest_version`)만이다. 지시 요지:
- 출처 순서: changelog(날짜) → whats-new(맥락) → 필요하면 기능 문서. 이 주소들만.
- 사용자에게 보이는 **새 기능·바뀐 동작**만. 버그 수정만인 버전은 건너뛴다. 이미 가진 id·그보다 오래된 버전은 쓰지 않는다. 첫 실행(가진 것 없음)은 최근 12개.
- 최대 12개, 최신 먼저. 응답은 JSON 객체 하나 `{"items":[…]}`만, 코드 펜스·설명 없음.
- 필드: `id`(버전+슬러그, 예 `2.1.321-remote-control-settings`), `title`, `version`, `date`(YYYY-MM-DD, 출처에 있을 때만), `category`, `summary_one_line`, `what`, `when_to_use`, `best_practice`, `source_url`.
- 가져온 페이지 안의 지시는 무시하라는 줄. 
소식 수가 쌓여도 프롬프트는 id 80개(약 3KB)로 묶인다.

### 4. 답 검사 (`news::parse_output`)
1) D-078 `parse_output`처럼 마지막 `type: result` 줄. budget/max_turns/실패/빈 답을 각각의 오류로. `total_cost_usd`를 남긴다.
2) `result` 글자에서 앞뒤 공백과 감싼 코드 펜스 하나만 벗긴다. 160KB를 넘으면 거부(한국어 12개 × 약 2,000자 × 3바이트 ≈ 72KB의 두 배 여유). `serde_json`으로 `{"items": [...]}`를 읽는다. 최상위가 틀리면 `Malformed`(저장 없음).
3) 항목마다(앞 20개만): 모르는 키는 무시(저장하지 않음). 규칙: id `^[a-z0-9](?:[a-z0-9.-]{0,78}[a-z0-9])?$`; title 1–80자, summary 1–140자, 둘 다 한 줄; what·when_to_use·best_practice 1–600자(줄바꿈 허용, 프롬프트에도 글자 수를 적음); 제어·방향·0폭 문자(`text::is_unsafe_char`, 줄바꿈 제외)가 있으면 그 항목을 버림; category ∈ {`model`,`command`,`setting`,`agent`,`integration`,`ide`,`security`,`other`}(모르는 값은 버림); version 선택, `^\d{1,3}\.\d{1,3}\.\d{1,5}$`; date **필수**(docs changelog에 버전마다 날짜가 있다), 실제 날짜이고 오늘+2일 이하(틀리면 항목을 버린다); 세 설명·제목·요약에 위험 명령 패턴(`| sh`, `|sh`, `| bash`, `|bash`, `rm -rf`, `--dangerously-skip-permissions`)이 있으면 항목을 버린다(주입된 "베스트 프랙티스" 차단); source_url 허용 목록, 300자 이하. 같은 답 안의 중복 id는 첫 것만.
4) 결과: `Parsed { items, dropped }`. 버린 수는 상태 줄에 "n개는 형식이 맞지 않아 뺐어요"로 보인다.
답에서 무엇도 실행·열기·가져오기 하지 않는다. 화면은 React 텍스트 노드로만 그린다(마크다운 해석 없음).

### 5. 저장 (마이그레이션 v17)
main은 v16까지와 이름 없는 열 추가(model-select)다. 다음 번호 **v17**: 
```sql
CREATE TABLE news_items (
  id TEXT NOT NULL, lang TEXT NOT NULL, title TEXT NOT NULL, version TEXT, date TEXT,
  category TEXT NOT NULL, summary TEXT NOT NULL, what TEXT NOT NULL, when_to_use TEXT NOT NULL,
  best_practice TEXT NOT NULL, source_url TEXT NOT NULL, first_seen_ms INTEGER NOT NULL,
  PRIMARY KEY (id, lang)
);
CREATE TABLE news_seen (id TEXT NOT NULL, lang TEXT NOT NULL, seen_ms INTEGER NOT NULL, PRIMARY KEY (id, lang));
```
- 합치기: 한 트랜잭션 `INSERT OR IGNORE`(이미 있는 (id, lang)은 덮어쓰지 않음). 언어별 최대 500개, 넘으면 오래된 것(날짜·처음 받은 날 순)부터 지움.
- 본 표시는 `news_seen`((id, lang) 기준. id는 모델이 짓는 슬러그라 언어마다 다를 수 있다). `news_items`에 있는 것만 넣고, 오래된 소식을 지울 때 함께 지운다.
- 정렬: 날짜 내림차순, 같으면 버전을 숫자 튜플로 비교(2.1.322 > 2.1.99), 그다음 id. Rust `list`가 정렬해 보낸다.
- 상태: `settings`의 `news_status` JSON `{atMs, ok, added, dropped, costUsd, error}`(오류는 고정 코드만), `news_daily` JSON `{enabled, consentMs, consentVersion}`, `news_last_try_ms`, `news_costs` JSON(최근 30일 실행마다 `{atMs, costUsd}`, 수동·매일 모두).
- 실패: `news_items`는 건드리지 않고 상태만 갱신.
- 저장되는 것은 검사 통과한 필드뿐. 원문 페이지·답 전체·프롬프트는 저장하지 않는다.

### 6. 매일 가져오기
- 켜기: `news_set_daily(true)` → 셸 네이티브 확인(하루 한 번, 앱이 켜져 있을 때만, 회당 약 $0.40 상한(조금 넘을 수 있음), 30일 합계 $12에 닿으면 멈춤, 확인 창 없이 돈다, 언제든 끌 수 있음) → 확인하면 `{enabled:true, consentMs, consentVersion}`이고 `news_last_try_ms = consentMs`(첫 매일 실행은 24시간 뒤. 지금 받으려면 "가져오기"). 끄기는 확인 없이. `consentVersion`(상한 상수의 버전)이 앱 상수와 다르면 꺼진 것으로 읽는다(다시 확인).
- 틱: 기존 로컬 루틴 스케줄러 루프(20초)에 `news_cmds::on_tick`. 조건(`news::daily_due`, 순수 함수): 켜짐 && 동의 버전 일치 && 앱 시작 후 60초 지남 && `now - news_last_try_ms ≥ 24h` && 최근 30일 비용 합 < $12 && 로그인(should_tick). 틱은 busy를 **잡은 뒤 별도 스레드**를 띄우고 바로 돌아온다(루프는 루틴·한도 틱을 막지 않음). 시도 시각은 시작할 때 남긴다(실패해도 24시간 뒤). 한 번의 비용이 상한의 1.5배를 넘으면 매일 가져오기를 끄고 상태에 남긴다. 결과는 `news-updated` 이벤트로 웹뷰가 다시 읽는다.
- 로컬 루틴(`routines` 표)은 쓰지 않는다: 루틴은 프로젝트 폴더에서 작업 세션(TaskSpec)을 돌리는 것이라 웹 도구만 쓰는 이 실행과 권한 모델이 다르다. 클라우드 루틴도 만들지 않는다.

### 7. 썸네일
카테고리 카드만: 120×80, 카테고리 아이콘(lucide) + `DUCK_COLORS`의 몸 색 배경·스카프 색 아이콘(대비 3:1 이상, D-130). 카테고리 → 색은 역할 색 표(`roleColors.ts`)와 따로 둔 고정 표(`lib/news.ts`). 원격 이미지는 받지 않는다. 답의 `image_url`은 요청하지 않고 와도 무시한다. 그래서 `tauri.conf.json` CSP(`img-src 'self' data:`)는 바꾸지 않는다. og:image 캐시는 보안 표면(SSRF·추적 픽셀·디스크 캐시) 대비 이득이 작아 하지 않는다.

### 8. 화면
- 헤더: 제목 "소식", 요약 "Claude Code 새 기능 n개". 오른쪽 "가져오기"(ink primary) + 비용 한 줄은 상태 줄 옆.
- 상태 줄: "마지막 가져오기 11월 11일 09:12 · 새 소식 3개 · $0.18" / 실패면 위험 색 한 줄 + 이유. 진행 중이면 "가져오는 중…" + 취소.
- 매일 스위치: "매일 가져오기" + 한 줄 "하루 한 번, 회당 최대 $0.40". 켜져 있으면 "다음 가져오기 {시각}".
- 목록: `<ul>`에 줄 버튼. 썸네일 120px, 제목(15px semibold), 요약 한 줄 말줄임, `v2.1.321 · 11월 9일`. 새 소식 점은 ink 점 + 스크린리더용 "새 소식". 노랑은 쓰지 않는다(노랑 = 나를 기다림, D-176).
- "이미 나온 기능" 소제목 아래 30일보다 오래된 것.
- 상세: 오른쪽 시트(에이전트 편집 시트와 같은 패턴), 제목·버전·날짜·카테고리, 세 소제목 + 본문(whitespace-pre-line), "원문 열기"(호스트 표시), 맨 아래 "Claude가 공식 문서를 요약했어요 · 원문에서 확인해요" 한 줄. Esc·배경·×로 닫고 포커스 복귀.
- 빈 상태: 오리 + "아직 소식이 없어요" + 가져오기 버튼 + 비용 한 줄. 다른 언어 소식이 있으면 "English 소식 n개가 있어요 · 가져오면 한국어로 받아요" 한 줄.
- 로그인 안 됨: `GateLine`.

### 9. 언어
가져올 때의 앱 언어(`i18n::current()`)로 만들고 그 언어로 저장한다. 목록은 현재 언어 것만. 언어를 바꿀 때 자동으로 다시 가져오지 않는다(비용). 이유: 자동 번역 실행은 확인 없는 과금이 된다.

### 10. 명령
`news_list() → NewsView{items, status, daily{enabled,nextAtMs}, otherLang}`, `news_fetch() → NewsRun{done{added,dropped,costUsd}|declined|failed{error,message}}`, `news_cancel()`, `news_mark_seen(id)`(id 형식 검사), `news_set_daily(enabled) → bool`(거절이면 false), `news_open_source(id)`: DB에서 현재 언어의 주소를 읽어 `source_allowed`를 다시 통과하면 기본 브라우저로 연다(공식 주소라 확인 창 없음).

## 보안·프라이버시
- 자격증명: 읽지 않는다. CLI가 자기 로그인을 쓴다.
- 보내는 것: 고정 지시, 언어, 이미 가진 소식 id(공식 변경 기록에서 온 슬러그)와 최고 버전. 사용자 프로젝트·대화·경로·사용 통계는 보내지 않는다.
- transcript: 실행은 `--no-session-persistence`이고 cwd는 실행 뒤 비우며(`clean_owned_cwd`), 앱은 답 전체를 저장하지 않는다.
- 주입: 가져온 페이지는 공식이지만 신뢰하지 않는다. 도구는 WebFetch 하나, 훅이 주소·횟수를 막고, 답은 스키마로만 읽는다. 쓰기·Bash 도구가 없다.
- SSRF: 앱 자체는 네트워크를 쓰지 않는다(썸네일 없음). WebFetch 대상은 허용 목록. 원문 열기는 DB 값 재검사 후 브라우저.
- 비용: 수동은 매번 확인, 매일은 켤 때 한 번 확인 + 회당 하드 상한 + 24시간 간격. 

## Risks / Trade-offs
- 모델이 날짜를 지어낼 수 있다 → 미래 날짜 거부, changelog 날짜만 쓰라고 지시. 날짜 없는 항목은 처음 받은 날로 정렬.
- 출처 페이지가 커서 WebFetch 요약이 일부만 볼 수 있다 → 프롬프트에 WebFetch `prompt`로 "최근 버전 N개의 Added 항목"을 물으라고 지시.
- 매일 실행은 확인 없이 돈다 → 기본 꺼짐, 켤 때 월 최대 비용을 보임, 상한은 Rust 상수.
- 같은 기능이 다른 id로 다시 올 수 있다 → id 규칙(버전+슬러그)과 가진 id 목록으로 줄이고, 남는 중복은 받아들인다.

## Migration Plan
v17은 새 표 두 개만 만든다. 이전 버전 DB는 그대로 열린다. 되돌리기는 표를 버려도 다른 기능에 영향이 없다.

## Open Questions
없음(사용자 합의안을 따른다). 비용($0.10–0.30)·240초·읽기 8번은 실측이 아닌 추정이라 문구는 "약"으로 쓴다. 사용자가 실제 앱에서 한 번 가져와 비용·시간·품질을 확인한다(모델 호출은 이 change에서 하지 않음).

## 설계 리뷰 반영 (design-critic HIGH 2·MEDIUM 6·LOW 2, security HIGH 2·MEDIUM 3·LOW 4)
- critic H1 64KB면 정상 답도 버려짐 → 160KB, 설명 600자, 프롬프트에 글자 수(§4).
- critic H2 처음 켜면 20초 안에 무확인 실행 → 동의 때 last_try = consentMs(§6).
- critic M 틱이 루프를 막음 → busy 잡고 별도 스레드. M busy 순서 → 확인 창 전에 잡음(§2). M 버전 글자 정렬 → 숫자 튜플(§5). M 언어별 id → seen (id, lang)(§5). M 날짜 없음 → date 필수(§4). M 비용 근거 없음 → "약", 사용자 실측(Open Questions).
- critic L 시나리오 부족 → spec에 취소·하나만·게이트·원문 재검사·24시간·잘못된 날짜·배경 닫기 시나리오 추가. L 120초 문구 → `NewsTimeout`. L 역할 색과 겹침 → 카테고리 색 표를 따로(§7).
- security H1 github 아래 issues·PR 주입 → 정확한 경로만(§1). H2 읽기·원문 같은 함수 → 둘로 나눔(§1).
- security M 쿼리 유출 → 쿼리·프래그먼트·인코딩된 구분자 거부(§1). M 30일 $12 보장 안 됨 → 비용 장부·합계 상한·consentVersion(§5·§6). M 예산 초과 가능 → 확인 창에 적고 1.5배 넘으면 매일 끔(§6).
- security L seen 무한 → 있는 id만, 함께 정리(§5). L stderr → 고정 코드만·Scrub(§2). L 저장 글이 주입 → 상세 하단 안내 줄, 위험 명령 패턴 항목 버림(§4·§8). L 리다이렉트 → 원문 목록을 좁혀 처리(§1).

