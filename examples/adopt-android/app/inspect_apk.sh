#!/bin/sh
set -eu
apk="$1"
app_id="$2"
lib="$3"
abis="$4"
readelf="$5"
min_sdk="$6"
label="$7"
asset="$8"
activity="$9"
symbol="${10}"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    echo "inspect_apk: ANDROID_HOME or ANDROID_SDK_ROOT names the installed SDK" >&2
    exit 2
fi
build_tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -n 1)"
badging="$("$build_tools/aapt" dump badging "$apk")"
for want in "package: name='$app_id'" "sdkVersion:'$min_sdk'" "application-label:'$label'" "launchable-activity: name='$activity'" "uses-permission: name='android.permission.VIBRATE'"; do
    echo "$badging" | grep -qF "$want" || { echo "badging misses $want"; exit 1; }
done
work="$(mktemp -d)"
unzip -p "$apk" classes.dex > "$work/classes.dex"
classes="$("$build_tools/dexdump" "$work/classes.dex")"
for class in NativeBridge Report MainActivity; do
    echo "$classes" | grep -qF "Lorg/example/adoptandroid/$class;" || { echo "dex misses $class"; exit 1; }
done
unzip -p "$apk" assets/greeting.txt > "$work/greeting.txt"
cmp "$work/greeting.txt" "$asset" || { echo "packaged asset differs from $asset"; exit 1; }
old_ifs="$IFS"
IFS=","
for abi in $abis; do
    IFS="$old_ifs"
    unzip -oq "$apk" "lib/$abi/lib$lib.so" -d "$work"
    "$readelf" --dyn-syms "$work/lib/$abi/lib$lib.so" | grep -q " $symbol$" || { echo "lib/$abi/lib$lib.so misses $symbol"; exit 1; }
done
"$build_tools/apksigner" verify "$apk" || { echo "apksigner rejects $apk"; exit 1; }
echo "inspect_apk: $apk installs $app_id with lib$lib.so for $abis"
