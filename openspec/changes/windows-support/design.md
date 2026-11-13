## Context

Inventory from 2026-11-13 (static read; nothing has been built for a Windows target yet). Paths are relative to app/; core = crates/dduck-core/src, shell = src-tauri/src.

**Blocks everything:**
- core/cli_path.rs:274 `is_shell_safe_path` rejects `\`.
- `canonicalize()` (112 calls) returns `\\?\` paths. This breaks `projects::real` (:103), project keys (cloud_routines.rs:332, memory.rs:1128) and mcp_config.rs:279.

**Launch:**
- claude_cli.rs spawns directly. With the npm `claude.cmd` shim, std runs cmd.exe and refuses args it cannot quote safely, and newlines in prompts are likely among them.
- `MAX_PROMPT_BYTES` 100 KB and `MAX_ARGV_BYTES` 512 KB are both above the CreateProcess limit (32,767 UTF-16 chars).
- No `CREATE_NO_WINDOW` anywhere.
- process.rs:40-44 and :118-129, plus claude_cli.rs:888: process groups exist only on unix. On Windows `kill()` hits only the shim.
- login_session.rs:167 has a Windows stub that never reports exit.

**Opening:**
- `/usr/bin/open` and osascript at shell/lib.rs:2991 and :3746-3792, login_cmds.rs:218 and :284, memory_cmds.rs:263.
- Trash uses NSFileManager (shell/lib.rs:2446).

**Hooks:**
- cloud_routines.rs:62 runs the self-test through `/bin/sh -c`. It gates News, insights, brief-refine, cloud routines and local-only.
- Hook commands are POSIX-quoted.
- The shell Claude Code uses on Windows to run hook commands is not verified.

**Guard:**
- delivery_guard covers only `Bash(...)`, not the Windows `PowerShell` tool.
- `basename`/`path_is_git` miss `git.exe` and `\` (:1110-1120, :1456, :1714-1731).
- `norm_key` is POSIX (:588-599).
- Windows managed-settings and registry policy sources are missing (:495-519, permissions.rs:412).

**HOME-only:** skills_catalog.rs:235, routines.rs:498, delivery_guard.rs:524 (also `USER`).

**Mac locations:** `~/Library/Application Support/...` at delivery_guard.rs:217 and routines.rs:502.

**UI:**
- `startsWith("/")` gates in Composer.tsx:121/123/153 and LocalOnlySection.tsx:79.
- `split("/")` display in lib/format.ts:94, lib/preferred.ts:87, lib/agents.ts:26, lib/memory.ts:162, PermissionsPage.tsx:69, ExtensionsPage.tsx:109/240, lib/rules.ts:188/215.
- ⌘ labels in BriefRefiner.tsx:517 and i18n tasks.ts:554/564.
- Fonts in index.css:9-11 and LogView.tsx:42; the terminal cell width is in lib/terminal.ts:152.
- Titlebar padding in Sidebar.tsx:85-87.

**Config/CI:**
- `bundle.targets` has only dmg and app.
- The tray icon is a black template icon.
- CI runs macOS only. Branch protection requires the two macOS check names.

**Tests:**
- About 140 unix-gated tests are skipped on Windows.
- About 51 `/`-literal paths would fail.
- testkit uses `python3`.

**Already fine:**
- No Keychain or credential reads.
- discover.rs rejects names containing "credential".
- CSP and single-instance work on Windows.

## Goals / Non-Goals

**Goals:**
- A Windows x64 preview that installs.
- It finds and runs Claude safely, shows projects, sessions, usage, extensions, memory and rules, and starts and reviews tasks, if group 0 confirms the Windows CLI supports background tasks.
- Every protection that exists on macOS either holds on Windows or the feature is refused there.
- macOS behavior is unchanged except for fail-closed hardening (decision 8).
- Windows CI.

**Non-Goals (this change):**
- Authenticode signing, ARM64, MSI, auto-update.
- Local-only tasks and their guard on Windows.
- Force-stop on Windows.
- Lifting the Mac-only restriction on hook-gated features (needs a real Windows CLI run plus exec-form hooks; follow-up).
- Zone.Identifier tagging.
- Windows UI polish beyond the listed items.

## Decisions

0. **Premises first (group 0):**
   - A non-required `windows-latest` CI job runs `cargo check`/clippy.
   - The same runner installs the CLI (native installer) and checks `--version` and `--help` for `--bg`, `agents`, `logs`, `stop` and `attach`.
   - If `--bg` or the supervisor is missing on Windows, task start and review on Windows become Mac-only and the Goals are rewritten before group 2 starts. The result is recorded in decisions.md.
1. **`platform` module in dduck-core** holds every OS split:
   - `canon(p)`: dunce, plus a manual strip of `\\?\C:\` → `C:\` and `\\?\UNC\` → `\\` when dunce keeps the prefix (>260 chars, reserved names).
   - clippy.toml `disallowed-methods` lists both `std::fs::canonicalize` and `std::path::Path::canonicalize`.
   - `is_abs`.
   - `home()`: `USERPROFILE` first on Windows, in core **and** in shell/lib.rs:180.
   - `same_path`: compares components, whole path case-insensitive on Windows (NTFS).
   - `file_id(&File)`: `GetFileInformationByHandle` returns (volume serial, file index, link count) and refuses on failure. It replaces every `(0,0,1)`/`nlink=1` fallback (rules, memory, agent_controls, ext_trash, authoring, permissions, skills_catalog, discover, artifact_preview, evidence, task_report, ingest), so hard-link and swap defenses hold on Windows (security MEDIUM 5).
   - Opens that must not follow links use `FILE_FLAG_OPEN_REPARSE_POINT` and reject a reparse-point result.
   - `lexical_guard(p)` runs before any filesystem call on an untrusted path. It rejects a non-Disk prefix (UNC, Verbatim, DeviceNS), reserved device names (CON, NUL, COM1…, LPT1…, also with extensions) and `:` streams. Deny-list comparisons (`.claude`, `.git`, `AppData`, `Library`) are case-insensitive on Windows. artifact_preview also denies `AppData` (security MEDIUM 6).
   - Relative-path checks split with `components()`, not `/` (authoring.rs:1174/1178/1457/1460).
2. **CLI discovery** (critic BLOCKER 1, security HIGH 3):
   - Windows order: `%USERPROFILE%\.local\bin\claude.exe`, then `%APPDATA%\npm\claude.cmd` (resolved), Volta, scoop, then PATH (`split_paths`) last.
   - A `.cmd` is resolved **before** any spawn: parse the npm shim for `%dp0%\node_modules\@anthropic-ai\claude-code\bin\claude.exe`, require it under the shim folder, then validate that exe. `claude.cmd` is removed from the allowed names; D.Duck never spawns `.cmd` or `.bat`.
   - The resolved program is stored in `CliInfo` and is the only thing the probe and every `command()` use.
   - The Windows ACL check (`GetNamedSecurityInfoW`) refuses the exe and its parent folder if any SID other than the current user, SYSTEM, Administrators or TrustedInstaller has write, append, delete or WRITE_DAC rights. git.exe gets the same check.
   - git is resolved once to an absolute path and used everywhere.
   - Safe-path rule on Windows: allow `\` and `X:`; reject control characters, `"`, and trailing dots or spaces. Terminal-bound values get their own rule (decision 6).
   - A postinstall-skipped placeholder exe fails `--version` and is reported as "CLI not installed correctly".
