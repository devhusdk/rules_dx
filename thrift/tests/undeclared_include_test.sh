#!/bin/bash
set -u
thrift="$(find -L "${RUNFILES_DIR:-$0.runfiles}" \( -name thrift_compiler -o -name thrift_compiler.exe \) -type f | head -n 1)"
work="$(mktemp -d)"
mkdir -p "$work/thrift/tests"
cp "$(find -L "${RUNFILES_DIR:-$0.runfiles}" -name undeclared.thrift | head -n 1)" "$work/thrift/tests/undeclared.thrift"
mkdir "$work/out"
if out="$("$thrift" --gen cpp -I "$work" -out "$work/out" "$work/thrift/tests/undeclared.thrift" 2>&1)"; then
  echo "thrift accepted an include outside the declared inputs" >&2
  exit 1
fi
case "$out" in
  *"Could not find include file"*) ;;
  *) echo "unexpected failure: $out" >&2; exit 1 ;;
esac
