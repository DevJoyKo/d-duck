//! Message table for text the backend shows without going through the webview (tray, OS alerts, native dialogs)
//! and fallback sentences sent along with error codes (app-language, D-171).
//!
//! Every string parameter passes `display_safe`; URLs enter only as their host and paths only as their last name,
//! so a template can never carry userinfo, a full path, or control characters. Substitution runs once over the
//! template: a parameter value is never scanned for further placeholders.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::text::display_safe;

/// The language actually used (a stored `system` is resolved before this point).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Ko,
    En,
}

impl Lang {
    /// The rule for "시스템 언어 따름": a locale starting with `ko` is Korean, anything else (or nothing) is English.
    /// The locale string itself is not kept or logged.
    pub fn from_locale(locale: Option<&str>) -> Lang {
        match locale {
            Some(l) if l.trim().to_ascii_lowercase().starts_with("ko") => Lang::Ko,
            _ => Lang::En,
        }
    }

    /// The OS default language through `sys-locale`.
    pub fn system() -> Lang {
        Lang::from_locale(sys_locale::get_locale().as_deref())
    }
}

/// The language for text made outside a request that names one (an error's fallback sentence, the tray). The shell
/// sets it at start and on every language change; Korean until then, so tests read as before.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_current(lang: Lang) {
    CURRENT.store(if lang == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 { Lang::En } else { Lang::Ko }
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Ko => "ko",
            Lang::En => "en",
        }
    }

    /// Only the two stored values; anything else is `None`.
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "ko" => Some(Lang::Ko),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// The one line the app appends to its fixed instructions so Claude answers in this language (design 6).
    pub fn reply_line(self) -> &'static str {
        match self {
            Lang::Ko => "사람이 읽는 출력은 한국어로 써요.",
            Lang::En => "Write human-readable output in English.",
        }
    }
}

/// One enum, its `ALL` list and its (ko, en) templates from a single list, so a code cannot miss a language.
macro_rules! table {
    ($(#[$m:meta])* $name:ident { $($v:ident => ($ko:expr, $en:expr $(,)?)),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($v),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),*];
            fn templates(self) -> (&'static str, &'static str) {
                match self { $($name::$v => ($ko, $en)),* }
            }
        }
    };
}

