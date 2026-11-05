#!/bin/bash
set -euo pipefail

ipa=""
for f in $1; do
  case "$f" in
  *.ipa) ipa="$f" ;;
  esac
done
[ -n "$ipa" ] || { echo "no .ipa in: $1" >&2; exit 1; }

dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
unzip -q "$ipa" -d "$dir"
app="$(echo "$dir"/Payload/*.app)"
plist="$app/Info.plist"

expect() {
  local what="$1" want="$2" got="$3"
  [ "$got" = "$want" ] || { echo "$what: want '$want', got '$got'" >&2; exit 1; }
}

expect CFBundleIdentifier "$2" "$(plutil -extract CFBundleIdentifier raw "$plist")"
expect MinimumOSVersion 17.0 "$(plutil -extract MinimumOSVersion raw "$plist")"
expect DTPlatformName "$3" "$(plutil -extract DTPlatformName raw "$plist")"
if [ "$3" = iphoneos ]; then
  [ -f "$app/embedded.mobileprovision" ] || { echo "embedded.mobileprovision missing" >&2; exit 1; }
fi

exe="$app/$(plutil -extract CFBundleExecutable raw "$plist")"
expect archs arm64 "$(lipo -archs "$exe")"
otool -l "$exe" | grep -Eq "^ +platform $4\$" || { echo "platform $4 not found" >&2; exit 1; }
nm "$exe" | grep -q '_ledger_total' || { echo "_ledger_total not found" >&2; exit 1; }
