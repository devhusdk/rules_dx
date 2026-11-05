#!/bin/sh
set -eu
umask 077

XCRUN="${XCRUN:-xcrun}"
SECURITY="${SECURITY:-security}"
CODESIGN="${CODESIGN:-codesign}"
OPENSSL="${OPENSSL:-/usr/bin/openssl}"
PLUTIL="${PLUTIL:-plutil}"
unzip_tool="${DX_UNZIP:-unzip}"
zip_tool="${DX_ZIP:-}"
DITTO="${DITTO:-ditto}"

me=dx-apple-export

die() {
    code="$1"
    shift
    echo "$me: $*" >&2
    exit "$code"
}

need_tool() {
    command -v "$1" >/dev/null 2>&1 || die 127 "no $2 executable $1 on PATH; export needs a macOS host with Xcode"
}

need_tool "$XCRUN" xcrun
need_tool "$SECURITY" security
need_tool "$CODESIGN" codesign
need_tool "$OPENSSL" openssl
need_tool "$PLUTIL" plutil
need_tool "$unzip_tool" unzip
if [ -n "$zip_tool" ]; then
    need_tool "$zip_tool" zip
else
    need_tool "$DITTO" ditto
fi

usage="usage: bazel run <export> -- --certificate <identity.p12> --password-file <file> --out <App.ipa>"
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
[ -n "$cert" ] || die 2 "--certificate names the signing identity .p12 file; $usage"
[ -n "$password_file" ] || die 2 "--password-file names the file holding the .p12 password; $usage"
[ -n "$out" ] || die 2 "--out names the .ipa to write; $usage"

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
    *.ipa) ;;
    *) die 2 "--out $out must name an .ipa file" ;;
