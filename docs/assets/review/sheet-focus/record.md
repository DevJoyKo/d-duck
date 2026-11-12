# sheet-focus 3.2: keyboard walk record

Date: 2026-11-13. Mock UI (`npm run dev:mock`, served from this branch on port 1431 because the walk had to run this
branch's code), Chromium via Playwright, Korean, reduced motion. Script: `walk.mjs`, raw results: `walk.json`.

For each sheet: focus the opening control and press Enter, read the focused element, press Tab ×25 then Shift+Tab ×25
(native key presses) checking after every press that focus is inside the top `aria-modal` dialog, take the
screenshot, press Escape, and check where focus went. Native confirm dialogs would be dismissed (none appeared: no edits).

| Sheet | Initial focus (expected) | Presses outside the sheet | Closed by Escape | Focus after close | Light 1440 / dark 900 |
|---|---|---|---|---|---|
| Agent settings | × 닫기 (×) | 0 / 50 | yes | 에이전트 설정 button (opener) | PASS / PASS |
| Agent definition editor | × 닫기 (×) | 0 / 50 | yes | 정의 고치기 card button (opener) | PASS / PASS |
| Extension editor | × 닫기 (×) | 0 / 50 | yes | 수정 row button (opener) | PASS / PASS |
| Add sheet (link) | link input (autoFocus) | 0 / 50 | yes | 추가 button (`returnTo`; the menu item is gone) | PASS / PASS |
| Usage session sheet | × 닫기 (×) | 0 / 50 | yes | session row button (opener) | PASS / PASS |
| News sheet | × 닫기 (×) | 0 / 50 | yes | news row (parent rAF refocus) | PASS / PASS |

12 / 12 PASS.

Found during the walk and fixed before this record: in the dev build (React StrictMode) the add sheet opened with ×
focused instead of the link field. StrictMode re-runs effects on mount; the first cleanup returned focus to the Add
button, so the re-run no longer saw focus inside and moved it to ×. The hook now skips focus return while its dialog
is still in the page (a re-run, not a close). Covered by a StrictMode test in `app/src/lib/useModal.test.tsx`.

Screenshots: `<sheet>-light-1440.png`, `<sheet>-dark-900.png`, taken after the 50 presses (focus back on the initial
control).
