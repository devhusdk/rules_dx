#!/usr/bin/env bash
set -euo pipefail
declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
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
