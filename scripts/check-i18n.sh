#!/usr/bin/env bash
# Enforce bilingual-docs stability for the Agate docs site.
#
# Two failure modes are guarded:
#
#   1. Structural parity — every docs/en/**/*.md must have a matching
#      docs/ru/**/*.md at the mirror path, and vice-versa. A missing
#      counterpart means a page would silently fall back to the other language
#      (or 404 on the primary tree), which we do not allow to land unnoticed.
#
#   2. Translation drift-guard — the Russian translation of the threat-model
#      design record (docs/design/agate-proxy-threat-model.ru.md) embeds the
#      git blob SHA of its English source. We recompute `git hash-object` of
#      the English doc and fail if it differs from the embedded SHA, meaning
#      the English canonical record changed and the translation is now stale.
#
# Requirements: bash, git.
# Usage: scripts/check-i18n.sh
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

en_dir="docs/en"
ru_dir="docs/ru"
en_doc="docs/design/agate-proxy-threat-model.md"
ru_doc="docs/design/agate-proxy-threat-model.ru.md"

fail=0

# --- 1. Structural parity -------------------------------------------------

if [ ! -d "${en_dir}" ] || [ ! -d "${ru_dir}" ]; then
  echo "::error::expected both ${en_dir}/ and ${ru_dir}/ to exist" >&2
  exit 1
fi

# Every EN page needs a RU mirror.
while IFS= read -r en_file; do
  rel="${en_file#"${en_dir}/"}"
  ru_file="${ru_dir}/${rel}"
  if [ ! -f "${ru_file}" ]; then
    echo "::error::missing Russian mirror: ${ru_file} (counterpart of ${en_file})"
    fail=1
  fi
done < <(find "${en_dir}" -type f -name '*.md' | sort)

# Every RU page needs an EN counterpart.
while IFS= read -r ru_file; do
  rel="${ru_file#"${ru_dir}/"}"
  en_file="${en_dir}/${rel}"
  if [ ! -f "${en_file}" ]; then
    echo "::error::missing English counterpart: ${en_file} (counterpart of ${ru_file})"
    fail=1
  fi
done < <(find "${ru_dir}" -type f -name '*.md' | sort)

if [ "${fail}" -eq 0 ]; then
  echo "Structural parity OK: docs/en and docs/ru mirror each other."
fi

# --- 2. Translation drift-guard ------------------------------------------

if [ ! -f "${en_doc}" ]; then
  echo "::error::English threat-model source not found: ${en_doc}" >&2
  exit 1
fi
if [ ! -f "${ru_doc}" ]; then
  echo "::error::Russian threat-model translation not found: ${ru_doc}" >&2
  exit 1
fi

# git hash-object works on the working-tree file (no full history needed).
actual_sha="$(git hash-object "${en_doc}")"

# The translation records the EN source SHA in an HTML comment near the top:
#   <!-- en-source-sha: <sha> -->
embedded_sha="$(grep -oE 'en-source-sha:[[:space:]]*[0-9a-f]{40}' "${ru_doc}" | head -n1 | grep -oE '[0-9a-f]{40}' || true)"

if [ -z "${embedded_sha}" ]; then
  echo "::error::no 'en-source-sha: <40-hex>' marker found in ${ru_doc}" >&2
  echo "  Add a comment near the top, e.g.: <!-- en-source-sha: ${actual_sha} -->" >&2
  fail=1
elif [ "${embedded_sha}" != "${actual_sha}" ]; then
  echo "::error::threat-model translation is stale." >&2
  echo "  English source ${en_doc} changed:" >&2
  echo "    embedded en-source-sha: ${embedded_sha}" >&2
  echo "    current  git hash-object: ${actual_sha}" >&2
  echo "  Remediation: re-translate ${ru_doc} to match the English source," >&2
  echo "  then update the embedded marker to: <!-- en-source-sha: ${actual_sha} -->" >&2
  fail=1
else
  echo "Drift-guard OK: ${ru_doc} is in sync with ${en_doc} (sha ${actual_sha})."
fi

if [ "${fail}" -ne 0 ]; then
  echo "i18n check failed." >&2
  exit 1
fi

echo "i18n check passed."
