#!/bin/sh
set -eu
fail() {
    echo "dx-android-sign: $1" >&2
    exit "${2:-2}"
}
need() {
    eval "value=\${$1:-}"
    [ -n "$value" ] || fail "$1 is not set: $2"
}
need DX_SIGN_APK "the unsigned APK comes from the launcher"
need DX_SIGN_BUILD_TOOLS "the pinned build-tools version comes from the launcher"
need DX_SIGN_KEYSTORE "name the release keystore file"
need DX_SIGN_STORE_PASS_FILE "name a file holding the keystore password"
need DX_SIGN_OUT "name the signed APK path to write"
absolute() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "${BUILD_WORKING_DIRECTORY:-$PWD}/$1" ;;
    esac
}
DX_SIGN_KEYSTORE="$(absolute "$DX_SIGN_KEYSTORE")"
DX_SIGN_STORE_PASS_FILE="$(absolute "$DX_SIGN_STORE_PASS_FILE")"
[ -z "${DX_SIGN_KEY_PASS_FILE:-}" ] || DX_SIGN_KEY_PASS_FILE="$(absolute "$DX_SIGN_KEY_PASS_FILE")"
DX_SIGN_OUT="$(absolute "$DX_SIGN_OUT")"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
[ -n "$sdk" ] || fail "ANDROID_HOME or ANDROID_SDK_ROOT names the installed SDK"
tools="$sdk/build-tools/$DX_SIGN_BUILD_TOOLS"
for tool in apksigner zipalign; do
    [ -x "$tools/$tool" ] || fail "no $tool in $tools: install build-tools $DX_SIGN_BUILD_TOOLS" 127
done
[ -f "$DX_SIGN_APK" ] || fail "no APK at $DX_SIGN_APK"
[ -f "$DX_SIGN_KEYSTORE" ] || fail "no keystore at $DX_SIGN_KEYSTORE"
[ -f "$DX_SIGN_STORE_PASS_FILE" ] || fail "no password file at $DX_SIGN_STORE_PASS_FILE"
if [ -n "${DX_SIGN_KEY_PASS_FILE:-}" ] && [ ! -f "$DX_SIGN_KEY_PASS_FILE" ]; then
    fail "no key password file at $DX_SIGN_KEY_PASS_FILE"
fi
case "$DX_SIGN_OUT" in
    "$DX_SIGN_APK") fail "DX_SIGN_OUT must not overwrite the input APK" ;;
esac
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
"$tools/zipalign" -f -p 4 "$DX_SIGN_APK" "$work/aligned.apk" || fail "zipalign rejected $DX_SIGN_APK" 1
set -- sign --ks "$DX_SIGN_KEYSTORE" --ks-pass "file:$DX_SIGN_STORE_PASS_FILE"
[ -z "${DX_SIGN_KEY_ALIAS:-}" ] || set -- "$@" --ks-key-alias "$DX_SIGN_KEY_ALIAS"
[ -z "${DX_SIGN_KEY_PASS_FILE:-}" ] || set -- "$@" --key-pass "file:$DX_SIGN_KEY_PASS_FILE"
[ -z "${DX_SIGN_MIN_SDK:-}" ] || set -- "$@" --min-sdk-version "$DX_SIGN_MIN_SDK"
for scheme in v1 v2 v3 v4; do
    case ",${DX_SIGN_SCHEMES:-v1,v2,v3}," in
        *",$scheme,"*) set -- "$@" "--$scheme-signing-enabled" true ;;
        *) set -- "$@" "--$scheme-signing-enabled" false ;;
    esac
done
set -- "$@" --out "$work/signed.apk" "$work/aligned.apk"
"$tools/apksigner" "$@" || fail "apksigner could not sign $DX_SIGN_APK" 1
set -- verify
[ -z "${DX_SIGN_MIN_SDK:-}" ] || set -- "$@" --min-sdk-version "$DX_SIGN_MIN_SDK"
"$tools/apksigner" "$@" "$work/signed.apk" || fail "signed APK failed verification" 1
mkdir -p "$(dirname "$DX_SIGN_OUT")"
mv "$work/signed.apk" "$DX_SIGN_OUT"
echo "dx-android-sign: wrote $DX_SIGN_OUT"
