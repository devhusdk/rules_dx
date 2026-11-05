#!/bin/sh
set -eu

kind="$1"
report="$2"
cell="$3"
sdk="$4"
platform="$5"
arch="$6"
deployment="$7"
xcode="$8"
sdk_version="$9"
shift 9
owner="${kind##*_}"
case "$sdk" in
    macosx) os=macOS ;;
    *) os=iOS ;;
esac
case "$arch" in
    x86_64) legacy=7 ;;
    *) legacy=2 ;;
esac

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

for input in "$@"; do
    name=$(basename "$input")
    case "$name" in
        *.a)
            what="library archive $name"
            key=archive
            ;;
        *)
            what="framework binary $name"
            key=framework
            ;;
    esac
    if ! archs=$(xcrun lipo -archs "$input" 2>/dev/null); then
        echo "$kind: $what is not a Mach-O file" >&2
        exit 1
    fi
    case " $archs " in
        *" $arch "*) ;;
        *)
            echo "$kind: $what carries $archs, want $arch for $cell" >&2
            exit 1
            ;;
    esac
    loads=$(xcrun otool -arch "$arch" -l "$input" 2>/dev/null || true)
    versions=$(printf '%s\n' "$loads" | awk -v legacy="$legacy" '
        $1 == "cmd" { command = $2 }
        command == "LC_BUILD_VERSION" && $1 == "platform" { print "platform", $2 }
        command == "LC_BUILD_VERSION" && $1 == "minos" { print "minos", $2 }
        command == "LC_VERSION_MIN_IPHONEOS" && $1 == "version" { print "platform", legacy; print "minos", $2 }
        command == "LC_VERSION_MIN_MACOSX" && $1 == "version" { print "platform", 1; print "minos", $2 }
    ')
    platforms=$(printf '%s\n' "$versions" | awk '$1 == "platform" { print $2 }' | sort -u)
    if [ -z "$platforms" ]; then
        echo "$kind: $what declares no build platform; rebuild it for $sdk" >&2
        exit 1
    fi
    for found in $platforms; do
        if [ "$found" != "$platform" ]; then
            echo "$kind: $what is built for $(platform_name "$found"), want $sdk for $cell" >&2
            exit 1
        fi
    done
    newest=$(printf '%s\n' "$versions" | awk '$1 == "minos" { print $2 }' | sort -u -t. -k1,1n -k2,2n -k3,3n | tail -n 1)
    if [ -n "$newest" ] && above "$newest" "$deployment"; then
        echo "$kind: $what needs $os $newest, above the $owner deployment $deployment" >&2
        exit 1
    fi
    echo "$key=$name arch=$arch platform=$sdk minos=$newest" >>"$report.tmp"
done

mv "$report.tmp" "$report"
