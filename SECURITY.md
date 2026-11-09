# Security policy

## Supported versions

Only the latest release gets security fixes. D.Duck is early (0.x), so please update before reporting.

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No |
| `main` | Best effort |

## Reporting a vulnerability

**Please don't open a public issue, discussion, or pull request for a security problem.**

Report it privately through GitHub instead:

1. Go to the repository's [Security tab](https://github.com/DevJoyKo/d-duck/security).
2. Click **Report a vulnerability** ([direct link](https://github.com/DevJoyKo/d-duck/security/advisories/new)).
3. Describe the issue, how to reproduce it, the D.Duck, macOS, and Claude Code versions, and the impact you expect.

Only the maintainers can see the report. Please don't include real credentials, tokens, or transcript content. Use made-up values.

## What to expect

- We confirm receipt within **7 days**.
- We send an initial assessment (accepted or not, and severity) within **14 days**.
- We fix accepted issues in a new release and publish a GitHub security advisory. If you'd like, we credit you in it.
- Please keep the details private until the fix is released, or for 90 days, whichever comes first. If we need more time, we'll tell you why.

## Scope

D.Duck is a local macOS app that reads Claude Code files and runs the `claude` CLI. These are in scope:

- **Credentials:** any path where the app reads, stores, logs, or sends Claude login data, API keys, or other secrets, including through the Keychain, environment variables, or child processes.
- **Transcripts:** raw transcript content written to the app database, logs, reports, or anywhere else on disk, or secrets shown unmasked by default.
- **Command and argument injection:** task text, file names, paths, or settings that can change the commands or arguments D.Duck passes to `claude`, `git`, a shell, or other subprocesses.
- **Local file access:** reading or writing outside the files the user picked, path traversal, symlink tricks, or settings changes made without the confirmation dialog.
- **Bypassing guard rails:** ways around "no merge", "Block remote push", locked team rules, or the restricted permission modes for strict task types.
- **Network:** any outside request that the user didn't trigger.
- **Webview:** script injection in the UI, Tauri capability or CSP gaps, or IPC commands that can be called with unexpected input.
- **Release artifacts:** a tampered DMG or a build script that runs untrusted code.

Out of scope:

- Problems in Claude Code, the Claude API, or claude.ai itself. Report those to [Anthropic](https://www.anthropic.com/responsible-disclosure-policy).
- Attacks that already require full control of the user's account or Mac.
- The macOS "unidentified developer" warning. Builds are ad-hoc signed and not notarized yet.
- Missing hardening with no demonstrated impact.
