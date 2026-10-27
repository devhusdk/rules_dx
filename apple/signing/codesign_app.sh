#!/bin/sh
set -eu
umask 077

XCRUN="${XCRUN:-xcrun}"
SECURITY="${SECURITY:-security}"
CODESIGN="${CODESIGN:-codesign}"
OPENSSL="${OPENSSL:-/usr/bin/openssl}"
PLUTIL="${PLUTIL:-plutil}"
DITTO="${DITTO:-ditto}"

me=dx-apple-sign

die() {
    code="$1"
    shift
    echo "$me: $*" >&2
    exit "$code"
}

need_tool() {
    command -v "$1" >/dev/null 2>&1 || die 127 "no $2 executable $1 on PATH; signing needs a macOS host with Xcode"
}

need_tool "$XCRUN" xcrun
need_tool "$SECURITY" security
need_tool "$CODESIGN" codesign
need_tool "$OPENSSL" openssl
need_tool "$PLUTIL" plutil
need_tool "$DITTO" ditto

usage="usage: bazel run <sign> -- --certificate <identity.p12> --password-file <file> --out <App.app>"
cert=""
password_file=""
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --certificate=*) cert="${1#*=}" ;;
        --password-file=*) password_file="${1#*=}" ;;
        --out=*) out="${1#*=}" ;;
        --certificate | --password-file | --out)
            [ $# -ge 2 ] || die 2 "$1 needs a value; $usage"
            case "$1" in
                --certificate) cert="$2" ;;
                --password-file) password_file="$2" ;;
                --out) out="$2" ;;
            esac
            shift
            ;;
        *) die 2 "unexpected argument; $usage" ;;
    esac
    shift
done
[ -n "$cert" ] || die 2 "--certificate names the Developer ID identity .p12 file; $usage"
[ -n "$password_file" ] || die 2 "--password-file names the file holding the .p12 password; $usage"
[ -n "$out" ] || die 2 "--out names the .app to write; $usage"

here="$(pwd -P)"
caller="${BUILD_WORKING_DIRECTORY:-$here}"
absolute() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$2" "$1" ;;
    esac
}
cert="$(absolute "$cert" "$caller")"
password_file="$(absolute "$password_file" "$caller")"
out="$(absolute "$out" "$caller")"
[ -f "$cert" ] || die 2 "--certificate $cert is not a file"
[ -f "$password_file" ] || die 2 "--password-file $password_file is not a file"
case "$out" in
    *.app) ;;
    *) die 2 "--out $out must name an .app bundle" ;;
