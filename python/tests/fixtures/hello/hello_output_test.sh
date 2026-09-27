#!/usr/bin/env bash
set -euo pipefail

source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

hello_bin="$(dx_realpath "$1")"

output="$("${hello_bin}")"
[[ "${output}" == "Hello, world!" ]] || {
  echo "unexpected hello output: ${output}" >&2
  exit 1
}