table! {
    /// Message codes for OS surfaces: tray, notifications, native dialogs and file panels. The tests check both
    /// languages and that both use the same placeholder names.
    Code {
        TrayOpen => ("창 열기", "Open window"),
        TrayWaitingHeader => ("입력 대기", "Waiting for input"),
        TraySessionFallback => ("Claude 세션", "Claude session"),
        TrayQuit => ("종료", "Quit"),
        TrayCheck => ("확인 필요 {count}", "Needs you {count}"),
        TrayPending => ("리뷰 대기 {count}", "Waiting for review {count}"),
        NotifyFinishedReview => ("오리가 끝냈어요 · 리뷰 대기: {name}", "Your duck finished · Waiting for review: {name}"),
        NotifyFinished => ("오리가 끝냈어요: {name}", "Your duck finished: {name}"),
        NotifyFailed => ("작업이 실패했어요: {name}", "Task failed: {name}"),
        NotifyWaiting => ("{name}: 입력을 기다리고 있어요", "{name}: Waiting for your input"),
        TimeUnknown => ("알 수 없는 시각", "Unknown time"),
        LimitBanner => ("사용량 한도 · {time} 리셋 · {counts}", "Usage limit · resets {time} · {counts}"),
        LimitNotice => ("사용량 한도에 닿았어요 · {time} 리셋", "Usage limit reached · Resets at {time}"),
        LimitAuto => ("{count}개는 이어서 할게요", "{count} continue on their own"),
        LimitManual => ("{count}개는 직접 이어야 해요", "{count} need you to continue"),
        LimitAwake => ("Mac이 깨어 있어야 이어요", "Your Mac needs to be awake"),
        LimitStarted => ("작업 {count}개를 이어서 시작했어요", "Continued tasks: {count}"),
        AuthClaudeAi => ("Claude 계정으로 연결돼 있어요", "Connected with your Claude account"),
        AuthToken => ("토큰으로 연결돼 있어요", "Connected with a token"),
        AuthOther => ("다른 방식으로 연결돼 있어요", "Connected another way"),
        // ---- memory, rules, extensions ----
        DlgMemTrashTitle => ("메모리 삭제", "Delete memory"),
        DlgMemTrashBody => (
            "메모리 '{name}'의 파일을 macOS 휴지통으로 옮겨요. MEMORY.md에 줄이 있으면 함께 빼요.\n{path}\n\n바로 지워지지 않으며 Finder 휴지통에서 되살릴 수 있어요. 휴지통으로 옮길까요?",
            "Moves the file of memory '{name}' to the macOS Trash, and removes its MEMORY.md line if there is one.\n{path}\n\nIt isn't deleted right away; you can restore it from the Trash in Finder. Move it to the Trash?",
        ),
        DlgAutoMemTitle => ("자동 메모리", "Auto memory"),
        DlgAutoMemOn => (
            "자동 메모리를 켜요.\n\nClaude Code가 대화 중 기억할 만한 것을 메모리 폴더에 저장하고, 다음 대화에서 MEMORY.md를 불러와요.\n전역 settings.json의 autoMemoryEnabled 값만 바꿔요.",
            "Turns on auto memory.\n\nClaude Code saves things worth remembering to the memory folder during a conversation, and loads MEMORY.md in the next one.\nOnly autoMemoryEnabled in the global settings.json changes.",
        ),
        DlgAutoMemOff => (
            "자동 메모리를 꺼요.\n\nClaude Code가 새 메모리를 저장하지 않고 MEMORY.md도 불러오지 않아요. 메모리 파일은 지우지 않아요.\n전역 settings.json의 autoMemoryEnabled 값만 바꿔요.",
            "Turns off auto memory.\n\nClaude Code stops saving new memories and doesn't load MEMORY.md. Memory files aren't deleted.\nOnly autoMemoryEnabled in the global settings.json changes.",
        ),
        DlgRuleTrashTitle => ("규칙 삭제", "Delete rule"),
        DlgRuleTrashBody => ("규칙 '{name}'을 macOS 휴지통으로 옮겨요.\n{path}\n\nFinder 휴지통에서 되살릴 수 있어요.", "Moves the rule '{name}' to the macOS Trash.\n{path}\n\nYou can restore it from the Trash in Finder."),
        DlgExtTrashTitle => ("{kind} 삭제", "Delete {kind}"),
        DlgExtTrashSkill => ("스킬을 휴지통으로 옮겨요 · {name}", "Moves the skill to the Trash · {name}"),
        DlgExtTrashCommand => ("커맨드를 휴지통으로 옮겨요 · {name}", "Moves the command to the Trash · {name}"),
        DlgExtTrashWorkflow => ("워크플로를 휴지통으로 옮겨요 · {name}", "Moves the workflow to the Trash · {name}"),
        DlgExtTrashFolder => ("폴더째 옮겨요(파일 {count}개).", "The whole folder moves ({count} files)."),
        DlgExtUnsaved => ("저장 안 한 내용도 사라져요.", "Unsaved changes are lost too."),
        PlaceGlobalShort => ("전역", "Global"),
        PlacePlugin => ("플러그인", "Plugin"),
        RoleOf => ("{label} 담당", "{label} role"),
        MakeKind => ("{kind} 만들기", "Make a {kind}"),
        LoadoutUsersMore => (" 외 {count}개", " and {count} more"),
        LoadoutUsers => ("{who}{particle} 기본 장비로 써요. 지우면 다음 시작 때 빠져요.", "{who}{particle} use this as a default loadout. Deleting it drops it from their next start."),
        LoadoutUsersStrict => (" 엄격 유형은 시작하지 못해요.", " Strict types can't start."),
        // ---- schedules (routines, cloud routines) ----
        SchedDaily => ("매일 {time}", "Daily {time}"),
        SchedWeekly => ("매주 {days} {time}", "Weekly {days} {time}"),
        SchedEvery => ("{hours}시간마다", "Every {hours} hours"),
        SchedOnce => ("한 번 {time}", "Once {time}"),
        SchedOnceBare => ("한 번", "Once"),
        ChangeName => ("이름", "Name"),
        ChangeSchedule => ("일정", "Schedule"),
        ChangeState => ("상태", "State"),
        StateOn => ("켜짐", "On"),
        StateOff => ("꺼짐", "Off"),
        // ---- limits (resume) ----
        DlgResumeTitle => ("이어서 하기", "Continue"),
        DlgResumeBody => (
            "사용량 한도로 멈춘 작업을 이어서 해요. 이어서 하는 동안 사용량을 써요.\n\n작업: {name}{danger}{persona}\n\n이어서 할까요?",
            "Continues a task that stopped at a usage limit. Continuing uses your usage.\n\nTask: {name}{danger}{persona}\n\nContinue?",
        ),
        DlgResumeDanger => ("\n권한 모드 '{mode}': Claude가 승인 없이 파일 수정과 명령 실행을 할 수 있어요.", "\nPermission mode '{mode}': Claude can edit files and run commands without approval."),
        // ---- delivery guard, worktree ----
        DlgGuardOffTitle => ("원격 push 막기 해제", "Allow remote push again"),
        DlgGuardOffBody => ("이 프로젝트에서 push와 PR 초안을 다시 허용할까요?\n실행 중인 작업은 그대로예요.", "Allow push and draft PRs in this project again?\nRunning tasks aren't affected."),
        DlgWorktreeTitle => ("worktree 정리", "Clean up worktree"),
        WtAsk => ("worktree를 지울까요?", "Delete the worktree?"),
        WtFolder => ("폴더: {path}", "Folder: {path}"),
        WtBranch => ("브랜치: {branch}", "Branch: {branch}"),
        WtIgnoredCount => ("무시된 파일 약 {count}개(빌드 결과 등)도 함께 지워져요", "About {count} ignored files (build output and such) are deleted too"),
        WtIgnored => ("무시된 파일(빌드 결과 등)도 함께 지워져요", "Ignored files (build output and such) are deleted too"),
        WtKeep => ("브랜치와 커밋은 남겨요", "The branch and commits stay"),
        // ---- add items (links, authoring) ----
        BtnInstall => ("설치", "Install"),
        BtnAdd => ("추가", "Add"),
        BtnCreate => ("만들기", "Create"),
        BtnCopy => ("복사", "Copy"),
        KindSkill => ("스킬", "skill"),
        KindCommand => ("커맨드", "command"),
        KindWorkflow => ("워크플로", "workflow"),
        PlaceGlobal => ("전역(모든 프로젝트)", "Global (all projects)"),
        PlaceProject => ("프로젝트 {path}", "Project {path}"),
        RiskAllowedTools => ("· allowed-tools: {tools} (확인 없이 쓸 수 있는 도구)", "· allowed-tools: {tools} (tools usable without asking)"),
        RiskSkillHooks => ("· hooks: 명령을 실행할 수 있어요", "· hooks: can run commands"),
        RiskShellLines => ("· 불러올 때 셸 명령을 실행하는 줄 {count}개", "· {count} lines that run shell commands when loaded"),
        DlgPluginInstallTitle => ("플러그인 설치", "Install plugin"),
        DlgPluginInstallMarket => (
            "{name} 플러그인을 설치해요.\n\n출처: github.com/{owner}/{repo}\n커밋: {sha} (미리보기 시점, 설치할 때 바뀌었을 수 있어요)\n마켓플레이스: {market}{added}\n설치 위치: 전역(모든 프로젝트)\n\n이 플러그인의 코드(훅·MCP 서버·커맨드·스킬)가 다음 Claude 실행부터 내 권한으로 동작해요.\n마켓플레이스가 추가 명령 승인을 요구하면 D.Duck은 실행하지 않고 터미널 안내만 해요.\n\n설치할까요?",
            "Installs the {name} plugin.\n\nSource: github.com/{owner}/{repo}\nCommit: {sha} (at preview time; it may have changed since)\nMarketplace: {market}{added}\nInstall location: Global (all projects)\n\nThis plugin's code (hooks, MCP servers, commands, skills) runs with your permissions from the next Claude run.\nIf the marketplace asks to approve extra commands, D.Duck doesn't run them and only shows Terminal steps.\n\nInstall?",
        ),
        MarketAlreadyAdded => (" (이미 추가됨)", " (already added)"),
        MarketAddFirst => (" — 먼저 추가해요", " — added first"),
        DlgPluginInstallId => (
            "{name} 플러그인을 설치해요.\n\n설치 위치: 전역(모든 프로젝트)\n\n이 플러그인의 코드(훅·MCP 서버·커맨드·스킬)가 다음 Claude 실행부터 내 권한으로 동작해요.\n마켓플레이스가 추가 명령 승인을 요구하면 D.Duck은 실행하지 않고 터미널 안내만 해요.\n\n설치할까요?",
            "Installs the {name} plugin.\n\nInstall location: Global (all projects)\n\nThis plugin's code (hooks, MCP servers, commands, skills) runs with your permissions from the next Claude run.\nIf the marketplace asks to approve extra commands, D.Duck doesn't run them and only shows Terminal steps.\n\nInstall?",
        ),
        DlgMarketAddTitle => ("마켓플레이스 추가", "Add marketplace"),
        DlgMarketAddBody => (
            "마켓플레이스 {market}을(를) 추가해요.\n\n출처: github.com/{owner}/{repo}\n커밋: {sha} (미리보기 시점)\n\n추가만 하고 플러그인은 설치하지 않아요. 이 마켓플레이스의 플러그인은 나중에 설치할 수 있어요.\n\n추가할까요?",
            "Adds the {market} marketplace.\n\nSource: github.com/{owner}/{repo}\nCommit: {sha} (at preview time)\n\nOnly the marketplace is added; no plugin is installed. You can install its plugins later.\n\nAdd?",
        ),
        DlgSkillInstallTitle => ("스킬 설치", "Install skill"),
        DlgSkillInstallBody => (
            "스킬 '{name}'을(를) 설치해요.\n\n출처: {source}\n커밋: {sha}\n설치 위치: {dest}\n파일 {files}개{exec}\n\n이 스킬의 지침은 Claude가 그대로 따르고, 스크립트는 다음 Claude 실행부터 내 권한으로 실행될 수 있어요.",
            "Installs the skill '{name}'.\n\nSource: {source}\nCommit: {sha}\nInstall location: {dest}\nFiles: {files}{exec}\n\nClaude follows this skill's instructions as written, and its scripts may run with your permissions from the next Claude run.",
        ),
        SkillExecutables => (" (실행 파일 {count}개 — 실행 권한은 주지 않아요)", " ({count} executables — not given run permission)"),
        DlgRiskSettings => ("⚠ 권한 관련 설정:", "⚠ Permission settings:"),
        DlgInstallAsk => ("설치할까요?", "Install?"),
        DlgCreateTitle => ("만들기 확인", "Confirm create"),
        DlgCreateBody => ("{kind} '{name}'을(를) {place}에 만들어요.\n\n이 내용에는 명령을 실행할 수 있는 설정이 있어요:\n{risks}\n\n만들까요?", "Creates the {kind} '{name}' in {place}.\n\nThis content is set up to run commands:\n{risks}\n\nCreate?"),
        DlgItemSaveTitle => ("저장 확인", "Confirm save"),
        DlgItemSaveBody => ("{kind} '{name}'을(를) 저장해요.\n\n이 내용에는 명령을 실행할 수 있는 설정이 있어요:\n{risks}\n\n저장할까요?", "Saves the {kind} '{name}'.\n\nThis content is set up to run commands:\n{risks}\n\nSave?"),
        DlgCopyTitle => ("복사 확인", "Confirm copy"),
        DlgCopyBody => ("{kind} '{name}'을(를) {place}에 '{new}'(으)로 복사해요.\n\n이 내용에는 명령을 실행할 수 있는 설정이 있어요:\n{risks}\n\n복사할까요?", "Copies the {kind} '{name}' to {place} as '{new}'.\n\nThis content is set up to run commands:\n{risks}\n\nCopy?"),
        DlgResultTitle => ("만든 항목 설치", "Install what Claude made"),
        DlgResultBody => ("Claude가 만든 {kind} '{name}'을(를) {place}에 설치해요.\n\n위치: {path}\n파일 {files}개", "Installs the {kind} '{name}' Claude made in {place}.\n\nLocation: {path}\nFiles: {files}"),
        DlgResultRisks => ("⚠ 명령을 실행할 수 있는 설정:", "⚠ Settings that can run commands:"),
        DlgDduckNewTitle => ("/dduck-new 설치", "Install /dduck-new"),
        DlgDduckNewBody => (
            "터미널의 Claude Code에서 /dduck-new 로 스킬·커맨드·워크플로를 만들 수 있게 커맨드 파일을 설치해요.\n\n위치: {path}\n\n내용은 D.Duck의 만들기 지침이며 도구 허용(allowed-tools)이나 셸 실행 줄은 없어요. 끄면 휴지통으로 옮겨요.\n\n설치할까요?",
            "Installs a command file so you can make skills, commands and workflows with /dduck-new in Claude Code in Terminal.\n\nLocation: {path}\n\nIt holds D.Duck's authoring guide, with no allowed-tools and no shell lines. Turning it off moves it to the Trash.\n\nInstall?",
        ),
        DlgLoginTitle => ("Claude 로그인", "Claude login"),
        DlgLoginBody => ("터미널에서 Claude 로그인을 열까요?", "Open Claude login in Terminal?"),
        // ---- buttons ----
        BtnCancel => ("취소", "Cancel"),
        BtnSave => ("저장", "Save"),
        BtnTrash => ("휴지통으로", "Move to Trash"),
        BtnDelete => ("지우기", "Delete"),
        BtnOpen => ("열기", "Open"),
        BtnTurnOn => ("켜기", "Turn on"),
        BtnTurnOff => ("끄기", "Turn off"),
        BtnKeep => ("남겨 두기", "Keep"),
        BtnRun => ("실행", "Run"),
        BtnRunAuto => ("실행 · 리셋 후 자동으로 이어서 하기", "Run · Continue on its own after a reset"),
        BtnStart => ("시작", "Start"),
        BtnAllow => ("허용", "Allow"),
        BtnUpdate => ("업데이트", "Update"),
        BtnChange => ("바꾸기", "Change"),
        BtnContinue => ("이어서", "Continue"),
        BtnCheck => ("확인", "Check"),
        BtnForceQuit => ("강제 종료", "Force quit"),
        BtnSendAnalyze => ("보내고 분석", "Send and analyze"),
        BtnSendRefine => ("보내고 다듬기", "Send and refine"),
        BtnTrustInTerminal => ("터미널에서 신뢰하기", "Trust in Terminal"),
        // ---- file panels ----
        PickDefaultCwd => ("기본 작업 폴더 선택", "Choose the default task folder"),
        PickCliFile => ("claude 실행 파일 선택", "Choose the claude executable"),
        PickConfigDir => ("Claude 설정 디렉터리 선택", "Choose the Claude config folder"),
        PickTaskDir => ("작업 디렉터리 선택", "Choose a task folder"),
        // ---- shared dialog lines ----
        DlgSaveAsk => ("저장할까요?", "Save?"),
        DlgNone => ("(없음)", "(none)"),
        DlgMore => ("  … 외 {count}개", "  … and {count} more"),
        DlgTrashRestore => ("Finder 휴지통에서 되살릴 수 있어요.", "You can restore it from the Trash in Finder."),
        // ---- usage insights ----
        DlgInsightsTitle => ("Claude에게 분석 받기", "Get an analysis from Claude"),
        DlgInsightsBody => (
            "설치된 Claude CLI로 집계 요약만 보내 개선점을 받아요. 프롬프트·코드·경로·프로젝트 이름·세션 제목은 보내지 않고, 스킬·에이전트·MCP 이름은 번호로 바꿔요. Claude는 Anthropic 공식 문서 검색·읽기만 할 수 있어요.\n\n보낼 내용: {brief}\n전체 내용은 사용 통계 탭의 '보낼 내용'에 그대로 표시되어 있어요.\n\n예상 비용: {cost}\nAPI 환산: 구독이면 실제 청구액이 아니라 API 단가로 바꾼 값이에요.",
            "Sends only an aggregated summary through the installed Claude CLI and gets suggestions back. Prompts, code, paths, project names and session titles aren't sent, and skill, agent and MCP names are replaced with numbers. Claude can only search and read official Anthropic docs.\n\nWhat's sent: {brief}\nThe full content is shown under 'What's sent' in the usage tab.\n\nEstimated cost: {cost}\nAPI equivalent: on a subscription this is the API price, not what you're billed.",
        ),
        InsightsBrief => (
            "기간 {days}일 집계: 모델 {models}개, 도구 {tools}개, 스킬 {skills}개, 서브에이전트 {subagents}개의 횟수·비율(구간 값), 캐시 비율, 세션 길이 분포, Claude Code 버전, 앱 안 점검 결과 이름 ({bytes} 바이트)",
            "{days}-day totals: counts and shares (as ranges) for {models} models, {tools} tools, {skills} skills and {subagents} subagents, cache ratio, session length spread, Claude Code version, and in-app check names ({bytes} bytes)",
        ),
        InsightsCost => ("약 $0.05–0.20 (최대 $0.30, API 환산)", "About $0.05–0.20 (up to $0.30, API equivalent)"),
        // ---- news ----
        DlgNewsTitle => ("소식 가져오기", "Fetch news"),
        DlgNewsBody => (
            "설치된 Claude CLI로 Claude Code 공식 변경 기록과 문서(code.claude.com, github.com/anthropics/claude-code의 CHANGELOG)만 읽어 새 기능 설명을 받아요. 프로젝트·대화·코드·경로는 보내지 않아요.\n\n보낼 내용: 앱 언어({lang}), 이미 받은 소식 id {count}개\n\n예상 비용: {cost}\nAPI 환산: 구독이면 실제 청구액이 아니라 API 단가로 바꾼 값이에요.",
            "Reads only the official Claude Code changelog and docs (code.claude.com and the CHANGELOG at github.com/anthropics/claude-code) through the installed Claude CLI and gets explainers of new features back. Projects, conversations, code and paths aren't sent.\n\nWhat's sent: the app language ({lang}) and the ids of {count} news items you already have\n\nEstimated cost: {cost}\nAPI equivalent: on a subscription this is the API price, not what you're billed.",
        ),
        NewsCost => ("약 $0.10–0.30 (상한 $0.40, 조금 넘을 수 있어요, API 환산)", "About $0.10–0.30 (limit $0.40, may go slightly over, API equivalent)"),
        DlgNewsDailyTitle => ("매일 소식 가져오기", "Fetch news daily"),
        DlgNewsDailyBody => (
            "앱이 켜져 있는 동안 하루 한 번, 확인 창 없이 소식을 가져와요. 첫 실행은 24시간 뒤예요.\n\n한 번에 약 $0.40까지예요(조금 넘을 수 있어요). 최근 30일 합계가 $12에 닿으면 멈추고, 한 번에 $0.60을 넘으면 스스로 꺼져요. API 환산 값이에요.\n\n소식 탭에서 언제든 끌 수 있어요. 켤까요?",
            "Fetches news once a day while the app is open, without asking each time. The first run is 24 hours from now.\n\nEach run is capped at about $0.40 (it may go slightly over). It stops when the last 30 days add up to $12, and turns itself off if one run goes over $0.60. These are API-equivalent amounts.\n\nYou can turn it off any time in the News tab. Turn it on?",
        ),
        BtnFetch => ("가져오기", "Fetch"),
        // ---- brief refine ----
        DlgRefineTitle => ("요구사항 다듬기", "Refine the brief"),
        DlgRefineBody => (
            "맡길 내용(목표·범위 밖·제약·완료 기준)과 고른 답을 Claude에 보내 질문과 고쳐 쓴 내용을 받아요. 프로젝트 이름·경로·파일은 보내지 않아요. Claude는 도구를 쓸 수 없고 기록도 남기지 않아요.\n\n{limits}\n이 확인으로 10분 동안 질문 {rounds}번까지 이어서 받아요. 상한을 넘으면 다시 물어요.{names}\n\n예상 비용: 구독이면 실제 청구액이 아니라 API 단가로 계산한 값이에요.",
            "Sends the brief (goal, out of scope, constraints, done criteria) and your answers to Claude, and gets questions and a rewrite back. Project names, paths and files aren't sent. Claude can't use tools and keeps no record.\n\n{limits}\nThis approval covers up to {rounds} rounds of questions for 10 minutes. Past that, you'll be asked again.{names}\n\nEstimated cost: on a subscription this is the API price, not what you're billed.",
        ),
        DlgRefineNames => ("\n\n근거에 내 도구 이름이 {count}개 있어요({names}). {action}", "\n\nThe brief names {count} of your tools ({names}). {action}"),
        DlgRefineRedact => ("이름은 지우고 보내요.", "Names are removed before sending."),
        DlgRefineKeepNames => ("이름도 그대로 보내요.", "Names are sent as they are."),
        RefineLimits => ("최대 {runs}회 · 예상 비용 최대 ${budget} · 맡길 내용이 Claude로 가요", "Up to {runs} runs · Estimated cost up to ${budget} · The brief goes to Claude"),
        // ---- links, plugins, MCP ----
        DlgLinkTitle => ("링크 열기", "Open link"),
        DlgLinkBody => ("외부 브라우저에서 열어요.\n\n사이트: {host}\n주소: {url}", "Opens in your browser.\n\nSite: {host}\nAddress: {url}"),
        DlgMcpCheckTitle => ("MCP 상태 확인", "Check MCP status"),
        DlgMcpCheckBody => (
            "Claude가 아직 신뢰하지 않은 프로젝트 '{name}'에서 상태를 확인해요.\n\n이 프로젝트의 .mcp.json에 적힌 서버도 잠깐씩 실행돼요. 내용을 모르면 취소해 주세요.\n\n확인할까요?",
            "Checks status in '{name}', a project Claude doesn't trust yet.\n\nServers listed in this project's .mcp.json will also run briefly. Cancel if you don't know what's in it.\n\nCheck now?",
        ),
        DlgPluginUpdateTitle => ("플러그인 업데이트", "Update plugin"),
        DlgPluginUpdateBody => (
            "{name} 플러그인을 업데이트해요.\n\n현재 {current} → {target}\n\n이 마켓플레이스({market})의 코드(훅·MCP·커맨드)가 다음 Claude 시작부터 내 권한으로 실행돼요.\n마켓플레이스가 추가 명령 승인을 요구하면 D.Duck은 실행하지 않고 터미널 안내만 해요.\n\n업데이트할까요?",
            "Updates the {name} plugin.\n\nNow {current} → {target}\n\nCode from this marketplace ({market}) — hooks, MCP, commands — runs with your permissions from the next Claude start.\nIf the marketplace asks to approve extra commands, D.Duck doesn't run them and only shows Terminal steps.\n\nUpdate?",
        ),
        PluginNoVersion => ("버전 없음", "No version"),
        DlgPluginBulkTitle => ("플러그인 {count}개 업데이트", "Update {count} plugins"),
        DlgPluginBulkIntro => ("다음 플러그인을 하나씩 차례로 업데이트해요. 하나가 실패해도 나머지는 계속해요.", "Updates these plugins one at a time. If one fails, the rest still run."),
        DlgPluginBulkItem => ("· {id} ({scope}) {current} → {target}", "· {id} ({scope}) {current} → {target}"),
        DlgPluginBulkSkipped => ("여기서 업데이트할 수 없는 {count}개는 건너뛰어요.", "{count} that can't be updated here are skipped."),
        DlgPluginBulkRisk => (
            "이 마켓플레이스({markets})의 코드(훅·MCP·커맨드)가 다음 Claude 시작부터 내 권한으로 실행돼요.\n마켓플레이스가 추가 명령 승인을 요구하면 D.Duck은 실행하지 않고 터미널 안내만 해요.\n\n업데이트할까요?",
            "Code from these marketplaces ({markets}) (hooks, MCP, commands) runs with your permissions from the next Claude start.\nIf a marketplace asks to approve extra commands, D.Duck doesn't run them and only shows Terminal steps.\n\nUpdate?",
        ),
        PluginBulkUnknown => ("새 버전 확인 불가(최신일 수 있음)", "New version unknown (may be up to date)"),
        PluginLatestSame => ("마켓플레이스 최신(로컬 사본 기준 같은 버전)", "Marketplace latest (same version as the local copy)"),
        PluginLatestUnknown => ("마켓플레이스 최신(버전 확인 불가)", "Marketplace latest (version unknown)"),
        // ---- settings ----
        DlgRemoteTitle => ("휴대폰 알림 켜기", "Turn on phone alerts"),
        DlgRemoteBody => (
            "D.Duck이 시작하는 작업을 Claude 앱의 Remote Control에 연결해요.\n\n· 끝나거나 답이 필요하면 휴대폰으로 알림이 와요.\n· Claude 계정에 로그인된 휴대폰에서 이 Mac의 작업에 지시를 보내고 권한을 승인할 수 있게 돼요.\n· 배포·보안·민감 영역·위험 권한 작업과 명령을 실행할 수 있는 페르소나에는 연결하지 않아요.\n\n켤까요?",
            "Connects tasks D.Duck starts to Remote Control in the Claude app.\n\n· Your phone gets an alert when a task finishes or needs an answer.\n· A phone signed in to your Claude account can send instructions to tasks on this Mac and approve permissions.\n· Deploy, security, sensitive and risky-permission tasks, and personas that can run commands, are never connected.\n\nTurn on?",
        ),
        DlgClearHistoryTitle => ("지난 작업 모두 지우기", "Delete all past tasks"),
        DlgClearHistoryBody => ("지난 작업 기록과 저장된 지시를 모두 지워요. 실행 중인 작업에는 영향이 없어요.\n\n지울까요?", "Deletes all past task records and saved instructions. Running tasks aren't affected.\n\nDelete?"),
        DlgReviewDeleteTitle => ("작업 기록 지우기", "Delete task record"),
        DlgReviewDeleteRoutineTitle => ("루틴 보고서 지우기", "Delete routine report"),
        DlgReviewDeleteAsk => ("{name}을(를) 지울까요?", "Delete {name}?"),
        DlgReviewDeleteAskUnnamed => ("이 작업을 지울까요?", "Delete this task?"),
        DlgReviewDeleteTask => ("작업 기록과 보고서(근거·첨부)를 지워요. 지난 작업에서도 사라져요.", "Deletes the task record and its report (evidence and attachments). It also leaves Past tasks."),
        DlgReviewDeleteRoutine => ("이 실행의 보고서(근거·첨부)를 지워요. 루틴 실행 기록은 남아요.", "Deletes this run's report (evidence and attachments). The routine's run history stays."),
        DlgReviewDeleteKeep => ("Claude 세션 기록과 git worktree·브랜치는 그대로예요.", "The Claude session and the git worktree and branch stay."),
        DlgReviewDeleteNoUndo => ("지운 기록과 보고서는 되돌릴 수 없어요.", "The deleted record and report can't be restored."),
        DlgReviewDeleteWorktree => ("worktree가 남아 있으면 앱에서 더 정리할 수 없어요. 먼저 작업 탭 정리에서 worktree를 지워 주세요.", "If its worktree is still there, the app can't clean it up afterwards. Please remove the worktree first from wrap-up on the Tasks tab."),
        DlgPromptsOffTitle => ("작업 지시 저장 끄기", "Stop saving task instructions"),
        DlgPromptsOffBody => (
            "앞으로 새 작업의 지시를 저장하지 않아요.\n\n이미 저장된 지시도 지울까요? (작업 유형·리뷰했어요 표시는 남아요)",
            "Instructions for new tasks won't be saved from now on.\n\nDelete the ones already saved too? (Task types and reviewed marks stay.)",
        ),
        // ---- agents ----
        RiskHooks => ("· hooks: 도구 실행 전후에 명령을 실행할 수 있어요", "· hooks: can run commands before and after tools"),
        RiskMcp => ("· mcpServers: 외부 서버·프로그램을 띄울 수 있어요", "· mcpServers: can start outside servers and programs"),
        RiskDangerMode => ("· 확인을 건너뛰는 권한 모드가 들어 있어요", "· includes a permission mode that skips confirmation"),
        RiskUnreadable => ("· D.Duck이 해석하지 못하는 frontmatter 구조가 있어요", "· has frontmatter D.Duck can't read"),
        DlgAgentSaveTitle => ("agent 정의 저장", "Save agent definition"),
        DlgAgentSaveBody => ("{file}\n{path}\n\n바뀐 줄: {changed}\n\n저장될 frontmatter:\n{frontmatter}", "{file}\n{path}\n\nChanged lines: {changed}\n\nFrontmatter to save:\n{frontmatter}"),
        DlgAgentSaveWarn => ("⚠ 이 agent가 실행할 수 있는 일이 바뀔 수 있어요.", "⚠ What this agent can run may change."),
        DlgAgentRisks => ("이 agent는 명령을 실행할 수 있는 설정을 가지고 있어요:", "This agent is set up to run commands:"),
        DlgLoadoutTitle => ("에이전트 기본 장비 저장", "Save the agent's default loadout"),
        DlgLoadoutServers => ("쓸 서버: {servers}\n미리 읽을 스킬: {skills}", "Servers: {servers}\nSkills to preload: {skills}"),
        LoadoutAllServers => ("모든 서버", "All servers"),
        LoadoutNoServers => ("없음(MCP 안 씀)", "None (no MCP)"),
        LoadoutNone => ("없음", "None"),
        DlgLoadoutOutside => ("목록 밖 서버(claude.ai 커넥터·플러그인 MCP)는 막지 못해요", "Servers outside this list (claude.ai connectors, plugin MCP) can't be blocked"),
        DlgLoadoutUnblock => ("⚠ 막아 둔 MCP 서버를 풀어요: {list}", "⚠ Unblocks these MCP servers: {list}"),
        DlgLoadoutStrict => ("⚠ 엄격 유형({types})이 이 에이전트로 시작해요 · 더한 스킬과 서버가 그 작업에도 들어가요", "⚠ Strict types ({types}) start with this agent · The added skills and servers go into those tasks too"),
        DlgLoadoutFoot => ("frontmatter의 이 줄만 바꾸고 본문은 그대로 둬요. 저장할까요?", "Only these frontmatter lines change; the body stays. Save?"),
        DlgToolsTitle => ("agent 도구 제한 저장", "Save the agent's tool limits"),
        DlgToolsAllowList => ("tools(허용 목록)", "tools (allow list)"),
        DlgToolsInheritAll => ("(전부 상속)", "(inherits all)"),
        DlgToolsFoot => ("frontmatter의 이 두 줄만 바꾸고 본문은 그대로 둬요. 저장할까요?", "Only these two frontmatter lines change; the body stays. Save?"),
        DlgAgentTrashTitle => ("agent 삭제", "Delete agent"),
        DlgAgentTrashBody => ("agent '{name}'의 정의 파일을 휴지통으로 옮겨요.\n{path}\n\nFinder 휴지통에서 되살릴 수 있어요. 옮길까요?", "Moves the definition file of agent '{name}' to the Trash.\n{path}\n\nYou can restore it from the Trash in Finder. Move it?"),
        // ---- permissions ----
        PermScopeUser => ("전역 권한", "Global permissions"),
        PermScopeShared => ("프로젝트(공유) 권한", "Project (shared) permissions"),
        PermScopeLocal => ("프로젝트(개인) 권한", "Project (personal) permissions"),
        DlgPermEdit => ("이 파일의 permissions만 바꿔요:", "Only the permissions in this file change:"),
        DlgPermNew => ("새 파일을 만들어요:", "Creates a new file:"),
        DlgPermAdded => ("추가:", "Added:"),
        DlgPermRemoved => ("삭제:", "Removed:"),
        DlgPermMode => ("기본 모드: {old} → {new}", "Default mode: {old} → {new}"),
        DlgPermHighAllow => ("⚠ 위험도 높음 규칙을 허용해요:", "⚠ Allows high-risk rules:"),
        DlgPermHighDenyRemoved => ("⚠ 위험도 높음 거부 규칙을 없애요:", "⚠ Removes high-risk deny rules:"),
        DlgPermRiskyMode => ("⚠ 이 기본 모드에서는 Claude가 확인 없이 파일을 고치거나 도구를 실행할 수 있어요.", "⚠ In this default mode Claude can edit files or run tools without asking."),
        DlgPermFoot => ("permissions 밖의 설정은 그대로 둬요. 이전 permissions는 앱 데이터 폴더에 백업해요.\n\n저장할까요?", "Settings outside permissions stay as they are. The previous permissions are backed up in the app data folder.\n\nSave?"),
        // ---- team ----
        DlgTeamTitle => ("팀 설정 저장", "Save team settings"),
        DlgTeamBody => (
            "이 변경은 D.Duck 작업이 확인 없이 할 수 있는 일을 넓혀요:\n\n{list}\n\n위험한 권한(auto·bypassPermissions)과 민감 영역 제한은 그대로 적용돼요. 저장할까요?",
            "This change widens what D.Duck tasks can do without asking:\n\n{list}\n\nRisky permissions (auto, bypassPermissions) and sensitive-area limits still apply. Save?",
        ),
        // ---- start ----
        DlgAdviceTitle => ("개선 제안에서 작업 시작", "Start a task from a suggestion"),
        DlgAdviceBody => (
            "외부 검색 기반 권고에서 시작한 작업이에요. 권고 문장은 지시가 아니라 참고 자료로만 들어가요. 계획만 세우기로 시작해요.\n\n폴더: {folder}\n\n브리프 전문:\n{brief}",
            "This task starts from advice based on a web search. The advice goes in only as reference, not as instructions. It starts in plan mode.\n\nFolder: {folder}\n\nFull brief:\n{brief}",
        ),
        DlgNewsTaskTitle => ("소식에서 작업 시작", "Start a task from news"),
        DlgNewsTaskBody => (
            "소식(공식 문서를 Claude가 요약한 글)에서 시작한 작업이에요. 소식 글은 지시가 아니라 참고 자료로만 들어가요. 계획 모드로 시작해요.\n\n폴더: {folder}\n\n브리프 전문:\n{brief}",
            "This task starts from news (Claude's summary of the official docs). The news text goes in only as reference, not as instructions. It starts in plan mode.\n\nFolder: {folder}\n\nFull brief:\n{brief}",
        ),
        DlgPersonaTitle => ("위험할 수 있는 페르소나", "Persona that may be risky"),
        DlgPersonaBody => (
            "페르소나 '{name}'의 정의에 확인 없이 명령을 실행할 수 있는 설정이 있어요.\n{path}\n\n{risks}\n\n작업: {type} · 권한 {mode} · 민감 영역 {sensitive} · worktree {worktree}\n폴더: {folder}\n\n이 페르소나로 실행할까요?",
            "Persona '{name}' is set up to run commands without asking.\n{path}\n\n{risks}\n\nTask: {type} · Permission {mode} · Sensitive {sensitive} · worktree {worktree}\nFolder: {folder}\n\nRun with this persona?",
        ),
        NoType => ("유형 없음", "No type"),
        Yes => ("예", "Yes"),
        No => ("아니오", "No"),
        On => ("켬", "On"),
        Off => ("끔", "Off"),
        NoName => ("(이름 없음)", "(no name)"),
        DlgDangerTitle => ("위험한 권한 모드", "Risky permission mode"),
        DlgDangerBody => (
            "권한 모드 '{mode}'로 실행하면 Claude가 승인 없이 파일 수정과 명령 실행을 할 수 있어요.\n\n디렉터리: {folder}\n이름: {name}\n프롬프트: {prompt}{persona}\n\n실행할까요?{auto}",
            "In permission mode '{mode}', Claude can edit files and run commands without approval.\n\nFolder: {folder}\nName: {name}\nPrompt: {prompt}{persona}\n\nRun?{auto}",
        ),
        DlgDangerPersona => ("\n페르소나 '{name}':\n{risks}", "\nPersona '{name}':\n{risks}"),
        DlgDangerAuto => ("\n사용량 한도로 멈추면 리셋 후 자동으로 이어서 할 수도 있어요(이어서 하는 동안 사용량을 써요).", "\nIf it stops at a usage limit, it can continue on its own after the reset (continuing uses your usage)."),
        DlgTrustTitle => ("폴더 신뢰 확인", "Trust this folder"),
        DlgTrustBody => (
            "Claude가 아직 이 폴더를 신뢰하지 않아 작업을 시작할 수 없어요.\n\n{folder}\n\n터미널에서 Claude를 열어 신뢰 여부를 물어요. 'Yes, I trust this folder'를 고르면 작업이 바로 시작돼요.",
            "Claude doesn't trust this folder yet, so the task can't start.\n\n{folder}\n\nClaude opens in Terminal and asks whether you trust it. Choose 'Yes, I trust this folder' and the task starts right away.",
        ),
        // ---- routines ----
        DlgCloudTitle => ("클라우드 루틴 바꾸기", "Change cloud routine"),
        DlgCloudBody => (
            "‘{name}’ 루틴을 claude.ai에서 바꿔요.\n\n{changes}\n\n이전 값은 {minutes}분 전에 불러온 목록 기준이에요. 보내기 전에 현재 값을 다시 확인하고, 다르면 바꾸지 않아요.\n설치된 Claude CLI로 보내요(약 $0.002). 지시문·커넥터는 바꾸지 않아요.",
            "Changes the ‘{name}’ routine on claude.ai.\n\n{changes}\n\nThe previous values are from the list loaded {minutes} min ago. The current values are checked again before sending, and nothing changes if they differ.\nSent through the installed Claude CLI (about $0.002). Instructions and connectors don't change.",
        ),
        ModePlan => ("계획만 세우고 파일을 고치거나 명령을 실행하지 않아요.", "Only plans; doesn't edit files or run commands."),
        ModeManual => ("고치거나 실행할 때마다 승인을 기다려요. 무인 실행 중에는 '답 기다림'으로 멈출 수 있어요.", "Waits for approval before each edit or command. Unattended runs may stop at 'Waiting for an answer'."),
        ModeAcceptEdits => (
            "사람 확인 없이 이 폴더의 파일을 고쳐요. git hook, 빌드 스크립트, Claude 설정 파일이 바뀌면 다음 실행 때 명령이 실행될 수 있어요.",
            "Edits files in this folder without asking. If git hooks, build scripts or Claude settings change, commands may run on the next run.",
        ),
        ModeDontAsk => ("설정의 허용 규칙에 있는 도구는 확인 없이 실행돼요.", "Tools in your allow rules run without asking."),
        DlgRoutineNewTitle => ("새 루틴 확인", "Confirm new routine"),
        DlgRoutineEditTitle => ("루틴 변경 확인", "Confirm routine change"),
        DlgRoutineBody => (
            "앱이 켜져 있으면 사용자가 없을 때도 이 루틴이 실행돼요.\n\n이름: {name}\n폴더: {folder}\n일정: {schedule}\n권한: {mode} — {meaning}\n지시({chars}자): {prompt}\n\n저장할까요?",
            "While the app is open, this routine runs even when you're away.\n\nName: {name}\nFolder: {folder}\nSchedule: {schedule}\nPermission: {mode} — {meaning}\nInstructions ({chars} characters): {prompt}\n\nSave?",
        ),
        // ---- sessions ----
        DlgKillTitle => ("세션 강제 종료", "Force quit session"),
        DlgKillBody => (
            "터미널에서 실행 중인 세션 '{name}'을(를) 강제 종료해요.\n보내지 않은 입력과 진행 중인 응답은 사라질 수 있어요. 대화 기록은 남아요.\n\n종료할까요?",
            "Force quits session '{name}' running in Terminal.\nUnsent input and the reply in progress may be lost. The conversation history stays.\n\nQuit?",
        ),
    }
}

