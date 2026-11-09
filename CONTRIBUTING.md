# Contributing to D.Duck

Thanks for your interest in D.Duck. Bug reports, fixes, translations, and ideas are all welcome.

By taking part, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). To report a security problem, see [SECURITY.md](SECURITY.md) and don't open a public issue.

## Before you start

- **Small fixes** (typos, obvious bugs, test gaps): open a pull request directly.
- **Larger changes** (new behavior, new screens, new dependencies, anything touching safety rails): open an issue first so we can agree on the approach before you spend time on it.
- Looking for something to pick up? Try issues labeled [`good first issue`](https://github.com/DevJoyKo/d-duck/labels/good%20first%20issue) or [`help wanted`](https://github.com/DevJoyKo/d-duck/labels/help%20wanted). Leave a comment so others know you're on it.
- Questions and early ideas go to [Discussions](https://github.com/DevJoyKo/d-duck/discussions).

## Development setup

Requirements: macOS, Rust 1.85+, Node 22+, Xcode Command Line Tools, and `python3` (the Rust tests generate their fixtures with it). [Claude Code](https://docs.claude.com/en/docs/claude-code) is only needed to try features that call the CLI.

```bash
git clone https://github.com/<you>/d-duck.git
cd d-duck/app
npm install
npm run tauri dev          # the full app in dev mode
npm run dev:mock           # UI only, with sample data, at http://localhost:1430
```

`dev:mock` is the fastest way to work on screens. It needs no Claude Code install and never touches your real `~/.claude` files.

## Tests and lint

Run these from `app/` before you push. CI runs the same commands on macOS.

```bash
cargo test -p dduck-core                    # Rust core tests (fixtures are generated on first run)
cargo clippy --workspace --lib --bins --tests   # Rust lint (new code should add no warnings)
npx tsc --noEmit -p .                       # type check
npx vitest run                              # UI tests
npm run lint                                # ESLint
```

## Spec-driven changes

D.Duck uses [OpenSpec](https://github.com/Fission-AI/OpenSpec). The current behavior of each capability is described in [`openspec/specs/`](openspec/specs/).

New or changed behavior starts as a change folder, `openspec/changes/<change-name>/`, with:

- `proposal.md`: why, what changes, and what is out of scope
- `specs/<capability>/spec.md`: the requirement deltas, each with at least one `#### Scenario:` (WHEN / THEN)
- `design.md`: the approach and trade-offs, when it isn't obvious
- `tasks.md`: small, checkable tasks

Bug fixes and refactors that don't change behavior don't need a change folder. When a change ships, a maintainer merges its deltas into `openspec/specs/` and archives it. If your change reverses an existing behavior in the specs, say so in the proposal.

## Branches and commits

- Branch from `main` in your fork, with a short name such as `fix/board-badge-count` or `feat/routine-export`.
- Keep pull requests small and focused: one change, one PR.
- [Conventional Commits](https://www.conventionalcommits.org/) are recommended: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.
- **Sign off every commit** under the [Developer Certificate of Origin](https://developercertificate.org/). Use `git commit -s`, which adds a `Signed-off-by:` line. It certifies that you have the right to submit the work under the project license. To fix earlier commits, run `git rebase --signoff main`.

## Pull request flow

1. Fork the repository and create a branch.
2. Open a **draft** pull request early and fill in the template.
3. Make CI pass.
4. Mark it ready for review. A maintainer reviews the design (spec and approach) and the result (tests and evidence).
5. A maintainer squash-merges it. The PR title becomes the commit message, so keep it in Conventional Commits form.

## What reviewers look for

- **Tests come with the change.** A bug fix includes a test that fails without it. New behavior has tests for its spec scenarios.
- **Evidence, not claims.** Paste the commands you ran and their results. For UI changes, attach screenshots in **light and dark** themes and at a **900px-wide** window.
- **Copy in both languages.** New UI text is added to both catalogs (see below).
- **The principles below hold.**

Maintainers use review agents in [`.claude/agents/`](.claude/agents/) (design critic, security and privacy reviewer, QA verifier) as a second pair of eyes at the design and test stages. You can run them on your own branch with Claude Code if you like. It's optional, and it doesn't replace human review.

## Principles that every change must keep

These are non-negotiable, and PRs that break them won't be merged:

- **No access to Claude credentials.** The app never reads, stores, or sends Claude login data or API keys. It only calls the official `claude` CLI the user has already logged in to.
- **No raw transcripts stored.** Session records are aggregated into numbers and names. Raw content may be read for display only when the user opens it, and is never written to the database, logs, or reports.
- **Local only.** No telemetry, no background network calls. Any outside request must come from an explicit user action.
- **Confirm risky actions.** Writing settings files, risky permission modes, installs, deletes, and anything hard to undo show what will change and ask first.
- **No personal data in the repo.** No real names, emails, or home paths in code, tests, fixtures, or screenshots. Use mock data.

## UI copy

App text lives in per-namespace catalogs, `app/src/i18n/ko/` (reference) and `app/src/i18n/en/`. A key missing from either one fails the type check. See [`app/src/i18n/README.md`](app/src/i18n/README.md) for usage and key names.

- **English:** short, plain sentences in sentence case ("Review capacity", not "Review Capacity"). No exclamation marks. Use contractions where natural.
- **Korean:** 해요체, one line per item.
- Use the same word for the same idea: review, task, agent, duck.
- User and Claude content (task names, paths, report text) is never translated.

## Translations

Corrections to the English or Korean copy are welcome as small PRs. To propose a new language, open an issue first: it needs a full catalog under `app/src/i18n/<lang>/` and someone willing to keep it up to date.

## License

D.Duck is licensed under the [Apache License 2.0](LICENSE). By contributing, you agree that your contributions are licensed under the same license (see section 5 of the license). Third-party components are listed in [`app/THIRD_PARTY.md`](app/THIRD_PARTY.md).
