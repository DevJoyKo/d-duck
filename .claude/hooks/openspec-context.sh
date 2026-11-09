#!/usr/bin/env bash
# UserPromptSubmit hook: reminds Claude of the OpenSpec workflow and current change status.
set -u
source "$(dirname "$0")/openspec-lib.sh"

changes="$(active_changes)"

echo "[OpenSpec 워크플로 강제] 이 프로젝트는 스펙 주도 개발을 따른다."
echo "기능 추가/변경/버그 수정은 /opsx:propose → (사용자 승인) → /opsx:apply → /opsx:archive 순서로 진행한다."
echo "openspec/, .claude/ 밖의 파일은 활성 change의 미완료 태스크 없이는 수정이 차단된다."
if [ -z "$changes" ]; then
  echo "현재 활성 change: 없음"
else
  echo "현재 활성 change (미완료/전체 태스크):"
  awk -F'\t' '{ printf "  - %s (%s/%s)\n", $1, $2, $3 }' <<<"$changes"
fi
