## Purpose

D.Duck runs on Windows 10/11 x64 as a preview. It finds and runs the Claude CLI safely, handles Windows paths, and refuses, with a clear message, the features whose protection is not verified on Windows.

## ADDED Requirements

### Requirement: Windows installer
The project SHALL produce a Windows x64 NSIS installer that installs WebView2 when it is missing, starts D.Duck from the Start menu, and keeps one running instance.

#### Scenario: Fresh Windows 11 machine [manual]
- **WHEN** a user runs the installer on Windows 11 x64
- **THEN** D.Duck installs, appears in the Start menu, and opens its main window

#### Scenario: Second launch [manual]
- **WHEN** D.Duck is already running and the user starts it again
- **THEN** the existing window comes to the front and no second instance runs

### Requirement: Claude CLI discovery on Windows
On Windows the app SHALL look for the Claude CLI in the native installer location (`%USERPROFILE%\\.local\\bin\\claude.exe`) first, then the npm global location, then PATH. An npm `claude.cmd` shim SHALL be resolved to the `claude.exe` it wraps before anything runs. The app SHALL refuse an executable whose file or any ancestor folder other users can modify, before running it even for a version check.

#### Scenario: Native installer [CI]
- **WHEN** a valid `claude.exe` exists at `%USERPROFILE%\\.local\\bin\\claude.exe`
- **THEN** Settings shows it as the found CLI

#### Scenario: npm shim [CI]
- **WHEN** only `%APPDATA%\\npm\\claude.cmd` exists and wraps a valid `claude.exe` under that folder
- **THEN** the wrapped `claude.exe` is shown as the found CLI and the `.cmd` is never run

#### Scenario: Shared-writable folder [CI]
- **WHEN** the only `claude.exe` found sits in a folder that Authenticated Users can modify
- **THEN** it is refused without being run, with the existing unsafe-location message

### Requirement: Starting Claude on Windows
Starting a Claude process on Windows SHALL show no console window and SHALL never pass arguments through cmd.exe: an npm `claude.cmd` shim is resolved to the native `claude.exe` it wraps before anything is run. A run the app waits for (brief refinement, version checks, plugin and MCP commands) SHALL end with its whole process tree when cancelled, timed out or when the app quits; a login ends only its own process, so a browser it opened stays open. A background task SHALL keep running after the app quits, as on macOS. A command line that exceeds the Windows limit SHALL be refused before starting.

#### Scenario: Cancel an awaited run [CI]
- **WHEN** an awaited Claude run is cancelled on Windows
- **THEN** that process and every process it started have exited within the existing grace period

#### Scenario: Background task survives app exit [manual]
- **WHEN** a background task is running and the user quits D.Duck on Windows
- **THEN** the task keeps running and appears again when D.Duck reopens

#### Scenario: Prompt too long for Windows [CI]
- **WHEN** the quoted command line would exceed the Windows limit
- **THEN** the task does not start and the composer shows the "too long on Windows" message

#### Scenario: No console window [manual]
- **WHEN** D.Duck runs claude or git on Windows
- **THEN** no console window appears

### Requirement: Default delivery rules on Windows
On Windows every task SHALL disallow the PowerShell and Monitor tools, SHALL refuse Bash commands that start another shell (powershell, pwsh, cmd), and SHALL apply each git/gh refusal rule also to the `git.exe`/`gh.exe` forms, so the default protections (draft PR stop point, no merge, no push to the default branch) hold as on macOS.

#### Scenario: Emitted rules [CI]
- **WHEN** a task starts on Windows
- **THEN** its disallowed tools include `PowerShell`, `Monitor`, the shell-launch rules and the `git.exe`/`gh.exe` variants of every git/gh rule

#### Scenario: Rules are enforced [manual]
- **WHEN** a Windows task asks Claude to run `git push origin main` through PowerShell or Bash
- **THEN** the tool call is refused

#### Scenario: No Git Bash [manual]
- **WHEN** Git for Windows is not installed
- **THEN** the composer says commands cannot run without it, and the task has no shell tool

