#!/bin/bash
set -eu

nm="$1"
binary="$2"
root="$(cd "$(dirname "$binary")" && pwd)"
images=("$binary")
while IFS= read -r library; do
    images+=("$library")
done < <(find -L "${TEST_SRCDIR:-$root}" \( -name '*.dylib' -o -name '*.so' -o -name '*.so.*' \) -not -path '*/external/llvm*' 2>/dev/null | sort -u)

defined="${TEST_TMPDIR:-/tmp}/defined.txt"
undefined="${TEST_TMPDIR:-/tmp}/undefined.txt"
listing="${TEST_TMPDIR:-/tmp}/listing.txt"
: >"$defined"
: >"$undefined"
for image in "${images[@]}"; do
    echo "== $image"
    "$nm" --defined-only "$image" >"$listing" || exit 1
    { grep 'cxxbridge1\$' "$listing" || true; } | tee -a "$defined"
    "$nm" --undefined-only "$image" >"$listing" || exit 1
    { grep 'cxxbridge1\$' "$listing" || true; } | tee -a "$undefined"
done

missing=0
while read -r name; do
    [ -n "$name" ] || continue
    if ! awk '{print $NF}' "$defined" | grep -qxF -- "$name"; then
        echo "undefined everywhere: $name" >&2
        missing=1
    fi
done < <(awk '{print $NF}' "$undefined" | sort -u)
exit "$missing"
