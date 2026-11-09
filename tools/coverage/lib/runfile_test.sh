#!/usr/bin/env bash
set -euo pipefail
lib="$1"
dir="${TEST_TMPDIR:-$(mktemp -d)}/runfile_test"
rm -rf "$dir"
mkdir -p "$dir/data" "$dir/up"
printf 'payload\n' > "$dir/data/file.txt"
printf '_main/pkg/file.txt %s\n' "$dir/data/file.txt" > "$dir/MANIFEST"
printf '#!/bin/sh\necho "upstream $*"\n' > "$dir/up/tool"
chmod +x "$dir/up/tool"

unset RUNFILES_DIR TEST_SRCDIR COVERAGE_DIR
export RUNFILES_MANIFEST_FILE="$dir/MANIFEST"
# shellcheck source=/dev/null
. "$lib"

got=$(runfile _main/pkg/file.txt || true)
test "$got" = "$dir/data/file.txt" || { echo "manifest lookup returned '$got'" >&2; exit 1; }

got=$(runfile _main/pkg/file.txt.bak || true)
test -z "$got" || { echo "prefix match returned '$got'" >&2; exit 1; }

got=$(runfile _main/pkg/missing.txt || true)
test -z "$got" || { echo "missing entry returned '$got'" >&2; exit 1; }

got=$(dx_coverage_passthrough "$dir/up/tool" a b)
test "$got" = "upstream a b" || { echo "passthrough returned '$got'" >&2; exit 1; }

got=$(COVERAGE_DIR="$dir/cov" dx_coverage_passthrough "$dir/up/tool" a b; echo done)
test "$got" = "done" || { echo "passthrough ran with COVERAGE_DIR set" >&2; exit 1; }
