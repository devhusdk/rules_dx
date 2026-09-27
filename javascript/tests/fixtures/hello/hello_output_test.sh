#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

hello_bin="$(dx_realpath "$1")"

output="$("${hello_bin}")"
[[ "${output}" == "hello world" ]] || {
  echo "unexpected hello output: ${output}" >&2
  exit 1
}
