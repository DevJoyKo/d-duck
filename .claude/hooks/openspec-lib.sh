#!/usr/bin/env bash
# Shared helpers for OpenSpec enforcement hooks.

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
CHANGES_DIR="$PROJECT_DIR/openspec/changes"

# Prints "name<TAB>open_tasks<TAB>total_tasks" for each active (non-archived) change
# that has both proposal.md and tasks.md.
active_changes() {
  [ -d "$CHANGES_DIR" ] || return 0
  for dir in "$CHANGES_DIR"/*/; do
    [ -d "$dir" ] || continue
    name="$(basename "$dir")"
    [ "$name" = "archive" ] && continue
    [ -f "$dir/proposal.md" ] && [ -f "$dir/tasks.md" ] || continue
    open=$(grep -cE '^\s*- \[ \]' "$dir/tasks.md")
    total=$(grep -cE '^\s*- \[[ xX]\]' "$dir/tasks.md")
    printf '%s\t%s\t%s\n' "$name" "$open" "$total"
  done
}
