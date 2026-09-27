#!/usr/bin/env bash
set -euo pipefail

source "${RUNFILES_DIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${TEST_SRCDIR:-/dev/null}/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$0.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "${BASH_SOURCE[0]}.runfiles/_main/tools/sh/bootstrap.sh" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/snapshot.sh"

dx_bootstrap "tools/sh/lib.sh"
dx_bootstrap "tools/sh/guards.sh"

expected="$(dx_realpath "$1")"
dx_bin="$(dx_realpath "$2")"
preset_rs="$(dx_realpath "$3")"

dx_test_init

dx_guards_contains "$expected" "preset.bazelrc missing contract lines" \
  'GENERATED, do not edit' \
  'Regenerate: `bazel run //tools/bazelrc:preset_update`' \
  'common --enable_bzlmod' \
  'build --verbose_failures' \
  'test --test_output=errors' \
  'common --enable_platform_specific_config' \
  'coverage --test_env=GENERATE_LLVM_LCOV=1' \
  'coverage --combined_report=lcov' \
  'coverage --test_tag_filters=-no-coverage' \
  'coverage --enable_runfiles' \
  'coverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov' \
  'coverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov' \
  'coverage --instrumentation_filter=^//' \
   'build:dx_debug --compilation_mode=dbg' \
   'build:dx_dev --compilation_mode=fastbuild' \
   'build:dx_release --compilation_mode=opt' \
   'build:dx_dev_remote --compilation_mode=fastbuild' \
   'build:dx_toolchain --compilation_mode=fastbuild' \
   'build:windows --enable_runfiles'
echo "preset schema: checked-in fragment carries the pinned contract"

dx_guards_contains "$preset_rs" "preset src lost its Bazel/dx version pins" \
  'PRESET_BAZEL_VERSION' \
  'PRESET_DX_VERSION' \
  '"9.2.0"' \
  '"0.0.0"'

dx_mkscratch scratch

touch "$scratch/MODULE.bazel"
printf '%s\n' \
  "import %workspace%/tools/bazelrc/preset.bazelrc" \
  "try-import %workspace%/user.bazelrc" \
  >"$scratch/.bazelrc"

if "$dx_bin" --workspace "$scratch" update go --quiet >/dev/null 2>&1; then
  ok
else
  bad "dx update go failed in scratch workspace"
fi

actual="$scratch/tools/bazelrc/preset.bazelrc"
if [[ ! -f "$actual" ]]; then
  bad "dx update did not create tools/bazelrc/preset.bazelrc"
fi

snapshot_diff "$expected" "$actual" "tools/bazelrc/preset.bazelrc"
echo "preset parity: Rust renderer output matches checked-in definition (snapshot)"

if "$dx_bin" --workspace "$scratch" update --check --quiet >/dev/null 2>&1; then
  ok
else
  bad "dx update --check failed on the fresh fragment"
fi

printf '%s\n' "# dirty" >"$actual"
if "$dx_bin" --workspace "$scratch" update --check --quiet >/dev/null 2>&1; then
  bad "dx update --check passed on a dirty fragment"
else
  ok
fi
if dx_hermetic_grep contains "$actual" --fixed -- "# dirty" >/dev/null 2>&1; then
  ok
else
  bad "check mode mutated the dirty fragment (check must never write)"
fi

if "$dx_bin" --workspace "$scratch" update go --quiet >/dev/null 2>&1; then
  ok
else
  bad "dx update failed to fix the dirty fragment"
fi
if "$dx_bin" --workspace "$scratch" update --check --quiet >/dev/null 2>&1; then
  ok
else
  bad "dx update --check failed after the fix"
fi

dx_test_summary "preset parity"
