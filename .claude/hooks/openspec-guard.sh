#!/usr/bin/env bash
# PreToolUse hook: blocks code edits unless an OpenSpec change with open tasks exists.
# Bypass for one session: OPENSPEC_BYPASS=1 claude
set -u
source "$(dirname "$0")/openspec-lib.sh"

[ "${OPENSPEC_BYPASS:-0}" = "1" ] && exit 0

input="$(cat)"
file="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")"
[ -z "$file" ] && exit 0
case "$file" in /*) ;; *) file="$PROJECT_DIR/$file" ;; esac

# Files outside the project, and OpenSpec/Claude config itself, are always editable.
case "$file" in
  "$PROJECT_DIR"/*) ;;
  *) exit 0 ;;
esac
rel="${file#"$PROJECT_DIR"/}"
case "$rel" in
  openspec/*|.claude/*|CLAUDE.md|AGENTS.md) exit 0 ;;
esac

changes="$(active_changes)"
if [ -z "$changes" ]; then
  cat >&2 <<MSG
[OpenSpec] 차단됨: '$rel' 를 수정하려면 먼저 OpenSpec change가 필요합니다.
활성 change(proposal.md + tasks.md 포함)가 openspec/changes/ 에 없습니다.
먼저 /opsx:propose "<변경 내용>" 으로 제안서·스펙·태스크를 만들고 사용자 승인을 받은 뒤,
/opsx:apply 로 tasks.md 의 태스크를 따라 구현하세요.
MSG
  exit 2
fi

if ! awk -F'\t' '$2 > 0 { found=1 } END { exit !found }' <<<"$changes"; then
  cat >&2 <<MSG
[OpenSpec] 차단됨: 활성 change의 tasks.md 태스크가 모두 완료되었습니다.
$(cut -f1 <<<"$changes" | sed 's/^/  - /')
추가 작업이면 tasks.md 에 태스크를 추가하거나 새 change를 /opsx:propose 로 만들고,
끝난 change는 /opsx:archive 로 아카이브하세요.
MSG
  exit 2
fi

exit 0
