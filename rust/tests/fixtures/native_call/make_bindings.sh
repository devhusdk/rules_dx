#!/bin/sh
set -eu
header="$1"
out="$2"
shift 2
for symbol in "$@"; do
  if ! grep -q "^int32_t ${symbol}(" "$header"; then
    echo "make_bindings: ${header} does not declare ${symbol}" >&2
    exit 1
  fi
done
{
  echo "pub const BINDING_VERSION: u32 = 1;"
  echo ""
  echo 'extern "C" {'
  grep '^int32_t [A-Za-z_]' "$header" | sed \
    -e 's/^int32_t \([A-Za-z_][A-Za-z0-9_]*\)(int32_t \([A-Za-z_][A-Za-z0-9_]*\), int32_t \([A-Za-z_][A-Za-z0-9_]*\), int32_t *\* *\([A-Za-z_][A-Za-z0-9_]*\));$/    pub fn \1(\2: i32, \3: i32, \4: *mut i32) -> i32;/' \
    -e 's/^int32_t \([A-Za-z_][A-Za-z0-9_]*\)(int32_t \([A-Za-z_][A-Za-z0-9_]*\), int32_t \([A-Za-z_][A-Za-z0-9_]*\));$/    pub fn \1(\2: i32, \3: i32) -> i32;/' \
    -e 's/^int32_t \([A-Za-z_][A-Za-z0-9_]*\)(void);$/    pub fn \1() -> i32;/'
  echo "}"
} > "$out"
if grep -q 'int32_t' "$out"; then
  echo "make_bindings: ${header} has a declaration with unsupported shape" >&2
  exit 1
fi
