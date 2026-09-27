#!/usr/bin/env bash
set -euo pipefail

source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

gazelle="$(dx_realpath "$1")"
root="${TEST_TMPDIR}/workspace"
mkdir -p "${root}/crate/src" "${root}/crate/tests"
touch "${root}/WORKSPACE"
printf 'pub fn current() {}\n\n#[test]\nfn works() {}\n' >"${root}/crate/src/lib.rs"
printf 'fn main() {}\n' >"${root}/crate/src/main.rs"
printf '#[test]\nfn smoke() {}\n' >"${root}/crate/tests/smoke.rs"

(cd "${root}" && "${gazelle}" -repo_root="${root}")
(cd "${root}" && find . -name 'BUILD.bazel' | LC_ALL=C sort | while IFS= read -r f; do printf '%s  %s\n' "$(dx_sha256_file "$f")" "$f"; done | LC_ALL=C sort >"${TEST_TMPDIR}/first.sums")

(cd "${root}" && "${gazelle}" -repo_root="${root}")
(cd "${root}" && find . -name 'BUILD.bazel' | LC_ALL=C sort | while IFS= read -r f; do printf '%s  %s\n' "$(dx_sha256_file "$f")" "$f"; done | LC_ALL=C sort >"${TEST_TMPDIR}/second.sums")

if ! diff -u "${TEST_TMPDIR}/first.sums" "${TEST_TMPDIR}/second.sums"; then
  echo "repository generation is not idempotent" >&2
  exit 1
fi
