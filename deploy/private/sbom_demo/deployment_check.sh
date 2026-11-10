#!/bin/sh
set -eu
found=$(find -L "${RUNFILES_DIR:-${TEST_SRCDIR:-.}}" -name app.spdx.json | head -n 1)
[ -n "$found" ] || { echo "app.spdx.json is not staged with the deployment" >&2; exit 1; }
