#!/usr/bin/env bash
# Check (or, with --fix, tidy) the two stylesheets:
#   commons/static/style.css   the node and commons pages
#   src-tauri/ui/style.css     the desktop app's own screens
# Prettier sets the layout (one declaration per line); stylelint checks the
# rules in tools/css-lint/.stylelintrc.json. Needs Node. The tools' versions
# are pinned in tools/css-lint/package-lock.json, so everyone, and CI, gets
# the same result.
set -euo pipefail
cd "$(dirname "$0")/.."
TOOLS=tools/css-lint
FILES=(commons/static/style.css src-tauri/ui/style.css)
[[ -d "$TOOLS/node_modules" ]] || npm ci --prefix "$TOOLS" --silent --no-audit --no-fund
BIN="$TOOLS/node_modules/.bin"
LINT=("$BIN/stylelint" --config "$TOOLS/.stylelintrc.json")
if [[ "${1:-}" == "--fix" ]]; then
  "$BIN/prettier" --write "${FILES[@]}"
  "${LINT[@]}" --fix "${FILES[@]}"
else
  "$BIN/prettier" --check "${FILES[@]}"
  "${LINT[@]}" "${FILES[@]}"
fi
