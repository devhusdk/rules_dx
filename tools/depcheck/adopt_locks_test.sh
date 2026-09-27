#!/usr/bin/env bash
set -euo pipefail

declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init

depcheck="${1:?usage: adopt_locks_test.sh <depcheck> <go-manifest> <go-lock> <ruby-manifest> <ruby-lock> <rust-manifest> <rust-lock> <js-manifest> <js-lock> <py-manifest> <py-lock> <pg-js-manifest> <pg-js-lock> <pg-py-manifest> <pg-py-lock> <pg-rust-manifest> <pg-rust-lock>}"

check_pair() { # ecosystem, manifest, lock, label
  local eco="$1" manifest="$2" lock="$3" label="$4"
  if "$depcheck" consistency --ecosystem "$eco" --manifest "$manifest" --lock "$lock" >/dev/null 2>&1; then
    ok "$label consistent"
  else
    bad "$label inconsistent (repin the arrival lock, see $label README)"
  fi
}

check_pair "go" "${2:?usage}" "${3:?usage}" "adopt-go go.mod plus go.sum"
check_pair "ruby" "${4:?usage}" "${5:?usage}" "adopt-ruby Gemfile plus Gemfile.lock"
check_pair "rust" "${6:?usage}" "${7:?usage}" "adopt-rust Cargo.toml plus Cargo.lock"
check_pair "js" "${8:?usage}" "${9:?usage}" "adopt-js-ts package.json plus pnpm-lock.yaml"
check_pair "python" "${10:?usage}" "${11:?usage}" "adopt-python pyproject.toml plus uv.lock"
check_pair "js" "${12:?usage}" "${13:?usage}" "adopt-polyglot package.json plus pnpm-lock.yaml"
check_pair "python" "${14:?usage}" "${15:?usage}" "adopt-polyglot pyproject.toml plus uv.lock"
check_pair "rust" "${16:?usage}" "${17:?usage}" "adopt-polyglot Cargo.toml plus Cargo.lock"

dx_mkscratch scratch
cp "${2:?usage}" "$scratch/go.mod"
cp "${3:?usage}" "$scratch/go.sum"
sed -E 's|github.com/google/go-cmp v0.6.0|github.com/google/go-cmp v0.7.0|' "$scratch/go.mod" >"$scratch/go.mod.dxtmp" && mv "$scratch/go.mod.dxtmp" "$scratch/go.mod"
if "$depcheck" consistency --ecosystem go --manifest "$scratch/go.mod" --lock "$scratch/go.sum" >/dev/null 2>&1; then
  bad "negative control broken: stale go.mod passed consistency (want failure)"
else
  ok "stale arrival lock fails closed"
fi

if "$depcheck" consistency --ecosystem go --manifest "${2:?usage}" --lock "$scratch/does-not-exist.sum" >/dev/null 2>&1; then
  bad "negative control broken: missing lock passed consistency (want failure)"
else
  ok "missing arrival lock fails closed"
fi

dx_test_summary "adopt locks"
