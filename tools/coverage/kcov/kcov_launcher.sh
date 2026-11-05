#!/usr/bin/env bash
# Runs one test under kcov and writes its LCOV report into COVERAGE_DIR.
set -uo pipefail

runfile() {
  local root=${RUNFILES_DIR:-${TEST_SRCDIR:-}}
  if [ -n "$root" ] && [ -e "$root/$1" ]; then
    printf '%s\n' "$root/$1"
  elif [ -n "${RUNFILES_MANIFEST_FILE:-}" ]; then
    grep -m1 "^$1 " "$RUNFILES_MANIFEST_FILE" | cut -d' ' -f2-
  fi
}

upstream=$(runfile "$DX_KCOV_UPSTREAM")
if [ -z "${COVERAGE_DIR:-}" ]; then
  exec "$upstream" "$@"
fi

sources=$(runfile "$DX_KCOV_SOURCES")
pattern=$(sed 's/[][\.*^$+?(){}|]/\\&/g' "$sources" | paste -sd'|')
out=${TEST_TMPDIR:-$(mktemp -d)}/dx_kcov
"$(runfile "$DX_KCOV")" --replace-src-path="^.*/(?=($pattern)\$):$PWD/" "$out" "$upstream" "$@"
status=$?

report=$out/$(basename "$upstream")/cobertura.xml
if [ ! -f "$report" ]; then
  echo "dx: kcov wrote no report for $upstream" >&2
  exit 1
fi
if ! "$(runfile "$DX_TO_LCOV")" --from cobertura --sources "$sources" "$report" "$COVERAGE_DIR/kcov.dat"; then
  exit 1
fi
exit "$status"
