# sheet-focus 3.2: keyboard walk record

Date: 2026-11-13, re-run after the test-review fixes. Mock UI (`npm run dev:mock`, served from this branch on port
1431 because the walk had to run this branch's code), Chromium via Playwright, Korean, reduced motion. Script:
`walk.mjs`, raw results: `walk.json`.

For each sheet: focus the opening control and press Enter, read the focused element, then go around once with Tab
and once with Shift+Tab (stops + 2 native presses each). After every press focus must be inside the top `aria-modal`
dialog and must have moved; after each pass every tab stop must have been visited. Tab stops are counted in the page
with the same rules as `useModal`'s `focusables` (visible per `checkVisibility`, closed `details` content left out,
one radio per named group). Then the screenshot, Escape, and where focus went. Native confirm dialogs would be
dismissed (none appeared: no edits).

| Sheet | Initial focus | Stops (1440 / 900) | Outside / stuck / missed | Focus after Escape | Light 1440 / dark 900 |
|---|---|---|---|---|---|
| Agent settings, agents tab | × 닫기 | 32 / 32 | 0 / 0 / 0 | 에이전트 설정 button (opener) | PASS / PASS |
| Agent settings, Rules tab | × 닫기 | 38 / 38 | 0 / 0 / 0 | 에이전트 설정 button | PASS / PASS |
| Agent settings, Types tab | × 닫기 | 41 / 32 | 0 / 0 / 0 | 에이전트 설정 button | PASS / PASS |
| Agent definition editor | × 닫기 | 2 / 2 | 0 / 0 / 0 | 정의 고치기 card button (opener) | PASS / PASS |
| Extension editor | × 닫기 | 3 / 3 | 0 / 0 / 0 | 수정 row button (opener) | PASS / PASS |
| Add sheet (link) | link input (autoFocus) | 3 / 3 | 0 / 0 / 0 | 추가 button (`returnTo`; the menu item is gone) | PASS / PASS |
| Usage session sheet | × 닫기 | 18 / 18 | 0 / 0 / 0 | session row button (opener) | PASS / PASS |
| News sheet | × 닫기 | 2 / 2 | 0 / 0 / 0 | news row (parent rAF refocus) | PASS / PASS |

16 / 16 PASS. The Types tab has fewer stops at 900 px because its type list is `hidden lg:block` and a select takes
its place: before the fix the hidden list's buttons were still candidates, focus() did nothing on them and Tab stuck
(test review HIGH 1).

Found during the first walk and fixed: in the dev build (React StrictMode) the add sheet opened with × focused
instead of the link field. StrictMode re-runs effects on mount; the first cleanup returned focus to the Add button,
so the re-run no longer saw focus inside and moved it to ×. The hook now skips focus return while its dialog is still
in the page (a re-run, not a close). Covered by a StrictMode test in `app/src/lib/useModal.test.tsx`.

Screenshots: `<sheet>-light-1440.png`, `<sheet>-dark-900.png`, taken after both passes.
