<!-- markdownlint-disable MD033 MD041 -->
<div align="center">

<img src="app/src-tauri/icons/128x128@2x.png" width="96" alt="D.Duck icon" />

# D.Duck

### Work like a tech lead with a team of Claude Code agents.

See what Claude Code keeps out of sight, delegate more without losing quality, and spend your attention only where it matters:
visibility, enforced guardrails, and review with evidence.

[![CI](https://github.com/DevJoyKo/d-duck/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/DevJoyKo/d-duck/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/DevJoyKo/d-duck?include_prereleases&sort=semver)](https://github.com/DevJoyKo/d-duck/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
![macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)

[한국어](README.ko.md) · [Install](#install) · [Build from source](#build-from-source)

<img src="docs/assets/screenshots/en/04-board-waiting-alert.png" width="860" alt="Task board: a task waiting for your answer sits at the top" />

</div>

---

## What it does

The more you delegate, the more your attention becomes the scarce resource. Most tools help you watch one session closely. D.Duck lets you work one level above that, the way a tech lead does: you set direction, draw the guardrails, and make the calls only you can make, while the agents carry out the work. You raise the level of delegation without lowering the quality bar. Running several Claude Code agents at once raises a set of problems, and D.Duck answers each one.

| Pain point | D.Duck feature |
|---|---|
| Claude Code keeps a lot out of sight: memory, settings spread over several scopes, skills and plugins I installed and forgot, folders left behind by finished tasks. I can't manage what I can't see. | **Visibility first:** memory you can read, edit, and switch off; effective permissions; skills, plugins, and MCP servers with when they were last used; usage by session; and a wrap-up step that cleans up after finished tasks |
| I keep control through memory and rules, and agents mostly follow them. But I can't see where they actually apply, and nothing stops the rare run that doesn't. To delegate more, trust-critical work needs a line that holds anyway. | **Guardrails enforced outside the prompt:** draft PR by default, "Block remote push" for trust-critical repos, risky modes blocked for strict task types |
| I can't tell which permissions actually apply when settings come from several files. | **Effective permissions** merged across every scope, next to each rule |
| A vague ask comes back off target, and I only find out at review time. | **Structured brief** (goal, out of scope, constraints, done criteria) with Claude's clarifying questions before it starts |
| "Done, tests pass" tells me little, so I end up re-checking everything. | **Reports with evidence:** each done criterion as met, partly met, or not met, with screenshots, test reports, or command output |
| The session that wrote the code is the worst reviewer of it. | **Fresh-eyes review:** hand finished work to a new QA or review session that never saw the conversation |
| Agents finish faster than I can review, so review becomes the bottleneck. | **Review capacity** per day, and a **Review tab** that groups finished work by role, routine runs included |
| With many sessions open, I check terminal tabs one by one to find what's waiting or failed. | **One task board** with tasks that need you on top, a live terminal preview, and a notification when an agent waits |
| Claude keeps reaching for the same few skills and rarely considers the ones I installed on purpose. | **Consider these first:** pick up to 10 installed plugins, skills, and agents for Claude to check before it starts (prompt guidance) |
| Every task runs on the default model unless I go back to the terminal. | **Model per task type, role, agent, or single task,** with an automatic fallback when a model is no longer supported |
| Skills, MCP servers, plugins, and memory are scattered across files. | **Extensions, MCP, Permissions, and Memory tabs:** see, create, edit, and delete without JSON |
| When a way of working proves itself, I still redo it by hand every time. | **Grow what works:** save it as a skill or command, chain skills into a workflow (checked so Claude Code can actually load it), and put it on a schedule as a routine |
| I don't notice my own bad habits: an expensive model for small tasks, failing commands repeated, outdated setups. | **Usage review:** finds patterns in your own usage that cost money or lower success rates, explains why, and turns each recommendation into a task you can hand off |

The five groups below explain how these fit together.

### 1. Guardrails that are enforced, not asked for

**Problem:** Memory and rules files already steer agents well, and for most work that's enough. But they are instructions, not enforcement: nothing stops the rare run that ignores them. To raise the level of delegation, trust-critical work needs a line that holds even then.

**D.Duck:** You decide where delegated work stops and waits for you, and that line is enforced outside the prompt by CLI deny rules, a pre-tool hook, and git settings.

- **Default guardrail: a draft PR.** Agents can push a work branch and open a draft PR. Merging stays with you.
- **Trust-critical repos: keep it on your machine.** Turn on "Block remote push" and nothing leaves until you've reviewed it locally.
- **Risky permission modes are blocked** for security, deploy, and QA tasks.

Not every setting is a guardrail. Team rules and "Consider these first" (up to 10 installed plugins, skills, and agents Claude should check before it starts; by default the `mattpocock-skills` plugin from `claude-plugins-official`, left out when it isn't installed or is turned off) are added to the prompt as guidance: each task gets one line asking Claude to check them first. They steer the agent but are not enforced like deny rules or hooks.

### 2. Review with evidence, sized to your time

**Problem:** Agents finish work faster than you can read it, and "done, tests pass" tells you little. Review becomes the new bottleneck.

**D.Duck:**

- **Brief before you hand off:** goal, out of scope, constraints, and done criteria. Claude asks clarifying questions first.
- **Reports with evidence:** each done criterion comes back as met, partly met, or not met, with screenshots, test reports, or command output. No evidence, no pass.
- **Fresh eyes:** send finished work to a new QA or review session that never saw the original conversation.
- **Review capacity:** set how many changes you can properly review in a day. Past that, D.Duck asks before starting more.
- **One place to review:** the Review tab lists routine runs' reports next to tasks, and under All, finished tasks without a report with a "No report" chip. Delete a finished card after a native confirmation; the Claude session and the git worktree and branch are kept.

### 3. See what is hidden, and what needs you

**Problem:** Claude Code stores a lot where you don't look: memory that goes into every session, permissions merged from several scopes, skills and plugins installed and forgotten, folders left behind by finished tasks. And with many sessions open, you check terminal tabs one by one to find what's waiting, failed, or done.

**D.Duck:** One board for terminal sessions and D.Duck tasks, with tasks that need your answer at the top, a live terminal preview, and a notification when an agent waits. What was hidden comes out too: memory you can read, edit, and switch off; the permissions that *actually* apply; skills, plugins, and MCP servers with when they were last used, so you can clear out what you never use; and a wrap-up step that removes what finished tasks left behind. All editable without JSON, and raw transcripts are never stored.

### 4. Grow what works into skills, workflows, and routines

**Problem:** Delegation pays off when a good way of working is reused, not retyped. A prompt that worked once lives in a chat history, and turning it into something repeatable means hand-editing files you can't easily check.

**D.Duck:** Move what works up a level, step by step.

- **Skill or command:** save a way of working as a skill or slash command, write it yourself or ask Claude to draft it, or install one from a link.
- **Workflow:** chain those skills into a larger multi-step workflow. D.Duck checks the workflow the way Claude Code reads it, so a file that Claude Code would silently skip is flagged before you save it.
- **Routine:** run the workflow on a schedule, on your Mac or as a claude.ai cloud routine, with the same guardrails as any other task.

Each step makes the unit of delegation bigger, so you hand off whole processes, not single prompts, and your attention stays on decisions.

### 5. Find the habits you don't see, and fix them

**Problem:** The ways you misuse a tool are the ones you can't see yourself. A heavy model for quick tasks, the same failing command retried, a cache that never pays off, a CLI several versions behind. None of it shows up as an error.

**D.Duck:** The usage review reads your own usage records and points out patterns that cost money or lower success rates, with the reason and a source for each. Each recommendation can become a task in one click, so the fix is handed off like any other work instead of staying a note. When the fix is the model, set it per task type, role, or agent, or for a single task.

<table>
<tr>
<td width="50%"><img src="docs/assets/screenshots/en/07-settings-block-remote-push.png" alt="Settings: Block remote push" /></td>
<td width="50%"><img src="docs/assets/screenshots/en/05-task-report.png" alt="Task report with evidence" /></td>
</tr>
<tr>
<td align="center">Guardrail: keep a trust-critical repo local</td>
<td align="center">A report with evidence for each done criterion</td>
</tr>
<tr>
<td><img src="docs/assets/screenshots/en/02-composer-refine-questions.png" alt="Claude asks before it starts" /></td>
<td><img src="docs/assets/screenshots/en/06-review-tab.png" alt="Review tab" /></td>
</tr>
<tr>
<td align="center">Claude asks before it starts</td>
<td align="center">Finished work, grouped by role</td>
</tr>
</table>

## Why I built this

I wanted to hand real work to a team of Claude Code agents without giving up control over permissions, and without spending all the time I saved on reviewing their output. D.Duck came out of about a week of thinking that through and building it. It's centered on running Claude Code locally.

**Why a duck?** D.Duck stands for **Developer Duck**, from [rubber duck debugging](https://en.wikipedia.org/wiki/Rubber_duck_debugging): you explain the problem to a duck, and the explaining helps. Here the ducks are your agents, each with a role, and they answer back.

## All features

| Area | What it does |
|---|---|
| **Agents** | Every agent (user, project, plugin, built-in) as a duck card showing whether it's working, waiting for you, failed, or idle. Ten built-in roles, injected into task sessions only. |
| **Hand off a task** | Pick a folder and one of 10 task types. Each type brings its role, tools, default permissions, and effort. Team rules are added to every task; safety rules are locked. "Consider these first" adds one line asking Claude to check up to 10 picked plugins, skills, and agents. |
| **Models** | Pick a model per task, or set one per task type, role, or agent. An unsupported model falls back to a same-family alias, then the default: never a pricier family, never past an allow list. The task shows the requested and the used model. |
| **Task board** | Queued → in progress → needs you → waiting for review → wrap-up, with a live terminal preview. |
| **Review** | Reports with evidence, a Review tab grouped by role, copy text and save attachments. Routine runs' reports are listed too, and under All, finished tasks with no report ("No report"). Delete a finished card after a native confirmation (the Claude session and git worktree and branch are kept). |
| **Guardrails** | Draft PR by default, "Block remote push" for trust-critical repos, no risky modes for strict task types. |
| **Routines and artifacts** | Scheduled work, locally or as claude.ai cloud routines. Artifacts Claude made across sessions. |
| **Configure without JSON** | Skills, commands, workflows, plugins, MCP servers, rules, memory, and effective permissions. Changes are shown before they're saved. Update all or selected plugins at once. |
| **Usage** | Cost, models, and tool and skill calls by period, usage-limit alerts, and a usage review that turns recommendations into tasks. |
| **Language** | Korean and English, switched from the sidebar or Settings. |

## Design principles

| | |
|---|---|
| **Local only** | Claude Code records are read on this Mac only, and app data stays in a local database. The only outside requests are ones you trigger yourself, such as installing from a link or running a usage review. |
| **No access to credentials** | D.Duck never reads, stores, or sends your Claude login. It only calls the official Claude CLI you've already logged in to. |
| **No raw transcripts** | Transcripts are aggregated into numbers and names, not stored. The raw view reads them only when you open it, with secrets masked by default. |
| **Confirm risky actions** | Writing settings files, risky permissions, approving routines, and installs are hard to undo. D.Duck shows what will change and asks first. |

Guardrails are enforced in layers (CLI deny rules, a hook, and git settings). Some gaps remain: a hand-written network client can get past the hook, so the git setting is the last layer. "Consider these first" and team rules are prompt-level guidance, not one of these layers.

## Install

> **Status: early (v0.1.2).** Apple Silicon Macs only for now. The UI comes in Korean and English. Switch it from the sidebar or in Settings. The app is signed with a Developer ID and notarized by Apple, so it opens without a warning.

1. Download the latest `.dmg` from [Releases](https://github.com/DevJoyKo/d-duck/releases) and drag **D.Duck** into Applications.
2. You need [Claude Code](https://docs.claude.com/en/docs/claude-code) installed. If you're not logged in, D.Duck shows a button that opens the browser login.

## Build from source

Requirements: Rust 1.85+, Node 22+, Xcode Command Line Tools.

```bash
git clone https://github.com/DevJoyKo/d-duck.git
cd d-duck/app
npm install
npm run tauri dev          # run the app in dev mode
npm run dev:mock           # UI only, with sample data, at http://localhost:1430
```

```bash
cargo test -p dduck-core   # Rust core tests
npx vitest run             # UI tests
npm run lint
npm run tauri build        # release build (scripts/release-local.sh for a signed local DMG)
```

## Project layout

| Path | What's there |
|---|---|
| `app/crates/dduck-core` | Ingest, SQLite, analytics, Claude CLI integration (Rust, no Tauri dependency) |
| `app/src-tauri` | Tauri 2 shell: window, tray, IPC commands, notifications, dialogs |
| `app/src` | React 19 + Vite + TypeScript + Tailwind UI |
| `openspec/` | A spec for every feature, one change at a time, and a log of every decision |
| `docs/features` | Feature list with IDs and test cases mapped to automated tests |

D.Duck was itself built the way it asks you to work. Every feature starts as an [OpenSpec](https://github.com/Fission-AI/OpenSpec) change, and at least two review agents check it at design time and again before it ships.

## Roadmap

- Rework requests straight from the review screen
- Intel, Linux, and Windows builds

## Contributing

Issues and pull requests are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) for setup, the spec-driven workflow, and review rules. Please follow the [Code of Conduct](CODE_OF_CONDUCT.md), and report security problems privately as described in [SECURITY.md](SECURITY.md).

## License

[Apache License 2.0](LICENSE). Third-party components are listed in [app/THIRD_PARTY.md](app/THIRD_PARTY.md).

---

<sub>D.Duck is an independent project. It is not affiliated with or endorsed by Anthropic. Claude and Claude Code are trademarks of Anthropic.</sub>
