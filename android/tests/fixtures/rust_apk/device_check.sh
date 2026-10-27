#!/bin/sh
set -eu
usage() {
    echo "usage: device_check.sh logcat <logcat_file> <asset_bytes>" >&2
    echo "       device_check.sh package <dumpsys_file> <app_id> <version_code> <min_sdk> <abi> [permission...]" >&2
    exit 2
}
has_field() {
    awk -v want="$1" '{ for (i = 1; i <= NF; i++) if ($i == want) found = 1 } END { exit !found }' "$2"
}
check_logcat() {
    [ "$#" -eq 2 ] || usage
    file="$1"
    bytes="$2"
    [ -f "$file" ] || { echo "device_check: no logcat file $file" >&2; exit 2; }
    case "$bytes" in
        '' | *[!0-9]*) usage ;;
    esac
    awk -v bytes="$bytes" '
        BEGIN {
            n = split("create start resume pause stop start resume", seq, " ")
            k = 1
        }
        {
            at = index($0, "DxRustApk")
            if (at == 0) next
            rest = substr($0, at + length("DxRustApk"))
            sub(/^[^A-Za-z]*/, "", rest)
            count = split(rest, tok, /[[:space:]]+/)
            asset = ""
            name = ""
            size = ""
            for (i = 1; i <= count; i++) {
                if (tok[i] ~ /^lifecycle=/) {
                    event = substr(tok[i], 11)
                    if (k <= n && event == seq[k]) k++
                } else if (tok[i] == "input=touch") {
                    touch++
                } else if (tok[i] == "input=key") {
                    key++
                } else if (tok[i] ~ /^asset=/) {
                    asset = substr(tok[i], 7)
                } else if (tok[i] ~ /^name=/) {
                    name = substr(tok[i], 6)
                } else if (tok[i] ~ /^bytes=/) {
                    size = substr(tok[i], 7)
                }
            }
            if (asset == "missing" || asset == "mismatch") bad[asset] = 1
            if (asset == "ok" && name == "message.txt" && size == bytes) ok = 1
        }
        END {
            fail = 0
            if (k <= n) {
                want = ""
                for (i = k; i <= n; i++) want = want " " seq[i]
                print "device_check: lifecycle sequence stops before:" want > "/dev/stderr"
                fail = 1
            }
            if (!touch) { print "device_check: no input=touch line" > "/dev/stderr"; fail = 1 }
            if (!key) { print "device_check: no input=key line" > "/dev/stderr"; fail = 1 }
            if (!ok) { print "device_check: no asset=ok name=message.txt bytes=" bytes " line" > "/dev/stderr"; fail = 1 }
            for (b in bad) { print "device_check: found asset=" b > "/dev/stderr"; fail = 1 }
            exit fail
        }
    ' "$file" || exit 1
}
check_package() {
    [ "$#" -ge 5 ] || usage
    file="$1"
    app="$2"
    version="$3"
    min_sdk="$4"
    abi="$5"
    shift 5
    [ -f "$file" ] || { echo "device_check: no dumpsys file $file" >&2; exit 2; }
    fail=0
    grep -qF "Package [$app]" "$file" || { echo "device_check: no Package [$app] entry" >&2; fail=1; }
    has_field "versionCode=$version" "$file" || { echo "device_check: no versionCode=$version" >&2; fail=1; }
    has_field "minSdk=$min_sdk" "$file" || { echo "device_check: no minSdk=$min_sdk" >&2; fail=1; }
    has_field "primaryCpuAbi=$abi" "$file" || { echo "device_check: no primaryCpuAbi=$abi" >&2; fail=1; }
    for perm in "$@"; do
        awk -v want="$perm" '{ t = $1; sub(/:$/, "", t); if (t == want) found = 1 } END { exit !found }' "$file" \
            || { echo "device_check: no permission $perm" >&2; fail=1; }
    done
    [ "$fail" -eq 0 ] || exit 1
}
[ "$#" -ge 1 ] || usage
mode="$1"
shift
case "$mode" in
    logcat) check_logcat "$@" ;;
    package) check_package "$@" ;;
    *) usage ;;
esac
echo "device_check: $mode ok"
