#!/usr/bin/env bash
# Runs one bisect_ppx instrumented OCaml test and writes its LCOV report into COVERAGE_DIR.
set -uo pipefail

runfile() {
  local root=${RUNFILES_DIR:-${TEST_SRCDIR:-}}
  if [ -n "$root" ] && [ -e "$root/$1" ]; then
    printf '%s\n' "$root/$1"
  elif [ -n "${RUNFILES_MANIFEST_FILE:-}" ]; then
    grep -m1 "^$1 " "$RUNFILES_MANIFEST_FILE" | cut -d' ' -f2-
  fi
}

upstream=$(runfile "$DX_BISECT_UPSTREAM")
if [ -z "${COVERAGE_DIR:-}" ]; then
  exec "$upstream" "$@"
fi

work=${TEST_TMPDIR:-$(mktemp -d)}/dx_bisect
mkdir -p "$work/data"
BISECT_FILE="$work/data/bisect" BISECT_SILENT=ERR "$upstream" "$@"
status=$?

if ! compgen -G "$work/data/*.coverage" > /dev/null; then
  exit "$status"
fi
sources=$(runfile "$DX_BISECT_SOURCES")
if ! "$(runfile "$DX_BISECT_REPORT")" cobertura --ignore-missing-files \
  --source-path "$(runfile "$DX_BISECT_WORKSPACE")" \
  --coverage-path "$work/data" "$work/cobertura.xml"; then
  echo "dx: bisect-ppx-report failed" >&2
  exit 1
fi
if ! "$(runfile "$DX_TO_LCOV")" --from cobertura --sources "$sources" "$work/cobertura.xml" "$COVERAGE_DIR/bisect.dat"; then
  exit 1
fi
exit "$status"
