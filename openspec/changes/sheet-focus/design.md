## Context

- UsagePage.tsx (SessionSheet, ~:136-165) and NewsPage.tsx (~:110-134) each carry their own end-wrap Tab trap with effect deps `[onClose]`.
- TaskReport.tsx:734-779 already has a stronger private `useModal`: a latest-ref onClose, an effect that runs once, Tab always handled (index < 0 → first/last), a `focusin` guard, inert siblings and a capture-phase Escape.
- AgentSettings.tsx:924, AgentsPage.tsx:54, AuthoringEditor.tsx:59 and AddSheet.tsx:746 are `aria-modal` with at most an Escape handler.
- Their `close` functions are recreated on render (AgentSettings:881, AgentsPage:38) or when `dirty` changes (AuthoringEditor:40). They hold the unsaved-edit `window.confirm`.

## Goals / Non-Goals

**Goals:** one shared hook for modal side sheets, giving the six sheets identical keyboard behavior; TaskReport uses the same hook.

**Non-Goals:**
- Visual changes.
- The non-modal task detail panel (TasksPage, aria-modal=false).
- ReportEvidence lightboxes.
- TourDeck and LoginBits: full-screen flows with step-specific keys. They stay as they are; revisit if a later review flags them.
- Screen-reader virtual-cursor isolation for the in-page sheets (portal plus inert). The spec scopes this change to keyboard focus.
- An unsaved-input guard for AddSheet. It is a pre-existing gap, not made worse here; follow-up candidate.

## Decisions

1. **Move TaskReport's `useModal` to `app/src/lib/useModal.ts` and generalize it** (design-critic HIGH 1):
   `useModal(dialogRef, onClose, { initial?, returnTo?, inert?: { root }, escapePhase?: "bubble" | "capture" })`.
   TaskReport passes `inert` and `escapePhase: "capture"` (current behavior). Sheets use the defaults: no inert, bubble.
2. **Latest-ref onClose; the effect runs once on mount** (HIGH 3 / security MEDIUM 1). Opener recording, initial focus and listener registration happen once. Typing that flips `dirty` never re-runs them.
3. **Tab:** `preventDefault` unless Ctrl, Alt or Cmd is held (those belong to the system). Focusables are recomputed per press. If the active element is outside the sheet or is the body, Tab goes to the first item and Shift+Tab to the last; if it is inside the sheet but not a tab stop (a row focused by script), Tab continues from it in page order. A candidate whose `focus()` does not take is skipped and the next one in the same direction is tried, at most once around the list, so a stop that cannot be focused never traps the keyboard. A `focusin` outside the top sheet moves focus back to its initial target (HIGH 2).
   Candidates: links, enabled form controls, the first `summary` of a `details`, and `[tabindex]`. Excluded by attributes: `disabled`, `[hidden]`, `[inert]` and `aria-hidden` ancestors, `tabindex="-1"`, anything inside a closed `details` except its summary, and every radio of a named group but the checked one (else the first). Where the browser has `checkVisibility()`, elements it reports as not rendered (`display:none`) are excluded too; jsdom lacks it, so tests stand in with a stub (MEDIUM 7, test review HIGH 1).
4. **Stack:** a module-level stack of mounted modals. Only the top one reacts to keys or focusin, so two sheets mounted together (AgentsPage.tsx:409-410) don't both close.
5. **Escape** (HIGH 4 / security MEDIUM 2, LOW 3): sheets listen on `window` in the bubble phase. The hook ignores the key when `e.isComposing || e.keyCode === 229` or `e.defaultPrevented`. Inner panels that stop propagation (LoadoutSection:39-46, ModelPicker:111, SessionBits:42) keep working. Escape calls the latest onClose, the sheet's own close path, so the confirmation stays in place.
6. **Initial focus** (MEDIUM 5 / security LOW 4): skipped if focus is already inside the sheet (autoFocus wins). Otherwise `initial`, else the first tab stop that takes focus; every sheet in this change passes its close button as `initial`. `initial` must never be a destructive or submit button.

   | Sheet | Initial focus |
   |---|---|
   | AgentSettings | existing autoFocus (:400), else close |
   | AgentsPage definition editor | close |
   | AuthoringEditor | close |
   | AddSheet | link input autoFocus (:261), else close |
   | Usage SessionSheet | close (as now) |
   | News sheet | close (as now) |

7. **Focus return** (MEDIUM 6): on unmount, focus `returnTo?.()` if given, else the recorded opener if it is still connected, else do nothing.
   - AddSheet passes the AddMenu trigger, because the menu item that opened it unmounts.
   - NewsPage keeps its parent-side rAF row refocus and passes `returnTo` returning null.
   - AgentSettings opened by a request from another screen has no opener and leaves focus alone.

## Risks / Trade-offs

- Changing TaskReport's internals could regress the report dialog. Its existing tests must pass unchanged.
- There is no user-event library. Tests set focus with `focus()` and dispatch keydown on `document.activeElement`. That verifies the hook's own wrapping and guards, not native Tab movement; the mock-UI keyboard walk covers native movement.

## Review fixes

- Design review 2026-11-13 (design-critic HIGH 4, MEDIUM 5, LOW 1; security MEDIUM 2, LOW 3): reflected in decisions 1–7, the Non-Goals and the tasks.
- The async fallback in the old Risks section was removed. All sheets render their close button synchronously (critic LOW 10).
- Test review 2026-11-13 (design-critic HIGH 1, MEDIUM 3, LOW 5): unfocusable stops (display:none, closed `details`) no longer trap Tab, `summary` is a stop, one stop per named radio group, modified Tab is left alone (decision 3); the fallback text in decision 6 matches the code; the walk checks that every key moves focus and visits every stop, and covers the Agent settings Rules and Types tabs.
