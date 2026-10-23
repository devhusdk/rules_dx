#!/bin/sh
set -eu

report="$1"
cell="$2"
sdk="$3"
platform="$4"
arch="$5"
deployment="$6"
xcode="$7"
sdk_version="$8"
shift 8

above() {
    awk -v left="$1" -v right="$2" 'BEGIN {
        n = split(left, a, ".")
        m = split(right, b, ".")
        width = n > m ? n : m
        for (i = 1; i <= width; i++) {
            x = (i <= n) ? a[i] + 0 : 0
            y = (i <= m) ? b[i] + 0 : 0
            if (x != y) { exit !(x > y) }
        }
        exit 1
    }'
}

platform_name() {
    case "$1" in
        1) echo macosx ;;
        2) echo iphoneos ;;
        7) echo iphonesimulator ;;
        *) echo "platform $1" ;;
    esac
}

{
    echo "cell=$cell"
    echo "sdk=$sdk"
    echo "sdk_version=$sdk_version"
    echo "deployment=$deployment"
    echo "xcode=$xcode"
} >"$report.tmp"

for archive in "$@"; do
    name=$(basename "$archive")
    if ! archs=$(xcrun lipo -archs "$archive" 2>/dev/null); then
        echo "apple_ios_app: library archive $name is not a Mach-O archive" >&2
        exit 1
    fi
    case " $archs " in
        *" $arch "*) ;;
        *)
            echo "apple_ios_app: library archive $name carries $archs, want $arch for $cell" >&2
            exit 1
            ;;
    esac
    loads=$(xcrun otool -arch "$arch" -l "$archive" 2>/dev/null || true)
    platforms=$(printf '%s\n' "$loads" | awk '$1 == "platform" { print $2 }' | sort -u)
    if [ -z "$platforms" ]; then
        echo "apple_ios_app: library archive $name declares no build platform; rebuild it for $sdk" >&2
        exit 1
    fi
    for found in $platforms; do
        if [ "$found" != "$platform" ]; then
            echo "apple_ios_app: library archive $name is built for $(platform_name "$found"), want $sdk for $cell" >&2
            exit 1
        fi
    done
    newest=$(printf '%s\n' "$loads" | awk '$1 == "minos" { print $2 }' | sort -u -t. -k1,1n -k2,2n -k3,3n | tail -n 1)
    if [ -n "$newest" ] && above "$newest" "$deployment"; then
        echo "apple_ios_app: library archive $name needs iOS $newest, above the app deployment $deployment" >&2
        exit 1
    fi
    echo "archive=$name arch=$arch platform=$sdk minos=$newest" >>"$report.tmp"
done

mv "$report.tmp" "$report"
