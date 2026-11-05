#!/bin/sh
set -eu
app_id="$1"
abi="$2"
activity="$3"
shift 3
apk=""
for f in "$@"; do
    case "$f" in
    *_unsigned.apk) ;;
    *.apk) apk="$f" ;;
    esac
done
[ -n "$apk" ] || { echo "no signed apk in: $*"; exit 1; }
case "$abi" in
x86_64) other=arm64-v8a ;;
*) other=x86_64 ;;
esac
list="$(unzip -l "$apk")"
for want in "lib/$abi/libflutter.so" "lib/$abi/libapp.so" classes.dex flutter_assets/packages/flutter_app/assets/items.json DxSquare.ttf; do
    echo "$list" | grep -q "$want" || { echo "apk misses $want"; exit 1; }
done
if echo "$list" | grep -q "lib/$other/"; then
    echo "apk has lib/$other/"
    exit 1
fi
manifest="$(unzip -p "$apk" AndroidManifest.xml | tr -d '\000')"
echo "$manifest" | grep -aqF "$app_id" || { echo "manifest misses $app_id"; exit 1; }
echo "$manifest" | grep -aqF "$activity" || { echo "manifest misses $activity"; exit 1; }
echo "inspect_apk: $apk packages $app_id with $activity for $abi"
