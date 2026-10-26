#!/bin/sh
set -eu
apk="$1"
app_id="$2"
lib="$3"
abis="$4"
readelf="$5"
min_sdk="$6"
permission="$7"
version_code="$8"
version_name="$9"
extra_permission="${10:-}"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    echo "inspect_apk: ANDROID_HOME or ANDROID_SDK_ROOT names the installed SDK" >&2
    exit 2
fi
build_tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -n 1)"
aapt="$build_tools/aapt"
apksigner="$build_tools/apksigner"
badging="$("$aapt" dump badging "$apk")"
echo "$badging" | grep -q "package: name='$app_id' versionCode='$version_code'" || { echo "badging misses package $app_id version $version_code"; exit 1; }
echo "$badging" | grep -q "sdkVersion:'$min_sdk'" || { echo "badging misses min sdk $min_sdk"; exit 1; }
echo "$badging" | grep -q "targetSdkVersion:'34'" || { echo "badging misses target sdk 34"; exit 1; }
echo "$badging" | grep -q "application-label:'DX Rust APK'" || { echo "badging misses label"; exit 1; }
echo "$badging" | grep -q "launchable-activity: name='android.app.NativeActivity'" || { echo "badging misses activity"; exit 1; }
echo "$badging" | grep -q "uses-permission: name='$permission'" || { echo "badging misses permission $permission"; exit 1; }
echo "$badging" | grep -q "versionName='$version_name'" || { echo "badging misses version name $version_name"; exit 1; }
if [ -n "$extra_permission" ]; then
    echo "$badging" | grep -q "uses-permission: name='$extra_permission'" || { echo "badging misses permission $extra_permission"; exit 1; }
fi
"$aapt" dump xmltree "$apk" AndroidManifest.xml | grep -q "android.app.lib_name" || { echo "manifest misses android.app.lib_name"; exit 1; }
"$aapt" dump xmltree "$apk" AndroidManifest.xml | grep -q "\"$lib\"" || { echo "manifest does not load $lib"; exit 1; }
listing="$(unzip -l "$apk")"
work="$(mktemp -d)"
old_ifs="$IFS"
IFS=","
for abi in $abis; do
    IFS="$old_ifs"
    echo "$badging" | grep -q "native-code: .*$abi" || { echo "badging misses abi $abi"; exit 1; }
    echo "$listing" | grep -q "lib/$abi/lib$lib.so" || { echo "apk misses lib/$abi/lib$lib.so"; exit 1; }
    unzip -oq "$apk" "lib/$abi/lib$lib.so" -d "$work"
    case "$abi" in
        arm64-v8a) machine="AArch64" ;;
        x86_64) machine="Advanced Micro Devices X86-64" ;;
        *) echo "unknown abi $abi"; exit 1 ;;
    esac
    "$readelf" -h "$work/lib/$abi/lib$lib.so" | grep -q "Machine:.*$machine" || { echo "lib/$abi/lib$lib.so is not built for $abi"; exit 1; }
    symbols="$("$readelf" --dyn-syms "$work/lib/$abi/lib$lib.so")"
    echo "$symbols" | grep -q "ANativeActivity_onCreate" || { echo "lib/$abi/lib$lib.so does not export ANativeActivity_onCreate"; exit 1; }
    needed="$("$readelf" -d "$work/lib/$abi/lib$lib.so" | grep NEEDED | sed 's/.*\[\(.*\)\]/\1/')"
    for dep in $needed; do
        case "$dep" in
            libc.so | libdl.so | libm.so | liblog.so | libandroid.so) ;;
            *) echo "lib/$abi/lib$lib.so needs $dep, which the apk does not package"; exit 1 ;;
        esac
    done
done
for other in arm64-v8a x86_64; do
    case ",$abis," in
        *",$other,"*) ;;
        *) echo "$listing" | grep -q "lib/$other/" && { echo "apk packages unrequested abi $other"; exit 1; } ;;
    esac
done
echo "$listing" | grep -q "assets/message.txt" || { echo "apk misses assets/message.txt"; exit 1; }
echo "$listing" | grep -q "resources.arsc" || { echo "apk misses resources.arsc"; exit 1; }
"$apksigner" verify --print-certs "$apk" | grep -q "Signer #1 certificate DN:" || { echo "apksigner rejects $apk"; exit 1; }
echo "inspect_apk: $apk installs $app_id with lib$lib.so for $abis"
