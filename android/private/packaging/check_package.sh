#!/bin/sh
set -eu
usage() {
    echo "dx-android-check: $1" >&2
    exit 2
}
fail() {
    echo "dx-android-check: $1" >&2
    exit 1
}
[ "$#" -ge 2 ] || usage "usage: check_package.sh <bundletool> <package> [--abi <abi>]... [--resource <type/name>]..."
bundletool="$1"
package="$2"
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
[ -f "$package" ] || usage "no package at $package"

check_apk() {
    [ -z "$abis$resources" ] || usage "--abi and --resource apply to App Bundles only"
    listing="$(unzip -Z1 "$package" 2>/dev/null)" || fail "$package is not a readable zip package"
    unzip -tq "$package" >/dev/null 2>&1 || fail "$package has corrupt entries"
    echo "$listing" | grep -qx "AndroidManifest.xml" || fail "$package has no AndroidManifest.xml"
    if echo "$listing" | grep -Eq '^META-INF/[^/]+\.(RSA|DSA|EC|SF)$'; then
        fail "$package already carries a signature: release signing needs the unsigned package"
    fi
    if grep -aq "APK Sig Block 42" "$package"; then
        fail "$package already carries an APK signing block: release signing needs the unsigned package"
    fi
    echo "dx-android-check: $package is an unsigned package ready to sign"
}

check_aab() {
    tmp="${TEST_TMPDIR:-${TMPDIR:-/tmp}}"
    run() {
        "$bundletool" --jvm_flag="-Djava.io.tmpdir=$tmp" "$@" 2>&1
    }
    out="$(run validate --bundle="$package")" || fail "bundletool validate rejects $package: $(echo "$out" | grep -v '^[[:space:]]*at ' | head -n 1)"
    run dump manifest --bundle="$package" --module=base >/dev/null || fail "$package has no readable base module manifest"
    listing="$(unzip -Z1 "$package" 2>/dev/null)" || fail "$package is not a readable zip bundle"
    for abi in $abis; do
        echo "$listing" | grep -Eq "^base/lib/$abi/[^/]+\.so$" || fail "$package packages no base/lib/$abi/*.so"
    done
    for resource in $resources; do
        found="$(run dump resources --bundle="$package" --resource="$resource")" || fail "$package does not resolve resource $resource"
        echo "$found" | grep -Fq "${resource#*/}" || fail "$package does not resolve resource $resource"
    done
    echo "dx-android-check: $package is a valid app bundle"
}

case "$package" in
    *.apk) check_apk ;;
    *.aab) check_aab ;;
    *) usage "package must end in .apk or .aab" ;;
esac
