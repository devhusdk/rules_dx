#!/bin/sh
set -eu
apk="$1"
app_id="$2"
lib="$3"
abis="$4"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    echo "inspect_apk: ANDROID_HOME or ANDROID_SDK_ROOT names the installed SDK" >&2
    exit 2
fi
build_tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -n 1)"
aapt="$build_tools/aapt"
apksigner="$build_tools/apksigner"
badging="$("$aapt" dump badging "$apk")"
echo "$badging" | grep -q "package: name='$app_id'" || { echo "badging misses package $app_id"; exit 1; }
echo "$badging" | grep -q "sdkVersion:'31'" || { echo "badging misses min sdk 31"; exit 1; }
echo "$badging" | grep -q "targetSdkVersion:'34'" || { echo "badging misses target sdk 34"; exit 1; }
echo "$badging" | grep -q "application-label:'DX Rust APK'" || { echo "badging misses label"; exit 1; }
echo "$badging" | grep -q "launchable-activity: name='android.app.NativeActivity'" || { echo "badging misses activity"; exit 1; }
echo "$badging" | grep -q "uses-permission: name='android.permission.INTERNET'" || { echo "badging misses permission"; exit 1; }
old_ifs="$IFS"
IFS=","
for abi in $abis; do
    echo "$badging" | grep -q "native-code: .*$abi" || { echo "badging misses abi $abi"; exit 1; }
    unzip -l "$apk" | grep -q "lib/$abi/lib$lib.so" || { echo "apk misses lib/$abi/lib$lib.so"; exit 1; }
done
IFS="$old_ifs"
unzip -l "$apk" | grep -q "assets/message.txt" || { echo "apk misses assets/message.txt"; exit 1; }
unzip -l "$apk" | grep -q "resources.arsc" || { echo "apk misses resources.arsc"; exit 1; }
"$apksigner" verify --print-certs "$apk" | grep -q "Signer #1 certificate DN:" || { echo "apksigner rejects $apk"; exit 1; }
echo "inspect_apk: $apk installs $app_id with lib$lib.so for $abis"
