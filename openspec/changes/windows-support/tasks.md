## 0. Premises (before groups 1–5)

- [x] 0.1 Non-required `windows-latest` CI job: `cargo check` + clippy for the workspace; list what fails — `.github/workflows/windows-probe.yml` (2026-11-13): core lib builds with ~10 unused/dead-code warnings (Mac-only paths); lib test fails on 2× `std::os::unix` + 1× `libc` (artifact_preview, procs, permissions, rules, memory, install_link, authoring); unused deps tokio/notify on Windows; the shell `dduck` not reached yet
- [x] 0.2 Same runner: install the CLI (native installer), record `--version`, `--help` of `--bg`/`agents`/`logs`/`stop`/`attach`, the tools list from a `-p` init message (no login: record what is observable), and whether `auth`/`plugin`/`mcp list` start the background service; decisions.md entry; rewrite Goals if `--bg` is missing — 2026-11-13, CLI 2.1.323 at `%USERPROFILE%\.local\bin\claude.exe`: `--bg` and `agents`/`logs`/`stop`/`attach` exist; `auth status`/`plugin list`/`mcp list` start no background process; `-p` without login prints the init message (tools include `Bash`, no `PowerShell` with Git Bash present) then "Not logged in". Goals unchanged

## 1. Core platform layer

- [ ] 1.1 `platform`: canon (dunce + manual `\\?\` strip), clippy disallowed-methods for both canonicalize forms, is_abs, home (core + shell lib.rs:180), same_path (case-insensitive on Windows), app-data handoff (Windows local app data). Replace all call sites. Tests: `\\?\`, long path, UNC, case
- [ ] 1.2 `platform::file_id` (GetFileInformationByHandle) replacing every `(0,0,1)`/`nlink=1` fallback; reparse-point no-follow opens. Tests (Windows CI): hard link refused, junction refused
- [ ] 1.3 `lexical_guard` (non-Disk prefix, device names, streams) before filesystem calls on untrusted paths (evidence, artifact_preview, attachment, authoring); case-insensitive deny lists, `AppData` denied; artifact_preview fd_path via GetFinalPathNameByHandleW. Tests
- [ ] 1.4 HOME/USER-only reads, `~/Library/Application Support` paths, project key/real()/mcp_config/JSONL cwd normalization, `components()` splits in authoring, CRLF frontmatter. macOS outputs unchanged (tests)
- [ ] 1.5 Git hardening constants on Windows (app-owned empty hooksPath; fail-closed branch does not run git); git resolved once to an absolute path

## 2. Claude CLI on Windows

- [ ] 2.1 cli_path: Windows candidate order, `split_paths`, safe-path rule, `.cmd` resolution before any spawn (remove `claude.cmd` from allowed names), resolved program in `CliInfo`, ACL check for exe/folder/git. Tests with fixture shims, ACL test on CI
- [ ] 2.2 `CREATE_NO_WINDOW` on all spawns; suspended-create + Job (KILL_ON_JOB_CLOSE) only for awaited runs; `--bg`/resume never in a job; login_session `try_wait`. Tests with `fake_claude` (job kills grandchild; bg child survives parent exit)
- [ ] 2.3 Command-line length after quoting (UTF-16) and `ReasonCode::WindowsCommandTooLong` ko/en; composer message
- [ ] 2.4 Windows delivery rules: `PowerShell` and `Monitor` in `--disallowedTools`, shell-launch refusals, `git.exe`/`gh.exe` variants; composer note when Git Bash is missing. Tests on emitted args

## 3. Opening, hooks, Mac-only

- [ ] 3.1 `open_url` (https re-check), `reveal_folder`, `open_in_editor` (.md/.txt, System32 notepad); Recycle Bin via `trash` (symlink test); WebView2 new-window denial test
- [ ] 3.2 Terminal: wt `-d` / powershell `current_dir`, attach via `-EncodedCommand`, terminal value rule (`;` quotes `$` control), five-quote escaping for copyable commands. Tests with `;` and smart-quote folder names
- [ ] 3.3 `run_hook` returns None off macOS (single choke point); `ReasonCode::MacOnly` for local-only, force-stop, News fetch, insights, brief-refine, cloud fetch/edit, report grant. Tests on both OSes
- [ ] 3.4 `"onFailure": "block"` on the news, insights, brief-refine and cloud-routine hook configs; CLI ≥ 2.1.295 gate for hook-gated features with an update message; macOS tests updated

## 4. UI

- [ ] 4.1 `platform()` command and `lib/platform.ts` (isMac, isAbsPath, splitPath, shortPath, modKey) with vitest; mock UI `?os=win`
- [ ] 4.2 Replace `startsWith("/")` gates and `split("/")` displays; ⌘ labels via modKey; i18n OS words
- [ ] 4.3 "Mac 전용" labels on disabled controls; Git Bash missing note; vitest
- [ ] 4.4 Fonts, runtime terminal cell width, Sidebar padding/drag strip mac-only (windowChrome tests both), Windows tray icon, tooltip count

## 5. Build, CI, release

- [ ] 5.1 tauri.conf.json nsis target and windows section
- [ ] 5.2 Portable tests: tempdir paths, testkit python discovery, `fake_claude` example binary; Windows clippy clean
- [ ] 5.3 ci.yml: Windows UI + Rust jobs under the "Test and lint (Windows)" gate (macOS gate untouched); separate NSIS job (60 min) with install/launch/kill smoke
- [ ] 5.4 `release-windows.yml`: tag/dispatch, no cache, `npm ci`, `cargo --locked`, provenance attestation, SHA256; release-notes section with SmartScreen steps, `gh attestation verify`, and the `[manual]` unverified list
- [ ] 5.5 Spec scenarios tagged `[CI]`/`[manual]`

## 6. Verification

- [x] 6.1 Design review (design-critic + security-privacy-reviewer), reflected in design.md "Review fixes"
- [ ] 6.2 Full macOS checks unchanged (core test, clippy, tsc, vitest, lint); Windows gate green; NSIS smoke green
- [ ] 6.3 Mock UI screenshots with `?os=win` (composer, settings, extensions, news) light/dark
- [ ] 6.4 Test review (qa-verifier + design-critic + security-privacy-reviewer + brand-reviewer)
- [ ] 6.5 After user approval: add the Windows gate to branch protection
