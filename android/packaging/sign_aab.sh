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
need DX_SIGN_AAB "the unsigned App Bundle comes from the launcher"
need DX_SIGN_KEYSTORE "name the upload keystore file"
need DX_SIGN_STORE_PASS_FILE "name a file holding the keystore password"
need DX_SIGN_KEY_ALIAS "name the upload key alias"
need DX_SIGN_OUT "name the signed App Bundle path to write"
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
runfile() {
    case "$1" in
        "") ;;
        /*) echo "$1" ;;
        *) echo "$PWD/$1" ;;
    esac
}
bundletool="$(runfile "${DX_SIGN_BUNDLETOOL:-}")"
javabase="$(runfile "${DX_SIGN_JAVABASE:-}")"
[ -f "$bundletool" ] && [ -x "$bundletool" ] || fail "no bundletool: the launcher supplies //android/packaging:bundletool" 127
[ -n "$javabase" ] && [ -x "$javabase/bin/jarsigner" ] && [ -x "$javabase/bin/keytool" ] || fail "no jarsigner: select a full JDK with --java_runtime_version" 127
jarsigner() {
    "$javabase/bin/jarsigner" -J-Duser.language=en -J-Duser.country=US "$@" </dev/null
}
keytool() {
    "$javabase/bin/keytool" -J-Duser.language=en -J-Duser.country=US "$@" </dev/null
}
[ -f "$DX_SIGN_AAB" ] || fail "no App Bundle at $DX_SIGN_AAB"
[ -f "$DX_SIGN_KEYSTORE" ] || fail "no keystore at $DX_SIGN_KEYSTORE"
[ -f "$DX_SIGN_STORE_PASS_FILE" ] || fail "no password file at $DX_SIGN_STORE_PASS_FILE"
if [ -n "${DX_SIGN_KEY_PASS_FILE:-}" ] && [ ! -f "$DX_SIGN_KEY_PASS_FILE" ]; then
    fail "no key password file at $DX_SIGN_KEY_PASS_FILE"
fi
if [ "$(runfile "$DX_SIGN_AAB")" = "$DX_SIGN_OUT" ] || [ "$DX_SIGN_AAB" -ef "$DX_SIGN_OUT" ]; then
    fail "DX_SIGN_OUT must not overwrite the input App Bundle"
fi
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
if jarsigner -verify "$DX_SIGN_AAB" 2>&1 | grep -q "^jar verified"; then
    fail "$DX_SIGN_AAB is already signed" 1
fi
"$bundletool" --jvm_flag="-Djava.io.tmpdir=$work" validate --bundle="$DX_SIGN_AAB" >"$work/validate" 2>&1 \
    || fail "bundletool validate rejects $DX_SIGN_AAB: $(grep -v '^[[:space:]]*at ' "$work/validate" | head -n 1)" 1
keytool -list -v -keystore "$DX_SIGN_KEYSTORE" -storepass:file "$DX_SIGN_STORE_PASS_FILE" \
    -alias "$DX_SIGN_KEY_ALIAS" >"$work/key" 2>&1 || fail "jarsigner could not sign $DX_SIGN_AAB: keystore or alias rejected" 1
case "$(grep '^Subject Public Key Algorithm:' "$work/key")" in
    *" RSA key"*) sigalg="SHA256withRSA" ;;
    *" EC "* | *" EC key"*) sigalg="SHA256withECDSA" ;;
    *) fail "jarsigner could not sign $DX_SIGN_AAB: the upload key must be RSA or EC" 1 ;;
esac
cp "$DX_SIGN_AAB" "$work/in.aab"
set -- -keystore "$DX_SIGN_KEYSTORE" -storepass:file "$DX_SIGN_STORE_PASS_FILE"
[ -z "${DX_SIGN_KEY_PASS_FILE:-}" ] || set -- "$@" -keypass:file "$DX_SIGN_KEY_PASS_FILE"
set -- "$@" -sigalg "$sigalg" -digestalg SHA-256 -signedjar "$work/signed.aab" "$work/in.aab" "$DX_SIGN_KEY_ALIAS"
jarsigner "$@" >"$work/sign" 2>&1 || fail "jarsigner could not sign $DX_SIGN_AAB: $(grep -v '^[[:space:]]*at ' "$work/sign" | grep -v '^$' | head -n 1)" 1
[ -f "$work/signed.aab" ] || fail "jarsigner could not sign $DX_SIGN_AAB" 1
set +e
jarsigner -verify -strict -verbose -certs "$work/signed.aab" >"$work/verify" 2>&1
code="$?"
set -e
case "$code" in
    0) ;;
    4)
        grep -q "self-signed" "$work/verify" || fail "signed App Bundle failed verification" 1
        if grep -Eq "has expired|is expired|not yet valid" "$work/verify"; then
            fail "signed App Bundle failed verification: the certificate is expired or not yet valid" 1
        fi
        ;;
    *) fail "signed App Bundle failed verification" 1 ;;
esac
grep -q "^jar verified" "$work/verify" || fail "signed App Bundle failed verification" 1
mkdir -p "$(dirname "$DX_SIGN_OUT")"
mv "$work/signed.aab" "$DX_SIGN_OUT"
echo "dx-android-sign: wrote $DX_SIGN_OUT"
