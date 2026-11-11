#!/usr/bin/env bash
# Usage: DX_JQ=<jq> lock_merge.sh <locks_dir> <out>
# <locks_dir> holds lock-<platform>/MODULE.bazel.lock for each lock-refresh.yml platform.
# Each platform lock contributes only the module extension entries keyed by its own factor.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: lock_merge.sh <locks_dir> <out>" >&2
  exit 2
fi
jq="${DX_JQ:?DX_JQ names the jq binary}"
locks="$1"
out="$2"

platforms=(linux_x86_64 linux_arm64 macos_arm64 windows_x86_64 windows_arm64)

factor_of() {
  case "$1" in
  linux_x86_64) echo "os:linux,arch:amd64" ;;
  linux_arm64) echo "os:linux,arch:aarch64" ;;
  macos_arm64) echo "os:osx,arch:aarch64" ;;
  windows_x86_64) echo "os:windows,arch:amd64" ;;
  windows_arm64) echo "os:windows,arch:aarch64" ;;
  *) return 1 ;;
  esac
}

for dir in "$locks"/*/; do
  [ -d "$dir" ] || continue
  name="$(basename "$dir")"
  if [[ "$name" != lock-* ]] || ! factor_of "${name#lock-}" >/dev/null; then
    echo "lock_merge.sh: unknown platform directory $name" >&2
    exit 1
  fi
done

factors='["general"'
for platform in "${platforms[@]}"; do
  lock="$locks/lock-$platform/MODULE.bazel.lock"
  if [ ! -f "$lock" ]; then
    echo "lock_merge.sh: missing $lock" >&2
    exit 1
  fi
  factors+=",\"$(factor_of "$platform")\""
done
factors+=']'

for platform in "${platforms[@]}"; do
  lock="$locks/lock-$platform/MODULE.bazel.lock"
  unknown="$("$jq" -r --argjson known "$factors" \
    '[.moduleExtensions // {} | .[] | keys[]] | unique | map(select(. as $k | $known | index($k) | not)) | join(" ")' "$lock")"
  if [ -n "$unknown" ]; then
    echo "lock_merge.sh: lock-$platform has extension keys that are not general or a platform factor: $unknown" >&2
    exit 1
  fi
done

merged="$(mktemp)"
next="$(mktemp)"
trap 'rm -f "$merged" "$next"' EXIT
cp "$locks/lock-${platforms[0]}/MODULE.bazel.lock" "$merged"
for platform in "${platforms[@]:1}"; do
  "$jq" --arg factor "$(factor_of "$platform")" \
    --slurpfile src "$locks/lock-$platform/MODULE.bazel.lock" \
    'reduce ($src[0].moduleExtensions // {} | to_entries[] | select(.value | has($factor))) as $e (.;
       .moduleExtensions[$e.key][$factor] = $e.value[$factor])
     | .facts *= ($src[0].facts // {})' "$merged" >"$next"
  cp "$next" "$merged"
done
cp "$merged" "$out"
