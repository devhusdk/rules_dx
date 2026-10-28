#!/bin/sh
set -eu
root="${TEST_SRCDIR:?}"
found=$(find -L "$root" -path '*ansible_collections/dxtest/tiny/MANIFEST.json' | head -n 1)
test -n "$found" || { echo "hub has no dxtest.tiny MANIFEST.json" >&2; exit 1; }
base=$(dirname "$found")
test -f "$base/plugins/modules/hello.py" || { echo "hub is missing hello.py" >&2; exit 1; }