/// A local clock time for OS text: "오후 1:50" / "1:50 PM", with the date when `date` is given
/// ("11월 12일(목) 오전 9:00" / "Thu, Nov 12, 9:00 AM"). `date` is (month 1–12, day, weekday from Monday 0–6).
pub fn clock_label(lang: Lang, pm: bool, h12: u32, minute: u32, date: Option<(u32, u32, usize)>) -> String {
    match lang {
        Lang::Ko => {
            let clock = format!("{} {h12}:{minute:02}", if pm { "오후" } else { "오전" });
            match date {
                Some((m, d, w)) => {
                    const DAYS: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];
                    format!("{m}월 {d}일({}) {clock}", DAYS[w % 7])
                }
                None => clock,
            }
        }
        Lang::En => {
            let clock = format!("{h12}:{minute:02} {}", if pm { "PM" } else { "AM" });
            match date {
                Some((m, d, w)) => {
                    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
                    format!("{}, {} {d}, {clock}", DAYS[w % 7], MONTHS[(m.max(1) as usize - 1) % 12])
                }
                None => clock,
            }
        }
    }
}

/// Bundled data with `{"ko": …, "en": …}` text fields (team defaults, permission catalog, templates): every such
/// object becomes the string for `lang`; everything else is kept. Data the user changed is stored as plain
/// strings and never goes through here, so it is never translated.
pub fn pick_lang(v: serde_json::Value, lang: Lang) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(m) if m.len() == 2 && m.get("ko").is_some_and(Value::is_string) && m.get("en").is_some_and(Value::is_string) => {
            m.get(lang.as_str()).cloned().unwrap_or(Value::Null)
        }
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, x)| (k, pick_lang(x, lang))).collect()),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| pick_lang(x, lang)).collect()),
        other => other,
    }
}

const WEEKDAYS_KO: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];
const WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Short weekday names, Monday first.
pub fn weekdays(lang: Lang) -> [&'static str; 7] {
    if lang == Lang::Ko { WEEKDAYS_KO } else { WEEKDAYS_EN }
}

/// "매일 09:00" / "Daily 09:00", "매주 월·수 09:00" / "Weekly Mon·Wed 09:00" (24-hour in both languages).
pub fn schedule_text(lang: Lang, weekdays_mask: Option<u8>, hour: u32, minute: u32) -> String {
    let time = format!("{hour:02}:{minute:02}");
    match weekdays_mask {
        None => msg(lang, Code::SchedDaily, &Params::new().text("time", &time, 8)),
        Some(mask) => {
            let days: Vec<&str> = (0..7).filter(|i| mask & (1 << i) != 0).map(|i| weekdays(lang)[i]).collect();
            msg(lang, Code::SchedWeekly, &Params::new().text("days", &days.join("·"), 40).text("time", &time, 8))
        }
    }
}

/// `code` in the current language, without parameters (dialog titles, buttons, fixed lines).
pub fn tr(code: Code) -> String {
    text(current(), code).to_string()
}

/// `code` in the current language with its parameters.
pub fn trp(code: Code, params: &Params) -> String {
    msg(current(), code, params)
}

/// The template of `code` in `lang`, without parameters (for fixed labels).
pub fn text(lang: Lang, code: Code) -> &'static str {
    let (ko, en) = code.templates();
    if lang == Lang::Ko { ko } else { en }
}

table! {
    /// Why an input was refused (`CliError::InvalidInput`). Sent as a string code with its params and a fallback
    /// sentence; the webview words it from its own catalog and uses the sentence only for a code it does not know.
    ReasonCode {
        TaskTypeFormat => ("작업 유형 형식이 올바르지 않아요", "The task type isn't valid"),
        TaskTypeUnknown => ("설정에 없는 작업 유형이에요", "This task type isn't in your settings"),
        PermissionNotAllowed => ("이 작업 유형에서는 쓸 수 없는 권한이에요", "This permission mode isn't allowed for this task type"),
        LoadoutChanged => (
            "장비 스킬이 바뀌어 엄격 유형 작업을 시작할 수 없어요. 에이전트 설정에서 장비를 다시 골라 주세요",
            "Loadout skills changed, so this strict task can't start. Pick the loadout again in agent settings",
        ),
        McpUnreadable => ("MCP 목록을 못 읽었어요 · 설정 파일을 확인한 뒤 다시 시작해 주세요", "Couldn't read the MCP list · Check the config file and start again"),
        RoleFailed => ("역할을 만들지 못했어요", "Couldn't build the role"),
        SensitiveModes => ("민감 영역 작업은 계획만 세우기나 매번 확인만 쓸 수 있어요", "Sensitive tasks can only use plan mode or ask every time"),
        WorktreeUnavailable => ("이 폴더는 worktree로 격리할 수 없어요", "This folder can't be isolated in a worktree"),
        RemoteRisky => ("위험한 작업에는 휴대폰 원격 조작을 켤 수 없어요", "Phone remote control can't be on for risky tasks"),
        TooLong => ("지시와 설정이 너무 길어요", "The instructions and settings are too long"),
        AdviceNoRemote => ("개선 제안에서 시작한 이 작업에는 휴대폰 원격 조작을 켤 수 없어요", "Phone remote control can't be on for a task started from a suggestion"),
        AdvicePlanOnly => ("개선 제안에서 시작한 이 작업은 계획만 세우기로만 시작할 수 있어요", "A task started from a suggestion can only start in plan mode"),
        NewsNoRemote => ("소식에서 시작한 작업에는 휴대폰 원격 조작을 켤 수 없어요", "Phone remote control can't be on for a task started from news"),
        NewsPlanOnly => ("소식에서 시작한 작업은 계획 모드로만 시작할 수 있어요", "A task started from news can only start in plan mode"),
        DirNotFound => ("디렉터리를 찾을 수 없어요", "Folder not found"),
        PromptSize => ("프롬프트는 1바이트 이상 100KB 이하로 써 주세요", "The prompt must be 1 byte to 100 KB"),
        NameFormat => ("이름은 1~64자로, '-'로 시작하지 않고 제어문자 없이 써 주세요", "Names are 1–64 characters, don't start with '-', and have no control characters"),
        AgentNameFormat => ("에이전트 이름 형식이 올바르지 않아요", "The agent name isn't valid"),
        ScopeFixed => ("바꿀 수 없는 설치 범위예요", "This install scope can't be changed"),
        PluginId => ("잘못된 플러그인 ID이에요", "The plugin ID isn't valid"),
        RepoName => ("잘못된 저장소 이름이에요", "The repository name isn't valid"),
        ResumeNoReattach => ("이어서 하기에는 worktree·원격 조작을 다시 붙이지 않아요", "Continuing doesn't reattach the worktree or remote control"),
        ResumeNoScope => ("전달 범위를 계산하지 못해 이어서 하지 않아요", "Couldn't work out the delivery scope, so the task wasn't continued"),
        AgentNotHere => ("이 폴더에서 쓸 수 없는 에이전트예요", "This agent isn't available in this folder"),
        RemoteAlertsOff => ("설정에서 휴대폰 알림을 먼저 켜세요", "Turn on phone alerts in Settings first"),
        AgentOff => ("꺼 둔 에이전트예요. 에이전트 설정에서 켜야 이 역할로 시작할 수 있어요", "This agent is off. Turn it on in agent settings to start with this role"),
        StrictPersonaEdits => ("엄격 유형 작업에는 파일을 고치거나 명령을 실행할 수 있는 페르소나를 쓸 수 없어요", "Strict tasks can't use a persona that can edit files or run commands"),
        StrictPersonaCommands => (
            "엄격 유형(배포·보안·QA 등) 작업에는 명령을 실행할 수 있는 설정을 가진 페르소나를 쓸 수 없어요",
            "Strict tasks (deploy, security, QA and similar) can't use a persona set up to run commands",
        ),
        PersonaNoRemote => ("명령을 실행할 수 있는 설정을 가진 페르소나에는 휴대폰 원격 조작을 켤 수 없어요", "Phone remote control can't be on for a persona set up to run commands"),
        SensitivePersona => ("민감 영역 작업에는 명령을 실행할 수 있는 설정을 가진 페르소나를 쓸 수 없어요", "Sensitive tasks can't use a persona set up to run commands"),
        ConfirmOpen => ("다른 확인 창이 열려 있어요", "Another confirmation is already open"),
        RecordFailedRunning => (
            "작업 기록을 남기지 못했고 작업도 멈추지 못했어요. 작업 목록에서 직접 멈춰 주세요",
            "Couldn't save the task record or stop the task. Stop it from the task list",
        ),
        RecordFailedStopped => ("작업 기록을 남기지 못해 작업을 멈췄어요", "Couldn't save the task record, so the task was stopped"),
        StoppedNotDone => ("작업을 멈췄지만 다음 칸으로 넘기지 못했어요", "Stopped the task but couldn't move it on"),
        SessionFolder => ("세션 폴더를 열 수 없어요", "Couldn't open the session folder"),
        AppDataFolder => ("앱 데이터 폴더를 만들지 못했어요", "Couldn't create the app data folder"),
        JobFolder => ("작업 폴더를 만들지 못했어요", "Couldn't create the task folder"),
        RequestLength => ("요청은 1자 이상 20,000자 이하로 써 주세요", "Write a request of 1 to 20,000 characters"),
        GuardUnreadable => (crate::delivery_guard::UNREADABLE, "Couldn't read the remote push block settings"),
        GuardHookMissing => (crate::delivery_guard::HOOK_MISSING, "Couldn't prepare the D.Duck hook that local-only tasks need"),
        GuardManagedBlocks => (crate::delivery_guard::MANAGED_BLOCKS, "Managed settings block the D.Duck hook, so a local-only task can't start"),
        GuardDangerRefused => (crate::delivery_guard::DANGER_REFUSED, "Local-only projects can't use this permission mode yet"),
        ModelNameFormat => ("모델 이름 형식이 아니에요", "Not a model name"),
        ModelStoredBad => ("저장된 모델 이름이 올바르지 않아요", "The saved model name isn't valid"),
        ModelAgentFixed => ("이 에이전트는 파일에 정한 모델로 돌아요 · 이번 작업에서 모델을 바꿀 수 없어요", "This agent runs on the model set in its file · The model can't change for this task"),
        ModelManagedBlock => ("관리 설정이 이 모델을 막아요", "Managed settings block this model"),
        ModelNotAllowed => ("설정의 허용 목록에 없는 모델이에요", "This model isn't in the settings allow list"),
        ModelAllowUnknown => ("허용 목록을 확인할 수 없어 막았어요", "Couldn't check the allow list, so it was blocked"),
        ModelNoFallback => ("대신 쓸 모델이 없어 시작하지 않았어요 · 더 비싼 모델로는 바꾸지 않아요", "No model to use instead, so it didn't start · It never switches to a pricier model"),
        ModelAgentUnsupported => ("에이전트 파일의 모델을 쓸 수 없어요 · 에이전트 설정에서 바꿔 주세요", "The model in the agent file isn't available · Change it in agent settings"),
        // Plain-string command errors the screens branch on (`reasonIs` in api.ts matches either language).
        Cancelled => ("취소했어요", "Cancelled"),
        ListChanged => ("목록이 바뀌었어요. 다시 열어 주세요", "The list changed. Open it again"),
        ChangedOutside => ("파일이 밖에서 바뀌었어요. 다시 불러오세요", "The file changed outside the app. Load it again"),
        FileChanged => ("파일이 바뀌었어요. 다시 불러오세요", "The file changed. Load it again"),
        // windows-support: features whose protection is not verified on Windows (decision 7), and the CreateProcess limit.
        MacOnly => ("Mac 전용이에요 · Windows 미리보기에서는 아직 쓸 수 없어요", "Mac only · Not available in the Windows preview yet"),
        WindowsCommandTooLong => ("Windows에서는 지시와 설정이 너무 길어요 · 줄여서 다시 보내 주세요", "The instructions and settings are too long for Windows · Shorten them and try again"),
        CliUpdateNeeded => ("Claude Code를 업데이트해 주세요 · 2.1.295 이상이 필요해요", "Update Claude Code · Version 2.1.295 or later is needed"),
    }
}

impl ReasonCode {
    /// The fallback sentence in the current language (for commands that answer with a plain string).
    pub fn now(self) -> String {
        Reason::from(self).text(current())
    }

