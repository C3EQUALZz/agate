#!/usr/bin/env bash
# PostToolUse hook: format a just-edited Rust file with rustfmt.
# Receives the tool-call JSON on stdin; formats only *.rs paths. Silent, idempotent, never blocks.
set -euo pipefail

input="$(cat)"
file="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')"

[ -z "$file" ] && exit 0
case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0

# Use the workspace edition/style via rustfmt.toml at the repo root.
rustfmt --edition 2024 "$file" >/dev/null 2>&1 || true
exit 0