3. **Process lifetime** (critic BLOCKER 2):
   - Every spawn gets `CREATE_NO_WINDOW`, with no inheritable handles beyond stdio.
   - A Job Object with `KILL_ON_JOB_CLOSE` is used **only for awaited runs**: `run_until` (brief-refine, insights, news, cloud fetch), `auth login`, `--version`, `--help`, `mcp list`, `plugin …`. The process is created suspended, assigned to the job, then resumed, so nothing escapes before assignment.
   - `--bg` starts and resumes are **not** put in a job. They hand off to the CLI's long-lived service, as on macOS, and survive app exit.
   - Task cancel stays `claude stop <id>`.
   - login_session uses `try_wait()` on Windows.
4. **Command-line length:** the length is measured after std's Windows quoting, in UTF-16 units, against `MAX_CMDLINE_UTF16 = 30_000`. Above that, `ReasonCode::WindowsCommandTooLong` (ko/en) is returned before spawn.
5. **Paths and data:**
   - The project key uses the existing non-alphanumeric → `-` rule on the `canon` path (cloud_routines.rs:332 already does this).
   - `real()` uses `same_path`. JSONL `cwd` goes through the same normalization.
   - App data on Windows is `app_local_data_dir()` (`%LOCALAPPDATA%`, not roaming); macOS is unchanged (security MEDIUM 7).
   - Git hardening constants: Windows uses an app-owned empty folder for `core.hooksPath`, and the fail-closed branch never runs git instead of pointing `GIT_DIR` at a nonexistent path (security MEDIUM 8).
   - Frontmatter parsing accepts CRLF (authoring.rs:1480).
