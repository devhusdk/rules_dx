#!/bin/sh
set -eu
here() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$PWD/$1" ;;
    esac
}
signer="$1"
unsigned="$2"
malformed="$3"
zipalign="$(here "$4")"
jar="$(here "$5")"
java="$(here "$6")"
javabase="$(here "$7")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export BUILD_WORKING_DIRECTORY="$work"
"$javabase/bin/keytool" -genkeypair -keystore "$work/test.jks" -storetype PKCS12 -alias dxtest -keyalg RSA -keysize 2048 \
    -validity 2 -dname "CN=dx disposable test key" -storepass "dx-store-secret" >/dev/null 2>&1
printf 'dx-store-secret\n' >"$work/store.pass"
printf 'wrong-secret\n' >"$work/wrong.pass"
export DX_SIGN_APK="$unsigned"
export DX_SIGN_ZIPALIGN="$zipalign"
export DX_SIGN_APKSIGNER_JAR="$jar"
export DX_SIGN_JAVA="$java"
export DX_SIGN_SCHEMES="v1,v2,v3"
export DX_SIGN_KEYSTORE="$work/test.jks"
export DX_SIGN_STORE_PASS_FILE="$work/store.pass"
export DX_SIGN_KEY_ALIAS="dxtest"
export DX_SIGN_OUT="$work/signed.apk"

"$java" -jar "$jar" verify "$unsigned" >/dev/null 2>&1 && { echo "fixture is already signed"; exit 1; }

printf 'stale' >"$work/signed.apk.idsig"
"$signer" >"$work/log" 2>&1 || { echo "signing failed"; cat "$work/log"; exit 1; }
[ -f "$work/signed.apk" ] || { echo "no signed apk written"; exit 1; }
[ ! -e "$work/signed.apk.idsig" ] || { echo "stale idsig survived a run without v4"; exit 1; }
"$java" -jar "$jar" verify --verbose --print-certs "$work/signed.apk" >"$work/verify" 2>&1 || { echo "apksigner rejects the signed apk"; cat "$work/verify"; exit 1; }
grep -q "Verified using v3 scheme (APK Signature Scheme v3): true" "$work/verify" || { echo "v3 scheme missing"; cat "$work/verify"; exit 1; }
grep -q "CN=dx disposable test key" "$work/verify" || { echo "signer is not the supplied test identity"; cat "$work/verify"; exit 1; }
"$zipalign" -c -p 4 "$work/signed.apk" || { echo "signed apk is not aligned"; exit 1; }
if grep -rq "dx-store-secret" "$work/log"; then
    echo "password reached the log"
    exit 1
fi
mkdir "$work/x"
(cd "$work/x" && "$javabase/bin/jar" xf "$work/signed.apk")
[ -f "$work/x/AndroidManifest.xml" ] || { echo "signed apk did not extract"; exit 1; }
if grep -raq "dx-store-secret" "$work/x"; then
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
    want="$3"
    rm -f "$work/signed.apk" "$work/signed.apk.idsig"
    set +e
    "$signer" >"$work/log" 2>&1
    code="$?"
    set -e
    if [ "$code" != "$want" ]; then
        echo "$label exited $code, want $want"
        cat "$work/log"
        exit 1
    fi
    grep -q "$pattern" "$work/log" || { echo "$label misses '$pattern'"; cat "$work/log"; exit 1; }
    if [ -e "$work/signed.apk" ]; then
        echo "$label left a signed apk behind"
        exit 1
    fi
    if [ -e "$work/signed.apk.idsig" ]; then
        echo "$label left an idsig behind"
        exit 1
    fi
}

check_v4() {
    out="$work/$1.apk"
    "$signer" >"$work/log" 2>&1 || { echo "v4 signing failed"; cat "$work/log"; exit 1; }
    [ -f "$out" ] && [ -f "$out.idsig" ] || { echo "v4 run wrote no apk or idsig"; cat "$work/log"; exit 1; }
    grep -q "wrote .*$1.apk.idsig" "$work/log" || { echo "log misses the idsig path"; cat "$work/log"; exit 1; }
    "$java" -jar "$jar" verify -v --v4-signature-file "$out.idsig" "$out" >"$work/verify4" 2>&1 || { echo "apksigner rejects the v4 signature"; cat "$work/verify4"; exit 1; }
    grep -q "Verified using v4 scheme (APK Signature Scheme v4): true" "$work/verify4" || { echo "v4 scheme missing"; cat "$work/verify4"; exit 1; }
    if grep -aq "dx-store-secret" "$work/log" "$out.idsig"; then
        echo "password reached the log or idsig"
        exit 1
    fi
}

(
    export DX_SIGN_STORE_PASS_FILE="$work/wrong.pass"
    expect_failure "wrong password" "could not sign" 1
)
(
    export DX_SIGN_KEY_ALIAS="nokey"
    expect_failure "wrong alias" "could not sign" 1
)
(
    export DX_SIGN_KEYSTORE="$work/missing.jks"
    expect_failure "missing keystore" "no keystore at" 2
)
(
    export DX_SIGN_STORE_PASS_FILE="$work/missing.pass"
    expect_failure "missing password file" "no password file at" 2
)
(
    unset DX_SIGN_KEYSTORE
    expect_failure "unset keystore" "DX_SIGN_KEYSTORE is not set" 2
)
(
    export DX_SIGN_APK="$malformed"
    expect_failure "malformed package" "zipalign rejected" 1
)
(
    export DX_SIGN_ZIPALIGN="$work/none"
    expect_failure "missing zipalign" "no zipalign" 127
)
(
    export DX_SIGN_APKSIGNER_JAR="$work/none"
    expect_failure "missing apksigner" "no apksigner" 127
)
(
    export DX_SIGN_JAVA="$work/none"
    expect_failure "missing java runtime" "no java runtime" 127
)
(
    export DX_SIGN_SCHEMES="v2,v4"
    export DX_SIGN_OUT="$work/v4.apk"
    check_v4 v4
)
(
    export DX_SIGN_SCHEMES="v3,v4"
    export DX_SIGN_OUT="$work/v34.apk"
    check_v4 v34
)
(
    export DX_SIGN_SCHEMES="v4"
    expect_failure "v4 only" "v4 needs v2 or v3" 2
)
