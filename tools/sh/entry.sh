#!/usr/bin/env bash
set -euo pipefail
source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$0.runfiles/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh" || { echo "entry: cannot locate tools/sh/bootstrap.sh" >&2; exit 1; }
dx_bootstrap "tools/sh/lib.sh"
payload="$(dx_resolve_runfile "${DX_HARNESS_SRC:?dx_shell_harness sets DX_HARNESS_SRC}")" || { echo "entry: cannot resolve payload ${DX_HARNESS_SRC:-}" >&2; exit 1; }
source "$payload" "$@"
