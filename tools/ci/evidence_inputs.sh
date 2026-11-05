#!/usr/bin/env bash
# Mirrors evidence_inputs.rs; evidence_inputs_test.rs keeps the two in agreement.
set -u
export LC_ALL=C

reject() {
  echo "evidence input rejected: $1"
  exit 1
}

name_chars='^[A-Za-z0-9_.+@=,/-]*$'

is_pattern() {
  local rest=$1 path target
  if [[ $rest == @* ]]; then
    rest=${rest#@}
    [[ $rest == *//* ]] || return 1
    local repo=${rest%%//*}
    [[ $repo == rules_dx_* && $repo =~ $name_chars ]] || return 1
    rest=${rest#*//}
  elif [[ $rest == //* ]]; then
    rest=${rest#//}
  else
    return 1
  fi
  path=${rest%%:*}
  [[ $path =~ $name_chars ]] || return 1
  [[ $rest == *:* ]] || return 0
  target=${rest#*:}
  [[ $target == "*" ]] && return 0
  [[ -n $target && $target =~ $name_chars ]]
}

set -f
patterns=(${DX_SCOPE-})
set +f
[ "${#patterns[@]}" -gt 0 ] || reject "scope must name at least one target pattern"
for pattern in "${patterns[@]}"; do
  is_pattern "$pattern" || reject "scope holds '$pattern', which is not a target pattern; write patterns such as //pkg/... or //pkg:target"
done
scope="${patterns[*]}"

task=${DX_TASK-}
case $task in
  test | build | coverage) ;;
  *) reject "task must be one of ['test', 'build', 'coverage'], got '$task'" ;;
esac

known='["linux_arm64","linux_x86_64","macos_arm64","windows_arm64","windows_x86_64"]'
platforms=$(jq -cs --argjson known "$known" '
  if length != 1 then error("platforms must be a JSON array") else .[0] end |
  if type != "array" then error("platforms must be a JSON array, got a JSON " + type)
  elif length == 0 then error("platforms must be a non-empty JSON array")
  elif any(.[]; type != "string") then error("platforms must hold strings")
  elif (. - $known | length) > 0 then error("unknown platforms " + tostring)
  elif length != (unique | length) then error("platforms lists a platform twice")
  else . end' <<<"${DX_PLATFORMS-}" 2>&1) || reject "$(sed -E 's/^jq: error[^:]*: //' <<<"$platforms")"

displays=${DX_DISPLAYS-}
case $displays in
  "" | xcb | wayland | "xcb,wayland" | "wayland,xcb") ;;
  *) reject "displays must be one of ['xcb', 'wayland', 'xcb,wayland', 'wayland,xcb'], got '$displays'" ;;
esac

timeout=${DX_TEST_TIMEOUT-}
[[ $timeout =~ ^[0-9]+$ ]] || reject "test_timeout must be whole seconds, got '$timeout'"

runners='{"linux_arm64":"ubuntu-24.04-arm","linux_x86_64":"ubuntu-latest","macos_arm64":"macos-15","windows_arm64":"windows-11-arm","windows_x86_64":"windows-latest"}'
{
  echo "scope=$scope"
  echo "task=$task"
  echo "platforms=$platforms"
  echo "runners=$runners"
  echo "test_timeout=$timeout"
} >>"$GITHUB_OUTPUT"
echo "evidence inputs accepted: $task $scope on $platforms"
