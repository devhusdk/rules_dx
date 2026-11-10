#!/bin/sh
set -eu

# Arguments, in order: identity zip archive name extension executable bundle_id
# version deployment arch xcode sdk_version frameworks plugins resources
# frameworks, plugins and resources are newline separated name lists.
identity="$1"
out="$2"
archive="$3"
name="$4"
extension="$5"
executable="$6"
bundle_id="$7"
version="$8"
deployment="$9"
arch="${10}"
xcode="${11}"
sdk_version="${12}"
frameworks="${13}"
plugins="${14}"
resources="${15}"

scratch="$identity.scratch"
tree="$scratch/tree"

fail() {
    rm -rf "$scratch" "$identity.tmp"
    echo "apple_macos_app: $*" >&2
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

esc() {
    printf '%s' "$1" | tr -d '[:cntrl:]' | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

array() {
    printf '['
    separator=""
    while IFS= read -r item; do
        if [ -n "$item" ]; then
            printf '%s"%s"' "$separator" "$(esc "$item")"
            separator=", "
        fi
    done <<EOF
$1
EOF
    printf ']'
}

rm -rf "$scratch"
mkdir -p "$tree"
if ! unzipped=$(unzip -q "$archive" -d "$tree" 2>&1 </dev/null); then
    fail "$(basename "$archive") does not unpack: $(printf '%s\n' "$unzipped" | tail -n 1)"
fi
root="$tree/$name$extension"
contents="$root/Contents"
if [ ! -d "$contents" ]; then
    fail "$name$extension has no Contents directory"
fi
plist="$contents/Info.plist"
if [ ! -f "$plist" ]; then
    fail "$name$extension has no Contents/Info.plist"
fi
expect CFBundleIdentifier "$plist" "$bundle_id" "$name$extension"
expect CFBundleExecutable "$plist" "$executable" "$name$extension"
expect CFBundleShortVersionString "$plist" "$version" "$name$extension"
expect CFBundleVersion "$plist" "$version" "$name$extension"
expect LSMinimumSystemVersion "$plist" "$deployment" "$name$extension"
expect CFBundlePackageType "$plist" APPL "$name$extension"
expect CFBundleSupportedPlatforms.0 "$plist" MacOSX "$name$extension"
if [ ! -f "$contents/MacOS/$executable" ]; then
    fail "$name$extension has no Contents/MacOS/$executable"
fi

while IFS= read -r framework; do
    if [ -z "$framework" ]; then
        continue
    fi
    bundle="$contents/Frameworks/$framework.framework"
    if [ ! -d "$bundle" ]; then
        fail "declared framework $framework.framework is missing from the bundle"
    fi
    found=""
    for base in "$bundle/Versions/Current" "$bundle"; do
        for candidate in "$base/Resources/Info.plist" "$base/Info.plist"; do
            if [ -z "$found" ] && [ -f "$candidate" ]; then
                found="$candidate"
                home="$base"
            fi
        done
    done
    if [ -z "$found" ]; then
        fail "framework $framework.framework has no Info.plist"
    fi
    inner=$(value CFBundleExecutable "$found" || true)
    if [ -z "$inner" ] || [ ! -f "$home/$inner" ]; then
        fail "framework $framework.framework has no executable '$inner'"
    fi
done <<EOF
$frameworks
EOF
while IFS= read -r plugin; do
    if [ -n "$plugin" ] && [ ! -e "$contents/PlugIns/$plugin" ]; then
        fail "declared plugin $plugin is missing from the bundle"
    fi
done <<EOF
$plugins
EOF
while IFS= read -r resource; do
    if [ -n "$resource" ] && [ ! -e "$contents/Resources/$resource" ]; then
        fail "declared resource $resource is missing from the bundle"
    fi
done <<EOF
$resources
EOF

find "$contents" -type f | sort >"$scratch/files"
: >"$scratch/rpaths"
: >"$scratch/loads"
: >"$scratch/minos"
while IFS= read -r file; do
    rel="$name$extension/${file#"$root"/}"
    if ! archs=$(xcrun lipo -archs "$file" 2>/dev/null </dev/null); then
        continue
    fi
    if [ "$archs" != "$arch" ]; then
        fail "$rel carries '$archs', want $arch"
    fi
    commands=$(xcrun otool -arch "$arch" -l "$file" 2>/dev/null </dev/null || true)
    platforms=$(printf '%s\n' "$commands" | awk '$1 == "platform" { print $2 }' | sort -u)
    if [ "$platforms" != 1 ]; then
        fail "$rel declares build platform '$(printf '%s' "$platforms" | tr '\n' ' ')', want 1 for macosx"
    fi
    minos=$(printf '%s\n' "$commands" | awk '$1 == "minos" { print $2 }' | sort -u -t. -k1,1n -k2,2n -k3,3n | tail -n 1)
    if [ -z "$minos" ]; then
        fail "$rel declares no minimum macOS version"
    fi
    if above "$minos" "$deployment"; then
        fail "$rel needs macOS $minos, above the app deployment $deployment"
    fi
    echo "$minos" >>"$scratch/minos"
    printf '%s\n' "$commands" | awk '$1 == "cmd" { command = $2 } command == "LC_RPATH" && $1 == "path" { print $2 }' >"$scratch/file_rpaths"
    while IFS= read -r rpath; do
        case "$rpath" in
            /*) fail "$rel has absolute rpath $rpath; use @executable_path or @loader_path" ;;
        esac
        echo "$rpath" >>"$scratch/rpaths"
    done <"$scratch/file_rpaths"
    printf '%s\n' "$commands" | awk '$1 == "cmd" { command = $2 } (command == "LC_LOAD_DYLIB" || command == "LC_LOAD_WEAK_DYLIB" || command == "LC_REEXPORT_DYLIB" || command == "LC_LOAD_UPWARD_DYLIB") && $1 == "name" { print $2 }' >"$scratch/file_loads"
    while IFS= read -r load; do
        case "$load" in
            /usr/lib/* | /System/Library/*) ;;
            @rpath/*)
                if [ ! -e "$contents/Frameworks/${load#@rpath/}" ]; then
                    fail "$rel loads $load, which is missing from the bundle"
                fi
                ;;
            @executable_path/*)
                if [ ! -e "$contents/MacOS/${load#@executable_path/}" ]; then
                    fail "$rel loads $load, which is missing from the bundle"
                fi
                ;;
            @loader_path/*)
                if [ ! -e "$(dirname "$file")/${load#@loader_path/}" ]; then
                    fail "$rel loads $load, which is missing from the bundle"
                fi
                ;;
            *) fail "$rel loads host runtime $load; embed it under Frameworks" ;;
        esac
        echo "$load" >>"$scratch/loads"
    done <"$scratch/file_loads"
done <"$scratch/files"
minos=$(sort -u -t. -k1,1n -k2,2n -k3,3n "$scratch/minos" | tail -n 1)
if [ -z "$minos" ]; then
    fail "$name$extension holds no Mach-O file"
fi

signed=$(codesign -dv "$root" 2>&1 </dev/null || true)
signature=$(printf '%s\n' "$signed" | awk -F= '$1 == "Signature" { print $2; exit }')
if [ "$signature" != adhoc ]; then
    fail "$name$extension has signature '${signature:-none}', want adhoc"
fi
sha=$(shasum -a 256 "$contents/MacOS/$executable" | awk '{ print $1 }')

{
    printf '%s\n' "{"
    printf '%s\n' "  \"identity\": {"
    printf '%s\n' "    \"bundle_id\": \"$(esc "$bundle_id")\","
    printf '%s\n' "    \"executable\": \"$(esc "$executable")\","
    printf '%s\n' "    \"version\": \"$(esc "$version")\","
    printf '%s\n' "    \"deployment\": \"$(esc "$deployment")\""
    printf '%s\n' "  },"
    printf '%s\n' "  \"target\": {"
    printf '%s\n' "    \"arch\": \"$(esc "$arch")\","
    printf '%s\n' "    \"platform\": \"macosx\","
    printf '%s\n' "    \"sdk_version\": \"$(esc "$sdk_version")\","
    printf '%s\n' "    \"xcode\": \"$(esc "$xcode")\","
    printf '%s\n' "    \"minos\": \"$(esc "$minos")\""
    printf '%s\n' "  },"
    printf '%s\n' "  \"linkage\": {"
    printf '%s\n' "    \"rpaths\": $(array "$(sort -u "$scratch/rpaths")"),"
    printf '%s\n' "    \"loads\": $(array "$(sort -u "$scratch/loads")")"
    printf '%s\n' "  },"
    printf '%s\n' "  \"contents\": {"
    printf '%s\n' "    \"frameworks\": $(array "$frameworks"),"
    printf '%s\n' "    \"plugins\": $(array "$plugins"),"
    printf '%s\n' "    \"resources\": $(array "$resources")"
    printf '%s\n' "  },"
    printf '%s\n' "  \"signing\": {"
    printf '%s\n' "    \"signature\": \"adhoc\","
    printf '%s\n' "    \"executable_sha256\": \"$(esc "$sha")\""
    printf '%s\n' "  }"
    printf '%s\n' "}"
} >"$identity.tmp"

rm -rf "$scratch"
cp "$archive" "$out"
mv "$identity.tmp" "$identity"