    /// The string code the webview reads (`errors.invalidInput.<code>`): the variant name in lowerCamelCase.
    pub fn code(self) -> String {
        let name = format!("{self:?}");
        let mut c = name.chars();
        c.next().map(|f| f.to_ascii_lowercase().to_string() + c.as_str()).unwrap_or_default()
    }

    /// A delivery-guard refusal (`delivery_guard` keeps its fixed Korean constants) as a code. Anything it does not
    /// recognise reads as "unreadable", the fail-closed meaning.
    pub fn from_guard(reason: &str) -> ReasonCode {
        use crate::delivery_guard as dg;
        if reason == dg::HOOK_MISSING {
            ReasonCode::GuardHookMissing
        } else if reason == dg::MANAGED_BLOCKS {
            ReasonCode::GuardManagedBlocks
        } else if reason == dg::DANGER_REFUSED {
            ReasonCode::GuardDangerRefused
        } else if reason == dg::MAC_ONLY {
            ReasonCode::MacOnly
        } else {
            ReasonCode::GuardUnreadable
        }
    }
}

table! {
    /// Plain-string command errors (the shell answers `Err(String)`): the sentence is made in the current language,
    /// and the webview shows it as sent (`messageOf` passes a string through).
    Fail {
        CliNotFound => ("Claude CLI를 찾을 수 없어요", "Couldn't find the Claude CLI"),
        AlreadyAnalyzing => ("이미 분석 중이에요", "Already analyzing"),
        NewsBusy => ("이미 소식을 가져오는 중이에요", "Already fetching news"),
        NewsBudget => ("예산 상한($0.40)에 닿아 멈췄어요", "Stopped at the budget limit ($0.40)"),
        NewsTurns => ("읽기·응답 횟수 상한에 닿아 멈췄어요", "Stopped at the read and response limit"),
        NewsTimeout => ("240초 안에 끝나지 않아 멈췄어요", "Stopped because it didn't finish within 240 seconds"),
        NewsMalformed => ("답의 형식이 맞지 않아 아무것도 저장하지 않았어요", "The answer wasn't in the expected format, so nothing was saved"),
        NewsNoSource => ("열 수 있는 원문 주소가 없어요", "There's no source link that can be opened"),
        GuardUnchecked => ("보안 가드를 확인하지 못해 실행하지 않았어요", "Couldn't check the safety guard, so nothing ran"),
        NonceFailed => ("난수를 만들지 못했어요", "Couldn't make a random value"),
        HttpsOnly => ("https 링크만 열 수 있어요", "Only https links can be opened"),
        UnknownSession => ("알 수 없는 세션이에요", "Unknown session"),
        BadRawRequest => ("잘못된 원문 요청이에요", "Invalid transcript request"),
        HistoryFolderMissing => ("기록 폴더를 찾지 못했어요", "Couldn't find the history folder"),
        SessionFileMissing => ("이 세션의 기록 파일을 찾지 못했어요", "Couldn't find this session's history file"),
        SessionFileChanged => ("세션 파일이 바뀌었어요. 흐름을 다시 여세요", "The session file changed. Open the flow again"),
        JustChecked => ("방금 확인했어요. 잠시 뒤 다시 해 주세요", "Just checked. Try again in a moment"),
        AlreadyChecking => ("이미 확인하는 중이에요", "Already checking"),
        Timeout60 => ("60초 안에 끝나지 않았어요", "Didn't finish within 60 seconds"),
        NoConnectorList => ("Claude CLI가 커넥터 목록을 돌려주지 않았어요", "The Claude CLI didn't return the connector list"),
        PluginListTimeout => ("20초 안에 플러그인 목록을 받지 못했어요", "Didn't get the plugin list within 20 seconds"),
        NoPluginList => ("Claude CLI가 플러그인 목록을 돌려주지 않았어요", "The Claude CLI didn't return the plugin list"),
        PluginListFormat => ("지원되지 않는 플러그인 목록 형식이에요", "Unsupported plugin list format"),
        PluginBusy => ("다른 플러그인 작업이 진행 중이에요", "Another plugin action is in progress"),
        BadPluginId => ("잘못된 플러그인 ID이에요", "The plugin ID isn't valid"),
        PluginNotInstalled => ("설치된 플러그인이 아니에요", "This plugin isn't installed"),
        GlobalPluginsOnly => ("전역 플러그인만 여기서 바꿀 수 있어요", "Only global plugins can be changed here"),
        BadTaskId => ("잘못된 작업 ID이에요", "Invalid task ID"),
        BadSaveScope => ("저장할 범위가 올바르지 않아요", "The scope to save isn't valid"),
        NoChanges => ("바뀐 내용이 없어요", "Nothing changed"),
        AgentNotListed => ("목록에 없는 에이전트예요", "This agent isn't in the list"),
        TooManyRules => ("규칙이 너무 많아요", "Too many rules"),
        TeamChanged => ("그 사이 팀 설정이 바뀌었어요. 다시 열어 주세요", "Team settings changed in the meantime. Open them again"),
        SettingsUnreadable => ("설정 파일을 읽지 못했어요", "Couldn't read the settings file"),
        ProjectFolderUnknown => ("프로젝트 폴더를 알 수 없어요", "The project folder is unknown"),
        ProjectNotListed => ("목록에 없는 프로젝트예요", "This project isn't in the list"),
        LocalNotIgnored => (".claude/settings.local.json이 .gitignore에 없어요. 커밋되지 않게 추가해 두세요.", ".claude/settings.local.json isn't in .gitignore. Add it so it isn't committed."),
        BadPath => ("경로를 처리할 수 없어요", "Couldn't handle the path"),
        TrashFailed => ("휴지통으로 옮기지 못했어요", "Couldn't move it to the Trash"),
        TrashMacOnly => ("휴지통 이동은 macOS에서만 지원해요", "Moving to the Trash works only on macOS"),
        AgentInUse => ("이 agent를 쓰는 작업이 실행 중이에요. 끝난 뒤에 지우세요", "A task using this agent is running. Delete it after it ends"),
        BadAgentName => ("agent 이름이 올바르지 않아요", "The agent name isn't valid"),
        LinkNotAllowed => ("허용되지 않은 링크예요", "This link isn't allowed"),
        TryLater => ("잠시 후 다시 시도하세요", "Try again in a moment"),
        BrowserFailed => ("브라우저를 열지 못했어요", "Couldn't open the browser"),
        CopyLinkInstead => ("이 OS에서는 링크 복사를 사용하세요", "On this OS, copy the link instead"),
        NotCollectedArtifact => ("수집된 아티팩트가 아니에요", "This isn't a collected artifact"),
        AlreadyLoading => ("이미 불러오는 중이에요", "Already loading"),
        CliSpawn => ("Claude CLI를 실행하지 못했어요", "Couldn't run the Claude CLI"),
        CliFailedRetry => ("Claude CLI가 실패했어요. 다시 시도하세요", "The Claude CLI failed. Try again"),
        CliFailed => ("Claude CLI가 실패했어요", "The Claude CLI failed"),
        EmptyReply => ("응답이 비어 있어요", "The reply is empty"),
        BadRoutineId => ("잘못된 루틴 ID이에요", "Invalid routine ID"),
        RequestFailed => ("요청을 만들지 못했어요", "Couldn't build the request"),
        CloudBusy => ("클라우드 루틴 작업이 이미 진행 중이에요", "A cloud routine action is already in progress"),
        BadRunId => ("잘못된 실행 ID이에요", "Invalid run ID"),
        CannotChange => ("바꿀 수 없는 내용이에요", "This can't be changed"),
        ChangedElsewhere => ("다른 곳에서 바뀌었어요. 다시 불러오세요", "It changed somewhere else. Load it again"),
        RoutineNotFound => ("루틴을 찾을 수 없어요", "Couldn't find the routine"),
        AlreadyRunning => ("이미 실행 중이에요", "Already running"),
        FolderNameTerminal => ("이 폴더 이름은 터미널에서 열 수 없어요", "This folder name can't be opened in Terminal"),
        ReportUnreadable => ("보고서를 읽지 못했어요", "Couldn't read the report"),
        EvidenceUnreadable => ("근거를 읽지 못했어요", "Couldn't read the evidence"),
        AttachmentUnreadable => ("첨부를 읽지 못했어요", "Couldn't read the attachment"),
        BadRequest => ("잘못된 요청이에요", "Invalid request"),
        ProjectFolderMissing => ("이 프로젝트 폴더를 찾지 못했어요", "Couldn't find this project folder"),
        ReviewListUnreadable => ("리뷰 목록을 읽지 못했어요", "Couldn't read the review list"),
        NotReviewable => ("아직 리뷰할 수 없는 작업이에요", "This task can't be reviewed yet"),
        BoardUnreadable => ("작업 보드를 읽지 못했어요", "Couldn't read the task board"),
        NotWrappable => ("아직 정리할 수 없는 작업이에요", "This task can't be wrapped up yet"),
        UndoFailed => ("되돌리지 못했어요", "Couldn't undo"),
        WrapupUnchecked => ("정리할 내용을 확인하지 못했어요", "Couldn't check what to clean up"),
        WrapRunning => ("실행 중이라 worktree를 지울 수 없어요", "It's still running, so the worktree can't be removed"),
        WrapChanges => ("커밋 안 된 변경이 있어 worktree를 지울 수 없어요", "There are uncommitted changes, so the worktree can't be removed"),
        DeleteRunning => ("실행 중이라 지울 수 없어요", "It's still running, so it can't be deleted"),
        DeleteResumePending => ("한도 뒤 이어서 하기가 예약돼 있어 지울 수 없어요", "A resume after the usage limit is scheduled, so it can't be deleted"),
        DeleteChanged => ("확인하는 동안 작업이 바뀌어 지우지 않았어요", "The task changed while you were confirming, so nothing was deleted"),
        DeleteFailed => ("지우지 못했어요", "Couldn't delete it"),
        DeleteUnknown => ("작업 상태를 확인하지 못해 지울 수 없어요", "Couldn't check the task's state, so it can't be deleted"),
        WrapResumePending => ("한도 뒤 이어서 하기가 예약돼 있어 정리할 수 없어요", "A resume after the usage limit is scheduled, so it can't be wrapped up"),
        WrapNotChecked => ("변경을 확인하지 못해 worktree를 지울 수 없어요", "Couldn't check the changes, so the worktree can't be removed"),
        WorktreeRemoveFailed => ("worktree를 지우지 못했어요", "Couldn't delete the worktree"),
        NoWorktree => ("지울 worktree가 없어요", "There's no worktree to delete"),
        FolderUnchecked => ("폴더를 확인하지 못했어요", "Couldn't check the folder"),
        NotResumable => ("이어서 할 수 없는 작업이에요", "This task can't be continued"),
        DeployResumeTerminal => ("배포 작업은 터미널에서 직접 이어 주세요", "Continue deploy tasks yourself in Terminal"),
        AlreadyHandled => ("이미 처리된 작업이에요", "This task was already handled"),
        PreviewExpired => ("미리보기가 만료되었어요. 링크를 다시 확인하세요", "The preview expired. Check the link again"),
        MarketFirst => ("먼저 마켓플레이스 링크(GitHub 저장소)를 붙여 넣으세요. 이 마켓플레이스는 아직 추가되지 않았어요", "Paste the marketplace link (GitHub repository) first. This marketplace isn't added yet"),
        PluginNotInMarket => ("마켓플레이스에 없는 플러그인이에요", "This plugin isn't in the marketplace"),
        LinkBusy => ("이미 다른 링크를 확인하는 중이에요", "Already checking another link"),
        UnknownLink => ("알 수 없는 링크예요", "Unknown link"),
        NotPluginLink => ("플러그인 링크가 아니에요", "This isn't a plugin link"),
        NotMarketLink => ("마켓플레이스 링크가 아니에요", "This isn't a marketplace link"),
        NotSkillLink => ("스킬 링크가 아니에요", "This isn't a skill link"),
        ConfigDirMissing => ("Claude 설정 폴더를 찾을 수 없어요", "Couldn't find the Claude config folder"),
        ItemNameFormat => ("이름은 영문·숫자·-·_ 만, 1–64자여야 해요", "Names use letters, numbers, - and _ only, 1–64 characters"),
        ItemNotListed => ("목록에 없는 항목이에요. 다시 열어 주세요", "This item isn't in the list. Open it again"),
        NoDefinitionFile => ("정의 파일이 없어요", "There's no definition file"),
        FileNotFound => ("파일을 찾을 수 없어요", "Couldn't find the file"),
        AlreadyEditable => ("이미 고칠 수 있는 항목이에요", "This item can already be edited"),
        SourceMissing => ("원본을 찾지 못했어요", "Couldn't find the original"),
        CopyPluginOnly => ("플러그인·내장 항목만 복사할 수 있어요", "Only plugin and built-in items can be copied"),
        PickProjectFirst => ("프로젝트를 먼저 골라 주세요", "Pick a project first"),
        BadSavePlace => ("저장 위치가 올바르지 않아요", "The save location isn't valid"),
        JobNotFound => ("작업을 찾을 수 없어요", "Couldn't find the task"),
        ProducedNotFound => ("만든 항목을 찾을 수 없어요", "Couldn't find what was made"),
        DduckNewExists => ("같은 이름(dduck-new.md)의 파일이 이미 있어 설치하지 않았어요", "A file named dduck-new.md already exists, so nothing was installed"),
        InstallRecordFailed => ("설치 기록을 남기지 못했어요", "Couldn't save the install record"),
        NotPristine => ("D.Duck이 설치한 그대로의 파일이 아니라 지우지 않았어요. 직접 확인해 주세요", "The file isn't as D.Duck installed it, so it wasn't deleted. Check it yourself"),
        NoMemoryFolder => ("이 프로젝트에는 메모리 폴더가 없어요", "This project has no memory folder"),
        ChangedWhileChecking => ("확인하는 동안 파일이 바뀌어 지우지 않았어요", "The file changed while checking, so it wasn't deleted"),
        MemoryLineLeft => ("파일은 휴지통으로 옮겼지만 MEMORY.md 줄을 빼지 못했어요", "Moved the file to the Trash but couldn't remove its MEMORY.md line"),
        BadPathValue => ("경로가 올바르지 않아요", "The path isn't valid"),
        EditorFailed => ("편집기를 열지 못했어요", "Couldn't open the editor"),
        UnsupportedOs => ("이 OS에서는 지원하지 않아요", "Not supported on this OS"),
        FileNotListed => ("목록에 없는 파일이에요", "This file isn't in the list"),
        LinkedInstruction => ("링크인 지시 파일은 열지 않아요. Finder에서 직접 여세요", "Linked instruction files aren't opened. Open it in Finder yourself"),
        MemoryIndexMissing => ("MEMORY.md가 없거나 일반 파일이 아니에요", "MEMORY.md is missing or isn't a regular file"),
        UnknownRule => ("저장할 규칙을 알 수 없어요", "Unknown rule to save"),
        HomeNotAllowed => ("홈 폴더는 고를 수 없어요", "The home folder can't be picked"),
        MdmNote => ("MDM 프로필이 있지만 그 안의 규칙은 읽지 않아 반영하지 않았어요", "There's an MDM profile, but its rules aren't read, so they aren't included"),
        RuleNotListed => ("목록에 없는 규칙이에요. 다시 불러오세요", "This rule isn't in the list. Load it again"),
        FileChangedShort => ("파일이 바뀌었어요", "The file changed"),
        RuleExists => ("같은 이름의 규칙이 이미 있어요", "A rule with this name already exists"),
        SaveFileFailed => ("파일을 저장하지 못했어요", "Couldn't save the file"),
        NoHomeRules => ("홈 폴더를 확인하지 못해 프로젝트 규칙은 다루지 않아요", "Couldn't check the home folder, so project rules aren't handled"),
        PathsMax => ("적용 범위는 50개까지 넣을 수 있어요", "Up to 50 paths"),
        PathsSpaces => ("적용 범위 패턴 앞뒤에 빈칸을 넣을 수 없어요", "A path pattern can't start or end with a space"),
        PathsLong => ("적용 범위 패턴이 너무 길어요", "A path pattern is too long"),
        PathsInvisible => ("적용 범위 패턴에 줄바꿈이나 보이지 않는 문자를 넣을 수 없어요", "A path pattern can't have line breaks or invisible characters"),
        FrontComplex => ("앞부분 형식이 복잡해요 · 파일에서 직접 편집해 주세요", "The front matter is complex · Edit it in the file"),
        FrontUnsafe => ("앞부분을 안전하게 바꿀 수 없어요 · 파일에서 직접 편집해 주세요", "The front matter can't be changed safely · Edit it in the file"),
        GlobBracket => ("{a0}: `[`의 짝이 없어 아무 파일과도 맞지 않아요 (`\\[`로 쓰세요)", "{a0}: `[` has no closing pair, so it matches no file (write `\\[`)"),
        GlobOutside => ("{a0}: 기준 폴더 밖은 가리킬 수 없어요", "{a0}: can't point outside the base folder"),
        BraceMax => ("중괄호 확장이 1,000개를 넘어 일부 패턴이 그대로 쓰여요", "More than 1,000 brace expansions, so some patterns are used as written"),
        SettingsLinked => ("설정 파일이 링크라 여기서 바꾸지 않아요", "The settings file is a link, so it isn't changed here"),
        SettingsTooBig => ("설정 파일이 너무 커요", "The settings file is too big"),
        SettingsUnparsable => ("설정 파일을 해석할 수 없어요", "Couldn't parse the settings file"),
        ExcludesFormat => ("claudeMdExcludes 형식이 달라 바꾸지 않아요", "claudeMdExcludes has a different format, so it isn't changed"),
        SharedLink => ("공유 링크 · 원본에서 편집해 주세요", "Shared link · Edit the original"),
        HardlinkReadOnly => ("다른 이름으로도 연결된 파일(하드링크)이라 읽기만 해요", "A file linked under another name too (hard link), so it's read-only"),
        Over256Unread => ("256KB가 넘는 파일이라 읽지 않아요", "The file is over 256 KB, so it isn't read"),
        NotUtf8ReadOnly => ("UTF-8 파일이 아니라 읽기만 해요", "Not a UTF-8 file, so it's read-only"),
        ConfigDirNotFound => ("Claude 설정 폴더를 찾지 못했어요", "Couldn't find the Claude config folder"),
        DotClaudeLinked => (".claude 폴더가 링크라 여기서 바꾸지 않아요", "The .claude folder is a link, so it isn't changed here"),
        RulesDirLinked => ("rules 폴더가 링크라 여기서 바꾸지 않아요", "The rules folder is a link, so it isn't changed here"),
        Text256Only => ("256KB 이하의 텍스트만 저장할 수 있어요", "Only text up to 256 KB can be saved"),
        RulesDirBad => ("규칙 폴더가 링크이거나 폴더가 아니에요", "The rules folder is a link or not a folder"),
        RuleNameFormat => ("이름은 영문·숫자·-·_만, 하위 폴더는 한 단계까지 쓸 수 있어요", "Names use letters, numbers, - and _, with at most one subfolder"),
        RuleBodyDashes => ("본문이 `---`로 시작하면 새 규칙으로 만들 수 없어요", "A body that starts with `---` can't become a new rule"),
        ProjectDirNotFound => ("프로젝트 폴더를 찾지 못했어요", "Couldn't find the project folder"),
        LinkedDirToggle => ("링크 폴더는 여기서 끄거나 켜지 않아요", "Linked folders aren't turned on or off here"),
        OffElsewhere => ("다른 설정에 의해 꺼져 있어요 · 그 설정에서 바꿔 주세요", "Turned off by another setting · Change it there"),
        PathSpecialChars => ("경로에 특수 문자가 있어 끌 수 없어요", "The path has special characters, so it can't be turned off"),
        ConfigDirLinked => ("설정 폴더가 링크이거나 없어요", "The config folder is a link or missing"),
        SaveJsonInvalid => ("저장할 설정이 올바른 JSON이 아니에요", "The settings to save aren't valid JSON"),
        SettingsShape => ("설정 파일 구조를 해석할 수 없어요", "Couldn't parse the settings file structure"),
        GoalRequired => ("목표를 입력하세요", "Enter a goal"),
        PluginItemReadOnly => ("플러그인 항목은 고칠 수 없어요", "Plugin items can't be edited"),
        SyncedReadOnly => ("claude.ai에서 동기화된 항목은 고칠 수 없어요", "Items synced from claude.ai can't be edited"),
        PluginSkillReadOnly => ("플러그인 폴더 안의 스킬이라 여기서 고칠 수 없어요", "A skill inside a plugin folder can't be edited here"),
        HardlinkNoEdit => ("다른 이름으로도 연결된 파일이라 고칠 수 없어요", "A file linked under another name too, so it can't be edited"),
        RefineGuidanceLen => ("다듬기 지침: {MAX_REFINE_GUIDANCE_CHARS}자 이하, 제어 문자 없이", "Refine guidance: up to {MAX_REFINE_GUIDANCE_CHARS} characters, no control characters"),
        RefineQuestions => ("다듬기 지침: 한 번에 묻는 질문은 3~8개", "Refine guidance: 3–8 questions at a time"),
        RefineRounds => ("다듬기 지침: 질문 받기는 1~6번", "Refine guidance: 1–6 rounds of questions"),
        TeamEmpty => ("{what}: 비어 있어요", "{what}: empty"),
        TeamTooLong => ("{what}: 너무 길어요(최대 {MAX_RULE_BYTES}바이트, 한글 약 160자)", "{what}: too long (up to {MAX_RULE_BYTES} bytes, about 160 Korean characters)"),
        TeamInvisible => ("{what}: 줄바꿈·제어 문자·보이지 않는 문자는 쓸 수 없어요", "{what}: line breaks, control and invisible characters aren't allowed"),
        TeamSafePrefix => ("{what}: \"[안전]\"·\"[필수]\"·\"[Safety]\"·\"[Required]\"로 시작할 수 없어요(앱이 자기 규칙에만 붙여요)", "{what}: can't start with an app rule marker such as [Safety] or [Required] (the app adds these to its own rules only)"),
        TypeKeyBad => ("유형 키 '{a0}'가 올바르지 않아요", "The type key '{a0}' isn't valid"),
        TypeLabel => ("유형 '{a0}': 이름은 1~{MAX_LABEL_CHARS}자, 제어 문자 없이", "Type '{a0}': names are 1–{MAX_LABEL_CHARS} characters, no control characters"),
        TypeAgentName => ("유형 '{name}': 에이전트 이름이 올바르지 않아요", "Type '{name}': the agent name isn't valid"),
        TypeBundledOnly => ("유형 '{name}': 배포·보안·QA는 번들 역할만 쓸 수 있어요(내 에이전트로 바꿀 수 없음)", "Type '{name}': deploy, security and QA can use bundled roles only (not your own agent)"),
        TypeRolePrompt => ("유형 '{name}': 역할 프롬프트는 1~8KB, 제어 문자 없이", "Type '{name}': the role prompt is 1–8 KB, no control characters"),
        TypeErr => ("유형 '{name}': {e}", "Type '{name}': {e}"),
        TypeToolsMax => ("유형 '{name}': 도구는 {MAX_TOOLS}개까지", "Type '{name}': up to {MAX_TOOLS} tools"),
        TypeToolErr => ("유형 '{name}': 도구 `{a0}`: {e}", "Type '{name}': tool `{a0}`: {e}"),
        TypeToolDup => ("유형 '{name}': 같은 도구가 두 번 있어요", "Type '{name}': the same tool is listed twice"),
        TypeStrictTools => ("유형 '{name}': 엄격 유형은 도구 목록을 비울 수 없어요(모든 도구 상속 불가)", "Type '{name}': a strict type can't have an empty tool list (can't inherit all tools)"),
        TypeStrictExtra => ("유형 '{name}': 엄격 유형에는 {extra} 도구를 넣을 수 없어요", "Type '{name}': a strict type can't have the {extra} tools"),
        TypeModesBad => ("유형 '{name}': 고를 수 있는 권한이 비었거나 겹쳐요", "Type '{name}': the permission choices are empty or repeated"),
        TypeModesRisky => ("유형 '{name}': 위험한 권한(auto·bypassPermissions)은 넣을 수 없어요", "Type '{name}': risky permissions (auto, bypassPermissions) aren't allowed"),
        TypeStrictModes => ("유형 '{name}': 엄격 유형은 계획만 세우기·매번 확인만 고를 수 있어요", "Type '{name}': a strict type can only choose plan mode or ask every time"),
        TypeDefaultMode => ("유형 '{name}': 기본 권한이 고를 수 있는 권한에 없어요", "Type '{name}': the default permission isn't among the choices"),
        TypeFocusLen => ("유형 '{name}': 다듬기 초점은 {MAX_REFINE_FOCUS_CHARS}자 이하, 제어 문자 없이", "Type '{name}': the refine focus is up to {MAX_REFINE_FOCUS_CHARS} characters, no control characters"),
        TypeAutoRead => ("유형 '{name}': 자동 허용 도구는 읽기 도구(Read·Glob·Grep·git 조회)만", "Type '{name}': auto-allowed tools can only be read tools (Read, Glob, Grep, git queries)"),
        TeamRule => ("규칙", "Rule"),
        TeamDefaultText => ("{e} — 기본 문구를 써요", "{e} — using the default wording"),
        TeamAddendum => ("덧붙인 설명", "Added note"),
        TeamDropAddendum => ("{e} — 덧붙임을 빼요", "{e} — the note is left out"),
        RuleIdBad => ("규칙 id가 올바르지 않아요", "The rule id isn't valid"),
        TeamAddedRule => ("추가한 규칙", "Added rule"),
        TeamDropRule => ("{e} — 그 규칙을 빼요", "{e} — that rule is left out"),
        TeamRulesTrim => ("규칙은 {MAX_RULES}개까지라 뒤쪽 규칙을 빼요", "Up to {MAX_RULES} rules, so the last ones are left out"),
        TeamDefaultValue => ("{e} — 기본값을 써요", "{e} — using the default"),
        TypeKeyDupDrop => ("유형 키 '{a0}'가 겹쳐 빼요", "The type key '{a0}' is repeated, so it's left out"),
        TeamDropType => ("{e} — 그 유형을 빼요", "{e} — that type is left out"),
        TeamStoredBad => ("저장된 팀 설정을 읽지 못해 기본값을 써요", "Couldn't read the saved team settings, so the defaults are used"),
        TeamRulesMax => ("규칙은 {MAX_RULES}개까지예요", "Up to {MAX_RULES} rules"),
        RuleIdDup => ("규칙 id가 올바르지 않거나 겹쳐요", "The rule id isn't valid or is repeated"),
        SafeRuleFixed => ("안전 규칙의 문구는 바꿀 수 없어요. 덧붙임만 쓸 수 있어요", "A safety rule's wording can't change. You can only add a note"),
        SafeRuleKeep => ("안전 규칙은 지울 수 없어요", "Safety rules can't be deleted"),
        TypeKeyDup => ("유형 키 '{a0}'가 올바르지 않거나 겹쳐요", "The type key '{a0}' isn't valid or is repeated"),
        TypeStrictKeep => ("'{a0}' 유형은 엄격 유형을 끌 수 없어요", "The '{a0}' type can't turn off strict"),
        TypesMax => ("유형은 {MAX_ADDED_TYPES}개까지 추가할 수 있어요", "Up to {MAX_ADDED_TYPES} types can be added"),
        TypeBuiltinKeep => ("기본 유형 '{a0}'은 지울 수 없어요. 숨기기를 쓰세요", "The built-in type '{a0}' can't be deleted. Hide it instead"),
        TypeUsedKeep => ("작업 기록에 쓰인 유형 '{a0}'은 지울 수 없어요. 숨기기를 쓰세요", "The type '{a0}' is used in task history, so it can't be deleted. Hide it instead"),
        TeamTooBig => ("팀 설정이 너무 커요", "The team settings are too big"),
        RiskStrictOff => ("'{name}' 유형의 엄격 설정을 꺼요", "Turns off strict for the '{name}' type"),
        RiskNewNotStrict => ("새 유형 '{name}'은 엄격 유형이 아니에요", "The new type '{name}' isn't strict"),
        RiskStrictRole => ("엄격 유형 '{name}'의 역할을 내 에이전트 '{a0}'로 바꿔요", "Changes the strict type '{name}' role to your agent '{a0}'"),
        AllTools => ("모든 도구", "All tools"),
        RiskEditTools => ("'{name}' 역할이 파일 수정·명령 도구를 얻어요: {what}", "The '{name}' role gets edit and command tools: {what}"),
        RiskAllMcp => ("'{name}' 역할이 모든 MCP 서버를 쓸 수 있게 돼요", "The '{name}' role can use every MCP server"),
        RiskMoreMcp => ("'{name}' 역할이 MCP 서버를 더 써요: {a0}", "The '{name}' role uses more MCP servers: {a0}"),
        RiskStrictSkills => ("엄격 유형 '{name}'에 장비 스킬을 더해요: {a0}", "Adds loadout skills to the strict type '{name}': {a0}"),
        RiskModes => ("'{name}' 유형에서 {a0} 권한을 고를 수 있게 돼요", "The '{name}' type can choose the {a0} permission"),
        RiskSafeNote => ("안전 규칙에 덧붙인 설명이 바뀌어요: {a0}", "The note on a safety rule changes: {a0}"),
        StrictAgentEdits => ("엄격 유형 '{a0}'에는 파일을 고치거나 명령을 실행할 수 있는 에이전트 '{a1}'를 쓸 수 없어요(tools에 Write·Edit·Bash 등이 있거나 tools가 없음)", "The strict type '{a0}' can't use the agent '{a1}', which can edit files or run commands (its tools include Write, Edit, Bash and similar, or it has no tools list)"),
        RevertBuiltinOnly => ("기본 유형만 되돌릴 수 있어요", "Only built-in types can be reset"),
        RevertBad => ("되돌릴 항목이 올바르지 않아요", "The item to reset isn't valid"),
        AgentBundledKeep => ("내장·플러그인 agent는 지울 수 없어요. 끄기를 쓰세요", "Built-in and plugin agents can't be deleted. Turn them off instead"),
        AgentMdOnly => ("agent 정의(.md) 파일만 지울 수 있어요", "Only agent definition (.md) files can be deleted"),
        OutsideAgents => ("agents 폴더 밖의 파일이에요", "The file is outside the agents folder"),
        AgentsDirBad => ("agents 폴더가 링크이거나 없어요", "The agents folder is a link or missing"),
        NotRegularOrLinked => ("일반 파일이 아니거나 다른 이름으로도 연결된 파일이에요", "Not a regular file, or a file linked under another name too"),
        ReadFileFailed => ("파일을 읽지 못했어요", "Couldn't read the file"),
        CliNotAbsolute => ("경로가 절대 경로가 아니에요", "The path isn't absolute"),
        CliNotClaude => ("파일 이름이 claude가 아니에요", "The file name isn't claude"),
        NotRegularFile => ("일반 파일이 아니에요", "Not a regular file"),
        CliNoExec => ("실행 권한이 없어요", "No permission to run it"),
        CliWritable => ("다른 사용자가 수정할 수 있는 파일 또는 폴더예요", "Other users can change this file or folder"),
        CliOwner => ("현재 사용자나 시스템 소유의 파일이 아니에요", "The file isn't owned by you or the system"),
        CliBadChars => ("경로에 허용되지 않는 문자(제어문자, \\, !)가 있어요", "The path has characters that aren't allowed (control characters, \\, !)"),
        CliVersionFailed => ("--version 실행에 실패했어요", "Running --version failed"),
        CliNotCode => ("Claude Code 실행 파일이 아니에요", "Not a Claude Code executable"),
        CliShimUnsupported => ("npm 설치본을 확인하지 못했어요 · 공식 설치 프로그램으로 설치해 주세요", "Couldn't check the npm install · Install with the official installer"),
        CliBadCharsWin => ("경로에 허용되지 않는 문자(제어문자, \")가 있거나 이름이 점·공백으로 끝나요", "The path has characters that aren't allowed (control characters, \") or a name ending in a dot or space"),
        RedactPrivateKey => ("[개인 키 숨김]", "[private key hidden]"),
        RedactKey => ("[키 숨김]", "[key hidden]"),
        RedactToken => ("[토큰 숨김]", "[token hidden]"),
        RedactSpace => ("$1 [숨김]", "$1 [hidden]"),
        RedactQuoted => ("$1$2\"[숨김]\"", "$1$2\"[hidden]\""),
        RedactPlain => ("$1$2[숨김]", "$1$2[hidden]"),
        RedactUserinfo => ("$1[인증 정보 숨김]@", "$1[credentials hidden]@"),
        RedactLong => ("[긴 값 숨김]", "[long value hidden]"),
        KindPrivateKey => ("개인 키", "Private key"),
        KindApiKey => ("API 키", "API key"),
        KindGithub => ("GitHub 토큰", "GitHub token"),
        KindSlack => ("Slack 토큰", "Slack token"),
        KindAws => ("AWS 키", "AWS key"),
        KindBearer => ("Bearer 토큰", "Bearer token"),
        KindBasic => ("Basic 인증", "Basic auth"),
        KindSecretFlag => ("비밀 옵션", "Secret option"),
        KindSecretJson => ("비밀 JSON 값", "Secret JSON value"),
        KindSecretSetting => ("비밀 설정값", "Secret setting"),
        KindUrlCreds => ("URL 인증 정보", "URL credentials"),
        KindOpaque => ("긴 불투명 값", "Long opaque value"),
        KindSecret => ("비밀값", "Secret"),
        ToolResult => ("도구 결과", "Tool result"),
        RawRedact => ("${1}[숨김]", "${1}[hidden]"),
        RawDoc => ("[문서 {kb} KB]", "[document {kb} KB]"),
        RawImage => ("[이미지 {kb} KB]", "[image {kb} KB]"),
        Hidden => ("[숨김]", "[hidden]"),
        MemNotListed => ("목록에 없는 메모리예요. 다시 불러오세요", "This memory isn't in the list. Load it again"),
        FileChanged2 => ("파일이 바뀌었어요. 다시 불러오세요", "The file changed. Load it again"),
        MemDirLinked => ("autoMemoryDirectory 경로에 링크가 있어 열지 않아요", "The autoMemoryDirectory path has a link, so it isn't opened"),
        IndexDup => ("MEMORY.md에 이 파일을 가리키는 줄이 둘 이상이에요. 편집기에서 MEMORY.md를 먼저 정리하세요", "MEMORY.md has more than one line pointing to this file. Tidy MEMORY.md in an editor first"),
        IndexOdd => ("MEMORY.md에 이 파일을 다른 형식으로 가리키는 링크가 있어요(한 줄에 링크 여러 개, ./ 경로 등). 편집기에서 MEMORY.md를 먼저 정리하세요", "MEMORY.md links to this file in another form (several links on a line, a ./ path and such). Tidy MEMORY.md in an editor first"),
        OffDup => ("같은 이름의 파일이 메모리 폴더와 .off에 모두 있어요. 하나를 정리한 뒤 다시 시도하세요", "A file with the same name is in both the memory folder and .off. Tidy one and try again"),
        IndexLinked => ("MEMORY.md가 링크이거나 다른 이름으로도 연결된 파일이에요", "MEMORY.md is a link or a file linked under another name too"),
        IndexBig => ("MEMORY.md가 256KB를 넘어 고치지 않아요", "MEMORY.md is over 256 KB, so it isn't changed"),
        IndexNotUtf8 => ("MEMORY.md가 UTF-8 파일이 아니에요", "MEMORY.md isn't a UTF-8 file"),
        MemLineMax => ("{what}은(는) {max}자 이하의 한 줄이어야 해요", "{what} must be one line of up to {max} characters"),
        LabelName => ("이름", "Name"),
        LabelDescription => ("설명", "Description"),
        MemBodyMax => ("본문은 256KB 이하의 텍스트여야 해요", "The body must be text up to 256 KB"),
        MemNoFront => ("frontmatter가 없는 파일은 본문만 고칠 수 있어요", "A file without front matter can only change its body"),
        MemFrontBad => ("frontmatter를 해석할 수 없어 본문만 고칠 수 있어요", "Couldn't parse the front matter, so only the body can change"),
        MemNameEmpty => ("이름을 비울 수 없어요", "The name can't be empty"),
        MemKind => ("종류는 user·feedback·project·reference 중 하나예요", "The type is one of user, feedback, project, reference"),
        Save256Only => ("256KB 이하로만 저장할 수 있어요", "Only up to 256 KB can be saved"),
        HardlinkReadOnly2 => ("다른 이름으로도 연결된 파일(하드링크)이라 읽기 전용이에요", "A file linked under another name too (hard link), so it's read-only"),
        Over256Unread2 => ("256KB를 넘는 파일이라 읽지 않아요", "The file is over 256 KB, so it isn't read"),
        NotUtf8ReadOnly2 => ("UTF-8 파일이 아니라 읽기 전용이에요", "Not a UTF-8 file, so it's read-only"),
        IndexUnreadable => ("MEMORY.md를 읽지 못했어요", "Couldn't read MEMORY.md"),
        LinkNotFollowed => ("폴더 밖이나 올바르지 않은 링크라 따라가지 않아요", "A link outside the folder or not valid, so it isn't followed"),
        CaseDup => ("대소문자만 다른 파일이 여러 개예요", "Several files differ only in letter case"),
        InOff => ("파일이 .off에 있어요 (Claude가 쓰지 않음)", "The file is in .off (Claude doesn't use it)"),
        FileMissing => ("파일이 없어요", "The file is missing"),
        IndexDescFailed => ("메모리는 저장했지만 MEMORY.md 설명은 바꾸지 못했어요", "Saved the memory but couldn't change its MEMORY.md description"),
        OffBad => (".off가 폴더가 아니거나 링크예요", ".off isn't a folder or is a link"),
        MemExists => ("메모리 폴더에 같은 이름의 파일이 이미 있어요", "A file with the same name is already in the memory folder"),
        IndexNameChars => ("파일 이름에 링크로 쓸 수 없는 문자가 있어 색인 줄을 만들 수 없어요", "The file name has characters a link can't use, so no index line was made"),
        IndexHasLine => ("MEMORY.md에 이미 이 메모리의 줄이 있어 새로 넣지 않았어요", "MEMORY.md already has a line for this memory, so none was added"),
        IndexOverflow => ("MEMORY.md가 200줄/25KB를 넘어 새 줄은 시작할 때 읽히지 않아요", "MEMORY.md is over 200 lines or 25 KB, so new lines aren't read at start"),
        OffExists => (".off에 같은 이름의 파일이 이미 있어요", "A file with the same name is already in .off"),
        SettingsDirLinked => ("설정 폴더가 링크라 여기서 바꾸지 않아요", "The settings folder is a link, so it isn't changed here"),
        SettingsJsonLinked => ("settings.json이 링크이거나 다른 이름으로도 연결된 파일이라 여기서 바꾸지 않아요", "settings.json is a link or a file linked under another name too, so it isn't changed here"),
        SettingsJsonBig => ("settings.json이 너무 커요", "settings.json is too big"),
        SettingsJsonBad => ("settings.json을 해석할 수 없어요", "Couldn't parse settings.json"),
        SaveContentJson => ("저장할 내용이 올바른 JSON이 아니에요", "The content to save isn't valid JSON"),
        PluginManaged => ("관리자 설정으로 설치되어 여기서 바꿀 수 없어요", "Installed by managed settings, so it can't be changed here"),
        PluginProjectScope => ("프로젝트 범위 – 해당 프로젝트에서 claude plugin으로 변경", "Project scope – change it with claude plugin in that project"),
        PluginScopeUnknown => ("설치 범위를 알 수 없어 여기서 바꿀 수 없어요", "The install scope is unknown, so it can't be changed here"),
        LinkUnsupported => ("지원하지 않는 링크예요. GitHub 저장소(owner/repo, https://github.com/…), 스킬 폴더 링크, 플러그인 이름(이름@마켓플레이스)을 붙여 넣으세요", "Unsupported link. Paste a GitHub repository (owner/repo, https://github.com/…), a skill folder link or a plugin name (name@marketplace)"),
        DirTooDeep => ("폴더가 너무 깊어요", "The folder is too deep"),
        DirNameChars => ("폴더 이름에 지원하지 않는 문자가 있어요(영문·숫자·. _ - 만)", "The folder name has unsupported characters (letters, numbers, . _ - only)"),
        MarketSubdir => ("하위 폴더의 마켓플레이스는 지원하지 않아요(저장소 루트만)", "Marketplaces in subfolders aren't supported (repository root only)"),
        FileLinkKinds => ("파일 링크는 SKILL.md나 .claude-plugin/marketplace.json만 지원해요", "File links work only for SKILL.md or .claude-plugin/marketplace.json"),
        LinkKinds => ("저장소, 폴더(tree), 파일(blob) 링크만 지원해요", "Only repository, folder (tree) and file (blob) links are supported"),
        GithubOnly => ("GitHub 링크만 지원해요(github.com, raw.githubusercontent.com)", "Only GitHub links are supported (github.com, raw.githubusercontent.com)"),
        GithubRate => ("GitHub 요청 한도(시간당 60회)에 걸렸어요. 잠시 후 다시 시도하세요", "Hit the GitHub request limit (60 per hour). Try again later"),
        RepoNotFound => ("저장소를 찾을 수 없어요. 공개 저장소만 지원해요(비공개는 터미널에서 git clone)", "Couldn't find the repository. Only public repositories are supported (clone private ones in Terminal)"),
        BranchUnknown => ("브랜치 이름을 확인할 수 없어요(이름에 /가 든 브랜치는 지원하지 않아요)", "Couldn't work out the branch name (branches with / in the name aren't supported)"),
        GithubError => ("GitHub가 오류({s})를 돌려주었어요", "GitHub returned an error ({s})"),
        GithubShape => ("GitHub 응답 형식을 알 수 없어요", "Unknown GitHub response format"),
        RepoTooBig => ("저장소가 너무 커서 파일 목록을 받지 못했어요", "The repository is too big to list its files"),
        MarketUnreadable => ("marketplace.json을 읽을 수 없어요", "Couldn't read marketplace.json"),
        MarketNameBad => ("marketplace.json의 이름이 올바르지 않아요", "The name in marketplace.json isn't valid"),
        Hooks => ("훅", "Hooks"),
        HasSubmodule => ("서브모듈({a0})이 들어 있어 설치할 수 없어요", "Has a submodule ({a0}), so it can't be installed"),
        HasSymlink => ("심볼릭 링크({a0})가 들어 있어 설치할 수 없어요", "Has a symbolic link ({a0}), so it can't be installed"),
        NameChars => ("이름에 지원하지 않는 문자가 있어요: {a0}", "A name has unsupported characters: {a0}"),
        DirDepthMax => ("폴더가 {MAX_DEPTH}단계보다 깊어요", "The folder is deeper than {MAX_DEPTH} levels"),
        CaseClash => ("대소문자만 다른 이름이 겹쳐요: {a0}", "Names that differ only in case clash: {a0}"),
        FileKind => ("지원하지 않는 파일 종류예요: {a0}", "Unsupported file type: {a0}"),
        FileOver1Mb => ("1MB가 넘는 파일이 있어요: {a0}", "A file is over 1 MB: {a0}"),
        FilesMax => ("파일이 {MAX_FILES}개보다 많아요", "More than {MAX_FILES} files"),
        FilesOver5Mb => ("파일 합계가 5MB를 넘어요", "Files total over 5 MB"),
        NoSkillMd => ("이 폴더에 SKILL.md가 없어요", "This folder has no SKILL.md"),
        TempDirFailed => ("임시 폴더를 만들지 못했어요", "Couldn't create a temporary folder"),
        FetchFailed => ("파일을 받지 못했어요({a0}): {a1}", "Couldn't download a file ({a0}): {a1}"),
        SkillNotUtf8 => ("SKILL.md가 UTF-8 텍스트가 아니에요", "SKILL.md isn't UTF-8 text"),
        SkillNameFormat => ("스킬 이름이 올바르지 않아요(영문·숫자·-·_, 64자 이하)", "The skill name isn't valid (letters, numbers, - and _, up to 64 characters)"),
        InstallDirBad => ("설치 폴더가 일반 폴더가 아니에요(링크 등)", "The install folder isn't a regular folder (a link or such)"),
        InstallDirFailed => ("설치 폴더를 만들지 못했어요", "Couldn't create the install folder"),
        InstallDirNotDir => ("설치 폴더가 폴더가 아니에요", "The install folder isn't a folder"),
        PathError => ("경로 오류", "Path error"),
        SkillExists => ("이미 있는 스킬이에요", "This skill already exists"),
        CrossDisk => ("앱 데이터와 설치 폴더가 다른 디스크라 설치할 수 없어요", "App data and the install folder are on different disks, so it can't be installed"),
        InstallFailed => ("설치하지 못했어요", "Couldn't install"),
        SkillNameBad => ("스킬 이름이 올바르지 않아요", "The skill name isn't valid"),
        MarketNameClash => ("이름이 같은 다른 마켓플레이스({a0})가 이미 추가되어 있어요", "Another marketplace with the same name ({a0}) is already added"),
        NothingFound => ("이 링크에서 SKILL.md나 플러그인 마켓플레이스(.claude-plugin/marketplace.json)를 찾지 못했어요", "Couldn't find a SKILL.md or a plugin marketplace (.claude-plugin/marketplace.json) at this link"),
        LinkNoTrash => ("링크라서 여기서 지우지 않아요", "It's a link, so it isn't deleted here"),
        LinkedOtherName => ("다른 이름으로도 연결된 파일이에요", "A file linked under another name too"),
        TrashViaToggle => ("터미널 커맨드 토글로 지워요", "Delete it with the terminal command toggle"),
        LinkedProjectKeep => ("링크로 연결된 프로젝트 폴더는 지울 수 없어요", "A project folder reached through a link can't be deleted"),
        PluginSyncKeep => ("플러그인·동기화 항목은 지울 수 없어요", "Plugin and synced items can't be deleted"),
        ChangedNoDelete => ("확인하는 동안 바뀌어 지우지 않았어요", "It changed while checking, so it wasn't deleted"),
        NotSkillDir => ("스킬 폴더가 아니에요", "Not a skill folder"),
        SkillDirPartial => ("스킬 폴더를 다 읽지 못했어요", "Couldn't read the whole skill folder"),
        SkillDirBig => ("스킬 폴더가 너무 커요(2,000개·8단계까지)", "The skill folder is too big (up to 2,000 items and 8 levels)"),
        SkillDirLink => ("스킬 폴더 안에 링크가 있어 지우지 않아요", "The skill folder has a link inside, so it isn't deleted"),
        HasHardlink => ("다른 이름으로도 연결된 파일이 들어 있어요", "Contains a file linked under another name too"),
        HasSpecial => ("특수 파일이 들어 있어 지우지 않아요", "Contains a special file, so it isn't deleted"),
        UnknownProject => ("알려진 프로젝트가 아니에요", "Not a known project"),
        HomeNotProject => ("홈 폴더는 프로젝트로 쓰지 않아요", "The home folder isn't used as a project"),
        OutsideRoots => ("정해진 폴더 밖의 파일이에요", "The file is outside the allowed folders"),
        InHiddenDir => ("숨김 폴더 안의 항목이에요", "The item is inside a hidden folder"),
        NotDirectChild => ("정해진 폴더 바로 아래 항목이 아니에요", "Not an item right under an allowed folder"),
        TitleFormat => ("제목은 1–80자이고 보이지 않는 제어 문자를 넣을 수 없어요", "Titles are 1–80 characters with no invisible control characters"),
        SessionNotFound => ("세션을 찾을 수 없어요", "Couldn't find the session"),
        SaveFailed => ("저장하지 못했어요", "Couldn't save"),
        CloudNoReply => ("클라우드 루틴 응답을 받지 못했어요", "Didn't get a cloud routine response"),
        CloudApiError => ("클라우드 루틴 API가 오류를 돌려주었어요", "The cloud routine API returned an error"),
        UnknownShape => ("응답 형식을 알 수 없어요", "Unknown response format"),
        CloudGuardBlocked => ("바꾸지 못했어요. 보안 가드가 요청을 막았어요", "Couldn't change it. The safety guard blocked the request"),
        CloudVerifyFailed => ("결과를 확인하지 못했어요. 다시 불러오세요", "Couldn't confirm the result. Load it again"),
        RefineBudget => ("이번 실행의 비용 상한에 도달해 중단되었어요", "Stopped at this run's cost limit"),
        RefineTurns => ("응답 횟수 상한에 도달해 중단되었어요", "Stopped at the response limit"),
        Refine90 => ("90초 안에 끝나지 않아 중단했어요", "Stopped because it didn't finish within 90 seconds"),
        ClaudeReplyBad => ("Claude 응답을 읽지 못했어요. 다시 시도하세요", "Couldn't read Claude's reply. Try again"),
        RefineInputBad => ("보낼 내용이 너무 길거나 쓸 수 없는 문자가 있어요", "The content to send is too long or has characters that can't be used"),
        RefineNoRounds => ("더 다듬을 라운드가 없어요", "No rounds left to refine"),
        AlreadyRefining => ("이미 다듬는 중이에요", "Already refining"),
        JobDirFailed => ("작업 폴더를 준비하지 못했어요", "Couldn't prepare the task folder"),
        ScheduleRange => ("일정 값이 범위를 벗어났어요", "A schedule value is out of range"),
        RoutineName => ("이름은 1~64자이고 보이지 않는 제어 문자를 넣을 수 없어요", "Names are 1–64 characters with no invisible control characters"),
        TaskDirAbsolute => ("작업 폴더는 절대 경로여야 해요", "The task folder must be an absolute path"),
        TaskDirMissing => ("작업 폴더를 찾을 수 없어요", "Couldn't find the task folder"),
        RoutinePrompt => ("지시는 1바이트 이상 100KB 이하여야 해요", "The instructions must be 1 byte to 100 KB"),
        RoutineRisky => ("루틴에는 확인을 건너뛰는 권한을 쓸 수 없어요", "Routines can't use a permission that skips confirmation"),
        RoutinesMax => ("루틴은 최대 20개까지 만들 수 있어요", "Up to 20 routines"),
        LoadoutSkillsMax => ("장비 스킬은 {MAX_SKILLS}개까지예요", "Up to {MAX_SKILLS} loadout skills"),
        LoadoutSkillBad => ("장비 스킬 이름이나 출처가 올바르지 않아요", "A loadout skill's name or source isn't valid"),
        LoadoutSkillDup => ("같은 장비 스킬이 두 번 있어요", "The same loadout skill is listed twice"),
        LoadoutMcpMax => ("장비 MCP 서버는 {MAX_MCP}개까지예요", "Up to {MAX_MCP} loadout MCP servers"),
        LoadoutMcpBad => ("장비 MCP 서버 이름이 올바르지 않아요", "A loadout MCP server name isn't valid"),
        LoadoutMcpUnderscore => ("이름에 __가 있는 MCP 서버는 따로 고를 수 없어요", "MCP servers with __ in the name can't be picked one by one"),
        LoadoutMcpDup => ("같은 MCP 서버가 두 번 있어요", "The same MCP server is listed twice"),
        PreferredMax => ("먼저 살펴볼 항목은 {MAX_PREFERRED}개까지예요", "Up to {MAX_PREFERRED} items to consider first"),
        PreferredBad => ("먼저 살펴볼 항목의 이름이나 출처가 올바르지 않아요", "An item to consider first has an invalid name or source"),
        PreferredDup => ("먼저 살펴볼 항목이 두 번 있어요", "An item to consider first is listed twice"),
        PreferredDrop => ("{e} — 먼저 살펴볼 항목에서 빼요", "{e} — left out of the items to consider first"),
        LoadoutDisallowed => ("MCP가 아닌 disallowedTools 항목은 기본 장비에서 바꿀 수 없어요", "Non-MCP disallowedTools entries can't change in the default loadout"),
        LoadoutTools => ("MCP가 아닌 tools 항목은 기본 장비에서 바꿀 수 없어요", "Non-MCP tools entries can't change in the default loadout"),
        LoadoutEmptyTools => ("tools 줄을 비우면 모든 도구를 쓰게 돼요 · 서버를 하나 이상 남겨 주세요", "An empty tools line allows every tool · Keep at least one server"),
        LoadoutMcpShape => ("MCP 항목은 mcp__서버__* 모양만 더할 수 있어요", "Only mcp__server__* entries can be added"),
        Other => ("기타", "Other"),
        InsUnder1 => (" (기간 비용의 1% 미만)", " (under 1% of the period's cost)"),
        InsAbout => (" (기간 비용의 약 {a0})", " (about {a0} of the period's cost)"),
        InsCacheTitle => ("캐시 활용", "Cache use"),
        InsCacheHead => ("캐시가 잘 쓰이지 않아요", "The cache isn't used much"),
        InsCacheBody => ("입력 {a0} 중 캐시 읽기 {a1} — 비율 {a2} (응답 {a3}번)", "Cache reads {a1} of {a0} input — ratio {a2} ({a3} replies)"),
        InsCacheSave => ("캐시 비율이 50%가 되면 대략 {a0} 절감{a1}", "At a 50% cache ratio, roughly {a0} saved{a1}"),
        InsCacheTip1 => ("한 작업은 한 세션에서 이어 하세요.", "Keep one task in one session."),
        InsCacheTip2 => ("다른 원인 후보: 5분 넘게 쉬어 캐시가 만료됨, 플러그인·MCP 구성이 자주 바뀌어 앞부분이 달라짐. 쓰지 않는 플러그인은 꺼 두면 앞부분이 안정돼요.", "Other likely causes: the cache expired after a break of more than 5 minutes, or plugins and MCP changed often so the start differs. Turning off unused plugins keeps the start stable."),
        InsModelTitle => ("모델 배분", "Model choice"),
        InsModelHead => ("짧은 작업에도 최상위 모델을 주로 써요", "Short tasks mostly use the top model"),
        InsModelBody => ("짧은 세션 {a0}개에서 응답의 {a1}가 Opus급 모델 (짧은 세션 = 응답 10번·도구 5번 이하)", "In {a0} short sessions, {a1} of replies came from an Opus-class model (short session = up to 10 replies and 5 tool calls)"),
        InsModelSave => ("이 세션들을 Sonnet으로 했다면 대략 {a0} 절감{a1}", "With Sonnet for these sessions, roughly {a0} saved{a1}"),
        InsModelTip1 => ("간단한 질문·작은 수정은 /model 로 Sonnet이나 Haiku로 바꾸세요.", "Switch to Sonnet or Haiku with /model for simple questions and small edits."),
        InsModelTip2 => ("/effort 를 낮추는 것도 방법이에요. 어려운 설계·디버깅에만 최상위 모델을 쓰면 돼요.", "Lowering /effort helps too. Keep the top model for hard design and debugging."),
        InsSubTitle => ("서브에이전트 모델", "Subagent model"),
        InsSubHead => ("서브에이전트가 대부분 Opus급 모델로 돌아요", "Subagents mostly run on an Opus-class model"),
        InsSubBody => ("서브에이전트 응답 {a0}번 중 {a1}가 Opus급 모델{a2}", "{a1} of {a0} subagent replies came from an Opus-class model{a2}"),
        InsSubMost => (" (많은 순: {a0})", " (most used: {a0})"),
        InsSubSave => ("이 응답들을 Sonnet으로 했다면 대략 {a0} 절감{a1}", "With Sonnet for these replies, roughly {a0} saved{a1}"),
        InsSubTip1 => ("조사·리뷰용 에이전트 정의에 model: sonnet 이나 haiku 를 적으세요.", "Add model: sonnet or haiku to research and review agent definitions."),
        InsSubTip2 => ("에이전트 정의에 model이 없으면 메인 세션의 모델을 그대로 써요. 어려운 설계를 맡는 에이전트만 Opus로 두면 돼요.", "An agent without model uses the main session's model. Keep Opus only for agents that do hard design."),
        InsToolTitle => ("도구 오류율", "Tool error rate"),
        InsToolLine => ("{a0} {a1}번 중 {a2} 실패", "{a0}: {a2} failed of {a1}"),
        InsToolHead => ("실패가 잦은 도구가 있어요", "Some tools fail often"),
        InsToolBody => ("실패한 호출마다 다시 시도하는 응답이 붙어 토큰과 시간이 늘어나요", "Every failed call adds a retry reply, costing tokens and time"),
        InsToolTip1 => ("자주 거부하는 도구는 권한 규칙을 정리하세요.", "Tidy the permission rules for tools you often deny."),
        InsToolTip2 => ("권한 거부도 실패로 세요. 명령 실패가 많다면 CLAUDE.md에 빌드·테스트 명령을 적어 두면 재시도가 줄어들어요.", "Permission denials count as failures. If commands fail a lot, writing build and test commands in CLAUDE.md cuts retries."),
        InsLongTitle => ("긴 세션", "Long sessions"),
        InsLongHead => ("대화 압축이 잦은 긴 세션이 많아요", "Many long sessions compact often"),
        InsLongBody => ("세션 {a0}개 중 {a1}개({a2})에서 압축이 2번 이상. 서브에이전트 호출 {a3}번", "{a1} of {a0} sessions ({a2}) compacted twice or more. Subagent calls: {a3}"),
        InsLongWhy => ("압축 뒤에는 앞의 결정·맥락이 줄어 실수가 늘고, 큰 컨텍스트를 매번 다시 읽어 비용이 커져요", "After compaction, earlier decisions and context shrink so mistakes rise, and re-reading a large context costs more"),
        InsLongTip1 => ("큰 작업은 단계마다 새 세션(/clear)으로 나누세요.", "Split big work into a new session (/clear) per stage."),
        InsLongTip2 => ("압축이 필요하면 /compact 뒤에 남길 내용을 적어(예: /compact 결정 사항과 남은 할 일만) 직접 압축하세요. 탐색·조사는 서브에이전트(Explore 등)에 맡기면 메인 컨텍스트가 덜 차요.", "When you need to compact, say what to keep after /compact (for example: /compact decisions and remaining to-dos only). Hand exploring and research to subagents (Explore and such) so the main context fills less."),
        InsTtlTitle => ("캐시 유지 시간", "Cache lifetime"),
        InsTtlHead => ("1시간 캐시를 많이 쓰지만 다시 읽히는 양이 적어요", "The 1-hour cache is used a lot but read back little"),
        InsTtlBody => ("캐시 쓰기의 {a0}가 1시간 캐시, 캐시 쓰기 1토큰당 읽기 {a1}토큰", "{a0} of cache writes are 1-hour cache, {a1} tokens read per token written"),
        InsTtlSave => ("5분 캐시였다면 대략 {a0} 덜 들어요{a1}", "With the 5-minute cache, roughly {a0} less{a1}"),
        InsTtlTip1 => ("오래 쉬었다 이어 하는 작업이 아니라면 기본(5분) 캐시로 충분한지 확인하세요.", "Unless you resume after long breaks, check whether the default (5-minute) cache is enough."),
        InsTtlTip2 => ("1시간 캐시는 쓰기 단가가 5분 캐시의 약 1.6배예요.", "The 1-hour cache costs about 1.6 times the 5-minute cache to write."),
        InsUnusedTitle => ("안 쓰는 스킬·플러그인", "Unused skills and plugins"),
        InsUnusedHead => ("30일 동안 쓰지 않은 스킬·플러그인이 있어요", "Some skills and plugins weren't used in 30 days"),
        InsUnusedBody => ("직접 만든 스킬 {a0}개, 스킬을 한 번도 쓰지 않은 플러그인 {a1}개{a2}", "{a0} of your own skills, {a1} plugins whose skills were never used{a2}"),
        InsUnusedEg => (" (예: {names})", " (for example: {names})"),
        InsUnusedWhy => ("스킬 설명과 플러그인 도구 목록은 매 세션 컨텍스트에 실려요. 줄이면 입력 토큰이 조금 줄고 스킬 선택이 정확해져요", "Skill descriptions and plugin tool lists load into every session. Trimming them saves a few input tokens and makes skill choice more accurate"),
        InsUnusedTip => ("쓰지 않는 플러그인은 끄고, 필요 없는 스킬은 정리하세요.", "Turn off unused plugins and tidy skills you don't need."),
        InsBudget => ("예산 상한($0.30)에 도달해 중단되었어요", "Stopped at the budget limit ($0.30)"),
        InsTurns => ("검색·응답 횟수 상한에 도달해 중단되었어요", "Stopped at the search and response limit"),
        Ins120 => ("120초 안에 끝나지 않아 중단했어요", "Stopped because it didn't finish within 120 seconds"),
        TypeModelBad => ("유형 '{name}': 모델 이름 형식이 아니에요", "Type '{name}': not a model name"),
        ModelCliRejected => ("이 모델을 쓸 수 없는 것 같아요 · 다른 모델을 골라 주세요", "This model seems unavailable · Pick another model"),
        ModelManagedPin => ("관리 설정 범위의 버전으로 바뀌어요", "Changes to the version within managed settings"),
        ModelRanOther => ("요청한 모델({r})과 다른 {a0}로 돌았어요", "Ran on {a0}, not the requested model ({r})"),
        PriceMultiple => ("기본값보다 토큰당 약 {a0}배 비싸요 · 앱 단가표 기준", "About {a0}× the default per token · From the app's price table"),
        AddrNotAllowed => ("허용하지 않는 주소예요", "This address isn't allowed"),
        GithubConnect => ("GitHub에 연결하지 못했어요", "Couldn't connect to GitHub"),
        ReplyTooBig => ("응답이 너무 커요", "The response is too big"),
        RedirectOut => ("리다이렉트가 허용 범위를 벗어났어요", "A redirect went outside the allowed range"),
        BuiltinGeneral => ("복잡한 질문 조사, 코드 검색, 여러 단계 작업을 맡는 범용 agent", "General agent for researching complex questions, searching code and multi-step tasks"),
        BuiltinExplore => ("여러 파일을 훑어 코드 위치를 찾는 읽기 전용 탐색 agent", "Read-only exploring agent that skims files to find code"),
        BuiltinPlan => ("구현 계획과 설계 트레이드오프를 세우는 agent", "Agent that plans implementation and design trade-offs"),
        BuiltinGuide => ("Claude Code·SDK·API 사용법에 답하는 agent", "Agent that answers how to use Claude Code, the SDK and the API"),
        BuiltinStatusline => ("Claude Code 상태 줄을 설정하는 agent", "Agent that sets up the Claude Code status line"),
        LinkedSuffix => ("{a0} (링크)", "{a0} (link)"),
        Builtin => ("내장", "Built-in"),
        PluginAgentReadOnly => ("플러그인 agent는 읽기 전용이에요", "Plugin agents are read-only"),
        OtherOwnerReadOnly => ("다른 사용자 소유이거나 다른 사용자가 쓸 수 있는 파일이라 읽기 전용이에요", "Owned by or writable by another user, so it's read-only"),
        BuiltinAgent => ("Claude Code 내장 에이전트예요", "A built-in Claude Code agent"),
        BuiltinNoFile => ("내장 agent는 파일이 없어요", "Built-in agents have no file"),
        NotUtf8File => ("UTF-8 파일이 아니에요", "Not a UTF-8 file"),
        NoFrontmatter => ("(frontmatter 없음)", "(no front matter)"),
        ToolsNoFront => ("frontmatter가 없는 정의라 도구 목록을 고칠 수 없어요", "A definition without front matter can't change its tools list"),
        ToolsMax60 => ("도구는 60개까지 넣을 수 있어요", "Up to 60 tools"),
        ToolNameFormat => ("도구 이름 형식이 올바르지 않아요 (예: Read, Bash(git status:*), mcp__서버)", "The tool name isn't valid (for example: Read, Bash(git status:*), mcp__server)"),
        ToolsLineDup => ("tools 줄이 두 번 있어 고칠 수 없어요", "The tools line appears twice, so it can't be changed"),
        ToolsMultiline => ("여러 줄 목록으로 쓴 도구 목록은 여기서 고칠 수 없어요. 정의 편집에서 고쳐 주세요", "A tools list written over several lines can't change here. Edit it in the definition"),
        LoadoutMax12 => ("장비 스킬은 12개까지예요", "Up to 12 loadout skills"),
        SkillNameBad2 => ("스킬 이름 형식이 올바르지 않아요", "The skill name isn't valid"),
        LoadoutNoFront => ("frontmatter가 없는 정의라 장비를 고칠 수 없어요", "A definition without front matter can't change its loadout"),
        SkillsLineDup => ("skills 줄이 두 번 있어 고칠 수 없어요", "The skills line appears twice, so it can't be changed"),
        SkillsMultiline => ("여러 줄 목록으로 쓴 skills는 여기서 고칠 수 없어요. 정의 편집에서 고쳐 주세요", "A skills list written over several lines can't change here. Edit it in the definition"),
        ModelNoFront => ("frontmatter가 없어 모델을 바꿀 수 없어요", "No frontmatter, so the model can't change"),
        ModelLineBad => ("model 줄이 두 번이거나 여러 줄이라 여기서 고칠 수 없어요. 정의 편집에서 고쳐 주세요", "The model line appears twice or spans lines, so it can't change here. Edit it in the definition"),
        ModelFileFormat => ("모델 이름 형식이 아니에요", "Not a model name"),
        LoadoutReread => ("장비를 고친 결과를 다시 읽지 못했어요", "Couldn't read back the changed loadout"),
        PermEmptyLong => ("비어 있거나 너무 길어요", "Empty or too long"),
        PermInvisible => ("제어 문자나 보이지 않는 문자가 있어요", "Has control or invisible characters"),
        PermSpaces => ("앞뒤 공백이 있어요", "Has spaces at the start or end"),
        PermParens => ("괄호가 맞지 않아요", "The parentheses don't match"),
        PermParensEmpty => ("괄호 안이 비어 있어요", "Nothing inside the parentheses"),
        PermToolName => ("도구 이름은 영문자로 시작하고 영문·숫자·_·-만 쓸 수 있어요", "Tool names start with a letter and use letters, numbers, _ and - only"),
        PermWildcard => ("와일드카드 도구 이름은 mcp__ 규칙에서만 쓸 수 있어요", "Wildcard tool names work only in mcp__ rules"),
        PermMcpParens => ("MCP 규칙에 괄호를 붙이면 Claude Code가 무시해요", "Claude Code ignores parentheses on an MCP rule"),
        PermLooseWild => ("허용 칸의 고정되지 않은 와일드카드는 Claude Code가 무시해요", "Claude Code ignores a loose wildcard in the allow list"),
        PermColonStar => ("`:*`는 규칙 끝에서만 동작해요", "`:*` works only at the end of a rule"),
        LinkedNoEdit => ("링크 파일이라 여기서 고칠 수 없어요", "A link file, so it can't be edited here"),
        HardlinkNoEditHere => ("다른 이름으로도 연결된 파일이라 여기서 고칠 수 없어요", "A file linked under another name too, so it can't be edited here"),
        ReadOr256 => ("파일을 읽지 못했거나 256KB보다 커요", "Couldn't read the file, or it's over 256 KB"),
        FileFormatBad => ("파일 형식을 해석할 수 없어요", "Couldn't parse the file format"),
        JsonAt => ("JSON 형식이 아니에요 ({a0}줄 {a1}칸)", "Not JSON (line {a0}, column {a1})"),
        TopNotObject => ("최상위가 객체가 아니에요", "The top level isn't an object"),
        PermNotObject => ("permissions가 객체가 아니에요", "permissions isn't an object"),
        PermNonString => ("allow·ask·deny에 문자열이 아닌 값이 있어 고칠 수 없어요", "allow, ask or deny has a non-string value, so it can't be changed"),
        ModeFormat => ("defaultMode 형식을 해석할 수 없어요", "Couldn't parse defaultMode"),
        ManagedReadOnly => ("관리자 설정은 읽기 전용이에요", "Managed settings are read-only"),
        PermKeyDup => ("permissions 키가 두 번 있어 고칠 수 없어요", "The permissions key appears twice, so it can't be changed"),
        KeyDup => ("{key} 키가 두 번 있어 고칠 수 없어요", "The {key} key appears twice, so it can't be changed"),
        PermListMax => ("{name} 규칙은 {MAX_RULES_PER_LIST}개까지예요", "Up to {MAX_RULES_PER_LIST} {name} rules"),
        PermRuleErr => ("{name} 규칙 `{a0}`: {e}", "{name} rule `{a0}`: {e}"),
        FolderMissing => ("폴더가 없어요", "The folder is missing"),
        BypassNotHere => ("bypassPermissions 모드는 여기서 고를 수 없어요", "bypassPermissions can't be chosen here"),
        ModeUnknown => ("알 수 없는 기본 모드예요", "Unknown default mode"),
        PermHighCount => ("{r} (위험 높음 {a0} 포함)", "{r} (including {a0} high risk)"),
        NoteReference => ("참고용이에요. 권한 모드·훅·CLI 플래그·샌드박스·bypass 모드는 반영하지 않아요", "For reference only. Permission modes, hooks, CLI flags, the sandbox and bypass mode aren't included"),
        NoteFormat => ("`Bash(git push origin main)`처럼 도구(입력) 형식으로 넣어 주세요", "Enter it as tool(input), like `Bash(git push origin main)`"),
        NoteAgentTools => ("에이전트 {name}의 tools 목록에 {tool}이(가) 없어 이 도구를 쓸 수 없어요", "Agent {name} can't use this tool because {tool} isn't in its tools list"),
        NoteManagedOnly => ("관리자 설정이 관리자 규칙만 쓰도록 해서 다른 범위의 규칙은 무시돼요", "Managed settings use managed rules only, so rules in other scopes are ignored"),
        NoteCompound => ("복합 명령을 {a0}개로 나눠 판단했어요. 거부·확인은 하나라도 맞으면, 허용은 모두 맞아야 적용돼요", "Split a compound command into {a0} parts. Deny and ask apply if any part matches; allow needs all parts to match"),
        NoteBashBypass => ("Bash 접두 거부는 `git -C x push`, `env X=1 …` 같은 형태로 우회될 수 있어요", "A Bash prefix deny can be bypassed with forms like `git -C x push` or `env X=1 …`"),
        NoteReadOnly => ("읽기 전용 명령이라 규칙 없이 허용돼요", "A read-only command, so it's allowed without a rule"),
        NoteNoRule => ("맞는 규칙이 없어요 → 권한 모드에 따라요(보통 확인 창)", "No rule matches → depends on the permission mode (usually a confirmation)"),
        ContentEmpty => ("내용이 비어 있어요", "The content is empty"),
        SkillFront => ("SKILL.md는 --- 로 감싼 frontmatter(name, description)로 시작해야 해요", "SKILL.md must start with front matter (name, description) between --- lines"),
        SkillNameMismatch => ("frontmatter의 name이 이름({name})과 달라요", "The front matter name differs from the name ({name})"),
        NoDescription => ("description이 없으면 Claude가 언제 쓸지 알기 어려워요", "Without a description, Claude can't tell when to use it"),
        HasAllowedTools => ("allowed-tools가 있어요: 이 도구들은 확인 없이 실행될 수 있어요", "Has allowed-tools: these tools may run without asking"),
        HasHooks => ("hooks가 있어요: 명령을 실행할 수 있어요", "Has hooks: it can run commands"),
        ShellLines => ("!`명령` 줄 {a0}개: 불러올 때 셸 명령이 실행돼요", "{a0} !`command` lines: shell commands run when it loads"),
        // Workflow meta reasons (D-183): the same sentences as extensions.wfcheck.reason.* in the app catalogs.
        MetaFirst => ("첫 문장은 export const meta 선언이어야 해요 · 그 앞의 코드를 meta 아래로 옮겨 주세요", "The first statement must be the export const meta declaration · Move any code above it below meta"),
        MetaParse => ("meta를 읽을 수 없어요(문법 오류) · 괄호·따옴표·쉼표를 확인해 주세요", "Couldn't read meta (syntax error) · Check the brackets, quotes and commas"),
        MetaNotLiteral => ("meta 값은 리터럴이어야 해요 · 변수·함수 호출 대신 값을 직접 써 주세요", "meta values must be literals · Write the value itself, not a variable or a call"),
        MetaTemplate => ("meta에 템플릿 보간을 쓸 수 없어요 · 보간을 빼고 따옴표 문자열로 써 주세요", "meta can't use template interpolation · Remove the interpolation and write a quoted string"),
        MetaUnary => ("단항 연산자는 음수(-1)만 쓸 수 있어요 · 계산한 값을 직접 써 주세요", "The only unary operator allowed is a minus on a number (-1) · Write the computed value"),
        MetaSparse => ("배열에 빈 칸을 둘 수 없어요 · 연달아 쓴 쉼표(,,)를 하나로 줄여 주세요", "Arrays can't have empty slots · Replace ,, with a single comma"),
        MetaReservedKey => ("__proto__·constructor·prototype은 키로 쓸 수 없어요 · 다른 키 이름으로 바꿔 주세요", "__proto__, constructor and prototype can't be keys · Rename the key"),
        MetaKey => ("키는 이름·문자열·숫자만 쓸 수 있어요 · [ ]로 계산한 키를 따옴표 문자열로 바꿔 주세요", "Keys must be names, strings or numbers · Replace a [computed] key with a quoted string"),
        MetaNoName => ("meta에 name이 없어요 · name에 파일 이름을 문자열로 넣어 주세요", "meta has no name · Add name with the file name as a string"),
        MetaEmptyName => ("meta.name이 비어 있어요 · 파일 이름을 넣어 주세요", "meta.name is empty · Put the file name in it"),
        MetaNoDesc => ("meta에 description이 없어요 · 언제 쓰는지 한 줄로 넣어 주세요", "meta has no description · Add one line on when to use it"),
        MetaEmptyDesc => ("meta.description이 비어 있어요 · 언제 쓰는지 한 줄로 적어 주세요", "meta.description is empty · Write one line on when to use it"),
        MetaNameDiffers => ("meta.name이 파일 이름과 달라요 · 파일 이름과 같게 맞춰 주세요", "meta.name differs from the file name · Make them the same"),
        MetaTooComplex => ("meta가 너무 크거나 깊어요 · 중첩은 32단계, 목록은 1,000개 아래로 줄여 주세요", "meta is too large or too deeply nested · Keep nesting under 32 levels and lists under 1,000 items"),
        MetaLine => ("{line}번째 줄", "Line {line}"),
        MetaName => ("meta.name을 {name}{ro} 맞춰 주세요", "Change meta.name to {name}{ro}"),
        MetaRegistered => ("Claude Code에는 {name}{ro} 등록돼요", "Claude Code registers it as {name}{ro}"),
        PhaseUnknown => ("meta.phases에 없는 phase() 제목: {a0}", "phase() titles not in meta.phases: {a0}"),
        PhaseUnused => ("meta.phases에 있지만 phase()로 부르지 않는 단계가 있어요", "Some steps are in meta.phases but never called with phase()"),
        NoNondeterminism => ("Date.now()·Math.random()·new Date()는 워크플로에서 쓸 수 없어요", "Workflows can't use Date.now(), Math.random() or new Date()"),
        FolderNotFound => ("폴더를 찾을 수 없어요", "Couldn't find the folder"),
        PartDirBad => ("{part} 폴더가 링크이거나 일반 폴더가 아니에요", "The {part} folder is a link or not a regular folder"),
        PartDirFailed => ("{part} 폴더를 만들지 못했어요", "Couldn't create the {part} folder"),
        NameExists => ("이미 있는 이름이에요", "The name already exists"),
        CreateFileFailed => ("파일을 만들지 못했어요", "Couldn't create the file"),
        WriteFileFailed => ("파일을 쓰지 못했어요", "Couldn't write the file"),
        LinkedProjectDir => ("프로젝트의 {e} — 링크된 .claude에는 만들지 않아요", "In the project, {e} — nothing is made in a linked .claude"),
        MakeDirFailed => ("폴더를 만들지 못했어요", "Couldn't create the folder"),
        NotRegularOr256 => ("일반 파일이 아니거나 256KB가 넘어요", "Not a regular file, or over 256 KB"),
        NotUtf8Text => ("UTF-8 텍스트 파일이 아니에요", "Not a UTF-8 text file"),
        BadJob => ("잘못된 작업이에요", "Invalid task"),
        NoAppData => ("앱 데이터 폴더가 없어요", "The app data folder is missing"),
        JobFolderFailed => ("작업 폴더를 만들지 못했어요", "Couldn't create the task folder"),
        JobSaveFailed => ("작업을 저장하지 못했어요", "Couldn't save the task"),
        ReadDirFailed => ("폴더를 읽지 못했어요", "Couldn't read the folder"),
        HasLink => ("링크가 들어 있어요: {a0}", "Contains a link: {a0}"),
        Depth4 => ("폴더가 4단계보다 깊어요", "The folder is deeper than 4 levels"),
        CaseClashShort => ("대소문자만 다른 이름이 겹쳐요", "Names that differ only in case clash"),
        TooBigOrMany => ("파일이 너무 크거나 많아요", "The files are too big or too many"),
        HasNonRegular => ("일반 파일이 아닌 항목이 있어요", "Has an item that isn't a regular file"),
        FileTooBig => ("파일이 너무 커요", "The file is too big"),
        NotUtf8Plain => ("UTF-8 텍스트가 아니에요", "Not UTF-8 text"),
        NameBad => ("이름이 올바르지 않아요", "The name isn't valid"),
        SkillMadeMissing => ("만든 스킬을 찾을 수 없어요", "Couldn't find the skill that was made"),
        JobDirPath => ("작업 폴더 경로를 처리할 수 없어요", "Couldn't handle the task folder path"),
        StillRunning => ("아직 작업 중이에요. 작업이 끝난 뒤 치우세요", "Still working. Clear it after the task ends"),
        JobDirRemove => ("작업 폴더를 지우지 못했어요", "Couldn't delete the task folder"),
        CopyTooBig => ("파일이 너무 크거나 많아 복사하지 않았어요", "The files are too big or too many, so nothing was copied"),
        ItemNameFormat2 => ("이름은 영문, 숫자, -, _ 로 1–64자여야 해요", "Names are 1–64 characters of letters, numbers, - and _"),
        WorkflowNoCopy => ("워크플로는 복사할 수 없어요", "Workflows can't be copied"),
        SourceDirMissing => ("원본 폴더를 찾지 못했어요", "Couldn't find the original folder"),
        CloudName => ("이름은 1–80자로 적으세요", "Write a name of 1–80 characters"),
        CloudNameChars => ("이름에 쓸 수 없는 문자가 있어요", "The name has characters that can't be used"),
        CloudEditOnWeb => ("이 일정은 claude.ai에서 고치세요", "Change this schedule on claude.ai"),
        PickLater => ("지금보다 뒤의 시각을 고르세요", "Pick a time later than now"),
        PickWithinYear => ("1년 안의 시각을 고르세요", "Pick a time within a year"),
        TimeBad => ("시각이 올바르지 않아요", "The time isn't valid"),
        CheckDayTime => ("요일과 시각을 확인하세요", "Check the day and time"),
        OnceRepeatWeb => ("한 번 ↔ 반복 전환은 claude.ai에서 하세요", "Switch between once and repeating on claude.ai"),
        PastOnce => ("지난 한 번 일정은 켤 수 없어요. 미래 시각으로 바꾸세요", "A past one-time schedule can't be turned on. Pick a future time"),
        HomeIsGlobal => ("홈 폴더는 전역과 같아요", "The home folder is the same as global"),
        CloudBlocked => ("로컬만 프로젝트는 클라우드 루틴을 쓸 수 없어요", "Local-only projects can't use cloud routines"),
        UserSettings => ("사용자 설정", "User settings"),
        RunInvalid => ("실행할 수 없어요 ({kind})", "Can't run it ({kind})"),
    }
}

