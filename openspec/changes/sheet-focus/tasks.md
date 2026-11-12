## 1. Hook

- [x] 1.1 `lib/useModal.ts` (moved from TaskReport, generalized). vitest: initial focus and autoFocus kept; Tab/Shift+Tab wrap; Tab from body or outside goes to first/last; focusin outside returns; Escape closes; Escape ignored while composing (isComposing/229) and when defaultPrevented; an inner panel stopping propagation keeps the sheet open; onClose identity changes while typing don't move focus; top-of-stack only; returnTo and opener return; disconnected opener leaves focus alone
- [x] 1.2 TaskReport uses the shared hook (inert, capture). Existing TaskReport tests pass unchanged

## 2. Sheets

- [x] 2.1 AgentSettings, AgentsPage definition editor, AuthoringEditor and AddSheet use the hook with the initial-focus table. vitest per sheet: exact initial element; Tab wraps; close returns focus (AddSheet → AddMenu trigger). For the three dirty-guarded sheets: edit → Escape → `window.confirm` called, declining keeps the sheet open
- [ ] 2.2 Usage SessionSheet and the News sheet move to the hook; their existing tests pass

## 3. Verification

- [x] 3.1 Design review (design-critic + security-privacy-reviewer), reflected in the design.md "Review fixes" section
- [ ] 3.2 Full checks (tsc, vitest, lint) plus a keyboard walk of the six sheets in the mock UI (Playwright: Tab ×N, then confirm activeElement is inside the sheet), with the record and screenshots under docs/assets/review/sheet-focus/
- [ ] 3.3 Test review (qa-verifier + design-critic)
