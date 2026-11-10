#!/bin/sh
set -eu
sbom="$1"
[ "$(basename "$sbom")" = "app.spdx.json" ] || { echo "unexpected sbom name: $sbom" >&2; exit 1; }
packages=$(grep -c '"downloadLocation"' "$sbom" || true)
[ "$packages" = 2 ] || { echo "want 2 packages, got $packages" >&2; cat "$sbom" >&2; exit 1; }
grep -q '"name": "greeter"' "$sbom" || { echo "greeter missing from $sbom" >&2; exit 1; }
