## 0. Premises (before groups 1–5)

- [x] 0.1 Non-required `windows-latest` CI job: `cargo check` + clippy for the workspace; list what fails — `.github/workflows/windows-probe.yml` (2026-11-13): core lib builds with ~10 unused/dead-code warnings (Mac-only paths); lib test fails on 2× `std::os::unix` + 1× `libc` (artifact_preview, procs, permissions, rules, memory, install_link, authoring); unused deps tokio/notify on Windows; the shell `dduck` not reached yet
- [x] 0.2 Same runner: install the CLI (native installer), record `--version`, `--help` of `--bg`/`agents`/`logs`/`stop`/`attach`, the tools list from a `-p` init message (no login: record what is observable), and whether `auth`/`plugin`/`mcp list` start the background service; decisions.md entry; rewrite Goals if `--bg` is missing — 2026-11-13, CLI 2.1.323 at `%USERPROFILE%\.local\bin\claude.exe`: `--bg` and `agents`/`logs`/`stop`/`attach` exist; `auth status`/`plugin list`/`mcp list` start no background process; `-p` without login prints the init message (tools include `Bash`, no `PowerShell` with Git Bash present) then "Not logged in". Goals unchanged

## 1. Core platform layer

- [x] 1.1 `platform`: canon (dunce + manual `\\?\` strip), clippy disallowed-methods for both canonicalize forms, is_abs, home (core + shell lib.rs:180), same_path (case-insensitive on Windows), app-data handoff (Windows local app data). Replace all call sites. Tests: `\\?\`, long path, UNC, case — platform::canon/strip_verbatim/same_path/home/app_data_under; 150+ call sites moved, clippy disallowed-methods; platform tests pass macOS and Windows CI (run 43338906338)
- [x] 1.2 `platform::file_id` (GetFileInformationByHandle) replacing every `(0,0,1)`/`nlink=1` fallback; reparse-point no-follow opens. Tests (Windows CI): hard link refused, junction refused — platform::ids/handle_ids/file_id_at via GetFileInformationByHandle on a reparse-point handle (unknown = refused); hard link + junction tests pass on Windows CI
- [x] 1.3 `lexical_guard` (non-Disk prefix, device names, streams) before filesystem calls on untrusted paths (evidence, artifact_preview, attachment, authoring); case-insensitive deny lists, `AppData` denied; artifact_preview fd_path via GetFinalPathNameByHandleW. Tests — platform::lexical_guard + windows_lexical_ok (UNC, device, stream, reserved names, trailing dot/space) at artifact_preview, reveal, editor, trash; AppData denied case-insensitively; fd_path = GetFinalPathNameByHandleW; tests pass both OSes
- [x] 1.4 HOME/USER-only reads, `~/Library/Application Support` paths, project key/real()/mcp_config/JSONL cwd normalization, `components()` splits in authoring, CRLF frontmatter. macOS outputs unchanged (tests) — HOME/USER reads via platform; app data under AppData\Local; `/` forms for claudeMdExcludes, rule/permission globs, git index, guard keys; rel_slash in authoring; JSON path fixtures escaped; macOS core tests 667 green
- [x] 1.5 Git hardening constants on Windows (app-owned empty hooksPath; fail-closed branch does not run git); git resolved once to an absolute path — git_program (ACL-checked git.exe, once), git_hooks_path (empty app-owned folder), system config read on Windows, fail-closed branch runs no git; app_git test per OS passes

## 2. Claude CLI on Windows

- [x] 2.1 cli_path: Windows candidate order, `split_paths`, safe-path rule, `.cmd` resolution before any spawn (remove `claude.cmd` from allowed names), resolved program in `CliInfo`, ACL check for exe/folder/git. Tests with fixture shims, ACL test on CI — Windows path rule, candidate order, npm shim read-and-resolve, ACL chain before --version; native/npm/shared-writable tests pass on Windows CI
- [x] 2.2 `CREATE_NO_WINDOW` on all spawns; suspended-create + Job (KILL_ON_JOB_CLOSE) only for awaited runs; `--bg`/resume never in a job; login_session `try_wait`. Tests with `fake_claude` (job kills grandchild; bg child survives parent exit) — CREATE_NO_WINDOW; suspended + KILL_ON_JOB_CLOSE job for awaited runs, detached for --bg/resume/agents/logs/stop; login exit without reaping; fake_claude job tests pass on Windows CI
- [x] 2.3 Command-line length after quoting (UTF-16) and `ReasonCode::WindowsCommandTooLong` ko/en; composer message — windows_cmdline_len (std quoting, UTF-16) vs 30,000; WindowsCommandTooLong ko/en in Rust and UI; quoting tests pass
- [x] 2.4 Windows delivery rules: `PowerShell` and `Monitor` in `--disallowedTools`, shell-launch refusals, `git.exe`/`gh.exe` variants; composer note when Git Bash is missing. Tests on emitted args — windows_variants: PowerShell, Monitor, powershell/pwsh/cmd refusals, git.exe/gh.exe forms; Git Bash note in composer; emitted-rule tests pass both OSes

## 3. Opening, hooks, Mac-only

- [x] 3.1 `open_url` (https re-check), `reveal_folder`, `open_in_editor` (.md/.txt, System32 notepad); Recycle Bin via `trash` (symlink test); WebView2 new-window denial test — platform::shell open_url/reveal/open_in_editor/trash (SHFileOperationW, junction test on CI) wired into the shell; new-window guard test (wry denies without a handler)
- [x] 3.2 Terminal: wt `-d` / powershell `current_dir`, attach via `-EncodedCommand`, terminal value rule (`;` quotes `$` control), five-quote escaping for copyable commands. Tests with `;` and smart-quote folder names — terminal_win: five-quote ps_quote, terminal value rule, EncodedCommand, wt -d / PowerShell current_dir; tests with ; and smart quotes pass
- [x] 3.3 `run_hook` returns None off macOS (single choke point); `ReasonCode::MacOnly` for local-only, force-stop, News fetch, insights, brief-refine, cloud fetch/edit, report grant. Tests on both OSes — run_hook None off macOS; MAC_ONLY start refusal + shell MacOnly guards; report_guard Grant::MacOnly; off-macOS tests pass on Windows CI
- [x] 3.4 `"onFailure": "block"` on the news, insights, brief-refine and cloud-routine hook configs; CLI ≥ 2.1.295 gate for hook-gated features with an update message; macOS tests updated — onFailure block on news/insights/brief-refine/cloud hooks (test); CLI ≥ 2.1.295 gate with CliUpdateNeeded ko/en; macOS tests green

## 4. UI

- [x] 4.1 `platform()` command and `lib/platform.ts` (isMac, isAbsPath, splitPath, shortPath, modKey) with vitest; mock UI `?os=win` — platform command + lib/platform.ts with vitest; mock ?os=win; IPC contract test; vitest 716/716, also green on Windows CI
- [x] 4.2 Replace `startsWith("/")` gates and `split("/")` displays; ⌘ labels via modKey; i18n OS words — path gates/displays via platform helpers, modKey labels, OS words; macOS strings unchanged (existing tests)
- [x] 4.3 "Mac 전용" labels on disabled controls; Git Bash missing note; vitest — MacOnly chips/notes, Git Bash note, WindowsPreview.test.tsx; vitest green
- [x] 4.4 Fonts, runtime terminal cell width, Sidebar padding/drag strip mac-only (windowChrome tests both), Windows tray icon, tooltip count — Windows fonts, measured cell width off macOS, Sidebar strip mac-only (windowChrome tests), colour tray icon on Windows, count in tooltip (badge skipped)

## 5. Build, CI, release

- [x] 5.1 tauri.conf.json nsis target and windows section — tauri.windows.conf.json (nsis, embedBootstrapper, currentUser), merged on Windows only; NSIS build green in CI
- [ ] 5.2 Portable tests: tempdir paths, testkit python discovery, `fake_claude` example binary; Windows clippy clean
- [x] 5.3 ci.yml: Windows UI + Rust jobs under the "Test and lint (Windows)" gate (macOS gate untouched); separate NSIS job (60 min) with install/launch/kill smoke — ci.yml UI (Windows) + Rust (Windows) under "Test and lint (Windows)"; macOS gate untouched; NSIS smoke job installed, showed a window, killed (run 43338906338)
- [x] 5.4 `release-windows.yml`: tag/dispatch, no cache, `npm ci`, `cargo --locked`, provenance attestation, SHA256; release-notes section with SmartScreen steps, `gh attestation verify`, and the `[manual]` unverified list — release-windows.yml written (never triggered), actions pinned by verified SHA; .github/release-notes/windows-preview.md
- [x] 5.5 Spec scenarios tagged `[CI]`/`[manual]` — every scenario in specs/windows-support/spec.md tagged [CI] or [manual]

## 6. Verification

- [x] 6.1 Design review (design-critic + security-privacy-reviewer), reflected in design.md "Review fixes"
- [ ] 6.2 Full macOS checks unchanged (core test, clippy, tsc, vitest, lint); Windows gate green; NSIS smoke green
- [ ] 6.3 Mock UI screenshots with `?os=win` (composer, settings, extensions, news) light/dark
- [ ] 6.4 Test review (qa-verifier + design-critic + security-privacy-reviewer + brand-reviewer)
- [ ] 6.5 After user approval: add the Windows gate to branch protection
