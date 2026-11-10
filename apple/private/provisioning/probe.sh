#!/bin/sh
set -eu

out=$1
route=$2
host=$3
xcode=$4
sdk=$5
sdk_version=$6
deployment=$7
macos_floor=$8
compiler=$9
shift 9
endpoint=${1:-}

fail() {
  printf 'apple: %s\n' "$1" >&2
  exit 1
}

triple() {
  printf '%s\n' "$1" | awk -F. '{ printf "%d.%d.%d\n", $1, $2, $3 }'
}

below() {
  awk -v left="$1" -v right="$2" 'BEGIN {
    split(left, a, ".")
    split(right, b, ".")
    for (i = 1; i <= 3; i++) {
      if (a[i] + 0 != b[i] + 0) {
        exit !(a[i] + 0 < b[i] + 0)
      }
    }
    exit 1
  }'
}

system=$(uname -s)
arch=$(uname -m)
if [ "$system" != Darwin ]; then
  fail "probe ran on $system $arch, not a macOS executor; the $route route needs $host with Xcode $xcode"
fi
if [ "$host" = macos_arm64 ] && [ "$arch" != arm64 ]; then
  fail "executor is $arch, not the arm64 host $host"
fi

macos=$(sw_vers -productVersion)
macos_build=$(sw_vers -buildVersion)
if below "$macos" "$macos_floor"; then
  fail "executor runs macOS $macos, below $macos_floor"
fi

if ! license=$(xcodebuild -license check 2>&1); then
  fail "the Xcode license is not accepted on this executor; an administrator runs 'sudo xcodebuild -license accept' there: $license"
fi

version=$(xcodebuild -version) || fail "xcodebuild failed under DEVELOPER_DIR=${DEVELOPER_DIR:-unset}"
actual=$(printf '%s\n' "$version" | awk '/^Xcode / { print $2 }')
build=$(printf '%s\n' "$version" | awk '/^Build version / { print $3 }')
if [ "$(triple "$actual")" != "$(triple "$xcode")" ]; then
  fail "executor runs Xcode $actual ($build) but Bazel resolved $xcode"
fi
resolved_build=$(printf '%s\n' "$xcode" | awk -F. 'NF > 3 { print $4 }')
if [ -n "$resolved_build" ] && [ "$resolved_build" != "$build" ]; then
  fail "executor runs Xcode build $build but Bazel resolved $xcode"
fi

sdk_actual=$(xcrun --sdk "$sdk" --show-sdk-version 2>/dev/null) || fail "Xcode $actual has no $sdk SDK"
if [ "$(triple "$sdk_actual")" != "$(triple "$sdk_version")" ]; then
  fail "executor has the $sdk $sdk_actual SDK but Bazel resolved $sdk_version"
fi
sdk_build=$(xcrun --sdk "$sdk" --show-sdk-build-version)
sdk_path=$(xcrun --sdk "$sdk" --show-sdk-path)
clang=$(xcrun --sdk "$sdk" --find clang)

identity=$("$compiler" --version 2>&1 | head -n 1 || true)
case $identity in
  "Apple clang"*) ;;
  *) fail "the C/C++ toolchain compiler $compiler reports '$identity', not Xcode's Apple clang; register @local_config_apple_cc_toolchains//:all ahead of other C/C++ toolchains" ;;
esac

{
  printf 'route=%s\n' "$route"
  printf 'endpoint=%s\n' "$endpoint"
  printf 'host=%s\n' "$host"
  printf 'arch=%s\n' "$arch"
  printf 'macos=%s\n' "$macos"
  printf 'macos_build=%s\n' "$macos_build"
  printf 'xcode=%s\n' "$actual"
  printf 'xcode_build=%s\n' "$build"
  printf 'developer_dir=%s\n' "${DEVELOPER_DIR:-}"
  printf 'sdk=%s\n' "$sdk"
  printf 'sdk_version=%s\n' "$sdk_actual"
  printf 'sdk_build=%s\n' "$sdk_build"
  printf 'sdk_path=%s\n' "$sdk_path"
  printf 'deployment=%s\n' "$deployment"
  printf 'clang=%s\n' "$clang"
  printf 'compiler=%s\n' "$identity"
  printf 'license=accepted\n'
} >"$out"
