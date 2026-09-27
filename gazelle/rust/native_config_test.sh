#!/usr/bin/env bash
set -euo pipefail

source "${RUNFILES_DIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${TEST_SRCDIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$0.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

gazelle="$(dx_realpath "$1")"
root="${TEST_TMPDIR}/workspace"
mkdir -p "${root}/crate/src" "${root}/crate/styles/org"
touch "${root}/WORKSPACE"
printf 'pub fn current() {}\n' >"${root}/crate/src/lib.rs"
printf 'edition = "2021"\n' >"${root}/crate/rustfmt.toml"
printf '[formatting]\n' >"${root}/crate/taplo.toml"
printf 'StylesPath = styles\n' >"${root}/crate/.vale.ini"
printf 'extends: existence\n' >"${root}/crate/styles/org/Example.yml"

(cd "${root}" && "${gazelle}" -repo_root="${root}")
build="${root}/crate/BUILD.bazel"
for want in 'name = "rustfmt_config"' 'name = "taplo_config"' 'name = "vale_config"' 'aspect_hints = [' '":rustfmt_config"' 'data = ["crate/styles/org/Example.yml"]' '//crate:__subpackages__'; do
  if ! dx_hermetic_grep contains "${build}" --fixed -- "${want}" >/dev/null 2>&1; then
    echo "fresh generation missing ${want}" >&2
    cat "${build}" >&2
    exit 1
  fi
done
if dx_hermetic_grep contains "${build}" --fixed -- '":taplo_config"' >/dev/null 2>&1; then
  echo "non-binding tool leaked into aspect_hints" >&2
  exit 1
fi

printf '%s  %s\n' "$(dx_sha256_file "${build}")" "${build}" >"${TEST_TMPDIR}/first.sums"
(cd "${root}" && "${gazelle}" -repo_root="${root}")
dx_sha256_check "${TEST_TMPDIR}/first.sums" >/dev/null || {
  echo "rerun was not idempotent" >&2
  exit 1
}

rm "${root}/crate/rustfmt.toml"
(cd "${root}" && "${gazelle}" -repo_root="${root}")
if dx_hermetic_grep contains "${build}" --fixed -- "rustfmt_config" >/dev/null 2>&1; then
  echo "removed config file left its target behind" >&2
  exit 1
fi
if dx_hermetic_grep contains "${build}" --fixed -- "rustfmt" >/dev/null 2>&1; then
  echo "removed config file left a stale hint behind" >&2
  exit 1
fi
if dx_hermetic_grep contains "${build}" --fixed -- "aspect_hints" >/dev/null 2>&1; then
  echo "removed config file left a stale hint behind" >&2
  exit 1
fi
if ! dx_hermetic_grep contains "${build}" --fixed -- 'name = "taplo_config"' >/dev/null 2>&1; then
  echo "removal dropped an unrelated target" >&2
  exit 1
fi

rm -rf "${root:?}/crate"
mkdir -p "${root}/broken"
printf '# gazelle:dx_native_tools rustfmt bogus\n' >"${root}/broken/BUILD.bazel"
set +e
output="$(cd "${root}" && "${gazelle}" -repo_root="${root}" 2>&1)"
status=$?
set -e
if [[ ${status} -eq 0 || "${output}" != *'unknown native tool "bogus"'* ]]; then
  echo "unknown tool did not fail closed" >&2
  echo "${output}" >&2
  exit 1
fi
if [[ -e "${root}/broken/BUILD" ]]; then
  echo "failed generation wrote a BUILD file" >&2
  exit 1
fi

rm -rf "${root:?}/broken"
mkdir -p "${root}/ambiguous"
cat >"${root}/ambiguous/BUILD.bazel" <<'EOF'
load("@rules_dx//quality:native_config.bzl", "rustfmt_config")

rustfmt_config(
    name = "rustfmt_cfg",
    src = "custom.toml",
)

rustfmt_config(
    name = "rustfmt_extra",
    src = "extra.toml",
)
EOF
set +e
output="$(cd "${root}" && "${gazelle}" -repo_root="${root}" 2>&1)"
status=$?
set -e
if [[ ${status} -eq 0 || "${output}" != *'2 config targets'* ]]; then
  echo "ambiguous configs did not fail closed" >&2
  echo "${output}" >&2
  exit 1
fi
