#!/usr/bin/env bash
# PreToolUse hook: keep the domain layer pure (AGENTS.md).
# Blocks an Edit/Write to crates/*/src/domain/** when the new content introduces async/I/O.
# Receives the tool-call JSON on stdin. exit 2 = block with reason; exit 0 = allow.
set -euo pipefail

input="$(cat)"
file="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')"

# Only guard files inside a domain layer.
case "$file" in
  */crates/*/src/domain/*|crates/*/src/domain/*) ;;
  *) exit 0 ;;
esac

# The content being written: new_string for Edit, content for Write.
content="$(printf '%s' "$input" | jq -r '.tool_input.new_string // .tool_input.content // empty')"
[ -z "$content" ] && exit 0

if printf '%s' "$content" | grep -Eq 'async fn|tokio::|reqwest::|hyper::|axum::|std::fs|std::net|extism'; then
  echo "Blocked: '$file' is in the pure domain layer (AGENTS.md). Detected async/I/O/framework use." >&2
  echo "Route time/ids through Clock/IdGenerator ports and persistence through application ports." >&2
  exit 2
fi
exit 0