impl Fail {
    /// The sentence in the current language.
    pub fn now(self) -> String {
        self.text(current()).to_string()
    }

    pub fn text(self, lang: Lang) -> &'static str {
        let (ko, en) = self.templates();
        if lang == Lang::Ko { ko } else { en }
    }

    /// The sentence in the current language as a `&'static str` (drop-in for a fixed literal).
    pub fn str(self) -> &'static str {
        self.text(current())
    }

    /// With parameters, in the current language.
    pub fn with(self, params: &Params) -> String {
        fill(self.text(current()), params)
    }
}

/// A [`Fail`] sentence in the current language with named values: `tr_fmt!(Fail::TypeErr, name = n, e = e)`.
/// Every value passes `display_safe` (no control characters or line breaks), like any other parameter.
#[macro_export]
macro_rules! tr_fmt {
    ($code:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $code.with(&$crate::i18n::Params::new()$(.text(stringify!($k), &::std::string::ToString::to_string(&$v), 4000))*)
    };
}

/// A refusal reason: the code and its typed, already-cleaned params.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub code: ReasonCode,
    pub params: Params,
}

impl Reason {
    pub fn text(&self, lang: Lang) -> String {
        let (ko, en) = self.code.templates();
        fill(if lang == Lang::Ko { ko } else { en }, &self.params)
    }
}

