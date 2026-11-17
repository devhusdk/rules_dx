#!/bin/sh
set -eu
apk="$1"
app_id="$2"
lib="$3"
abis="$4"
readelf="$5"
min_sdk="$6"
label="$7"
board="$8"
notes="$9"
aapt="${DX_AAPT:?inspect_apk: DX_AAPT names the aapt binary}"
apksigner_jar="${DX_APKSIGNER_JAR:?inspect_apk: DX_APKSIGNER_JAR names apksigner.jar}"
java="${DX_JAVA:?inspect_apk: DX_JAVA names the java binary}"
badging="$("$aapt" dump badging "$apk")"
for want in "package: name='$app_id'" "sdkVersion:'$min_sdk'" "application-label:'$label'" "launchable-activity: name='android.app.NativeActivity'"; do
    echo "$badging" | grep -qF "$want" || { echo "badging misses $want"; exit 1; }
done
work="$(mktemp -d)"
unzip -p "$apk" assets/board.txt > "$work/board.txt"
unzip -p "$apk" assets/notes.txt > "$work/notes.txt"
cmp "$work/board.txt" "$board" || { echo "packaged board differs from $board"; exit 1; }
cmp "$work/notes.txt" "$notes" || { echo "packaged notes differ from $notes"; exit 1; }
old_ifs="$IFS"
IFS=","
for abi in $abis; do
    IFS="$old_ifs"
    unzip -oq "$apk" "lib/$abi/lib$lib.so" -d "$work"
    symbols="$("$readelf" --dyn-syms "$work/lib/$abi/lib$lib.so")"
    for symbol in ANativeActivity_onCreate tasks_load tasks_apply; do
        echo "$symbols" | grep -q " $symbol$" || { echo "lib/$abi/lib$lib.so misses $symbol"; exit 1; }
    done
done
for other in arm64-v8a x86_64; do
    case ",$abis," in
        *",$other,"*) ;;
        *) unzip -l "$apk" | grep -q "lib/$other/" && { echo "apk packages unrequested abi $other"; exit 1; } ;;
    esac
done
"$java" -jar "$apksigner_jar" verify "$apk" || { echo "apksigner rejects $apk"; exit 1; }
echo "inspect_apk: $apk installs $app_id with lib$lib.so for $abis"
