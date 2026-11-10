#!/bin/sh
set -eu
here() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$PWD/$1" ;;
    esac
}
signer="$(here "$1")"
unsigned="$(here "$2")"
malformed="$(here "$3")"
bundletool="$(here "$4")"
javabase="$(here "$5")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export BUILD_WORKING_DIRECTORY="$work"
"$javabase/bin/keytool" -genkeypair -keystore "$work/test.jks" -storetype PKCS12 -alias dxtest -keyalg RSA -keysize 2048 \
    -validity 2 -dname "CN=dx disposable test key" -storepass "dx-store-secret" >/dev/null 2>&1
printf 'dx-store-secret\n' >"$work/store.pass"
printf 'wrong-secret\n' >"$work/wrong.pass"
export DX_SIGN_AAB="$unsigned"
export DX_SIGN_BUNDLETOOL="$bundletool"
export DX_SIGN_JAVABASE="$javabase"
export DX_SIGN_KEYSTORE="$work/test.jks"
export DX_SIGN_STORE_PASS_FILE="$work/store.pass"
export DX_SIGN_KEY_ALIAS="dxtest"
export DX_SIGN_OUT="$work/signed.aab"

"$javabase/bin/jarsigner" -verify "$unsigned" 2>&1 | grep -q "^jar verified" && { echo "fixture is already signed"; exit 1; }

"$signer" >"$work/log" 2>&1 || { echo "signing failed"; cat "$work/log"; exit 1; }
[ -f "$work/signed.aab" ] || { echo "no signed bundle written"; exit 1; }
"$javabase/bin/jarsigner" -verify -verbose -certs "$work/signed.aab" >"$work/verify" 2>&1 || { echo "jarsigner rejects the signed bundle"; cat "$work/verify"; exit 1; }
grep -q "^jar verified" "$work/verify" || { echo "signed bundle does not verify"; cat "$work/verify"; exit 1; }
grep -q "CN=dx disposable test key" "$work/verify" || { echo "signer is not the supplied test identity"; cat "$work/verify"; exit 1; }
grep -q "Signature algorithm: SHA256withRSA" "$work/verify" || { echo "signature is not SHA256withRSA"; cat "$work/verify"; exit 1; }
"$bundletool" --jvm_flag="-Djava.io.tmpdir=$work" validate --bundle="$work/signed.aab" >/dev/null 2>&1 || { echo "bundletool rejects the signed bundle"; exit 1; }
if grep -rq "dx-store-secret" "$work/log"; then
    echo "password reached the log"
    exit 1
fi
mkdir "$work/x"
(cd "$work/x" && "$javabase/bin/jar" xf "$work/signed.aab")
[ -f "$work/x/base/manifest/AndroidManifest.xml" ] || { echo "signed bundle did not extract"; exit 1; }
if grep -raq "dx-store-secret" "$work/x"; then
    echo "password reached the bundle"
    exit 1
fi
if cmp -s "$unsigned" "$work/signed.aab"; then
    echo "signed bundle equals the unsigned input"
    exit 1
