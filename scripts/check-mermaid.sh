#!/usr/bin/env bash
# Validate every ```mermaid code block under docs/ with mermaid-cli (mmdc).
#
# Why this exists: `mkdocs build --strict` does NOT validate Mermaid diagrams —
# Material renders them client-side in the browser, so a malformed diagram builds
# cleanly yet shows "Syntax error in text" on the live site. This script parses
# each diagram with the real Mermaid engine (via mmdc) and fails on any error, so
# CI catches broken diagrams before they ship.
#
# Requirements: bash, python3, and `mmdc` (npm i -g @mermaid-js/mermaid-cli).
# Usage: scripts/check-mermaid.sh
set -euo pipefail

if ! command -v mmdc >/dev/null 2>&1; then
  echo "error: mmdc not found — install with: npm install -g @mermaid-js/mermaid-cli" >&2
  exit 127
fi

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

# Chromium under CI needs the sandbox disabled.
printf '{"args":["--no-sandbox","--disable-setuid-sandbox"]}\n' > "${workdir}/puppeteer.json"

# Extract each fenced mermaid block to its own .mmd, recording its source location.
python3 - "${workdir}" <<'PY'
import pathlib
import sys

workdir = pathlib.Path(sys.argv[1])
manifest = []
for md in sorted(pathlib.Path("docs").rglob("*.md")):
    lines = md.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        if lines[index].strip().startswith("```mermaid"):
            start = index + 1
            body = []
            index += 1
            while index < len(lines) and not lines[index].strip().startswith("```"):
                body.append(lines[index])
                index += 1
            target = workdir / f"block-{len(manifest)}.mmd"
            target.write_text("\n".join(body) + "\n", encoding="utf-8")
            manifest.append(f"{target}\t{md}:{start}")
        index += 1

(workdir / "manifest.txt").write_text("\n".join(manifest) + ("\n" if manifest else ""), encoding="utf-8")
print(f"found {len(manifest)} mermaid block(s)")
PY

fail=0
count=0
while IFS=$'\t' read -r mmd src; do
  [ -z "${mmd}" ] && continue
  count=$((count + 1))
  if ! mmdc --quiet -p "${workdir}/puppeteer.json" -i "${mmd}" -o "${mmd}.svg" >/dev/null 2>"${mmd}.err"; then
    echo "::error::Mermaid syntax error in ${src}"
    sed 's/^/    /' "${mmd}.err" || true
    fail=1
  fi
done < "${workdir}/manifest.txt"

if [ "${fail}" -eq 0 ]; then
  echo "All ${count} Mermaid diagram(s) parsed successfully."
else
  echo "Mermaid validation failed." >&2
fi
exit "${fail}"
