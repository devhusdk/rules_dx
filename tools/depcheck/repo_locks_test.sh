#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init

depcheck="${1:?usage: repo_locks_test.sh <depcheck> <cargo-manifest> <cargo-lock> <uv-manifest> <uv-lock> <pnpm-manifest> <pnpm-lock> <go-manifest> <go-lock> <maven-artifacts> <maven-lock> <paket-manifest> <paket-lock> <ruby-manifest> <ruby-lock>}"

if "$depcheck" locks \
  --cargo-manifest "${2:?usage}" --cargo-lock "${3:?usage}" \
  --uv-manifest "${4:?usage}" --uv-lock "${5:?usage}" \
  --pnpm-manifest "${6:?usage}" --pnpm-lock "${7:?usage}" \
  --go-manifest "${8:?usage}" --go-lock "${9:?usage}" \
  --maven-artifacts "${10:?usage}" --maven-lock "${11:?usage}" \
  --paket-manifest "${12:?usage}" --paket-lock "${13:?usage}" \
  --ruby-manifest "${14:?usage}" --ruby-lock "${15:?usage}"; then
  ok "all workspace locks consistent"
else
  bad "workspace locks inconsistent (repin with bazel run //tools:repin-all)"
fi

dx_test_summary "repo locks"
