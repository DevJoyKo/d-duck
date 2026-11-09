## What and why

<!-- What does this change, and why is it needed? -->

## Related issue

<!-- Closes #123 -->

## OpenSpec change

<!-- openspec/changes/<change-name>/, or "None: bug fix or refactor with no behavior change" -->

## How I tested it

<!-- Commands you ran and their results. For UI changes, add screenshots below. -->

## Checklist

- [ ] Tests added or updated for the change, and they pass (`cargo test -p dduck-core`, `npx vitest run`)
- [ ] Lint and type check pass (`cargo clippy`, `npx tsc --noEmit -p .`, `npm run lint`)
- [ ] UI changes: screenshots in light and dark themes, and at a 900px-wide window
- [ ] New UI text is in both catalogs (`app/src/i18n/ko` and `app/src/i18n/en`)
- [ ] Doesn't read, store, or send Claude credentials, and doesn't store raw transcripts
- [ ] No personal data (real names, emails, home paths) in code, tests, or screenshots
- [ ] Every commit is signed off (`git commit -s`, [DCO](https://developercertificate.org/))

See [CONTRIBUTING.md](https://github.com/DevJoyKo/d-duck/blob/main/CONTRIBUTING.md) for details.