esac
out_dir="$(cd -P "$(dirname "$out")" 2>/dev/null && pwd -P)" || die 2 "--out directory $(dirname "$out") does not exist"
out="$out_dir/$(basename "$out")"
if [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -e "$BUILD_WORKSPACE_DIRECTORY/bazel-out" ]; then
    bazel_out="$(cd -P "$BUILD_WORKSPACE_DIRECTORY/bazel-out" && pwd -P)"
    output_base="${bazel_out%/execroot/*}"
    for root in "$bazel_out" "$output_base"; do
        case "$out_dir/" in
            "$root"/*) die 2 "--out $out is inside the Bazel output base; write the app outside bazel-out" ;;
        esac
    done
fi

meta="${DX_SIGN_META:-}"
[ -n "$meta" ] && [ -f "$meta" ] || die 2 "DX_SIGN_META names the signing metadata of one macOS app"
field() {
    sed -n "s/^$1=//p" "$meta" | head -n 1
}
bundle_id="$(field bundle_id)"
archive="$(field archive)"
declared_entitlements="$(field entitlements)"
[ -n "$bundle_id" ] || die 2 "$meta names no bundle_id"
[ -n "$archive" ] || die 2 "$meta names no archive"
archive="$(absolute "$archive" "$here")"
[ -e "$archive" ] || die 2 "built app archive $archive is missing"
if [ -n "$declared_entitlements" ]; then
    declared_entitlements="$(absolute "$declared_entitlements" "$here")"
    [ -f "$declared_entitlements" ] || die 2 "declared entitlements $declared_entitlements are missing"
fi

work=""
keychain=""
keychains_saved=""

# Restores the user keychain search list and deletes the temporary keychain.
keychain_close() {
    if [ -n "$keychains_saved" ]; then
        set --
        while IFS= read -r line; do
            line="$(printf '%s\n' "$line" | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//')"
            [ -n "$line" ] && set -- "$@" "$line"
        done <"$keychains_saved"
        "$SECURITY" list-keychains -d user -s "$@" >/dev/null 2>&1 || true
        keychains_saved=""
    fi
    if [ -n "$keychain" ]; then
        "$SECURITY" delete-keychain "$keychain" >/dev/null 2>&1 || true
        keychain=""
    fi
}

cleanup() {
    keychain_close
    if [ -n "$work" ]; then
        rm -rf "$work"
    fi
}
trap 'status=$?; trap - EXIT; cleanup; exit "$status"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

work="$(mktemp -d "${TMPDIR:-/tmp}/dx-sign.XXXXXX")"
unpacked="$work/unpacked"
mkdir "$unpacked"
case "$archive" in
    *.app)
        [ -d "$archive" ] || die 2 "built app $archive is not a bundle directory"
        "$DITTO" "$archive" "$unpacked/$(basename "$archive")" || die 2 "cannot copy $archive"
        ;;
    *) "$DITTO" -x -k "$archive" "$unpacked" || die 2 "cannot unpack $archive" ;;
esac
app=""
for bundle in "$unpacked"/*.app; do
    [ -d "$bundle" ] || continue
    [ -z "$app" ] || die 2 "$archive holds more than one .app bundle"
    app="$bundle"
done
[ -n "$app" ] || die 2 "$archive holds no .app bundle"
name="$(basename "$app")"
for nested in PlugIns XPCServices Library/LoginItems; do
    [ ! -e "$app/Contents/$nested" ] || die 2 "$name carries Contents/$nested; signing nested PlugIns, XPCServices and LoginItems is not supported yet"
done
if [ -d "$app/Contents/Frameworks" ]; then
    for framework in "$app"/Contents/Frameworks/*; do
        [ -d "$framework" ] && [ ! -L "$framework" ] || continue
        inner="$(find "$framework" -mindepth 1 \( -name '*.app' -o -name '*.appex' -o -name '*.framework' -o -name '*.xpc' -o -name Helpers -o -name XPCServices -o -name PlugIns \) | head -n 1)"
        [ -z "$inner" ] || die 2 "$name carries nested code at ${inner#"$app"/}; signing code nested in a framework is not supported yet"
    done
fi

value() {
    "$PLUTIL" -extract "$1" raw -o - "$2" 2>/dev/null
}
keys() {
    awk '
        /<dict>/ { depth++; next }
        /<\/dict>/ { depth--; next }
        depth == 1 && /<key>/ {
            key = $0
            sub(/.*<key>/, "", key)
            sub(/<\/key>.*/, "", key)
            print key
        }
    ' "$1" | LC_ALL=C sort -u
}
entitlements=""
: >"$work/keys.txt"
if [ -n "$declared_entitlements" ]; then
    entitlements="$work/entitlements.plist"
    "$PLUTIL" -convert xml1 -o "$entitlements" "$declared_entitlements" >/dev/null 2>&1 ||
        die 2 "cannot read the entitlements $declared_entitlements"
    keys "$entitlements" >"$work/keys.txt"
    debuggable="$(awk '
        found { if ($0 ~ /<true\/>/) print "true"; exit }
        /<key>com\.apple\.security\.get-task-allow<\/key>/ {
            if ($0 ~ /<true\/>/) { print "true"; exit }
            found = 1
        }
    ' "$entitlements")"
    [ "$debuggable" != true ] || die 2 "entitlement 'com.apple.security.get-task-allow' is true; notarization rejects debuggable apps"
fi

fingerprint() {
    "$OPENSSL" x509 "$@" -noout -fingerprint -sha1 2>/dev/null | sed 's/.*=//' | tr -d ':' | tr 'abcdef' 'ABCDEF'
}

