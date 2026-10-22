#!/bin/sh
set -u
bin="$1"
tmp="${TEST_TMPDIR:-/tmp}/invoice_exit_codes"
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
  check 3 "invoice: $3" --file "$tmp/$1.json" list
}

range="quantity must be a whole number from 0 to 1000000"
bad negative '[{"id":"A","customer":"c","items":[{"sku":"s","quantity":-1,"cents":5}]}]' "$range"
bad fractional '[{"id":"A","customer":"c","items":[{"sku":"s","quantity":1.5,"cents":5}]}]' "$range"
bad wrong_type '[{"id":"A","customer":"c","items":[{"sku":"s","quantity":"2","cents":5}]}]' "quantity must be a number"
bad not_array '{}' "invoice file must hold an array"
bad not_json 'this is not json {' "invoice file is not valid JSON"
bad single_quoted "[{'id':'A','customer':'c','items':[]}]" "invoice file is not valid JSON"

check 4 "invoice: no invoice INV-9" show INV-9
check 3 "invoice: cannot read invoice file" --file "$tmp/absent.json" list

total=$("$bin" total)
if [ "$total" != "total 68.00" ]; then
  echo "FAIL: total -> '$total'"
  failures=$((failures + 1))
fi

"$bin" sell >/dev/null 2>&1
if [ "$?" != 2 ]; then
  echo "FAIL: unknown command did not exit 2"
  failures=$((failures + 1))
fi

exit "$failures"
