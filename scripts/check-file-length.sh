#!/usr/bin/env bash
# The 400-line rule (docs/CODING_STANDARDS.md), enforced across the whole
# monorepo. Counts every line — blanks and comments included.
#
# Lists tracked AND untracked-but-not-ignored files, so a new file fails
# locally before it is ever committed; ignored output (target/, dist/,
# node_modules/) is skipped by .gitignore.
set -euo pipefail

MAX=400
cd "$(dirname "$0")/.."

failed=0
while IFS= read -r file; do
  [ -f "$file" ] || continue
  lines=$(wc -l <"$file" | tr -d ' ')
  if [ "$lines" -gt "$MAX" ]; then
    echo "FAIL: $file has $lines lines (max $MAX)"
    failed=1
  fi
done < <(git ls-files --cached --others --exclude-standard -- \
  '*.rs' '*.ts' '*.tsx' '*.js' '*.mjs' '*.cjs' '*.css' '*.astro' '*.sh')

if [ "$failed" -ne 0 ]; then
  echo ""
  echo "Files over $MAX lines — split them by responsibility; never raise the limit."
  exit 1
fi
echo "OK: all source files are ≤ $MAX lines"
