#!/bin/sh
set -eu
sbom="$1"
packages=$(grep -c '"downloadLocation"' "$sbom" || true)
[ "$packages" = 5 ] || { echo "want 5 packages, got $packages" >&2; cat "$sbom" >&2; exit 1; }
for want in pkg:npm/left-pad@1.3.0 pkg:npm/is-odd@3.0.1 pkg:golang/github.com/google/uuid@v1.6.0; do
  grep -q "$want" "$sbom" || { echo "$want missing from $sbom" >&2; exit 1; }
done
