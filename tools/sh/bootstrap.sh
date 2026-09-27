#!/usr/bin/env bash
set -euo pipefail

_dx_git_toplevel() {
  local budget="${DX_BOOTSTRAP_TIMEOUT:-5}"
  if command -v timeout >/dev/null 2>&1; then
    timeout "$budget" git rev-parse --show-toplevel 2>/dev/null || true
  else
    git rev-parse --show-toplevel 2>/dev/null || true
  fi
}

dx_bootstrap() {
  local rel="${1:-}" top="" caller="" caller_dir="" dir="" parent=""
  if [[ -z "$rel" ]]; then
    echo "dx_bootstrap: want workspace-relative path (e.g. tools/sh/lib.sh)" >&2
    return 1
  fi
  source "${RUNFILES_DIR:-/dev/null}/_main/${rel}" 2>/dev/null && return 0 || true
  source "${TEST_SRCDIR:-/dev/null}/_main/${rel}" 2>/dev/null && return 0 || true
  source "$0.runfiles/_main/${rel}" 2>/dev/null && return 0 || true
  source "${BASH_SOURCE[0]}.runfiles/_main/${rel}" 2>/dev/null && return 0 || true
  caller="${BASH_SOURCE[1]:-${BASH_SOURCE[0]:-$0}}"
  source "${caller}.runfiles/_main/${rel}" 2>/dev/null && return 0 || true
  source "${RUNFILES_DIR:-/dev/null}/${rel}" 2>/dev/null && return 0 || true
  source "${TEST_SRCDIR:-/dev/null}/${rel}" 2>/dev/null && return 0 || true
  source "$0.runfiles/${rel}" 2>/dev/null && return 0 || true
  source "${BASH_SOURCE[0]}.runfiles/${rel}" 2>/dev/null && return 0 || true
  source "${caller}.runfiles/${rel}" 2>/dev/null && return 0 || true
  if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" && -f "${BUILD_WORKSPACE_DIRECTORY}/${rel}" ]]; then
    source "${BUILD_WORKSPACE_DIRECTORY}/${rel}" && return 0 || return 1
  fi
  top="$(_dx_git_toplevel)"
  if [[ -n "$top" && -f "$top/${rel}" ]]; then
    source "$top/${rel}" && return 0 || return 1
  fi
  caller_dir="$(dirname "$caller")"
  dir="$caller_dir"
  while true; do
    if [[ -f "$dir/${rel}" ]]; then
      source "$dir/${rel}" && return 0 || return 1
    fi
    if [[ "$dir" == "/" || "$dir" == "." ]]; then
      break
    fi
    parent="$(dirname "$dir")"
    if [[ "$parent" == "$dir" ]]; then
      break
    fi
    dir="$parent"
  done
  echo "dx_bootstrap: cannot locate ${rel} (runfiles plus BUILD_WORKSPACE_DIRECTORY plus git top-level plus source-tree walk failed)" >&2
  return 1
}
