# Decisions summary

D.Duck keeps a detailed decision log in Korean while it is developed. This page is the public English summary of that log, grouped by topic. Each bullet keeps the original decision numbers (`D-xxx`), so a decision can be traced through the specs in [`openspec/specs/`](../openspec/specs/) and the active changes in [`openspec/changes/`](../openspec/changes/).

- Range: D-001 to D-176, plus the review log.
- When a decision was replaced, the bullet says "superseded by D-xxx". The old decision is kept, not deleted.
- The project was first called DevDeck. It is called D.Duck everywhere below.

## Contents

- [Process](#process)
- [Data and privacy](#data-and-privacy)
- [Task execution and safety rails](#task-execution-and-safety-rails)
- [Reviews and evidence](#reviews-and-evidence)
- [UI and copy](#ui-and-copy)
- [Distribution and operations](#distribution-and-operations)
- [Review log highlights](#review-log-highlights)

## Process

- Log format: every product, technical and process decision goes in one log, newest first, with ID, date, decision, rationale, alternatives and source. When a decision is reversed, the old entry stays and gets a "replaced by" mark.
- **D-001** — Detailed technical design lives in each change's `design.md` (core crate split, in-app background tasks, SQLite, file watching plus rescan, bundled price table, a single effective config dir, React/Tailwind/xterm stack, anonymized fixtures plus a fake `claude` for tests). The log keeps only summaries.
- **D-010** — Development is spec-driven with OpenSpec. A hook blocks code edits outside `openspec/` and `.claude/` when no active change has open tasks.
- **D-011** — The agent goes ahead with its recommended option without asking, logs the choice, and asks for confirmation only before hard-to-undo or external actions (deploy, push, payment, delete).
- **D-012** — Each design stage and each test stage gets at least two review agents. BLOCKER/HIGH/FAIL findings must be fixed before moving on.
- **D-109** — Changes that touch UI get an extra brand and visual reviewer that works from screenshots. Fix and re-review repeat until no HIGH findings remain.
- **D-145** — Review and QA agents must attach evidence files to every verdict. Without evidence, the verdict is "no evidence" instead of PASS.
- **D-148** — Work goes to role- and skill-specific agents (implementer, brand reviewer, code reviewer) instead of a general-purpose agent.
- **D-170** — Each OpenSpec requirement gets a stable feature ID and each scenario a test-case ID, kept in one JSON source with render and check scripts.
- **D-172** — Completed changes are archived into the main specs, and their unchecked tasks move to a local follow-up list.

## Data and privacy

- **D-004** — The app never reads, stores or sends Claude credentials. It only calls the official `claude` CLI the user is logged into, and it does not use "Claude" in its name or logo, to comply with the provider's terms.
- **D-017** — The app process makes no network requests (only the child `claude` does), under a strict CSP and with no HTTP or shell/fs/opener/updater plugins (superseded in part by D-053, D-087).
- **D-087, D-092** — The first allowed network use is install-from-link only: a Rust rustls client with timeouts, no cookies or credentials, ignoring proxies, and only api.github.com and raw.githubusercontent.com. Each fetch pins a commit SHA, and file names are restricted to ASCII.
- **D-021, D-022** — Ingestion is limited to known transcript file patterns: no symlinks, credential and history files rejected by name, oversized lines skipped, no unwrap/expect. Canary strings check that raw text never reaches the DB, WAL or logs.
- **D-023, D-033** — Session titles are stored but can be turned off, which scrubs them (secure_delete, VACUUM). Titles, paths and branches stay out of notifications and logs. The DB is 0600 and identifiers are validated strictly.
- **D-035** — Accepted residual risks: some in-place file rewrites go undetected (integrity only), there is one transaction per file, and link-following is defended by canonical path checks.
- **D-019, D-020** — Slash commands, skills and agents are parsed only from specific fields. Duplicate usage records are upserted by message and request ID, and session cost is computed per cost segment.
- **D-048** — A session's first instruction is read from the transcript on demand, sent only to the screen, and never written to disk.
- **D-050, D-069** — Two narrow exceptions to "no raw text stored": routine instructions and task instructions the user typed into D.Duck are kept. There are retention limits, and deleting them scrubs the DB.
- **D-071** — The session flow reads transcripts only when opened, under size caps. Thinking blocks and tool output bodies are dropped, secret-like strings are masked, and nothing is persisted (superseded in part by D-074).
- **D-074, D-082** — Users can open the raw JSON for a session on request. It is read by byte offset and never cached, secrets are masked by default, and an unmasked view resets when the window closes.
- **D-052, D-055, D-056** — Artifact, agent-setting and subagent-activity data are read from an allowlist of fields based on observed formats. Other fields are never touched.
- **D-102** — Artifact cards show a preview only when the user asks. The app reads the local source file named in the session's publish call and renders it in a sandboxed iframe, with only an in-memory cache. This partly replaces D-052's "never read result paths", for previews only.
- **D-073** — Connector icons are bundled SVGs (CC0), so no external favicons are fetched.
- **D-078, D-093** — Optional "ask Claude for usage analysis" sends only bucketed numbers and built-in names, anonymizes all other names, and caps budget and time. Cost is always shown as an API-equivalent estimate, because detecting a subscription would mean reading credentials.
- **D-047** — D.Duck never writes the Claude config file, to avoid concurrent-write loss. For untrusted folders it opens a terminal so the user can answer the trust prompt, then retries.
- **D-054** — A narrow exception allows saving user- or project-owned agent definition files. Saves need native confirmation with permission warnings, hash and inode rechecks, and atomic writes.
- **D-076, D-088** — The permissions tab reads and changes only the `permissions` key of settings files. It swaps that value's text range in place to keep everything else, backs up only `permissions`, and rejects links.
- **D-091, D-097** — Memory toggles move files to `memory/.off/` and restore the original index lines, with no app DB storage. CLAUDE.md stays read-only and opens in an external editor. Git status is read from `.git/index` without running git.
- **D-114, D-126, D-138** — The MCP tab reads only server names, transport and scope from config files. A custom parser skips env, headers and args, so credentials are never read. claude.ai connectors appear through `claude mcp list`, which runs once per app launch when the tab opens (this replaces D-114's "run nothing on open").
- **D-150** — The app checks Claude login with `claude auth status --json`, using only six allowed fields. Model-backed features are blocked when logged out. Login is browser-only through `claude auth login`, so secrets never pass through the app.
- **D-166** — Cloud routines show last run status, run history and run logs through exact-shape, read-only calls. Remote content is kept in memory only, sanitized, size-capped and shown as plain text, because it is data and not instructions.
- **D-169** — Extension discovery also reads `.claude` folders in parent directories (up to 8 levels, stopping before home or root), which matches Claude Code. A Refresh button reloads the lists, and only usage counts are stored.
- **D-101, D-175** — Personal traces (real memory examples, session and routine IDs, project names, real paths) are replaced with generic examples in source, mocks, test data and docs before publishing. The JSONL test fixture is generated at test time instead of committed.

## Task execution and safety rails

- **D-007** — Remote execution is opt-in only.
- **D-014** — Task status comes from polling `claude agents --json --all`. Logs, stop and attach work only for background sessions, and logs are rendered with xterm.js and not stored.
- **D-015** — `bypassPermissions` and `auto` are dangerous modes that need a native Rust confirmation, because webview confirmations are not trusted. Skip-permission flags are banned, and arguments use the joined `--flag=` form with the prompt after `--`.
- **D-037** — The permission mode is always passed explicitly (default `manual`), so settings files cannot raise it silently. CLI path ownership and permissions are checked strictly.
- **D-018, D-032** — The CLI is located via settings, then known locations, then a login shell. The binary is validated, and `CLAUDE_CONFIG_DIR` is passed only when the user set it explicitly.
- **D-025** — Opening a terminal passes only argv to osascript. If Automation permission is denied, the app shows a copyable attach command instead.
- **D-034, D-038** — Child processes run in their own process group and are killed as a group on timeout. Logs keep a 1 MB tail and redraw only on change.
- **D-031, D-039** — Panics unwind and are caught at thread boundaries. All IPC is async. The watcher filters events, workers restart with backoff, and polling errors keep the last snapshot to avoid repeat alerts.
- **D-030, D-040** — Webview capabilities are cut to event listen/unlisten. Dialogs and notifications are Rust-only, navigation is guarded, terminal links are disabled, and control characters are stripped from displayed names.
- **D-043** — Every session gets "open terminal" and "force stop". Before each signal, the pid is rechecked as a `claude` binary owned by the current user.
- **D-049, D-051** — Routines need native approval, stored as a SHA-256 digest that must match before each run. The default mode is plan. The scheduler claims runs atomically and handles sleep, DST and concurrency limits.
- **D-057, D-058, D-059** — A chosen agent persona is re-parsed right before start. Risky fields, or unreadable files, trigger the dangerous-mode confirmation (fail-closed). Only personas usable in that folder are allowed.
- **D-061, D-063** — Background work follows a tech-lead model: bundled roles injected per session, task-type presets, structured briefs, advisory team rules and review capacity. Risky personas are refused for deploy, security and QA tasks.
- **D-062** — Worktree isolation was tested on a separate local repository and is on by default for dev tasks in safe repos.
- **D-066, D-068** — Cloud routines are listed through a locked-down headless `claude -p` run. A guard mode of the D.Duck binary allows only `{"action":"list"}`, and the app parses the result itself.
- **D-077, D-098** — Cloud routine edits cover only enable, name and schedule. The guard allows exactly one expected request, conflicts are checked in a separate get run, and update was tested on a disabled test routine.
- **D-095** — `--allowedTools` was dropped from headless runs (superseding part of D-066 and D-078). The guard now emits an explicit allow, and the app self-tests the guard command before each run.
- **D-067** — Phone notifications via remote control are off by default, need native confirmation, and are refused for dangerous or sensitive task types.
- **D-080** — A "learning" task type was added, and team rules now apply to D.Duck routines as well.
- **D-081, D-090** — Team rules and task types can be edited in agent settings, with only diffs stored and safety locks kept. Agents are turned off with the official `Agent(name)` deny rule. Delete moves files to the Trash.
- **D-053** — The only external URLs opened are an allowlist of two: collected artifact links and the routines page, opened via `/usr/bin/open` with a rate limit.
- **D-075, D-083** — Plugin enable/disable/update uses the CLI with validated IDs and no auto-approval, limited to user-scope plugins. Update status comes from local plugin metadata files, not `--available` (D-075 superseded in part by D-083).
- **D-085, D-086, D-096** — Install-from-link and skill/command/workflow authoring show a preview and need confirmation, never overwrite, and use color-coded warnings for hooks and auto-run tools.
- **D-079** — The permissions catalog toggles exact-match rules with risk levels and suggested values, and leaves other rules untouched.
- **D-060, D-070, D-072** — Bug fixes: error variant serialization, the first instruction missing on new tasks, and cloud routine parsing that ran after cleanup had already deleted the output.
- **D-105, D-110, D-115, D-121** — "Brief refining" lets Claude ask rounds of multiple-choice questions to improve a task brief. It runs as a headless, tool-less, guarded run. Consent, cost and round limits are enforced in Rust, and nothing is saved. Later the limits were raised, and design and architecture task types were added with their own bundled roles.
- **D-120, D-131, D-134** — Each role can have a default loadout of pre-read skills and allowed MCP servers. Servers that are not picked are blocked with `disallowedTools`. When a pinned skill changes, the task drops it or refuses to start. Any widening of access needs a native confirmation.
- **D-139, D-141, D-144** — The app alerts when a usage limit is hit. It can resume D.Duck-started background tasks after the reset, re-running all start-time safety checks. Auto-resume ships disabled behind a compile-time flag until it is verified on a real limit hit. Dangerous modes resume only on explicit per-task opt-in.
- **D-142, D-143** — By default, delegated tasks may push work branches and open draft PRs, but never merge, mark a PR ready, or push to the default branch. These rules are always added as deny rules. "Local only" projects also get a PreToolUse hook and git/gh environment backstops that block any remote push. Known limits are documented, and a local git post-check flags pushes or default-branch moves.
- **D-147** — Editable skills, commands and workflows can be edited or deleted. Delete moves items to the Trash after a native confirmation and refuses linked or hard-linked paths.
- **D-153, D-154** — Models can be picked per agent and per task. The list is built each time from local sources (CLI help aliases, settings, recent usage) instead of a hardcoded table. Managed allowlists are enforced fail-closed, and the exact model arguments are stored so resume and rework reproduce them.
- **D-155** — The login gate runs before any claim or confirmation dialog, auto-resume pauses while logged out, and the login child process gets strict lifetime and cleanup rules.
- **D-158, D-159** — Brief refinement runs with low effort and shorter guidance to cut wait time. Cancel uses a generation counter that kills the process group at any stage, and the board shows a "starting" card right away.
- **D-161** — A self-deadlock on the single DB mutex is fixed with a small connection pool. Slow guard checks run without holding the lock, and an in-flight task start can be cancelled.
- **D-162** — The plugin and skill tabs share one skill finder that accepts manifest paths pointing straight at skill folders, and the path cap goes from 20 to 200.
- **D-163** — Local-only projects may use auto and bypassPermissions modes, because live tests showed the hook blocks remote pushes. Redundant `Write(...)` deny rules are dropped since `Edit(...)` rules already cover them.
- **D-164** — Task start output without a name is parsed correctly. The task record write retries up to 3 times, a combined record-and-stop failure is shown to the user, and the start window closes on success.
- **D-167** — "Mark as done" stops a waiting task cleanly and moves it to review (if an unreviewed report exists) or to done, without skipping the review gate.

## Reviews and evidence

- **D-028** — Fixes from a re-check review: symlinked CLI checks, scan depth, canary placement, the 24-hour rule for ended sessions, and a shared config dir.
- **D-094** — Correction: D-087 wrongly said the CLI lacks `install --marketplace`. It has the option, and the two-step flow stays for now.
- **D-100** — Three critical reviewers ranked improvement proposals: cost visibility first, then in-app answers, a review queue and risk banners. Each is a separate change.
- **D-107, D-108, D-112, D-122** — Delegated tasks produce a "task report": a JSON file the agent updates, which the app renders. The top section, the agreed brief, comes from the app and the agent cannot change it. Strict size and format limits apply, and a computed one-line verdict sits at the top (D-107 superseded by D-108).
- **D-140** — A Review tab groups finished tasks with reports by role. It reuses the `reviewed_at` field, loads only card summaries until a card is opened, and pages ten cards at a time.
- **D-149, D-152** — Reviewers can request rework, which always creates a new linked task. Report criteria need concrete evidence files (screenshots, test reports, command output, file:line). Review and QA roles get locked evidence rules, and a task is only "done" after review.
- **D-156** — Task reports may carry free-form HTML or Markdown attachments, shown through sanitization, a strict CSP and a sandboxed iframe. Report types decide whether criteria are required.
- **D-165** — Report instructions ask agents to put the final deliverable in `attachments/result.md` or `result.html` (or a PR link), so reviews show the result and not only the process.
- **D-174** — Report and review card content can be selected and copied, and attachments can be saved through a native save dialog or copied as Markdown.

## UI and copy

- **D-005, D-044** — The working name DevDeck was replaced by D.Duck, including the product name and identifier (D-005 superseded by D-044).
- **D-036, D-045** — The app icon is an original white duck in three colors (navy, white, yellow), drawn to avoid copyrighted characters. It was enlarged for the Dock, and the menu-bar tray uses a template image.
- **D-041** — A full redesign added a navy sidebar, an overlay title bar and the bundled Pretendard font. Yellow is reserved for "waiting for input", contrast meets AA, and a browser mock mode is used for design checks.
- **D-042** — Tasks are shown as a four-column board (waiting, in progress, needs review, done).
- **D-016, D-024** — Notification clicks are replaced by a tray menu with fixed notification text. Closing the window hides it, Cmd+Q quits, and the app runs as a single instance.
- **D-046** — Chart colors are muted navy and blue-gray, validated for contrast.
- **D-064** — Duck colors and chips are assigned by role, with sort by name or custom order and active agents first (supersedes the card order in D-059).
- **D-065** — The theme cycles system, light and dark, and is applied before first render.
- **D-089, D-099** — A first-run tour with cards and a "getting started" checklist. Tour state lives in the app DB, and the install hint uses the official native installer command.
- **D-106, D-119** — Rules files are edited in a Rules segment of the Memory tab. A rule is disabled by adding it to `claudeMdExcludes`, not by moving files. Each screen description is one line.
- **D-111, D-125, D-127, D-130, D-132** — Project selection moved from one global sidebar picker to a [Global | Project] scope bar in each tab, remembered per tab, in the same spot on every tab, with unsaved-change guards (D-111 superseded by D-127).
- **D-112, D-117** — Skills, commands, workflows and plugins share one Extensions tab, and MCP got its own top-level tab. Locked plugin or built-in items can be copied into an editable `-custom` version.
- **D-113, D-116, D-133, D-135, D-136, D-137** — The tutorial was rewritten as a six-card story around the core message, with real UI fragments as demos. Ducks always appear as role-labeled agents, not mascots.
- **D-128, D-129** — Role ducks wear role-specific items in fixed colors: belt, beanie or bow tie.
- **D-123** — App-wide copy uses "review" wording and the polite Korean form throughout.
- **D-146** — Double-clicking the window header zooms the window.
- **D-157** — The routine detail header wraps its buttons onto the next line, and its tables scroll inside the card.
- **D-168** — The whole app uses `overflow-wrap: anywhere`, so long URLs, paths and logs wrap instead of widening the screen.
- **D-171** — The app supports Korean and English (following the system language by default) through a lightweight typed catalog, and backend messages reuse the existing error codes.
- **D-176** — Design direction: a quiet native macOS look with a neutral sidebar, graphite dark mode and shadows instead of borders for hierarchy, keeping the current duck character.

## Distribution and operations

- **D-002** — The stack is Tauri 2 with React/Vite/TypeScript and a Tauri-independent Rust core crate.
- **D-003** — Future multi-device sync would prefer a Supabase Realtime relay, with Tailscale as an advanced option.
- **D-006, D-008, D-009** — MVP scope: single device, usage stats and local tasks, Claude Code CLI only (with an adapter kept for others), and the artifacts tab limited to Claude artifacts.
- **D-013** — Rust was upgraded in place. Cross-platform builds move to CI later.
- **D-026** — Markdownlint rules that conflict with the OpenSpec format are turned off.
- **D-027, D-029** — The project uses the default template versions (React 19, Vite 8, TypeScript 6) without the opener plugin, and ESLint is pinned to 9.x. Dev-only audit findings are accepted after checking runtime dependencies.
- **D-084** — Leftovers of the old name were cleaned up: code names and the DB file were renamed with a one-time migration.
- **D-103, D-104** — The service intro is a separate slide deck for developers. Its core promise is templated task delegation, and every claim must be backed by code.
- **D-118, D-124** — Local DMG builds go to fixed paths and overwrite the old ones, using a single release script. Uploading anywhere needs user confirmation (D-118's dated file names superseded by D-124).
- **D-151** — Intro video subtitles are short declarative lines that state what the app does.
- **D-160** — Local release bundles get a full ad-hoc deep code signature, so other Macs no longer flag them as "damaged". They are not notarized, so the first launch needs right-click → Open.
- **D-161** — Release builds keep debug symbols in separate dSYM files, archived outside the app (last 10), so hangs can be diagnosed without a rebuild.
- **D-173** — The project is licensed under Apache-2.0, with an English README and a note that it is not affiliated with Anthropic.
- **D-175** — The public repository is created in the working folder, with private material kept out by `.gitignore` and a scan before the first commit. Test fixtures are generated by `dduck_core::testkit::ensure_basic()`, which needs `python3`.

## Review log highlights

Each design stage gets `design-critic` and `security-privacy-reviewer`. Each test stage gets `qa-verifier` and `design-critic`. BLOCKER/HIGH/FAIL findings are fixed before the next stage.

- **MVP bootstrap** (design) — Security found that a confirm dialog could be skipped over IPC and that a command-tag regex picked up raw transcript fragments. Design found offline contradictions, a missing `--all` and no session id for interactive sessions. Tasks were rewritten; see D-014 to D-023, D-025, D-028.
- **CLI assumptions** (design probes) — The real CLI was run to confirm `--bg` output, untrusted-directory exit codes and log format before building on them.
- **Project skeleton** (test) — QA PASS. Design MAJOR findings were fixed with clippy config, a tokio feature and jsdom stubs; see D-030, D-031.
- **Collector core** (test) — Turning titles off left their bytes on disk; fixed with secure delete and VACUUM plus a 300-row test. Mutation testing exposed weak fixtures, which were tightened; see D-033, D-035.
- **Shell, CLI, stats, UI** (test) — A re-alert after transient errors was fixed with poll state, and an inherited default permission mode was closed; see D-037 to D-040.
- **UI overhaul** (design and test) — The board spec was not updated (BLOCKER), and confirm state carried over to another session and could stop the wrong one. Fixed with a component key and a regression test; see D-041 to D-043.
- **Artifacts and routines** (design) — Real artifact output did not match the assumed format (BLOCKER), and saving a routine had no native confirm (HIGH); see D-049 to D-053.
- **Agent profiles** (design and test) — Persona frontmatter could bypass risky-mode checks, and YAML tricks could hide warnings. Risk is now judged by an allowlist and the raw frontmatter is shown at confirm; see D-054 to D-060.
- **Tech-lead workflow** (design and test) — The webview could inject roles, policy and env into the task spec, and worktree names allowed path traversal. The webview now sends only choices and names are normalized; see D-062, D-063.
- **Cloud routines and notifications** (design and test) — A headless model could change routines, so an app-level PreToolUse hook allows list calls only. A nested-regex guard bypass was replaced with an executable JSON guard; see D-066 to D-068.
- **Task prompt history** (design) — Turning history off asks for native confirmation and clears old records; history is capped; see D-069.
- **Sessions, skills, connectors** (design and test) — Secrets could leak in the timeline, so redaction runs first and only the first error line is kept; see D-071.
- **Routines, plugins, permissions, raw view, insights** (design) — Output could exceed the size cap, a permission parser rejected valid rules, and a permission backup copied env secrets. Only permissions are backed up now.
- **Editable team presets** (design and test) — Allowed tools come only from a Rust allowlist and risk levels are fixed in Rust. Switching a strict type to a custom agent is refused, and hidden Unicode characters are blocked.
- **Tool permissions** (test) — Saving rewrote existing rules with display-trimmed text; it now keeps the original text and applies only the diff. Each row shows the effective result.
- **Install from link and skill authoring** (design) — The preview ran a marketplace add before confirmation, and authoring could edit global settings and hooks. Previews now read manifest data only, and drafts are built in an app temp folder; see D-096.
- **Memory tab** (design and test) — Deleting index lines conflicted with Claude's own rewrites, so items move to `.off/`, and user-written index lines are kept and restored.
- **Usage insights** (test) — The guard hook failed open; the pre-allowed tools flag was removed and the guard emits an explicit allow; see D-095.
- **Rename leftovers** (test) — A failed DB migration could move only the WAL file; the main file now moves first and everything rolls back on failure; see D-084.
- **Artifact previews** (design and test) — A TOCTOU gap in path validation was fixed with no-follow open and an fd re-check; thumbnails got a larger frame; see D-102.
- **Rules editor** (design and test) — Moving rules off would disable git-tracked rules for the whole team, so rules are turned off through local settings excludes.
- **Brief refining** (design and test) — Web-search advice could become instructions for a write-capable agent, so advice is quoted and plan mode is forced; see D-121.
- **Task report** (design and test) — The app did not know worktree paths, so reports go to an app data folder. Dialog focus leaked to the terminal behind, fixed with a focus trap; see D-122.
- **Onboarding tour** (design and test) — Mock data held a real personal name, replaced with generic names plus a banned-pattern test. Focus now returns on close.
- **Navigation and tour v2** (design and test) — Listing MCP servers risked starting them or exposing env, so a strict struct parses only name, transport, scope and host; see D-126, D-135, D-136.
- **Project switcher** (design and test) — Screens used mixed id schemes, now unified as opaque sha256 ids; checking status in an untrusted project asks for trust first; see D-132.
- **Agent loadout** (design and test) — The plan relied on a field that adds MCP servers rather than limiting them, so unselected servers are blocked via disallowed tools; see D-134.
- **Review inbox and rework evidence** (design and test) — Reviewers could stall on write prompts in plan mode, and a `git diff --output` arbitrary-write gap was found. The change was split in three and output options are denied; see D-140, D-152.
- **Usage limit resume** (design) — Unattended resume skipped the risk re-check; it is re-checked now, and dangerous modes need a per-task opt-in; see D-141.
- **Delivery guard** (design, test and three security re-audits) — Alias, config-env and git-config smuggling bypasses were found. The backstop became a fixed protocol-deny config; remaining known gaps are recorded; see D-143.
- **Extension edit and delete** (design) — Deleted skills reappeared as built-in rows; hashing now streams and the confirm lock is held until the move ends; see D-147.
- **Model select and login** (design and test) — Stored model values are re-validated before args are built, and logging out no longer lets auto-resume claim rows; see D-154, D-155.
- **Report attachments** (design and test) — Table rendering and source links were fixed, `data:` images and CSS were checked, and evidence marked as passed without proof was caught; see D-156.
- **App language** (design) — Missing bundle data, overlap with the existing code system and Rust-side display strings were fixed with parameter allowlists; see D-171.
- **Report copy and save** (design) — Review cards were buttons, so text could not be selected; they became articles with an open button; see D-174.
- **Open-source publishing prep** (design) — The private-data scan missed real-name paths and private content; patterns were generalized, the archive was excluded and test fixtures moved into a testkit module; see D-175.
- **Palette refresh** (design) — Proposed colors failed contrast and broke the single-yellow rule; contrast-safe values and a yellow allowlist were added; see D-176.

### Recurring lessons

- Probe real CLI behavior before designing around it. Several BLOCKERs came from assumed output formats or flags.
- Guards must fail closed. Prefix rules, regex checks and pre-allowed tools kept failing open and were replaced by hooks with explicit allow/deny and fixed config backstops.
- The webview is untrusted. It sends opaque ids and choices only, and Rust re-validates right before writing or spawning.
- UI reviews kept flagging the same issues: more than one yellow, contrast misses, focus loss in dialogs, and layouts breaking at 900–960 px.
