#!/bin/sh
set -eu
bundletool="$1"
aab="$2"
app_id="$3"
lib="$4"
abis="$5"
readelf="$6"
min_sdk="$7"
version_code="$8"
version_name="$9"
tmp="${TEST_TMPDIR:-${TMPDIR:-/tmp}}"
xpath() {
    "$bundletool" --jvm_flag="-Djava.io.tmpdir=$tmp" dump manifest --bundle="$aab" --module=base --xpath="$1"
}
[ "$(xpath /manifest/@package)" = "$app_id" ] || { echo "bundle misses package $app_id"; exit 1; }
[ "$(xpath /manifest/@android:versionCode)" = "$version_code" ] || { echo "bundle misses version code $version_code"; exit 1; }
[ "$(xpath /manifest/@android:versionName)" = "$version_name" ] || { echo "bundle misses version name $version_name"; exit 1; }
[ "$(xpath /manifest/uses-sdk/@android:minSdkVersion)" = "$min_sdk" ] || { echo "bundle misses min sdk $min_sdk"; exit 1; }
[ "$(xpath /manifest/uses-sdk/@android:targetSdkVersion)" = "34" ] || { echo "bundle misses target sdk 34"; exit 1; }
[ "$(xpath "/manifest/application/activity/meta-data[@android:name='android.app.lib_name']/@android:value")" = "$lib" ] || { echo "bundle manifest does not load $lib"; exit 1; }
listing="$(unzip -Z1 "$aab")"
echo "$listing" | grep -qx "base/manifest/AndroidManifest.xml" || { echo "bundle misses base/manifest/AndroidManifest.xml"; exit 1; }
echo "$listing" | grep -qx "base/assets/message.txt" || { echo "bundle misses base/assets/message.txt"; exit 1; }
echo "$listing" | grep -qx "base/resources.pb" || { echo "bundle misses base/resources.pb"; exit 1; }
echo "$listing" | grep -Eq "^base/dex/classes[0-9]*\.dex$" || { echo "bundle misses base dex"; exit 1; }
work="$tmp/inspect_aab"
rm -rf "$work"
mkdir -p "$work"
old_ifs="$IFS"
IFS=","
for abi in $abis; do
    IFS="$old_ifs"
    so="base/lib/$abi/lib$lib.so"
    echo "$listing" | grep -qx "$so" || { echo "bundle misses $so"; exit 1; }
    unzip -oq "$aab" "$so" -d "$work"
    case "$abi" in
        arm64-v8a) machine="AArch64" ;;
        x86_64) machine="Advanced Micro Devices X86-64" ;;
        *) echo "unknown abi $abi"; exit 1 ;;
    esac
    "$readelf" -h "$work/$so" | grep -q "Machine:.*$machine" || { echo "$so is not built for $abi"; exit 1; }
    "$readelf" --dyn-syms "$work/$so" | grep -q "ANativeActivity_onCreate" || { echo "$so does not export ANativeActivity_onCreate"; exit 1; }
done
for other in arm64-v8a x86_64; do
    case ",$abis," in
        *",$other,"*) ;;
        *) echo "$listing" | grep -q "^base/lib/$other/" && { echo "bundle packages unrequested abi $other"; exit 1; } ;;
    esac
done
echo "inspect_aab: $aab holds $app_id with lib$lib.so for $abis"