6. **Opening things** (security HIGH 2, LOW 10; critic MEDIUM 9). Three functions, no shared `open_target`:
   - `open_url`: Rust re-checks https (and the existing allow-list), then `ShellExecuteW`.
   - `reveal_folder`: `SHOpenFolderAndSelectItems`.
   - `open_in_editor`: `.md`/`.txt` only, via `%SystemRoot%\System32\notepad.exe <path>`. Same intent as `open -t`.
   - Terminal: Folder runs `wt.exe -d <dir>` if wt is found in `%LOCALAPPDATA%\Microsoft\WindowsApps`; otherwise it starts `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe -NoExit` with `current_dir(dir)` and no command string.
   - Attach uses `-EncodedCommand` (UTF-16LE base64) built from validated parts.
   - Terminal-bound values (dir, CLI path, config dir) pass a Windows terminal rule that rejects control characters and ``;`'‘’‚‛$``. The session id already passes ID_RE.
   - Copyable PowerShell commands escape all five quote characters.
   - Recycle Bin uses the `trash` crate; a test checks that a symlink itself is trashed, not its target.
   - WebView2 new-window requests are denied (test).
7. **Mac-only on Windows** (security HIGH 1):
   - `run_hook` returns `None` (self-test fails) unless `cfg!(target_os = "macos")`. That is the single choke point for every hook-gated feature.
   - On top of that, starts return `ReasonCode::MacOnly` for: local-only, force-stop, News fetch, insights, brief-refine, cloud routine fetch and edit, and `report_guard::grant`.
   - The UI shows "Mac 전용" on the disabled controls. Stored data stays viewable.
   - The unix self-test keeps `/bin/sh -c` (critic HIGH 5): it verifies the exact command line the CLI runs.
8. **Fail-closed hooks everywhere:** every app-owned hook config (news, insights, brief-refine, cloud routines; delivery_guard and report_guard already have it) gets `"onFailure": "block"`. On macOS this only turns a failed hook start from "allow" into "block". The CLI must be at least v2.1.295; older CLIs ignore the key, so behavior there is unchanged.
9. **Default delivery rules on Windows** (critic HIGH 4, security HIGH 4):
   - Every Windows task adds `PowerShell` to `--disallowedTools`, so the only shell tool is Bash (Git Bash), which the existing `Bash(...)` rules cover.
   - For each `Bash(git …)` and `Bash(gh …)` rule, the Windows build also emits `git.exe` and `gh.exe` variants.
   - If Git Bash is missing, the task still starts but has no shell tool. The composer says "Git for Windows가 없으면 명령을 실행할 수 없어요".
   - Tests check the emitted rules on Windows.
10. **Bundle and CI** (critic MEDIUM 6–7, security MEDIUM 9):
    - tauri.conf.json adds the `nsis` target, `webviewInstallMode = embedBootstrapper` and `installMode: currentUser`.
    - Windows tray PNG. The pending count moves into the tray tooltip on Windows. The badge is skipped.
    - **CI:** `windows-latest` UI and Rust jobs under a separate gate named "Test and lint (Windows)". The macOS gate is untouched. Adding the Windows gate to branch protection is an external change made after user approval.
    - The NSIS build is a separate job with a 60-minute timeout. It runs a smoke script: NSIS `/S` install, launch, `Get-Process` MainWindowHandle ≠ 0, then `taskkill /F`.
    - **Release:** a separate `release-windows.yml` (tag or workflow_dispatch) with no cache restore, `npm ci` and `cargo --locked`. It runs `actions/attest-build-provenance` and publishes a SHA256. PR artifacts are never attached to a release.
    - testkit finds Python as `python3`, `python`, then `py -3`.
    - Tests: tempdir paths instead of `/` literals. A `fake_claude` example binary covers spawn and job tests on Windows. It also stands in for "native installer found" by being placed in `%USERPROFILE%\.local\bin` on CI.
11. **Spec scenario tags:** each scenario is marked `[CI]` or `[manual]`. `[manual]` scenarios are listed 1:1 in the release notes' "Unverified on real Windows" section.

## Risks / Trade-offs

- **No Windows machine here.** CI covers compile, tests, the installer and a launch smoke. The real CLI, login, the UI look and the `[manual]` scenarios are unverified until someone runs it.
- **The npm shim format may change.** If it does, the shim is refused with a message to use the native installer.
- **The scope is now large** (about 30 tasks). v0.1.3 ships when all groups pass. If group 0 shows no `--bg` on Windows, the Windows preview shrinks to viewing only.
- **`onFailure: "block"` changes macOS behavior when a hook fails to start.** That is the intended hardening.

## Facts from the docs (claude-code-guide, 2026-11-13, code.claude.com/docs/en)

- **Hook shell:**
  - Shell-form hook `command` strings run in Git Bash if it's installed, else in PowerShell (hooks.md).
  - Exec form (`command` = a real .exe plus `args`) runs no shell and passes args verbatim.
  - A PreToolUse hook that fails to start, or exits with something other than 0 or 2, is **non-blocking** unless the hook sets `"onFailure": "block"` (v2.1.295+).
- **Shell tools:** Git for Windows is optional. Without it the `PowerShell` tool is the shell tool. With it, `PowerShell` is still on by default for claude.ai accounts. A `Bash` deny rule turns PowerShell off only when Git Bash is present. So `Bash(...)` deny rules alone don't cover Windows; that confirms local-only stays Mac-only.
- **Paths:**
  - Native exe: `%USERPROFILE%\.local\bin\claude.exe`.
  - Config: `%USERPROFILE%\.claude`, `%USERPROFILE%\.claude.json`, overridable with `CLAUDE_CONFIG_DIR`.
  - Project folder key: every non-alphanumeric character becomes `-`; names over 200 chars are truncated plus a hash. So `C:\Users\me\app` → `C--Users-me-app`. The rule is verified; the Windows example is unverified.
- **Managed settings:** `C:\Program Files\ClaudeCode\managed-settings.json` (plus `managed-settings.d\` and `managed-mcp.json`), and `HKLM\SOFTWARE\Policies\ClaudeCode` value `Settings`. HKCU is lowest priority. `C:\ProgramData` is not read.
- **CLI flags:** `-p` (piped stdin up to 10 MB), `--append-system-prompt`, `--permission-mode`, `--settings` (≤2 MiB), `attach`, `logs`, `stop`, `agents --json` and `auth login` are documented without Windows exceptions. Whether the background supervisor (`--bg`, agents, logs, stop) runs on Windows is **unverified**.

## Follow-ups noted (not in this change)

- Exec-form hooks (`command` + `args`) on Windows, plus a real Windows CLI run, are the path to lifting the Mac-only restriction on hook-gated features.

## Re-check fixes (security re-review 2026-11-13: BLOCKER/HIGH 0, MEDIUM 6, LOW 4)

- **Spec aligned with decisions 2 and 7:** the self-test keeps `/bin/sh -c` on macOS and fails off macOS. Shims resolve to the exe, and the Windows path rule follows decision 2.
- **Decision 9 additions:**
  - `Monitor` is also in `--disallowedTools` on Windows.
  - Windows adds `Bash(powershell*)`, `Bash(pwsh*)`, `Bash(cmd *)` and `Bash(cmd.exe *)` refusals.
  - Group 0 records the tools list from a CLI `init` message. A wrong tool name there fails the check.
  - Real enforcement is a `[manual]` scenario.
  - Known remaining gaps, documented: absolute-path and letter-case variants of git. macOS has the same gap.
- **Decision 2, ACL:**
  - Refuse when an untrusted SID holds any of: write, append, delete, WRITE_DAC, WRITE_OWNER, GENERIC_WRITE, GENERIC_ALL or MAXIMUM_ALLOWED; or FILE_DELETE_CHILD / ADD_SUBDIR on an ancestor.
  - Also refuse an untrusted owner or a NULL DACL.
  - Inherited ACEs count; INHERIT_ONLY ACEs are skipped for the object itself.
  - Trusted SIDs are exactly the current user, S-1-5-18, S-1-5-32-544 and TrustedInstaller.
  - Every ancestor of the final path (after GetFinalPathNameByHandleW) is checked, **before** any `--version` probe.
  - The npm shim target is canonicalized and must stay under the shim folder (junction case).
- **Decision 8:** hook-gated features require CLI ≥ 2.1.295, the version that supports `onFailure`. Older CLIs get an "update Claude Code" refusal. The earlier claim that "older CLIs ignore the key, behavior unchanged" is withdrawn.
- **Decision 3:**
  - The Job is assigned at creation with `PROC_THREAD_ATTRIBUTE_JOB_LIST` (direct CreateProcessW), since std `Child` exposes no thread handle.
  - No BREAKAWAY_OK. The job handle is not inheritable.
  - Group 0 checks whether `auth login`, `plugin` or `mcp list` start the background service. If they do, those commands leave the job list.
- **Decision 1, lexical_guard:** rejects components ending in a dot or space. Deny lists are re-applied to the final path in evidence, attachment and authoring, as artifact_preview already does (8.3 short names).
- **Decision 6:**
  - The terminal dir must be a drive-letter absolute path, so a leading `-` can't become a wt option.
  - Copyable commands also reject or escape “ ” „.

## Review fixes

- **Design review 2026-11-13:**
  - design-critic: BLOCKER 2, HIGH 3, MEDIUM 4, LOW 1.
  - security-privacy-reviewer: HIGH 4, MEDIUM 5, LOW 1.
- All folded into decisions 0–11.
- Rejected: none. The critic's MEDIUM 9 and the security reviewer's HIGH 2 were merged into decision 6.

## Open Questions

- Does the background supervisor run on Windows? Task 5.4 installs the CLI on the Windows runner and checks `claude --version`, `claude agents --help`, `claude logs --help` and `claude stop --help` (no login needed). Anything missing gets the Mac-only treatment.
- Does the Windows CLI support `--bg`, `agents --json`, `attach`, `logs` and `stop`? Features that need missing ones get the Mac-only treatment.

## Implementation corrections (2026-11-13)

Each item supersedes the decision text it names; the original text above stays for the record.
1. **Decision 3, job assignment** — ~~direct CreateProcessW with `PROC_THREAD_ATTRIBUTE_JOB_LIST`~~: std spawns the
   child with `CREATE_SUSPENDED | CREATE_NO_WINDOW`, it is assigned to the job, then its only thread is resumed
   (Toolhelp snapshot, short retry). Nothing runs before assignment, and std's pipes and `Child` stay in use.
2. **Decision 3, `auth login`** — ~~in the job~~: the CLI opens the browser as its own child, and a kill-on-close job
   would close it with the login (review H2). Login runs without a job; cancelling ends only that process.
   "Login with the browser closed" is a required [manual] release check. Group 0 only checked `auth status`, not
   `auth login` (0.2 corrected).
3. **Decision 2, ACL** — ~~FILE_DELETE_CHILD / ADD_SUBDIR on an ancestor~~: ADD_FILE/ADD_SUBDIR refuse only on the
   executable's own folder (DLL planting). On higher folders only DELETE, FILE_DELETE_CHILD, WRITE_DAC, WRITE_OWNER,
   GENERIC_ALL and MAXIMUM_ALLOWED refuse: `C:\` grants Authenticated Users "create folders" by default, which
   would refuse every path. The checked final path is what runs, is stored and cached (CLI and git).
4. **Decision 10, bundle** — ~~nsis added to `bundle.targets`~~: `src-tauri/tauri.windows.conf.json` (merged only on
   Windows) holds the nsis target, `embedBootstrapper` and `currentUser`; the macOS config is untouched.
5. **Decision 6, Recycle Bin** — ~~the `trash` crate~~: `SHFileOperationW` with `FOF_ALLOWUNDO | FOF_NOCONFIRMATION |
   FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING` (never `FOF_NO_UI`), only on a fixed drive whose Recycle Bin
   answers `SHQueryRecycleBinW`, and the bin must hold more items afterwards (review H1). Wildcards are refused.
   Restoring from the bin is [manual]. Re-review 8.3: when the count does not confirm it but the original is gone,
   ~~the caller finishes its cleanup and refresh and reports `TrashUnconfirmed`~~ (superseded by 9.1) the delete is a
   success with `{done: true, unconfirmed: true}` (agents: `unconfirmed` after off/hidden), the screens follow the
   success flow and add a warn note that it may be gone for good; the shell's warning is owned by the main window. `subst` drives and mounted volumes are [manual].
6. **Decision 6, WebView2** — ~~a run-time new-window test~~: wry 0.57 denies every new-window request when no
   handler is registered; a source guard test fails if the shell ever registers one.
7. **Decision 5, git** — ~~the app's git reads the system config on Windows; moving that to an ACL check is
   deferred~~ (superseded by the re-review, 8.5): `GIT_CONFIG_NOSYSTEM=1` on every OS (the system config can also
   come from ProgramData), and on Windows only `core.autocrlf` is read from Git for Windows' own `etc\gitconfig` next
   to the checked `git.exe` and passed with `-c` (missing or unknown: `true`, Git for Windows' default).
8. **Decision 8, version gate** — one core gate (`cli_path::hooks_allowed`, set from the resolved CLI) inside the hook
   self-test, so the daily News run and local-only starts pass it too; `report_guard::grant` (no self-test) checks it
   directly and answers `NoHooks`. Local-only and News say "update Claude Code" before the self-test runs. A CLI
   downgraded outside the app passes until the next resolve (deferred).
9. **Decision 7, local-only** — the core start refuses a local-only folder off macOS (`MAC_ONLY`) before any git or
   hook run; Windows managed settings come from `%ProgramFiles%\ClaudeCode` only (the registry policy is not read).
10. **Decision 1, untrusted paths** — the Windows rule also covers relative evidence files, attachment names and the
    parts of installed or copied skill folders (`relative_ok`: no drive, stream, device name, `?*<>"|`), review H3.
    Authoring item names refuse Windows device names on every OS (`platform::is_device_name`, QA re-review).
11. **Deferred LOWs** — a Windows owner-SID check for agent files is not done (the file-index swap check now runs on
    Windows too; the folder ACL is the user's profile); see decisions.md.

