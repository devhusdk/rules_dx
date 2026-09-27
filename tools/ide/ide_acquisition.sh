#!/usr/bin/env bash
set -euo pipefail
source "${RUNFILES_DIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${TEST_SRCDIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$0.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"
bin="$(dx_realpath "$1")"
expected="$2"
shift 2
out="$("$bin" "$@" 2>&1)"
echo "$out"
case "$out" in
  *"$expected"*) ;;
  *) echo "ide acquisition missing $expected" >&2; echo "$out" >&2; exit 1;;
esac
echo "ide acquisition: $bin answers $*"
