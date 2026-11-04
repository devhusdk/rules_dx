#!/bin/sh
set -eu
readelf="$1"
binary="$2"

work="$TEST_TMPDIR/reloc dir"
mkdir -p "$work"
cp "$binary" "$work/ledger"
chmod +x "$work/ledger"

code=0
out=$(env -i "$work/ledger" tea=250 milk=120) || code=$?
[ "$code" = 0 ] || { echo "exit $code: $out" >&2; exit 1; }
case "$out" in
*"mode: strict"*"total: 370"*) ;;
*) echo "unexpected output: $out" >&2; exit 1 ;;
esac

if "$readelf" --dynamic "$work/ledger" | grep -E 'RUNPATH|RPATH'; then
    echo "binary has a RUNPATH" >&2
    exit 1
fi
if grep -a -E 'external/|execroot' "$work/ledger" >/dev/null; then
    echo "binary embeds a build path" >&2
    exit 1
fi
