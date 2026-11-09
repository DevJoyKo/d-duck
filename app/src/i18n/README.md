# i18n — screen group migration guide

App text lives in per-namespace catalogs (app-language, D-171). `ko/<ns>.ts` is the reference catalog. `en/<ns>.ts` must satisfy `Shape<typeof ko>`, so a missing or extra key fails `tsc`. `index.ts` already registers every namespace. A screen group edits only its own two files plus its components.

| Namespace | Screens |
|---|---|
| common | Shared words (theme, generic actions, the "system language" choice) |
| nav | Sidebar |
| settings | Settings page |
| tasks · report · agents · extensions · usage · artifacts · routines · auth · tour | tasks.md 3.1–3.7 |

## Using it

```tsx
const t = useT();                                   // re-renders on a language change; no remount, drafts stay
t("settings.title");                                // plain string
t("nav.cli.found", { version: s.cli.version });     // every {placeholder} is a required, typed param
t("settings.history.skipped", { count: n });        // plural: { one, other }, picked by Intl.PluralRules
tx("tasks.blocked.fix", { link: <a …>{t("tasks.blocked.fixLink")}</a> });  // JSX in a slot
```

- `t()` returns a string only. Substitution runs once, and values are never re-scanned or treated as HTML. Never use `dangerouslySetInnerHTML`.
- Outside React (lib code), call `t()` at render or call time, never at module load. A `const LABEL = t(...)` freezes the language.
- Dates and numbers: use `dateTime`, `timeOfDay`, `relativeTime` and `num` from `lib/format.ts` (current locale by default). 24-hour schedule and picker spots stay `clock24` / `HH:MM` in both languages.
- A number param is grouped for the locale (`12,345`). Pass a string when it must stay as written (ids, years).
- Tests: `src/test/setup.ts` pins Korean, so existing tests keep their Korean queries. To test English, call `applyLanguage("en")` and reset it in `afterEach`.
- `i18n.test.ts` checks every namespace for the same keys, the same placeholder names and plural kind in both languages. Run `npx vitest run src/i18n`.

## Key names

- `<ns>.<area>.<item>` in camelCase, by meaning rather than wording: `settings.capacity.unlimited`, not `settings.noLimitText`.
- A row or card usually has `title`, `description` and `more` (folded detail), plus an action name such as `open`, `pick` or `clear`.
- `other` and the other plural category names are reserved for plural messages. Don't use them as group names.
- Plural messages always use the `{count}` parameter. Korean needs only `other`. English gives `one` and `other`.

## Wording

- Korean (reference): 해요체, one line per item, "리뷰" (not 검토), "전역". `copyRules.test.ts` scans the catalogs.
- English: plain sentences, sentence case ("Review capacity", not "Review Capacity"), one line per item. Use the same word for the same idea: review, global, task, agent, duck. No "Please kindly", no exclamation marks. Use contractions where natural ("Couldn't read history").
- Language names are never translated. Use `LANGUAGE_NAMES` (한국어 / English).

## Not translated

User and Claude content stays as written: task names, goals, briefs, agent, skill and plugin names, paths, session titles and report text. Mock data plays the role of user data: Korean, or English example data when the mock page opens with `?lang=en` (for captures; `src/mock/lang.ts`, `english.ts`). A new Korean fixture needs its English line in `english.ts`, or `src/mock/english.test.ts` fails. Only the labels the app adds around that content go through `t()`.
