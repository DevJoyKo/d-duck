## Purpose

Modal side sheets keep keyboard focus inside the sheet while it is open, and return them to where they were when it closes.

## ADDED Requirements

### Requirement: Focus enters a modal sheet
When a modal side sheet opens, keyboard focus SHALL move to a control inside the sheet (its close button, or its first field when the sheet is a form).

#### Scenario: Agent settings opens
- **WHEN** the user opens Agent settings, the agent definition editor, the extension editor or the add-extension sheet
- **THEN** the focused element is inside that sheet

### Requirement: Tab stays inside a modal sheet
While a modal side sheet is open, Tab from its last focusable control SHALL move to its first, and Shift+Tab from its first SHALL move to its last.

#### Scenario: Tab past the last control
- **WHEN** focus is on the last focusable control of an open modal sheet and the user presses Tab
- **THEN** focus moves to the first focusable control of the same sheet, not to the page behind it

#### Scenario: Focus is lost inside an open sheet
- **WHEN** focus is on the page body or on an element outside the sheet (for example after clicking sheet text or after a re-render) and the user presses Tab
- **THEN** focus moves to the first focusable control of the sheet

#### Scenario: Shift+Tab before the first control
- **WHEN** focus is on the first focusable control and the user presses Shift+Tab
- **THEN** focus moves to the last focusable control of the same sheet

### Requirement: Escape and focus return
Escape SHALL close the sheet unless an inner control or panel handles that Escape itself, or text is being composed by an input method. On close focus SHALL return to the control that opened the sheet when it still exists.

#### Scenario: Close with Escape
- **WHEN** the user presses Escape outside a text field in an open modal sheet
- **THEN** the sheet closes and focus returns to the button or row that opened it

#### Scenario: Unsaved edits
- **WHEN** a sheet asks before discarding unsaved edits
- **THEN** Escape follows the same confirmation the close button does

#### Scenario: Escape while composing Korean text
- **WHEN** the user presses Escape while an input method is composing text in a sheet field
- **THEN** the sheet stays open and the field keeps its text

#### Scenario: Inner panel handles Escape
- **WHEN** an inner panel of a sheet (for example a picker) is open and the user presses Escape
- **THEN** only the inner panel closes and the sheet stays open

#### Scenario: Opener is gone
- **WHEN** the control that opened the sheet no longer exists at close time
- **THEN** focus goes to the sheet's designated fallback control (for the add-extension sheet, the Add button), or stays where it is if there is none
