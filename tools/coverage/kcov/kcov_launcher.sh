#!/usr/bin/env bash
# Runs one test under kcov and writes its LCOV report into COVERAGE_DIR.
set -uo pipefail

f=bazel_tools/tools/bash/runfiles/runfiles.bash
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  { echo >&2 "ERROR: cannot find $f"; exit 1; }
runfiles_export_envvars
lib=$(rlocation _main/tools/coverage/lib/runfile.sh 2>/dev/null || rlocation rules_dx+/tools/coverage/lib/runfile.sh) || { echo >&2 "ERROR: cannot find runfile.sh"; exit 1; }
source "$lib"

upstream=$(runfile "$DX_KCOV_UPSTREAM")
dx_coverage_passthrough "$upstream" "$@"

sources=$(runfile "$DX_KCOV_SOURCES")
pattern=$(sed 's/[][\.*^$+?(){}|]/\\&/g' "$sources" | paste -sd'|')
out=${TEST_TMPDIR:-$(mktemp -d)}/dx_kcov
"$(runfile "$DX_KCOV")" --replace-src-path="^.*?/(?=($pattern)\$):$PWD/" "$out" "$upstream" "$@"
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
