#!/usr/bin/env bash
set -u
runner="$1"
tmp="${TEST_TMPDIR:-$(mktemp -d)}"
fake="$tmp/fake_tidy"
printf '#!/usr/bin/env bash\nfor a in "$@"; do echo "ARG:$a"; done\n' >"$fake"
chmod +x "$fake"
fail() { echo "FAIL: $1" >&2; exit 1; }

env DEVELOPER_DIR=/dev SDKROOT=/sdk "$runner" "$tmp/a.log" "$fake" -isysroot __BAZEL_XCODE_SDKROOT__ -F__BAZEL_XCODE_DEVELOPER_DIR__/x -I__BAZEL_EXECUTION_ROOT__/inc || fail "expansion exits 0"
grep -qx 'ARG:-isysroot' "$tmp/a.log" && grep -qx 'ARG:/sdk' "$tmp/a.log" || fail "sdkroot not expanded"
grep -qx 'ARG:-F/dev/x' "$tmp/a.log" || fail "developer dir not expanded"
grep -qx "ARG:-I$PWD/inc" "$tmp/a.log" || fail "execution root not expanded"

if env -u DEVELOPER_DIR -u SDKROOT "$runner" "$tmp/b.log" "$fake" -isysroot __BAZEL_XCODE_SDKROOT__ 2>/dev/null; then
  fail "unset env with a placeholder must fail"
fi
[[ ! -e "$tmp/b.log" ]] || fail "tidy must not run with a raw placeholder"

env -u DEVELOPER_DIR -u SDKROOT "$runner" "$tmp/c.log" "$fake" -DX=1 a.c || fail "pass-through exits 0"
grep -qx 'ARG:-DX=1' "$tmp/c.log" || fail "pass-through lost an argument"
echo PASS
