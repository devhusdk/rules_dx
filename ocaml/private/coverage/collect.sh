#!/usr/bin/env bash
# Runs one bisect_ppx instrumented OCaml test and writes its LCOV report into COVERAGE_DIR.
set -uo pipefail

f=bazel_tools/tools/bash/runfiles/runfiles.bash
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  { echo >&2 "ERROR: cannot find $f"; exit 1; }
runfiles_export_envvars
lib=$(rlocation _main/tools/coverage/lib/runfile.sh 2>/dev/null || rlocation rules_dx+/tools/coverage/lib/runfile.sh) || { echo >&2 "ERROR: cannot find runfile.sh"; exit 1; }
source "$lib"

upstream=$(runfile "$DX_BISECT_UPSTREAM")
dx_coverage_passthrough "$upstream" "$@"

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
