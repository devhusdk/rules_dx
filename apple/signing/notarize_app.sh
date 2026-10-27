#!/bin/sh
set -eu
umask 077

XCRUN="${XCRUN:-xcrun}"
CODESIGN="${CODESIGN:-codesign}"
PLUTIL="${PLUTIL:-plutil}"
DITTO="${DITTO:-ditto}"
SPCTL="${SPCTL:-spctl}"

me=dx-apple-notarize

die() {
    code="$1"
    shift
    echo "$me: $*" >&2
    exit "$code"
}

need_tool() {
    command -v "$1" >/dev/null 2>&1 || die 127 "no $2 executable $1 on PATH; notarization needs a macOS host with Xcode"
}

need_tool "$XCRUN" xcrun
need_tool "$CODESIGN" codesign
need_tool "$PLUTIL" plutil
need_tool "$DITTO" ditto
need_tool "$SPCTL" spctl

usage="usage: bazel run <notarize> -- --app <App.app> (--keychain-profile <name> | --key <AuthKey.p8> --key-id <id> --issuer <uuid>) [--timeout <duration>]"
app=""
profile=""
key=""
key_id=""
issuer=""
timeout="2h"
while [ $# -gt 0 ]; do
    case "$1" in
        --app=*) app="${1#*=}" ;;
        --keychain-profile=*) profile="${1#*=}" ;;
        --key=*) key="${1#*=}" ;;
        --key-id=*) key_id="${1#*=}" ;;
        --issuer=*) issuer="${1#*=}" ;;
        --timeout=*) timeout="${1#*=}" ;;
        --app | --keychain-profile | --key | --key-id | --issuer | --timeout)
            [ $# -ge 2 ] || die 2 "$1 needs a value; $usage"
            case "$1" in
                --app) app="$2" ;;
                --keychain-profile) profile="$2" ;;
                --key) key="$2" ;;
                --key-id) key_id="$2" ;;
                --issuer) issuer="$2" ;;
                --timeout) timeout="$2" ;;
            esac
            shift
            ;;
        *) die 2 "unexpected argument; $usage" ;;
    esac
    shift
done
[ -n "$app" ] || die 2 "--app names the signed .app to notarize; $usage"
[ -n "$timeout" ] || die 2 "--timeout needs a duration like 2h; $usage"
if [ -n "$profile" ]; then
    [ -z "$key$key_id$issuer" ] || die 2 "pass --keychain-profile or --key with --key-id and --issuer, not both; $usage"
elif [ -n "$key$key_id$issuer" ]; then
    [ -n "$key" ] && [ -n "$key_id" ] && [ -n "$issuer" ] || die 2 "--key, --key-id and --issuer go together; $usage"
else
    die 2 "pass --keychain-profile or --key with --key-id and --issuer; $usage"
fi

here="$(pwd -P)"
caller="${BUILD_WORKING_DIRECTORY:-$here}"
absolute() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$2" "$1" ;;
    esac
}
app="$(absolute "$app" "$caller")"
case "$app" in
    *.app) ;;
    *) die 2 "--app $app must name an .app bundle" ;;
esac
[ -d "$app" ] || die 2 "--app $app is not a bundle directory"
app_dir="$(cd -P "$(dirname "$app")" && pwd -P)"
app="$app_dir/$(basename "$app")"
if [ -n "$key" ]; then
    key="$(absolute "$key" "$caller")"
    [ -f "$key" ] || die 2 "--key $key is not a file"
