#!/bin/sh
set -u
bin="$1"
tmp="${TEST_TMPDIR:-/tmp}/registry_exit_codes"
mkdir -p "$tmp"
failures=0

check() {
  want_code="$1"
  want_err="$2"
  shift 2
  err=$("$bin" "$@" 2>&1 >/dev/null)
  code=$?
  if [ "$code" != "$want_code" ] || [ "$err" != "$want_err" ]; then
    echo "FAIL: $* -> $code '$err' (want $want_code '$want_err')"
    failures=$((failures + 1))
  fi
}

bad() {
  printf '%s\n' "$2" >"$tmp/$1.json"
  check 3 "registry: $3" --file "$tmp/$1.json" list
}

bad not_array '{}' "registry file must hold an array"
bad not_json 'this is not json {' "registry file is not valid JSON"
bad bad_version '[{"name":"core","versions":["1.0"]}]' "package core has an invalid version 1.0"
bad duplicate '[{"name":"core","versions":[]},{"name":"core","versions":[]}]' "duplicate package core"

check 4 "registry: no package absent" latest absent
check 4 "registry: no version of core satisfies >=9.0.0" satisfy core ">=9.0.0"
check 2 "registry: not a range is not a valid range" satisfy core "not a range"
check 3 "registry: cannot read registry file" --file "$tmp/absent.json" list

list=$("$bin" list)
if [ "$list" != "core 1.1.3
net 2.1.1
edge none" ]; then
  echo "FAIL: list -> '$list'"
  failures=$((failures + 1))
fi

where=$("$bin" where one "two words")
case "$where" in
*"/examples/node-app/cli/main.js"*) ;;
*)
  echo "FAIL: where entry -> '$where'"
  failures=$((failures + 1))
  ;;
esac
case "$where" in
*"/semver@7.6.3/node_modules/semver 7.6.3"*) ;;
*)
  echo "FAIL: where semver -> '$where'"
  failures=$((failures + 1))
  ;;
esac
case "$where" in
*"same true"*) ;;
*)
  echo "FAIL: where identity -> '$where'"
  failures=$((failures + 1))
  ;;
esac

"$bin" sell >/dev/null 2>&1
if [ "$?" != 2 ]; then
  echo "FAIL: unknown command did not exit 2"
  failures=$((failures + 1))
fi

exit "$failures"
