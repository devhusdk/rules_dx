#!/bin/sh
set -eu

report="$1"
out="$2"
archive="$3"
name="$4"
bundle_id="$5"
version="$6"
deployment="$7"
xcode="$8"
slices="$9"
resources="${10}"
shift 10

work="$report.unpacked"

fail() {
    rm -rf "$work" "$report.tmp"
    echo "apple_ios_framework: $*" >&2
    exit 1
}

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

value() {
    extracted=$(plutil -extract "$1" raw -o - "$2" 2>/dev/null) || return 1
    printf '%s\n' "$extracted"
}

expect() {
    got=$(value "$1" "$2" || true)
    if [ "$got" != "$3" ]; then
        fail "$4 has $1 '$got', want '$3'"
    fi
}

rm -rf "$work"
mkdir -p "$work"
if ! unzipped=$(unzip -q "$archive" -d "$work" 2>&1 </dev/null); then
    fail "$(basename "$archive") does not unpack: $(printf '%s\n' "$unzipped" | tail -n 1)"
fi
root="$work/$name.xcframework"
if [ ! -f "$root/Info.plist" ]; then
    fail "$name.xcframework has no Info.plist"
fi

{
    echo "xcframework=$name"
    echo "bundle_id=$bundle_id"
    echo "version=$version"
    echo "deployment=$deployment"
    echo "xcode=$xcode"
    echo "resources=$resources"
} >"$report.tmp"

found=""
index=0
while identifier=$(value "AvailableLibraries.$index.LibraryIdentifier" "$root/Info.plist"); do
    entry="AvailableLibraries.$index"
    platform=$(value "$entry.SupportedPlatform" "$root/Info.plist" || true)
    if [ "$platform" != ios ]; then
        fail "library $identifier is for '$platform', want ios"
    fi
    variant=$(value "$entry.SupportedPlatformVariant" "$root/Info.plist" || true)
    case "$variant" in
        "")
            environment=device
            sdk=iphoneos
            code=2
            supported=iPhoneOS
            ;;
        simulator)
            environment=simulator
            sdk=iphonesimulator
            code=7
            supported=iPhoneSimulator
            ;;
        *) fail "library $identifier has variant '$variant', want device or simulator" ;;
    esac
    path=$(value "$entry.LibraryPath" "$root/Info.plist" || true)
    if [ "$path" != "$name.framework" ]; then
        fail "library $identifier has path '$path', want $name.framework"
    fi
    framework="$root/$identifier/$path"
    binary="$framework/$name"
    if [ ! -f "$binary" ]; then
        fail "library $identifier has no binary $path/$name"
    fi
    if ! archs=$(xcrun lipo -archs "$binary" 2>/dev/null); then
        fail "binary $identifier/$path/$name is not a Mach-O file"
    fi
    plist="$framework/Info.plist"
    if [ ! -f "$plist" ]; then
        fail "library $identifier has no $path/Info.plist"
    fi
    expect CFBundleIdentifier "$plist" "$bundle_id" "$identifier/$path"
    expect CFBundleExecutable "$plist" "$name" "$identifier/$path"
    expect CFBundlePackageType "$plist" FMWK "$identifier/$path"
    expect CFBundleShortVersionString "$plist" "$version" "$identifier/$path"
    expect MinimumOSVersion "$plist" "$deployment" "$identifier/$path"
    expect CFBundleSupportedPlatforms.0 "$plist" "$supported" "$identifier/$path"
    modulemap="$framework/Modules/module.modulemap"
    if [ ! -f "$modulemap" ]; then
        fail "library $identifier has no $path/Modules/module.modulemap"
    fi
    if ! grep -q "framework module $name " "$modulemap"; then
        fail "$identifier/$path/Modules/module.modulemap declares no framework module $name"
    fi
    for header in "$@"; do
        if [ ! -f "$framework/Headers/$header" ]; then
            fail "library $identifier misses public header Headers/$header"
        fi
    done
    for resource in $(printf '%s\n' "$resources" | tr ',' ' '); do
        if [ ! -f "$framework/$resource" ]; then
            fail "library $identifier misses resource $path/$resource"
        fi
    done
    listed=""
    position=0
    while arch=$(value "$entry.SupportedArchitectures.$position" "$root/Info.plist"); do
        cell="$environment/$arch"
        case " $found " in
            *" $cell "*) fail "slice $cell appears in more than one library" ;;
        esac
        found="$found $cell"
        listed="$listed $arch"
        case " $archs " in
            *" $arch "*) ;;
            *) fail "binary $identifier/$path/$name carries $archs, want $arch for $cell" ;;
        esac
        loads=$(xcrun otool -arch "$arch" -l "$binary" 2>/dev/null || true)
        platforms=$(printf '%s\n' "$loads" | awk '$1 == "platform" { print $2 }' | sort -u)
        if [ "$platforms" != "$code" ]; then
            fail "slice $cell declares build platform '$(printf '%s' "$platforms" | tr '\n' ' ')', want $code for $sdk"
        fi
        minos=$(printf '%s\n' "$loads" | awk '$1 == "minos" { print $2 }' | sort -u -t. -k1,1n -k2,2n -k3,3n | tail -n 1)
        if [ -z "$minos" ]; then
            fail "slice $cell declares no minimum iOS version"
        fi
        if above "$minos" "$deployment"; then
            fail "slice $cell needs iOS $minos, above the framework deployment $deployment"
        fi
        install=$(printf '%s\n' "$loads" | awk '$1 == "cmd" { command = $2 } command == "LC_ID_DYLIB" && $1 == "name" { print $2; exit }')
        if [ "$install" != "@rpath/$name.framework/$name" ]; then
            fail "slice $cell installs as '$install', want @rpath/$name.framework/$name"
        fi
        echo "slice=$cell library=$identifier platform=$sdk minos=$minos install_name=$install" >>"$report.tmp"
        position=$((position + 1))
    done
    if [ -z "$listed" ]; then
        fail "library $identifier lists no architectures"
    fi
    for arch in $archs; do
        case " $listed " in
            *" $arch "*) ;;
            *) fail "binary $identifier/$path/$name carries $arch, which library $identifier does not list" ;;
        esac
    done
    index=$((index + 1))
done
if [ "$index" -eq 0 ]; then
    fail "$name.xcframework lists no libraries"
fi
for want in $(printf '%s\n' "$slices" | tr ',' ' '); do
    case " $found " in
        *" $want "*) ;;
        *) fail "$name.xcframework misses the declared slice $want" ;;
    esac
done
for cell in $found; do
    case ",$slices," in
        *",$cell,"*) ;;
        *) fail "$name.xcframework carries the undeclared slice $cell" ;;
    esac
done

rm -rf "$work"
cp "$archive" "$out"
mv "$report.tmp" "$report"
