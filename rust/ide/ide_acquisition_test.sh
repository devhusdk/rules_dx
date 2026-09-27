#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

discover="$(dx_realpath "$1")"
flycheck="$(dx_realpath "$2")"

discover_help="$("${discover}" --help)"
case "${discover_help}" in
  *"TARGETS"*) ;;
  *)
    echo "gen_rust_project --help missing TARGETS usage" >&2
    echo "${discover_help}" >&2
    exit 1
    ;;
esac

flycheck_help="$("${flycheck}" --help)"
case "${flycheck_help}" in
  *"rust-analyzer flycheck wrapper backed by \`bazel build\`"*) ;;
  *)
    echo "flycheck --help missing wrapper usage" >&2
    echo "${flycheck_help}" >&2
    exit 1
    ;;
esac

echo "ide acquisition: gen_rust_project + flycheck answer --help"