# Imports the .p12 identity into a temporary keychain on the user search list; sets keychain and sha1.
keychain_open() {
    "$OPENSSL" rand -hex 24 >"$work/transfer.pass"
    "$OPENSSL" pkcs12 -in "$cert" -passin "file:$password_file" -nodes 2>/dev/null |
        "$OPENSSL" pkcs12 -export -passout "file:$work/transfer.pass" \
            -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
            -out "$work/identity.p12" 2>/dev/null ||
        die 2 "cannot read $cert with the password in --password-file"
    [ -s "$work/identity.p12" ] || die 2 "cannot read $cert with the password in --password-file"
    sha1="$("$OPENSSL" pkcs12 -in "$cert" -passin "file:$password_file" -nokeys -clcerts 2>/dev/null | fingerprint)"
    [ -n "$sha1" ] || die 2 "$cert holds no signing certificate"

    "$SECURITY" list-keychains -d user >"$work/keychains.txt" || die 2 "cannot list the user keychains"
    "$OPENSSL" rand -hex 24 >"$work/keychain.pass"
    keychain_pass="$(cat "$work/keychain.pass")"
    keychain="$work/sign.keychain-db"
    "$SECURITY" create-keychain -p "$keychain_pass" "$keychain" >/dev/null || die 2 "cannot create the signing keychain"
    "$SECURITY" set-keychain-settings "$keychain" >/dev/null || die 2 "cannot configure the signing keychain"
    "$SECURITY" unlock-keychain -p "$keychain_pass" "$keychain" >/dev/null || die 2 "cannot unlock the signing keychain"
    "$SECURITY" import "$work/identity.p12" -k "$keychain" -f pkcs12 -P "$(cat "$work/transfer.pass")" -T /usr/bin/codesign >/dev/null ||
        die 2 "cannot import $cert into the signing keychain"
    "$SECURITY" set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_pass" "$keychain" >/dev/null 2>&1 ||
        die 2 "cannot grant codesign access to the signing keychain"
    keychains_saved="$work/keychains.txt"
    set -- "$keychain"
    while IFS= read -r line; do
        line="$(printf '%s\n' "$line" | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//')"
        [ -n "$line" ] && set -- "$@" "$line"
    done <"$keychains_saved"
    "$SECURITY" list-keychains -d user -s "$@" >/dev/null || die 2 "cannot add the signing keychain to the search list"
    "$SECURITY" find-identity -v -p codesigning "$keychain" 2>/dev/null | grep -q "$sha1" ||
        die 2 "$cert holds no valid code signing identity $sha1"
}

sha1=""
keychain_open

# Signs one path with the hardened runtime and a secure timestamp, never with --deep.
sign() {
    "$CODESIGN" --force --options runtime --timestamp --keychain "$keychain" --sign "$sha1" "$@" 2>"$work/codesign.err" || {
        code=$?
        cat "$work/codesign.err" >&2
        if grep -q 'timestamp service is not available' "$work/codesign.err"; then
            die 3 "the Apple timestamp service is not reachable; retry when online"
        fi
        exit "$code"
    }
}

if [ -d "$app/Contents/Frameworks" ]; then
    for nested in "$app"/Contents/Frameworks/*; do
        [ -e "$nested" ] && [ ! -L "$nested" ] || continue
        sign "$nested"
    done
fi
if [ -n "$entitlements" ]; then
    sign --entitlements "$entitlements" "$app"
else
    sign "$app"
fi

"$CODESIGN" --verify --deep --strict "$app" >/dev/null 2>&1 || die 1 "signed $name fails codesign --verify --deep --strict"
details="$("$CODESIGN" -dvv "$app" 2>&1)" || die 1 "cannot read the signature of $name"
team="$(printf '%s\n' "$details" | sed -n 's/^TeamIdentifier=//p' | head -n 1)"
authority="$(printf '%s\n' "$details" | sed -n 's/^Authority=//p' | head -n 1)"
timestamp="$(printf '%s\n' "$details" | sed -n 's/^Timestamp=//p' | head -n 1)"
printf '%s\n' "$details" | grep -q '^CodeDirectory .*flags=[^ ]*runtime' || die 1 "signed $name carries no hardened runtime flag"
[ -n "$authority" ] || die 1 "signed $name carries no signing authority"
[ -n "$timestamp" ] || die 1 "signed $name carries no secure timestamp"
executable="$(value CFBundleExecutable "$app/Contents/Info.plist")" || die 1 "$name names no CFBundleExecutable"
archs="$("$XCRUN" lipo -archs "$app/Contents/MacOS/$executable" 2>/dev/null)" || die 1 "$executable is not a Mach-O file"

entitlement_keys="$(tr '\n' ',' <"$work/keys.txt" | sed 's/,$//')"
rm -rf "$out" "$out.txt"
"$DITTO" "$app" "$out" || die 2 "cannot write $out"
{
    echo "bundle_id=$bundle_id"
    echo "team=$team"
    echo "authority=$authority"
    echo "certificate_sha1=$sha1"
    echo "timestamp=$timestamp"
    echo "entitlements=$entitlement_keys"
    echo "archs=$archs"
    echo "runtime=yes"
} >"$out.txt"
echo "$me: wrote $out and $out.txt"
