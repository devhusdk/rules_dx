#!/bin/sh
set -eu
usage() {
    echo "dx-android-check-aab: $1" >&2
    exit 2
}
fail() {
    echo "dx-android-check-aab: $1" >&2
    exit 1
}
[ "$#" -ge 2 ] || usage "usage: check_aab.sh <bundletool> <aab> [--abi <abi>]... [--resource <type/name>]..."
bundletool="$1"
aab="$2"
shift 2
abis=""
resources=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --abi)
            [ "$#" -ge 2 ] && [ -n "$2" ] || usage "--abi needs an ABI name"
            abis="$abis $2"
            shift 2
            ;;
        --resource)
            [ "$#" -ge 2 ] && [ -n "$2" ] || usage "--resource needs a type/name"
            resources="$resources $2"
            shift 2
            ;;
        *) usage "unknown argument '$1'" ;;
    esac
done
[ -f "$bundletool" ] || usage "no bundletool at $bundletool"
[ -f "$aab" ] || usage "no app bundle at $aab"
tmp="${TEST_TMPDIR:-${TMPDIR:-/tmp}}"
run() {
    "$bundletool" --jvm_flag="-Djava.io.tmpdir=$tmp" "$@" 2>&1
}
out="$(run validate --bundle="$aab")" || fail "bundletool validate rejects $aab: $(echo "$out" | grep -v '^[[:space:]]*at ' | head -n 1)"
run dump manifest --bundle="$aab" --module=base >/dev/null || fail "$aab has no readable base module manifest"
listing="$(unzip -Z1 "$aab" 2>/dev/null)" || fail "$aab is not a readable zip bundle"
for abi in $abis; do
    echo "$listing" | grep -Eq "^base/lib/$abi/[^/]+\.so$" || fail "$aab packages no base/lib/$abi/*.so"
done
for resource in $resources; do
    found="$(run dump resources --bundle="$aab" --resource="$resource")" || fail "$aab does not resolve resource $resource"
    # run merges stderr, so JVM warnings are non-empty; require the resource name itself.
    echo "$found" | grep -Fq "${resource#*/}" || fail "$aab does not resolve resource $resource"
done
echo "dx-android-check-aab: $aab is a valid app bundle"
