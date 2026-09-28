#!/usr/bin/env bash
set -euo pipefail
workspace="$(git rev-parse --show-toplevel)"
cd "$workspace"
echo "repin-all: deprecated wrapper for dx update (owns the 12-dialect repin table)"
exec bazel run //cli/cli:dx -- update "$@"