### Requirement: Windows paths
On Windows the app SHALL treat `C:\...` and `\\server\share\...` paths as absolute, SHALL compare paths without the `\\?\` prefix and without letter-case differences of the drive, SHALL map a project folder to the same `~/.claude/projects` key Claude Code uses, and SHALL display paths with `~` for the user folder.

#### Scenario: Project list [manual]
- **WHEN** Claude Code has sessions for `C:\Users\me\work\app`
- **THEN** the project appears once, under that folder, with its sessions

#### Scenario: Composer folder [CI]
- **WHEN** the user picks `C:\Users\me\work\app` as the task folder
- **THEN** model selection and the folder-dependent options are enabled as they are for a macOS absolute path

### Requirement: Opening and deleting on Windows
On Windows, opening a link, file or folder SHALL use the Windows shell, deletion SHALL move items to the Recycle Bin, and actions that open a terminal SHALL use Windows Terminal when installed and PowerShell otherwise. Copyable commands SHALL use PowerShell syntax.

#### Scenario: Open original article [manual]
- **WHEN** the user clicks an external link
- **THEN** the default browser opens it

#### Scenario: Delete an agent definition [CI] (restore: [manual])
- **WHEN** the user deletes an agent definition and confirms
- **THEN** the file is in the Recycle Bin and can be restored

### Requirement: App-owned hooks on Windows
On macOS the hook self-test SHALL keep running the exact hook command line through `/bin/sh -c`, as the CLI does. On other OSes the self-test SHALL fail without running anything, so every feature whose safety depends on an app-owned hook (News fetch, usage insights, brief refinement, cloud routines fetch and edit, report-guard grants) stays unavailable ("Mac 전용") until a later change verifies hooks on the real Windows CLI. Hook-gated features SHALL require a Claude CLI version that supports blocking on hook start failure.

#### Scenario: Self-test on macOS unchanged [CI]
- **WHEN** a hook-gated feature starts on macOS
- **THEN** the self-test runs the hook command line through `/bin/sh -c` as before

#### Scenario: News fetch on Windows preview [CI]
- **WHEN** the user opens News on Windows
- **THEN** the fetch button is disabled with a "Mac 전용" label, stored items still display, and the backend refuses a fetch request

#### Scenario: Old CLI [CI]
- **WHEN** the CLI version is below the one that supports blocking on hook start failure
- **THEN** hook-gated features refuse to start with an "update Claude Code" message

### Requirement: Mac-only features on Windows
In the Windows preview, local-only (원격 금지) tasks, force-stopping stray Claude processes and report-guard grants SHALL be unavailable and labeled "Mac 전용" ("Mac only"), and the backend SHALL refuse them on Windows even if requested directly.

#### Scenario: Local-only toggle [CI]
- **WHEN** the user opens the composer on Windows
- **THEN** the local-only option is disabled with a "Mac 전용" label and explanation

#### Scenario: Direct request [CI]
- **WHEN** a local-only task start reaches the backend on Windows
- **THEN** it is refused with the Mac-only reason and no process starts

### Requirement: Windows look and feel
On Windows the app SHALL use the native title bar without macOS traffic-light padding, show Ctrl instead of ⌘ in shortcut labels, use a Windows monospace font, show a tray icon visible on light and dark taskbars, and name Windows places (File Explorer, Recycle Bin) instead of macOS ones.

#### Scenario: Shortcut label [CI]
- **WHEN** the composer shows its send shortcut on Windows
- **THEN** it reads "Ctrl+Enter"

### Requirement: Untrusted paths on Windows
On Windows the app SHALL refuse, before touching the filesystem, untrusted paths that are UNC, device or stream paths or reserved device names, SHALL refuse files that are hard links or reparse points where macOS refuses hard links or symlinks, and SHALL refuse a CLI or git executable in a folder other users can write to.

#### Scenario: Report evidence points to a network share [CI]
- **WHEN** a task report cites evidence at `\\\\host\\share\\a.png`
- **THEN** the evidence is refused without any network access

#### Scenario: Memory file hard-linked to credentials [CI]
- **WHEN** a memory file is a hard link to another file
- **THEN** the app refuses to read or edit it

### Requirement: Windows installer provenance
The Windows installer attached to a release SHALL be built by a release workflow from a tag without restored caches, with locked dependencies, and SHALL be published with a build-provenance attestation and SHA256 checksum.

#### Scenario: Verify a download [manual]
- **WHEN** a user runs the documented verification command on the downloaded installer
- **THEN** it confirms the installer was built by the project's release workflow

### Requirement: Credentials and transcripts unchanged
On Windows, as on macOS, the app SHALL NOT read, store or send Claude credentials (including `~/.claude/.credentials.json`) and SHALL NOT store raw transcripts.

#### Scenario: Discovery skips credentials [CI]
- **WHEN** the app scans `%USERPROFILE%\.claude`
- **THEN** `.credentials.json` is never opened