impl From<ReasonCode> for Reason {
    fn from(code: ReasonCode) -> Self {
        Reason { code, params: Params::new() }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(current()))
    }
}

/// `{code, params, text, reason}`: `text` is the fallback sentence in the current language and `reason` the same
/// sentence under the old field name, so a screen that still shows `reason` keeps working.
impl serde::Serialize for Reason {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let text = self.to_string();
        let mut m = s.serialize_map(Some(4))?;
        m.serialize_entry("code", &self.code.code())?;
        m.serialize_entry("params", &self.params)?;
        m.serialize_entry("text", &text)?;
        m.serialize_entry("reason", &text)?;
        m.end()
    }
}

/// Typed parameters. Values are cleaned on the way in; there is no way to add a raw string. Serialized as a
/// `{name: value}` object of strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params(Vec<(&'static str, String)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    /// A name or short label: unsafe characters removed, cut to `max_chars`.
    pub fn text(mut self, key: &'static str, value: &str, max_chars: usize) -> Self {
        self.0.push((key, display_safe(value, max_chars)));
        self
    }

    pub fn num(mut self, key: &'static str, n: i64) -> Self {
        self.0.push((key, n.to_string()));
        self
    }

    /// Only the host of a URL (no scheme, userinfo, port, path or query). Unparseable input becomes empty.
    pub fn url_host(mut self, key: &'static str, url: &str) -> Self {
        let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
        self.0.push((key, display_safe(&host, 253)));
        self
    }

    /// A full https URL only when it carries no userinfo (a link the user must see before opening); anything else
    /// is reduced to its host.
    pub fn checked_url(mut self, key: &'static str, url: &str) -> Self {
        let shown = match url::Url::parse(url) {
            Ok(u) if u.scheme() == "https" && u.username().is_empty() && u.password().is_none() => u.to_string(),
            Ok(u) => u.host_str().unwrap_or_default().to_string(),
            Err(_) => String::new(),
        };
        self.0.push((key, display_safe(&shown, 2048)));
        self
    }

    /// Only the last component of a path.
    pub fn basename(mut self, key: &'static str, path: &str) -> Self {
        let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        self.0.push((key, display_safe(name, 120)));
        self
    }

    /// Several lines (a list in a dialog): each cleaned and cut to `max_chars`, at most `max_lines`, joined by `\n`.
    /// The only way a newline enters a value.
    pub fn lines<I: IntoIterator<Item = S>, S: AsRef<str>>(mut self, key: &'static str, lines: I, max_chars: usize, max_lines: usize) -> Self {
        let v: Vec<String> = lines.into_iter().take(max_lines).map(|l| display_safe(l.as_ref(), max_chars)).collect();
        self.0.push((key, v.join("\n")));
        self
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str())
    }
}

