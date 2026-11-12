## Why

The user asked to run D.Duck on Windows too, shipping in v0.1.3 (D-209). The 2026-11-13 inventory shows the Rust code mostly compiles for Windows already: `libc` is unix-only and most `std::os::unix` code is behind `cfg(unix)`. At runtime, though, nothing works:
- The CLI path check rejects `\`.
- `canonicalize()` returns `\\?\` paths.
- Six features self-test their hook through `/bin/sh`.
- No process tree is killed.
- Every "open" goes through `/usr/bin/open`.
- CI only runs on macOS.

The local-only delivery guard also has Windows-specific holes: the PowerShell tool, `git.exe`, backslash paths, and Windows managed-settings sources. Those are security regressions, not just missing features.

## What Changes

- **Windows 10/11 x64 preview build:**
  - NSIS installer with the WebView2 bootstrapper.
  - Unsigned. The release notes explain the SmartScreen warning.
  - Built and tested by a new `windows-latest` CI job that becomes a required check.
- **Find and launch the Claude CLI on Windows:**
  - Windows-aware path validation.
  - npm `claude.cmd` resolved to `node.exe cli.js`, or the native `claude.exe` is used.
  - Prompts too long for a Windows command line are refused with a clear message.
  - No console window flashes.
  - The whole process tree ends on cancel or timeout (Job Object). Login exit is detected.
- **Paths:**
  - One canonicalization without `\\?\`.
  - Path comparisons by components, not `/` strings.
  - Home from `USERPROFILE`.
  - App data under the Tauri app data folder.
  - The UI accepts and displays `C:\` paths.
- **Opening things:**
  - URLs, files and folders open with the Windows shell.
  - Deletion goes to the Recycle Bin.
  - Terminal actions use Windows Terminal or PowerShell.
  - Copyable commands use PowerShell syntax.
- **Hook self-test** spawns the exe with an argument list instead of `/bin/sh`. Hook-gated features (News fetch, usage insights, brief refinement, cloud routines) stay Mac-only on Windows until a real Windows CLI run shows the hook is enforced.
- **Mac-only in this preview** (shown as "Mac 전용", not hidden silently), because their protection can't be verified on Windows yet:
  - local-only (원격 금지) tasks and their delivery guard
  - force-stop of stray Claude processes
- **Windows UI:**
  - Native title bar without the macOS traffic-light padding.
  - Ctrl shortcut labels.
  - Windows monospace fonts.
  - A tray icon visible on dark taskbars.
  - Text that says "Finder"/"macOS" adapts per OS.

## Capabilities

### New Capabilities
- `windows-support`: what D.Duck does and refuses to do on Windows.

### Modified Capabilities
(none; existing behavior on macOS is unchanged)

## Impact

- **Rust core:**
  - cli_path, claude_cli, process, login_session, procs, projects, mcp_config, cloud_routines, memory, delivery_guard, permissions, preferred, artifact_preview, ingest, install_link, skills_catalog, routines, testkit.
  - A new `platform` module (paths, open, trash, terminal, job).
  - New Windows-only dependencies: `windows-sys`, `dunce`, `trash`.
- **Tauri shell:** lib.rs (open_url, trash, terminal, tray, badge), login_cmds, memory_cmds, news_cmds; tauri.conf.json bundle targets and Windows section.
- **UI:**
  - Path helpers in lib/format.ts and the files that split on `/`.
  - Composer and LocalOnlySection absolute-path gates.
  - Shortcut labels, fonts, Sidebar titlebar padding.
  - i18n OS words.
- **CI:** windows job in .github/workflows/ci.yml plus a gate. Release: a Windows bundle job that uploads the NSIS installer as a workflow artifact, attached to the release only after the user approves.
- **macOS:** no behavior change. The existing macOS CI stays required.
