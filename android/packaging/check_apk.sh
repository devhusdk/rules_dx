#!/bin/sh
set -eu
apk="$1"
fail() {
    echo "dx-android-check: $1" >&2
    exit 1
}
[ -f "$apk" ] || fail "no APK at $apk"
listing="$(unzip -Z1 "$apk" 2>/dev/null)" || fail "$apk is not a readable zip package"
unzip -tq "$apk" >/dev/null 2>&1 || fail "$apk has corrupt entries"
echo "$listing" | grep -qx "AndroidManifest.xml" || fail "$apk has no AndroidManifest.xml"
if echo "$listing" | grep -Eq '^META-INF/[^/]+\.(RSA|DSA|EC|SF)$'; then
    fail "$apk already carries a signature: release signing needs the unsigned package"
fi
echo "dx-android-check: $apk is an unsigned package ready to sign"
