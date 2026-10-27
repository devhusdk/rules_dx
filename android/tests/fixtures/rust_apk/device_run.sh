#!/bin/sh
set -u
if [ "$#" -ne 2 ]; then
    echo "usage: device_run.sh <serial> <evidence_dir>" >&2
    exit 2
fi
serial="$1"
if [ "$serial" != "emulator-5554" ]; then
    echo "device_run: the fixture launchers name emulator-5554, got $serial" >&2
    exit 2
fi
root="${BUILD_WORKSPACE_DIRECTORY:-$(pwd)}"
cd "$root" || exit 2
if [ ! -x tools/dx ]; then
    echo "device_run: run from the repository root" >&2
    exit 2
fi
mkdir -p "$2" || exit 2
evidence="$(cd "$2" && pwd)"
ADB="${ADB:-adb}"
export ADB
BAZEL="${BAZEL:-bazel}"
pkg="//android/tests/fixtures/rust_apk"
app="com.example.dxrustapk"
custom="org.example.custom.dxrustapk"
bytes=30
failed=0
: > "$evidence/exit-codes.txt"

step() {
    name="$1"
    shift
    echo "== $name"
    (
        set -e
        "$@"
    ) > "$evidence/$name.log" 2>&1
    code="$?"
    cat "$evidence/$name.log"
    echo "$name $code" >> "$evidence/exit-codes.txt"
    if [ "$code" -ne 0 ]; then
        echo "== $name failed with $code"
        failed=1
    fi
}

device() {
    "$ADB" -s "$serial" "$@"
}

wait_log() {
    want="$1"
    count="$2"
    waited=0
    while [ "$waited" -lt 60 ]; do
        seen="$(device logcat -d -s DxRustApk:I 2>/dev/null | grep -c -- "$want" || true)"
        if [ "${seen:-0}" -ge "$count" ]; then
            echo "device_run: saw $want x$seen"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    echo "device_run: logcat shows '$want' fewer than $count times after 60 s" >&2
    return 1
}

not_installed() {
    if device shell pm path "$1" 2>&1 | grep -q '^package:'; then
        echo "device_run: $1 is installed before any explicit run" >&2
        return 1
    fi
    echo "device_run: $1 is not installed"
}

no_signed_outputs() {
    for target in sign sign_app sign_app_aab sign_test_bundle; do
        "$BAZEL" cquery --output=files "$pkg:$target" > "$evidence/cquery-$target.txt"
        while read -r out; do
            if [ -e "$out" ]; then
                echo "device_run: a build produced $out" >&2
                return 1
            fi
        done < "$evidence/cquery-$target.txt"
    done
    echo "device_run: no signing launcher output exists"
}

no_effect() {
    tools/dx build "$pkg:app_apk_file" "$pkg:custom_apk_file"
    tools/dx test "$pkg:inspect_apk_test" "$pkg:inspect_custom_apk_test"
    no_signed_outputs
    tools/dx check "$pkg/..."
    not_installed "$app"
    not_installed "$custom"
}

default_app() {
    device logcat -c
    tools/dx run "$pkg:install" "$pkg:start"
    wait_log "lifecycle=resume" 1
    device shell input tap 200 400
    wait_log "input=touch" 1
    device shell input keyevent KEYCODE_ENTER
    wait_log "input=key" 1
    device shell input keyevent KEYCODE_HOME
    wait_log "lifecycle=stop" 1
    tools/dx run "$pkg:start"
    wait_log "lifecycle=resume" 2
    tools/dx run "$pkg:log" > "$evidence/logcat-default.txt"
    tools/dx run "$pkg:device_check" -- logcat "$evidence/logcat-default.txt" "$bytes"
}

custom_app() {
    device logcat -c
    tools/dx run "$pkg:install_custom" "$pkg:start_custom"
    wait_log "asset=ok" 1
    device shell dumpsys package "$custom" > "$evidence/dumpsys-custom.txt"
    tools/dx run "$pkg:device_check" -- package "$evidence/dumpsys-custom.txt" "$custom" 7 33 x86_64 \
        android.permission.VIBRATE android.permission.ACCESS_NETWORK_STATE
    device logcat -d -s DxRustApk:I > "$evidence/logcat-custom.txt"
    grep "asset=ok name=message.txt bytes=$bytes" "$evidence/logcat-custom.txt"
}

record() {
    echo "== $1"
    shift
    "$@" 2>&1 || echo "(exit $?)"
}

versions() {
    sdk="${ANDROID_HOME:-}"
    {
        record uname uname -a
        record os-release cat /etc/os-release
        record bazel "$BAZEL" version
        record adb "$ADB" version
        record emulator "${EMULATOR:-emulator}" -version
        record ndk grep Pkg.Revision "$sdk/ndk/${ANDROID_NDK_REVISION:-}/source.properties"
        for props in "$sdk"/build-tools/*/source.properties; do
            record build-tools grep Pkg.Revision "$props"
        done
        record sdkmanager "${SDKMANAGER:-sdkmanager}" --list_installed
        for prop in ro.build.version.release ro.build.version.sdk ro.product.cpu.abi ro.build.fingerprint; do
            record "$prop" device shell getprop "$prop"
        done
        echo "x86_64 only; arm64-v8a lib not executed"
    } > "$evidence/versions.txt"
}

versions
step no-effect no_effect
step default-app default_app
step custom-app custom_app
cat "$evidence/exit-codes.txt"
exit "$failed"
