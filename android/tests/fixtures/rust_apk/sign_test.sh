#!/bin/sh
set -eu
signer="$1"
unsigned="$2"
malformed="$3"
build_tools="$4"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    echo "sign_test: ANDROID_HOME or ANDROID_SDK_ROOT names the installed SDK" >&2
    exit 2
fi
apksigner="$sdk/build-tools/$build_tools/apksigner"
zipalign="$sdk/build-tools/$build_tools/zipalign"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export BUILD_WORKING_DIRECTORY="$work"
keytool -genkeypair -keystore "$work/test.jks" -storetype PKCS12 -alias dxtest -keyalg RSA -keysize 2048 \
    -validity 2 -dname "CN=dx disposable test key" -storepass "dx-store-secret" >/dev/null 2>&1
printf 'dx-store-secret\n' >"$work/store.pass"
printf 'wrong-secret\n' >"$work/wrong.pass"
export DX_SIGN_APK="$unsigned"
export DX_SIGN_BUILD_TOOLS="$build_tools"
export DX_SIGN_SCHEMES="v1,v2,v3"
export DX_SIGN_KEYSTORE="$work/test.jks"
export DX_SIGN_STORE_PASS_FILE="$work/store.pass"
export DX_SIGN_KEY_ALIAS="dxtest"
export DX_SIGN_OUT="$work/signed.apk"

"$apksigner" verify "$unsigned" >/dev/null 2>&1 && { echo "fixture is already signed"; exit 1; }

"$signer" >"$work/log" 2>&1 || { echo "signing failed"; cat "$work/log"; exit 1; }
[ -f "$work/signed.apk" ] || { echo "no signed apk written"; exit 1; }
"$apksigner" verify --verbose --print-certs "$work/signed.apk" >"$work/verify" 2>&1 || { echo "apksigner rejects the signed apk"; cat "$work/verify"; exit 1; }
grep -q "Verified using v3 scheme (APK Signature Scheme v3): true" "$work/verify" || { echo "v3 scheme missing"; cat "$work/verify"; exit 1; }
grep -q "CN=dx disposable test key" "$work/verify" || { echo "signer is not the supplied test identity"; cat "$work/verify"; exit 1; }
"$zipalign" -c -p 4 "$work/signed.apk" || { echo "signed apk is not aligned"; exit 1; }
if grep -rq "dx-store-secret" "$work/log"; then
    echo "password reached the log"
    exit 1
fi
if unzip -p "$work/signed.apk" | grep -aq "dx-store-secret"; then
    echo "password reached the apk"
    exit 1
fi
if cmp -s "$unsigned" "$work/signed.apk"; then
    echo "signed apk equals the unsigned input"
    exit 1
fi

expect_failure() {
    label="$1"
    pattern="$2"
    rm -f "$work/signed.apk"
    set +e
    "$signer" >"$work/log" 2>&1
    code="$?"
    set -e
    if [ "$code" = "0" ]; then
        echo "$label exited 0"
        exit 1
    fi
    grep -q "$pattern" "$work/log" || { echo "$label misses '$pattern'"; cat "$work/log"; exit 1; }
    if [ -e "$work/signed.apk" ]; then
        echo "$label left a signed apk behind"
        exit 1
    fi
}

(
    export DX_SIGN_STORE_PASS_FILE="$work/wrong.pass"
    expect_failure "wrong password" "could not sign"
)
(
    export DX_SIGN_KEY_ALIAS="nokey"
    expect_failure "wrong alias" "could not sign"
)
(
    export DX_SIGN_KEYSTORE="$work/missing.jks"
    expect_failure "missing keystore" "no keystore at"
)
(
    export DX_SIGN_STORE_PASS_FILE="$work/missing.pass"
    expect_failure "missing password file" "no password file at"
)
(
    unset DX_SIGN_KEYSTORE
    expect_failure "unset keystore" "DX_SIGN_KEYSTORE is not set"
)
(
    export DX_SIGN_APK="$malformed"
    expect_failure "malformed package" "zipalign rejected"
)
(
    export ANDROID_HOME="$work/no-sdk"
    unset ANDROID_SDK_ROOT
    expect_failure "missing tools" "no apksigner"
)
