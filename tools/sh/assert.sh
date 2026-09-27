#!/usr/bin/env bash
set -euo pipefail

dx_assert_json_valid() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want valid JSON)"
    return 0
  fi
  if python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$file" >/dev/null 2>&1; then
    ok
  else
    bad "$file is not valid JSON"
  fi
  return 0
}

dx_assert_json_field() {
  local file="$1" path="$2" want="$3"
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want $path=$want)"
    return 0
  fi
  local got
  if ! got="$(python3 -c '
import json,sys
doc = json.load(open(sys.argv[1]))
node = doc
for part in sys.argv[2].split("."):
    if isinstance(node, list):
        node = node[int(part)]
    else:
        node = node[part]
if isinstance(node, (dict, list)):
    print(json.dumps(node, sort_keys=True))
else:
    print(str(node))
' "$file" "$path" 2>/dev/null)"; then
    bad "$file missing JSON path [$path] (want [$want])"
    return 0
  fi
  if [[ "$got" == "$want" ]]; then
    ok
  else
    bad "$file JSON path [$path] is [$got] (want [$want])"
  fi
  return 0
}

dx_assert_exit() {
  local got="$1" want="$2" label="$3"
  if [[ "$got" == "$want" ]]; then
    ok
  else
    bad "$label exit is [$got] (want [$want])"
  fi
  return 0
}

dx_assert_targets_contains() {
  local file="$1"
  shift
  local missing="" lit
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want targets: $*)"
    return 0
  fi
  for lit in "$@"; do
    if ! dx_hermetic_grep contains "$file" --fixed -- "$lit" >/dev/null 2>&1; then
      missing="$missing [$lit]"
    fi
  done
  if [[ -z "$missing" ]]; then
    ok
  else
    bad "$file missing targets:$missing"
  fi
  return 0
}

dx_assert_targets_absent() {
  local file="$1"
  shift
  local present="" lit
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want absence of targets: $*)"
    return 0
  fi
  for lit in "$@"; do
    if dx_hermetic_grep contains "$file" --fixed -- "$lit" >/dev/null 2>&1; then
      present="$present [$lit]"
    fi
  done
  if [[ -z "$present" ]]; then
    ok
  else
    bad "$file must not contain targets:$present"
  fi
  return 0
}

dx_assert_target_set() {
  local file="$1"
  shift
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want target set: $*)"
    return 0
  fi
  local got want
  got="$(python3 -c '
import re,sys
names = sorted(set(re.findall(r"name\s*=\s*\"([^\"]+)\"", open(sys.argv[1]).read())))
print("\n".join(names))
' "$file" 2>/dev/null || true)"
  want="$(printf '%s\n' "$@" | LC_ALL=C sort -u)"
  if [[ "$got" == "$want" ]]; then
    ok
  else
    bad "$file target set differs (got: $(printf '%s' "$got" | tr '\n' ' ') want: $(printf '%s' "$want" | tr '\n' ' '))"
  fi
  return 0
}