fi
cp "$work/signed.aab" "$work/presigned.aab"
(
    export DX_SIGN_KEY_PASS_FILE="$work/store.pass"
    export DX_SIGN_OUT="$work/keypass.aab"
    "$signer" >"$work/log" 2>&1 || { echo "signing with a key password file failed"; cat "$work/log"; exit 1; }
)
(
    "$javabase/bin/keytool" -genkeypair -keystore "$work/ec.jks" -storetype PKCS12 -alias dxec -keyalg EC -groupname secp256r1 \
        -validity 2 -dname "CN=dx disposable ec key" -storepass "dx-store-secret" >/dev/null 2>&1
    export DX_SIGN_KEYSTORE="$work/ec.jks"
    export DX_SIGN_KEY_ALIAS="dxec"
    export DX_SIGN_OUT="$work/ec.aab"
    "$signer" >"$work/log" 2>&1 || { echo "signing with an EC key failed"; cat "$work/log"; exit 1; }
    "$javabase/bin/jarsigner" -verify -verbose -certs "$work/ec.aab" >"$work/verify" 2>&1 || { echo "jarsigner rejects the EC-signed bundle"; cat "$work/verify"; exit 1; }
    grep -q "CN=dx disposable ec key" "$work/verify" || { echo "signer is not the EC test identity"; cat "$work/verify"; exit 1; }
    grep -q "Signature algorithm: SHA256withECDSA" "$work/verify" || { echo "signature is not SHA256withECDSA"; cat "$work/verify"; exit 1; }
)
(
    export LANG=de_DE.UTF-8 LC_ALL=de_DE.UTF-8 JAVA_TOOL_OPTIONS="-Duser.language=de -Duser.country=DE"
    export DX_SIGN_OUT="$work/german.aab"
    "$signer" >"$work/log" 2>&1 || { echo "signing under a German locale failed"; cat "$work/log"; exit 1; }
    export DX_SIGN_AAB="$work/presigned.aab"
    export DX_SIGN_OUT="$work/german-resigned.aab"
    if "$signer" >"$work/log" 2>&1 || ! grep -q "is already signed" "$work/log"; then
        echo "an already-signed bundle passed under a German locale"
        cat "$work/log"
        exit 1
    fi
)
"$javabase/bin/keytool" -genkeypair -keystore "$work/split.jks" -storetype JKS -alias dxsplit -keyalg RSA -keysize 2048 \
    -validity 2 -dname "CN=dx disposable split key" -storepass "dx-store-secret" -keypass "dx-key-secret" >/dev/null 2>&1
printf 'dx-key-secret\n' >"$work/key.pass"
(
    export DX_SIGN_KEYSTORE="$work/split.jks"
    export DX_SIGN_KEY_ALIAS="dxsplit"
    export DX_SIGN_KEY_PASS_FILE="$work/key.pass"
    export DX_SIGN_OUT="$work/split.aab"
    "$signer" >"$work/log" 2>&1 || { echo "signing with a separate key password failed"; cat "$work/log"; exit 1; }
)

expect_failure() {
    label="$1"
    pattern="$2"
    want="$3"
    rm -f "$work/signed.aab"
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
    if grep -q "dx-store-secret" "$work/log"; then
        echo "$label leaked the password"
        exit 1
    fi
    if [ -e "$work/signed.aab" ]; then
        echo "$label left a signed bundle behind"
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
    export DX_SIGN_AAB="$work/presigned.aab"
    expect_failure "signed input" "is already signed" 1
)
(
    export DX_SIGN_AAB="$malformed"
    expect_failure "malformed bundle" "bundletool validate rejects" 1
)
(
    export DX_SIGN_OUT="$unsigned"
    expect_failure "overwrite input" "must not overwrite the input" 2
)
(
    ln -s "$unsigned" "$work/link.aab"
    export DX_SIGN_OUT="$work/link.aab"
    expect_failure "overwrite input through a symlink" "must not overwrite the input" 2
    cmp -s "$unsigned" "$work/link.aab" || { echo "overwrite through a symlink replaced the input"; exit 1; }
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
    export DX_SIGN_KEY_PASS_FILE="$work/missing.pass"
    expect_failure "missing key password file" "no key password file at" 2
)
(
    unset DX_SIGN_KEYSTORE
    expect_failure "unset keystore" "DX_SIGN_KEYSTORE is not set" 2
)
(
    unset DX_SIGN_KEY_ALIAS
    expect_failure "unset alias" "DX_SIGN_KEY_ALIAS is not set" 2
)
(
    export DX_SIGN_BUNDLETOOL="$work/none"
    expect_failure "missing bundletool" "no bundletool" 127
)
(
    export DX_SIGN_JAVABASE="$work/none"
    expect_failure "missing jarsigner" "no jarsigner" 127
)
(
    export DX_SIGN_KEYSTORE="$work/split.jks"
    export DX_SIGN_KEY_ALIAS="dxsplit"
    unset DX_SIGN_KEY_PASS_FILE
    expect_failure "separate key password without a key password file" "could not sign" 1
)
