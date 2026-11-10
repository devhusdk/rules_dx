#!/bin/sh
set -eu
grep -q '"sbom_rloc"' "$1" || { echo "sbom_rloc missing from $1" >&2; cat "$1" >&2; exit 1; }
grep -q 'app.spdx.json' "$1" || { echo "sbom path missing from $1" >&2; exit 1; }