esac
out_dir="$(cd -P "$(dirname "$out")" 2>/dev/null && pwd -P)" || die 2 "--out directory $(dirname "$out") does not exist"
out="$out_dir/$(basename "$out")"
if [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -e "$BUILD_WORKSPACE_DIRECTORY/bazel-out" ]; then
    bazel_out="$(cd -P "$BUILD_WORKSPACE_DIRECTORY/bazel-out" && pwd -P)"
    output_base="${bazel_out%/execroot/*}"
    for root in "$bazel_out" "$output_base"; do
        case "$out_dir/" in
            "$root"/*) die 2 "--out $out is inside the Bazel output base; write the IPA outside bazel-out" ;;
        esac
    done
fi

meta="${DX_EXPORT_META:-}"
[ -n "$meta" ] && [ -f "$meta" ] || die 2 "DX_EXPORT_META names the export metadata of one device app"
field() {
    sed -n "s/^$1=//p" "$meta" | head -n 1
}
bundle_id="$(field bundle_id)"
ipa="$(field ipa)"
profile="$(field profile)"
[ -n "$bundle_id" ] || die 2 "$meta names no bundle_id"
[ -n "$ipa" ] || die 2 "$meta names no ipa"
[ -n "$profile" ] || die 2 "$meta names no profile"
ipa="$(absolute "$ipa" "$here")"
profile="$(absolute "$profile" "$here")"
[ -f "$ipa" ] || die 2 "built IPA $ipa is missing"
[ -f "$profile" ] || die 2 "declared profile $profile is missing"

work=""
keychain=""
keychains_saved=""
cleanup() {
    if [ -n "$keychains_saved" ]; then
        set --
        while IFS= read -r line; do
            line="$(printf '%s\n' "$line" | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//')"
            [ -n "$line" ] && set -- "$@" "$line"
        done <"$keychains_saved"
        "$SECURITY" list-keychains -d user -s "$@" >/dev/null 2>&1 || true
    fi
    if [ -n "$keychain" ]; then
        "$SECURITY" delete-keychain "$keychain" >/dev/null 2>&1 || true
    fi
    if [ -n "$work" ]; then
        rm -rf "$work"
    fi
}
trap 'status=$?; trap - EXIT; cleanup; exit "$status"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

work="$(mktemp -d "${TMPDIR:-/tmp}/dx-export.XXXXXX")"
unpacked="$work/unpacked"
mkdir "$unpacked"
"$unzip_tool" -q "$ipa" -d "$unpacked" || die 2 "cannot unpack $ipa"
app=""
for bundle in "$unpacked"/Payload/*.app; do
    [ -d "$bundle" ] || continue
    [ -z "$app" ] || die 2 "$ipa holds more than one Payload/*.app bundle"
    app="$bundle"
done
[ -n "$app" ] || die 2 "$ipa holds no Payload/*.app bundle"
[ ! -e "$app/PlugIns" ] || die 2 "$ipa carries app extensions; export signs plain apps only"

listing() {
    "$unzip_tool" -Z1 "$1" | grep -v '/$' | grep -v '/_CodeSignature/' | LC_ALL=C sort
}
listing "$ipa" >"$work/before.txt"

[ -f "$app/embedded.mobileprovision" ] || die 2 "$ipa embeds no provisioning profile"
cmp -s "$app/embedded.mobileprovision" "$profile" || die 2 "$ipa embeds a provisioning profile other than the declared $profile"

decoded="$work/profile.plist"
"$SECURITY" cms -D -i "$profile" >"$decoded" 2>/dev/null || die 2 "cannot decode the provisioning profile $profile"
value() {
    "$PLUTIL" -extract "$1" raw -o - "$2" 2>/dev/null
}
team="$(value TeamIdentifier.0 "$decoded")" || die 2 "provisioning profile $profile names no TeamIdentifier"
profile_name="$(value Name "$decoded" || true)"
profile_uuid="$(value UUID "$decoded" || true)"
expires="$(value ExpirationDate "$decoded")" || die 2 "provisioning profile $profile names no ExpirationDate"
now="${DX_NOW:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
digits() {
    printf '%s\n' "$1" | tr -cd '0-9' | cut -c1-14
}
expires_digits="$(digits "$expires")"
now_digits="$(digits "$now")"
[ "${#expires_digits}" -eq 14 ] && [ "${#now_digits}" -eq 14 ] || die 2 "cannot read the profile ExpirationDate '$expires'"
[ "$expires_digits" -gt "$now_digits" ] || die 2 "provisioning profile $profile expired on $expires"
app_identifier="$(value Entitlements.application-identifier "$decoded")" || die 2 "provisioning profile $profile names no application-identifier"
covered=""
case "$app_identifier" in
    *\*)
        case "$team.$bundle_id" in
            "${app_identifier%\*}"*) covered=1 ;;
        esac
        ;;
    *) [ "$app_identifier" != "$team.$bundle_id" ] || covered=1 ;;
esac
[ -n "$covered" ] || die 2 "provisioning profile $profile covers $app_identifier, not $team.$bundle_id"

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
"$PLUTIL" -extract Entitlements xml1 -o - "$decoded" >"$work/allowed.plist" 2>/dev/null || die 2 "provisioning profile $profile names no Entitlements"
keys "$work/allowed.plist" >"$work/allowed.txt"
entitlements=""
"$CODESIGN" -d --entitlements - --xml "$app" >"$work/entitlements.plist" 2>/dev/null || die 2 "cannot read the entitlements of $(basename "$app")"
if [ -s "$work/entitlements.plist" ]; then
    "$PLUTIL" -convert xml1 "$work/entitlements.plist" || die 2 "cannot read the entitlements of $(basename "$app")"
    entitlements="$work/entitlements.plist"
fi
: >"$work/keys.txt"
if [ -n "$entitlements" ]; then
    keys "$entitlements" >"$work/keys.txt"
fi
extra="$(LC_ALL=C comm -23 "$work/keys.txt" "$work/allowed.txt" | head -n 1)"
[ -z "$extra" ] || die 2 "entitlement '$extra' is not allowed by the provisioning profile $profile"

fingerprint() {
    "$OPENSSL" x509 "$@" -noout -fingerprint -sha1 2>/dev/null | sed 's/.*=//' | tr -d ':' | tr 'abcdef' 'ABCDEF'
}
"$OPENSSL" rand -hex 24 >"$work/transfer.pass"
"$OPENSSL" pkcs12 -in "$cert" -passin "file:$password_file" -nodes 2>/dev/null |
    "$OPENSSL" pkcs12 -export -passout "file:$work/transfer.pass" \
        -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
        -out "$work/identity.p12" 2>/dev/null ||
    die 2 "cannot read $cert with the password in --password-file"
[ -s "$work/identity.p12" ] || die 2 "cannot read $cert with the password in --password-file"
sha1="$("$OPENSSL" pkcs12 -in "$cert" -passin "file:$password_file" -nokeys -clcerts 2>/dev/null | fingerprint)"
[ -n "$sha1" ] || die 2 "$cert holds no signing certificate"
allowed=""
index=0
while encoded="$(value "DeveloperCertificates.$index" "$decoded")"; do
    if [ "$(printf '%s' "$encoded" | "$OPENSSL" base64 -d -A 2>/dev/null | fingerprint -inform DER)" = "$sha1" ]; then
        allowed=1
        break
    fi
    index=$((index + 1))
done
[ -n "$allowed" ] || die 2 "certificate $sha1 is not in the provisioning profile $profile"

"$SECURITY" list-keychains -d user >"$work/keychains.txt" || die 2 "cannot list the user keychains"
"$OPENSSL" rand -hex 24 >"$work/keychain.pass"
keychain_pass="$(cat "$work/keychain.pass")"
keychain="$work/export.keychain-db"
"$SECURITY" create-keychain -p "$keychain_pass" "$keychain" >/dev/null || die 2 "cannot create the export keychain"
"$SECURITY" set-keychain-settings "$keychain" >/dev/null || die 2 "cannot configure the export keychain"
"$SECURITY" unlock-keychain -p "$keychain_pass" "$keychain" >/dev/null || die 2 "cannot unlock the export keychain"
"$SECURITY" import "$work/identity.p12" -k "$keychain" -f pkcs12 -P "$(cat "$work/transfer.pass")" -T /usr/bin/codesign >/dev/null ||
    die 2 "cannot import $cert into the export keychain"
"$SECURITY" set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_pass" "$keychain" >/dev/null 2>&1 ||
    die 2 "cannot grant codesign access to the export keychain"
keychains_saved="$work/keychains.txt"
set -- "$keychain"
while IFS= read -r line; do
    line="$(printf '%s\n' "$line" | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//')"
    [ -n "$line" ] && set -- "$@" "$line"
done <"$keychains_saved"
"$SECURITY" list-keychains -d user -s "$@" >/dev/null || die 2 "cannot add the export keychain to the search list"
"$SECURITY" find-identity -v -p codesigning "$keychain" 2>/dev/null | grep -q "$sha1" ||
    die 2 "$cert holds no valid code signing identity $sha1"

if [ -d "$app/Frameworks" ]; then
    for nested in "$app"/Frameworks/*; do
        [ -e "$nested" ] || continue
        "$CODESIGN" --force --keychain "$keychain" --sign "$sha1" "$nested" 2>"$work/codesign.err" || {
            code=$?
            cat "$work/codesign.err" >&2
            exit "$code"
        }
    done
fi
set -- --force --keychain "$keychain" --sign "$sha1"
if [ -n "$entitlements" ]; then
    set -- "$@" --entitlements "$entitlements"
fi
"$CODESIGN" "$@" "$app" 2>"$work/codesign.err" || {
    code=$?
    cat "$work/codesign.err" >&2
    exit "$code"
}

signed="$work/signed.ipa"
if [ -n "$zip_tool" ]; then
    (cd "$unpacked" && "$zip_tool" -qry "$signed" Payload) || die 2 "cannot write the signed IPA"
else
    (cd "$unpacked" && "$DITTO" -c -k --sequesterRsrc --keepParent Payload "$signed") || die 2 "cannot write the signed IPA"
fi

"$CODESIGN" --verify --deep --strict "$app" >/dev/null 2>&1 || die 1 "signed $(basename "$app") fails codesign --verify --deep --strict"
details="$("$CODESIGN" -dvv "$app" 2>&1)" || die 1 "cannot read the signature of $(basename "$app")"
signed_team="$(printf '%s\n' "$details" | sed -n 's/^TeamIdentifier=//p' | head -n 1)"
authority="$(printf '%s\n' "$details" | sed -n 's/^Authority=//p' | head -n 1)"
[ "$signed_team" = "$team" ] || die 1 "signed $(basename "$app") carries TeamIdentifier '$signed_team', want $team"
[ -n "$authority" ] || die 1 "signed $(basename "$app") carries no signing authority"
executable="$(value CFBundleExecutable "$app/Info.plist")" || die 1 "$(basename "$app") names no CFBundleExecutable"
binary="$app/$executable"
archs="$("$XCRUN" lipo -archs "$binary" 2>/dev/null)" || die 1 "$executable is not a Mach-O file"
[ "$archs" = "arm64" ] || die 1 "$executable carries $archs, want arm64"
build="$("$XCRUN" otool -l "$binary" 2>/dev/null | awk '
    $1 == "cmd" { command = $2 }
    command == "LC_BUILD_VERSION" && $1 == "platform" && platform == "" { platform = $2 }
    command == "LC_BUILD_VERSION" && $1 == "minos" && minos == "" { minos = $2 }
    END { print platform, minos }
')"
platform="${build% *}"
minos="${build#* }"
[ "$platform" = "2" ] || die 1 "$executable is built for platform '$platform', want 2 (iphoneos)"
listing "$signed" >"$work/after.txt"
lost="$(LC_ALL=C comm -3 "$work/before.txt" "$work/after.txt" | sed 's/^[[:space:]]*//' | head -n 1)"
[ -z "$lost" ] || die 1 "signing changed the bundle contents at $lost"

entitlement_keys="$(tr '\n' ',' <"$work/keys.txt" | sed 's/,$//')"
mv -f "$signed" "$out"
{
    echo "bundle_id=$bundle_id"
    echo "team=$team"
    echo "profile_name=$profile_name"
    echo "profile_uuid=$profile_uuid"
    echo "profile_expires=$expires"
    echo "certificate_sha1=$sha1"
    echo "authority=$authority"
    echo "entitlements=$entitlement_keys"
    echo "archs=$archs"
    echo "platform=iphoneos"
    echo "minos=$minos"
} >"$out.txt"
echo "$me: wrote $out and $out.txt"
