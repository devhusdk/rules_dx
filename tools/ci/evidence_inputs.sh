#!/usr/bin/env bash
# Validates the evidence workflow dispatch inputs and appends them to GITHUB_OUTPUT.
# evidence_inputs_test checks it against evidence_inputs.rs, with and without jq.
set -euo pipefail
fail() { echo "evidence input rejected: $1"; exit 1; }
known=(linux_x86_64 linux_arm64 macos_arm64 windows_x86_64 windows_arm64)
name='[A-Za-z0-9_.+@=,/-]'

read -d '' -ra patterns <<< "${DX_SCOPE:-}" || true
[ "${#patterns[@]}" -gt 0 ] || fail "scope must name at least one target pattern"
for p in "${patterns[@]}"; do
  [[ "$p" =~ ^(@rules_dx_${name}*)?//${name}*(:(\*|${name}+))?$ ]] || fail "scope holds '$p', which is not a target pattern"
done

task=${DX_TASK:-}
case "$task" in test|build|coverage) ;; *) fail "task must be test, build or coverage, got '$task'" ;; esac

if [ -z "${DX_EVIDENCE_NO_JQ:-}" ] && command -v jq > /dev/null; then
  platforms=$(jq -sce --arg known "${known[*]}" '
    ($known | split(" ")) as $known
    | if length == 1 and (.[0] | type == "array" and length > 0 and all(type == "string")
        and (unique | length) == length and all(IN($known[])))
      then .[0] else error end' <<< "${DX_PLATFORMS:-}" 2> /dev/null) ||
    fail "platforms must be a JSON array of known platforms, each once"
else
  one="($(IFS='|'; echo "${known[*]}"))"
  re="^\[ *\"$one\" *(, *\"$one\" *)*\]$"
  [[ "${DX_PLATFORMS:-}" =~ $re ]] || fail "platforms must be a JSON array of known platforms"
  list=${DX_PLATFORMS//[\[\]\" ]/}
  IFS=, read -ra items <<< "$list"
  seen=" "
  for item in "${items[@]}"; do
    [[ "$seen" != *" $item "* ]] || fail "platforms lists '$item' twice"
    seen+="$item "
  done
  platforms="[\"${list//,/\",\"}\"]"
fi

case "${DX_DISPLAYS:-}" in ''|xcb|wayland|xcb,wayland|wayland,xcb) ;; *) fail "displays must be xcb, wayland or both" ;; esac
timeout=${DX_TEST_TIMEOUT:-}
[[ "$timeout" =~ ^[0-9]+$ ]] || fail "test_timeout must be whole seconds, got '$timeout'"

read -d '' -ra flags <<< "${DX_FLAGS:-}" || true
for f in ${flags[@]+"${flags[@]}"}; do
  [[ "$f" =~ ^--config=${name}+$ || "$f" =~ ^--//[A-Za-z0-9_.+@,/:-]+=${name}+$ ]] ||
    fail "flags holds '$f'; only --//label=value and --config=name are accepted"
done

case "${DX_RELOCATE:-}" in ''|false) relocate=false ;; true) relocate=true ;; *) fail "relocate must be true or false" ;; esac
[ "$relocate" = false ] || [ "$task" != coverage ] || fail "relocate supports test and build"

{
  echo "scope=${patterns[*]}"
  echo "task=$task"
  echo "test_timeout=$timeout"
  echo "flags=${flags[*]+${flags[*]}}"
  echo "relocate=$relocate"
} >> "$GITHUB_OUTPUT"
echo "evidence inputs accepted: $task ${patterns[*]} on $platforms"
