#!/bin/sh
set -eu
sbom="$1"
packages=$(grep -c '"downloadLocation"' "$sbom" || true)
[ "$packages" = 7 ] || { echo "want 7 packages, got $packages" >&2; cat "$sbom" >&2; exit 1; }
for want in pkg:npm/left-pad@1.3.0 pkg:npm/is-odd@3.0.1 pkg:golang/github.com/google/uuid@v1.6.0 pkg:maven/com.google.guava:guava@33.0.0-jre pkg:pypi/requests@2.32.3; do
  grep -q "$want" "$sbom" || { echo "$want missing from $sbom" >&2; exit 1; }
done
