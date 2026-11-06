#!/usr/bin/env bash
set -euo pipefail
fail() { echo "evidence input rejected: $1"; exit 1; }
name='[A-Za-z0-9_.+@=,/-]'
read -ra patterns <<< "${DX_SCOPE:-}"
[ "${#patterns[@]}" -gt 0 ] || fail "scope must name at least one target pattern"
for p in "${patterns[@]}"; do
  [[ "$p" =~ ^(@rules_dx_${name}*)?//${name}*(:(\*|${name}+))?$ ]] || fail "scope holds '$p', which is not a target pattern"
done
case "${DX_TASK:-}" in test|build|coverage) ;; *) fail "task must be test, build or coverage, got '${DX_TASK:-}'" ;; esac
if ! command -v jq > /dev/null; then
  [[ "${DX_PLATFORMS:-}" =~ ^\[\"(linux_x86_64|linux_arm64|macos_arm64|windows_x86_64|windows_arm64)\"(,\"(linux_x86_64|linux_arm64|macos_arm64|windows_x86_64|windows_arm64)\")*\]$ ]] || fail "platforms must be a JSON array of known platforms"
  platforms="$DX_PLATFORMS"
else
platforms="$(jq -ce 'if type == "array" and length > 0 and all(type == "string") and (unique | length) == length and all(IN("linux_x86_64","linux_arm64","macos_arm64","windows_x86_64","windows_arm64")) then . else error end' <<< "${DX_PLATFORMS:-}" 2>/dev/null)" || fail "platforms must be a JSON array of known platforms, each once"
fi
case "${DX_DISPLAYS:-}" in ''|xcb|wayland|xcb,wayland|wayland,xcb) ;; *) fail "bad displays" ;; esac
[[ "${DX_TEST_TIMEOUT:-}" =~ ^[0-9]+$ ]] || fail "test_timeout must be whole seconds"
read -ra flags <<< "${DX_FLAGS:-}"
for f in ${flags[@]+"${flags[@]}"}; do
  [[ "$f" =~ ^--config=${name}+$ || "$f" =~ ^--//[A-Za-z0-9_.+@=,/:-]+=${name}+$ ]] || fail "flags holds '$f'"
done
case "${DX_RELOCATE:-}" in ''|false) relocate=false ;; true) relocate=true ;; *) fail "relocate must be true or false" ;; esac
[ "$relocate" = false ] || [ "$DX_TASK" != coverage ] || fail "relocate supports test and build"
{
  echo "scope=${patterns[*]}"
  echo "task=$DX_TASK"
  echo "platforms=$platforms"
  echo "test_timeout=$DX_TEST_TIMEOUT"
  echo "flags=${flags[*]+${flags[*]}}"
  echo "relocate=$relocate"
} >> "$GITHUB_OUTPUT"
echo "evidence inputs accepted: $DX_TASK ${patterns[*]} on $platforms"
