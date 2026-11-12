## Why

The v0.1.2 random test (2026-11-12, decisions.md "리뷰 기록") found that four side sheets are marked `aria-modal` but neither take keyboard focus nor keep Tab inside: Agent settings, the agent definition editor, the extension editor and the add-extension sheet. Tab walks the page behind the sheet, so keyboard and screen-reader users lose their place. The Usage session sheet and the News sheet already do this correctly.

## What Changes

- One shared hook gives every modal side sheet the same keyboard behavior: focus moves into the sheet on open, Tab and Shift+Tab cycle inside it, Escape closes it (unless a text field handles Escape itself), and focus returns to the control that opened it.
- The four sheets above use the hook. The Usage and News sheets move to it too so there is one implementation.

## Capabilities

### New Capabilities
- `sheet-focus`: keyboard focus behavior of modal side sheets.

### Modified Capabilities
(none)

## Impact

- app/src/components: AgentSettings.tsx, AgentsPage.tsx, AuthoringEditor.tsx, AddSheet.tsx, UsagePage.tsx, NewsPage.tsx; new hook in app/src/lib.
- No backend change, no i18n change, no visual change.