fi
if [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -e "$BUILD_WORKSPACE_DIRECTORY/bazel-out" ]; then
    bazel_out="$(cd -P "$BUILD_WORKSPACE_DIRECTORY/bazel-out" && pwd -P)"
    output_base="${bazel_out%/execroot/*}"
    for root in "$bazel_out" "$output_base"; do
        case "$app_dir/" in
            "$root"/*) die 2 "--app $app is inside the Bazel output base; sign it to a path outside bazel-out first" ;;
        esac
    done
fi

meta="${DX_SIGN_META:-}"
[ -n "$meta" ] && [ -f "$meta" ] || die 2 "DX_SIGN_META names the signing metadata of one macOS app"
bundle_id="$(sed -n 's/^bundle_id=//p' "$meta" | head -n 1)"
[ -n "$bundle_id" ] || die 2 "$meta names no bundle_id"

name="$(basename "$app")"
value() {
    "$PLUTIL" -extract "$1" raw -o - "$2" 2>/dev/null
}
app_id="$(value CFBundleIdentifier "$app/Contents/Info.plist")" || die 2 "$name names no CFBundleIdentifier"
[ "$app_id" = "$bundle_id" ] || die 2 "$name is $app_id, not the declared $bundle_id"
details="$("$CODESIGN" -dv "$app" 2>&1)" || die 2 "$name is not signed; run apple_macos_sign first"
if printf '%s\n' "$details" | grep -q '^Signature=adhoc'; then
    die 2 "$name carries an ad hoc signature; run apple_macos_sign first"
fi
printf '%s\n' "$details" | grep -q '^CodeDirectory .*flags=[^ ]*runtime' ||
    die 2 "$name is not signed with the hardened runtime; run apple_macos_sign first"

work=""
cleanup() {
    if [ -n "$work" ]; then
        rm -rf "$work"
    fi
}
trap 'status=$?; trap - EXIT; cleanup; exit "$status"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
work="$(mktemp -d "${TMPDIR:-/tmp}/dx-notarize.XXXXXX")"

zip="$work/${name%.app}.zip"
"$DITTO" -c -k --keepParent "$app" "$zip" || die 2 "cannot zip $app"

if [ -n "$profile" ]; then
    set -- --keychain-profile "$profile"
else
    set -- --key "$key" --key-id "$key_id" --issuer "$issuer"
fi

submitted=0
"$XCRUN" notarytool submit "$zip" "$@" --wait --timeout "$timeout" --output-format json \
    >"$work/submit.json" 2>"$work/submit.err" || submitted=$?
id="$(value id "$work/submit.json" || true)"
status="$(value status "$work/submit.json" || true)"
if [ -z "$id" ]; then
    cat "$work/submit.err" >&2
    if grep -Eq 'NSURLErrorDomain|appears to be offline' "$work/submit.err"; then
        die 3 "the Apple notary service is not reachable; retry when online"
    fi
    [ "$submitted" -ne 0 ] || submitted=1
    echo "$me: notarytool submit returned no submission id" >&2
    exit "$submitted"
fi

log="$app.notary.json"
rm -f "$log"
if ! "$XCRUN" notarytool log "$id" "$log" "$@" >/dev/null 2>"$work/log.err"; then
    echo "$me: cannot fetch the notary log of submission $id" >&2
fi

case "$status" in
    Accepted) ;;
    "In Progress") die 3 "submission $id is still In Progress after --timeout $timeout; check it with xcrun notarytool info $id" ;;
    Invalid | Rejected) die 1 "submission $id of $name is $status; see the notary log $log" ;;
    *) die 1 "submission $id of $name ended with status '$status'; see the notary log $log" ;;
esac

"$XCRUN" stapler staple "$app" >"$work/staple.out" 2>&1 || {
    cat "$work/staple.out" >&2
    die 1 "cannot staple the notarization ticket to $name"
}
"$XCRUN" stapler validate "$app" >"$work/validate.out" 2>&1 || {
    cat "$work/validate.out" >&2
    die 1 "stapled $name fails stapler validate"
}
"$SPCTL" --assess --type execute -vv "$app" >"$work/spctl.out" 2>&1 || {
    cat "$work/spctl.out" >&2
    die 1 "stapled $name fails spctl --assess --type execute"
}
echo "$me: notarized and stapled $app (submission $id); notary log at $log"
