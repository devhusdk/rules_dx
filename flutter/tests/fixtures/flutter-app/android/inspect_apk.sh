#!/bin/sh
set -eu
apk="$1"
app_id="$2"
list="$(unzip -l "$apk")"
for want in lib/x86_64/libflutter.so lib/x86_64/libapp.so flutter_assets/assets/items.json; do
    echo "$list" | grep -q "$want" || { echo "apk misses $want"; exit 1; }
done
unzip -p "$apk" AndroidManifest.xml | tr -d '\000' | grep -aqF "$app_id" || { echo "manifest misses $app_id"; exit 1; }
echo "inspect_apk: $apk packages $app_id"