impl serde::Serialize for Params {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// The sentence for `code` in `lang`. A placeholder without a parameter is left as written.
pub fn msg(lang: Lang, code: Code, params: &Params) -> String {
    fill(text(lang, code), params)
}

/// One pass over the template; values are copied, never re-scanned.
fn fill(template: &str, params: &Params) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if is_name(&after[..end]) => {
                let name = &after[..end];
                match params.get(name) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The particle "로" or "으로" after `word` in Korean (none in English): "으로" after a final consonant other than ㄹ.
/// A name in Latin letters or digits is read aloud: digits by their Korean names, and a word ending in m, n or ng
/// takes "으로" (team → 팀으로); anything else takes "로". Trailing symbols are skipped.
pub fn ro(lang: Lang, word: &str) -> &'static str {
    if lang == Lang::En {
        return "";
    }
    let w: String = word.trim_end_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    let Some(last) = w.chars().last() else { return "로" };
    let euro = match last {
        '가'..='힣' => {
            let jong = (last as u32 - 0xAC00) % 28;
            jong != 0 && jong != 8
        }
        // 영·삼·육 end in a consonant; 일·칠·팔 in ㄹ.
        '0' | '3' | '6' => true,
        'm' | 'n' => true,
        'g' => w.ends_with("ng"),
        _ => false,
    };
    if euro { "으로" } else { "로" }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ro_picks_the_particle_by_the_final_sound() {
        for (w, p) in [("수집", "으로"), ("메일", "로"), ("분류", "로"), ("plan", "으로"), ("team", "으로"), ("sync", "로"), ("triage-v2", "로"), ("v10", "으로"), ("step3", "으로"), ("v1", "로"), ("ping", "으로"), ("bug", "로"), ("a-b)", "로"), ("", "로")] {
            assert_eq!(ro(Lang::Ko, w), p, "{w}");
        }
        assert_eq!(ro(Lang::En, "plan"), "");
    }

    fn placeholders(t: &str) -> Vec<String> {
        let mut v: Vec<String> = t.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(n, _)| n.to_string())).filter(|n| is_name(n)).collect();
        v.sort();
        v
    }

    #[test]
    fn every_code_has_both_languages_with_the_same_placeholders() {
        for &c in Code::ALL {
            let (ko, en) = c.templates();
            assert!(!ko.trim().is_empty() && !en.trim().is_empty(), "{c:?}");
            assert_eq!(placeholders(ko), placeholders(en), "{c:?}");
            assert!(!en.chars().any(|ch| ('가'..='힣').contains(&ch)), "{c:?} en has Korean");
        }
    }

    #[test]
    fn every_reason_has_both_languages_and_a_unique_code() {
        let mut codes = std::collections::HashSet::new();
        for &c in ReasonCode::ALL {
            let (ko, en) = c.templates();
            assert!(!ko.trim().is_empty() && !en.trim().is_empty(), "{c:?}");
            assert_eq!(placeholders(ko), placeholders(en), "{c:?}");
            assert!(!en.chars().any(|ch| ('가'..='힣').contains(&ch)), "{c:?} en has Korean");
            let code = c.code();
            assert!(code.chars().next().is_some_and(|f| f.is_ascii_lowercase()) && code.chars().all(|ch| ch.is_ascii_alphanumeric()), "{code}");
            assert!(codes.insert(code), "{c:?} duplicate code");
        }
        assert_eq!(ReasonCode::TaskTypeFormat.code(), "taskTypeFormat");
    }

    #[test]
    fn every_fail_has_both_languages() {
        for &c in Fail::ALL {
            let (ko, en) = c.templates();
            assert!(!ko.trim().is_empty() && !en.trim().is_empty(), "{c:?}");
            assert_eq!(placeholders(ko), placeholders(en), "{c:?}");
            assert!(!en.chars().any(|ch| ('가'..='힣').contains(&ch)), "{c:?} en has Korean");
        }
        assert_eq!((Fail::BadTaskId.text(Lang::En), Fail::BadTaskId.text(Lang::Ko)), ("Invalid task ID", "잘못된 작업 ID이에요"));
        assert_eq!(fill(Fail::RunInvalid.text(Lang::En), &Params::new().text("kind", "x", 20)), "Can't run it (x)");
    }

    #[test]
    fn guard_constants_map_to_codes_and_keep_their_wording() {
        use crate::delivery_guard as dg;
        assert_eq!(ReasonCode::from_guard(dg::HOOK_MISSING), ReasonCode::GuardHookMissing);
        assert_eq!(ReasonCode::from_guard(dg::MANAGED_BLOCKS), ReasonCode::GuardManagedBlocks);
        assert_eq!(ReasonCode::from_guard(dg::DANGER_REFUSED), ReasonCode::GuardDangerRefused);
        assert_eq!(ReasonCode::from_guard(dg::UNREADABLE), ReasonCode::GuardUnreadable);
        assert_eq!(ReasonCode::from_guard("anything else"), ReasonCode::GuardUnreadable, "fail closed");
        assert_eq!(Reason::from(ReasonCode::GuardHookMissing).text(Lang::Ko), dg::HOOK_MISSING);
    }

    #[test]
    fn reason_serializes_code_params_and_fallback_text() {
        let r = Reason { code: ReasonCode::AgentNotHere, params: Params::new().url_host("host", "https://u:secret@example.com/x").basename("f", "/Users/me/a/b.md") };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["code"], "agentNotHere");
        assert_eq!(v["params"], serde_json::json!({"host": "example.com", "f": "b.md"}));
        assert_eq!(v["text"], "이 폴더에서 쓸 수 없는 에이전트예요");
        assert_eq!(v["reason"], v["text"], "old field kept for screens that show it as is");
        let raw = v.to_string();
        assert!(!raw.contains("secret") && !raw.contains("/Users/me"), "no userinfo or full path: {raw}");
        assert_eq!(r.text(Lang::En), "This agent isn't available in this folder");
    }

    #[test]
    fn checked_url_keeps_a_clean_https_link_and_drops_userinfo() {
        assert_eq!(fill("{u}", &Params::new().checked_url("u", "https://docs.claude.com/a?b=1")), "https://docs.claude.com/a?b=1");
        assert_eq!(fill("{u}", &Params::new().checked_url("u", "https://me:pw@example.com/x")), "example.com");
        assert_eq!(fill("{u}", &Params::new().checked_url("u", "http://example.com/x")), "example.com");
        assert_eq!(fill("{u}", &Params::new().checked_url("u", "nope")), "");
    }

    #[test]
    fn pick_lang_takes_one_side_of_every_pair() {
        let v = serde_json::json!({"a": {"ko": "가", "en": "A"}, "b": [{"ko": "나", "en": "B"}, "x"], "c": {"ko": "다"}, "n": 1});
        assert_eq!(pick_lang(v.clone(), Lang::En), serde_json::json!({"a": "A", "b": ["B", "x"], "c": {"ko": "다"}, "n": 1}));
        assert_eq!(pick_lang(v, Lang::Ko)["a"], "가");
    }

    #[test]
    fn lines_param_cleans_each_line_and_caps_the_count() {
        let p = Params::new().lines("l", ["a\u{202E}b", "c\nd", "e"], 10, 2);
        assert_eq!(fill("{l}", &p), "ab\ncd");
    }

    #[test]
    fn reply_line_and_lang_codes() {
        assert_eq!(Lang::parse("en"), Some(Lang::En));
        assert_eq!(Lang::parse("system"), None);
        assert_eq!(Lang::Ko.as_str(), "ko");
        assert!(Lang::En.reply_line().contains("English") && Lang::Ko.reply_line().contains("한국어"));
    }

    #[test]
    fn system_rule_is_ko_prefix_only() {
        assert_eq!(Lang::from_locale(Some("ko-KR")), Lang::Ko);
        assert_eq!(Lang::from_locale(Some("ko")), Lang::Ko);
        assert_eq!(Lang::from_locale(Some("KO_kr")), Lang::Ko);
        assert_eq!(Lang::from_locale(Some("en-US")), Lang::En);
        assert_eq!(Lang::from_locale(Some("ja-JP")), Lang::En);
        assert_eq!(Lang::from_locale(Some("")), Lang::En);
        assert_eq!(Lang::from_locale(None), Lang::En);
    }

    #[test]
    fn url_params_keep_only_the_host() {
        let p = Params::new().url_host("host", "https://user:token@example.com:8443/x.git?k=v");
        let filled = fill("push to {host}", &p);
        assert_eq!(filled, "push to example.com");
        assert!(!filled.contains("user") && !filled.contains("token"));
        assert_eq!(fill("{host}", &Params::new().url_host("host", "not a url")), "");
    }

    #[test]
    fn path_params_keep_only_the_last_name_and_text_is_cleaned() {
        assert_eq!(fill("{f}", &Params::new().basename("f", "/Users/me/secret/report.md")), "report.md");
        assert_eq!(fill("{f}", &Params::new().basename("f", "/Users/me/dir/")), "dir");
        assert_eq!(fill("{n}", &Params::new().text("n", "job\u{202E}name\n", 64)), "jobname");
        assert_eq!(fill("{n}", &Params::new().text("n", "가나다라", 2)), "가나");
    }

    #[test]
    fn substitution_runs_once() {
        let p = Params::new().text("a", "{b}", 10).text("b", "X", 10).num("n", 3);
        assert_eq!(fill("{a}-{b}-{n}-{missing}-{ not}-{", &p), "{b}-X-3-{missing}-{ not}-{");
    }
}
