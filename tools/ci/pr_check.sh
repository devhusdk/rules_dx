#!/usr/bin/env bash
# Analysis-only checks for a pull request on linux_x86_64. Usage: pr_check.sh <set> [bazel flags...]
set -euo pipefail

set_name="${1:?usage: pr_check.sh <workflow|preflight|preflight-windows-arm64|starter|hermetic|docs|families|cli|all> [bazel flags...]}"
shift

workflow() {
  bazel test "$@" //tools/ci:actionlint_test //tools/ci:workflow_expressions_test \
    //tools/ci:setup_bazel_test //tools/ci:secret_flags_test //tools/ci:workflow_pins_test
}

PREFLIGHT_TARGETS='//cli/cli:dx //cc/tests/fixtures/hello:hello //cc/tests/fixtures/c-app/store:store_build_h
    //tools/testing:preflight_smoke_test //cli/env:bootstrap_test //cc/tests/fixtures/hello:hello_test
    @rules_dx_powershell//tests/fixtures/hello:hello_test //zig/tests/fixtures/zig-app/... //dart/tests/fixtures/dart-app/...
    //dart/tests/runtime:all //dart/tools:all //zig/tests/fixtures/hello:all //julia/tests/fixtures/hello:all
    //julia/tests/fixtures/julia-app/... //julia/tools/... //deploy/rules:pypi_probe_verify
    //quality/testdata:matrix_python_format_pass //quality/testdata:matrix_jupyter_format_pass //quality/testdata:matrix_jupyter_lint_fail //quality/testdata:matrix_jupyter_outputs_fail //quality/testdata:matrix_javascript_format_pass //quality/testdata:matrix_api_spec_openapi_pass //quality/testdata:matrix_yaml_format_pass //quality/testdata:matrix_yaml_format_fail //quality/testdata:matrix_yaml_format_syntax_error //quality/testdata:matrix_yaml_lint_pass //quality/testdata:matrix_yaml_lint_fail //quality/testdata:matrix_yaml_lint_syntax_error //quality/testdata:matrix_flatbuffers_lint_pass //quality/testdata:matrix_capnp_lint_pass //quality/testdata:matrix_thrift_lint_pass //quality/testdata:matrix_thrift_lint_warning_fail //quality/testdata:matrix_thrift_lint_syntax_error //quality/testdata:matrix_thrift_lint_semantic_fail //quality/testdata:matrix_thrift_lint_include_undeclared //quality/testdata:matrix_thrift_typecheck_pass //quality/testdata:matrix_thrift_typecheck_semantic_fail //quality/testdata:matrix_thrift_typecheck_warning_ignored //quality/testdata:matrix_avro_lint_avsc_pass //quality/testdata:matrix_avro_lint_avsc_undefined_name //quality/testdata:matrix_avro_lint_avsc_syntax_error //quality/testdata:matrix_avro_lint_avdl_import_pass //quality/testdata:matrix_avro_lint_avdl_syntax_error //quality/testdata:matrix_avro_typecheck_avsc_pass //quality/testdata:matrix_avro_typecheck_avsc_undefined_name'

preflight() {
  # shellcheck disable=SC2086
  bazel build --nobuild --lockfile_mode=error "$@" $PREFLIGHT_TARGETS
}

# shellcheck disable=SC2034
# The labels in WINDOWS_ARM64_BLOCKED are left out; pr_check_test fails when a preflight label is in neither place.
# Toolchain resolution for windows_arm64 only shows up when that platform is the target.
# #7384: runner_matrix_windows_arm64 still fails: no erlang toolchain for windows_arm64 (//erlang/tools:dialyzer_bin)
WINDOWS_ARM64_BLOCKED='//quality/testdata:runner_matrix_windows_arm64'
preflight-windows-arm64() {
  bazel build --nobuild --skip_incompatible_explicit_targets --lockfile_mode=error "$@" \
    --platforms=@llvm//platforms:windows_aarch64 \
    //cc/tests/fixtures/hello:hello //cc/tests/fixtures/hello:hello_test @rules_dx_powershell//tests/fixtures/hello:hello_test \
    //cc/tests/fixtures/c-app/store:store_build_h \
    //dart/tests/fixtures/dart-app/... //dart/tests/runtime:all //dart/tools:all \
    //cli/cli:dx //tools/testing:preflight_smoke_test //cli/env:bootstrap_test //deploy/rules:pypi_probe_verify \
    //zig/tests/fixtures/hello:all //zig/tests/fixtures/zig-app/... //julia/tests/fixtures/hello:all \
    //julia/tests/fixtures/julia-app/... //julia/tools/...
}

starter() {
  local targets
  targets="$(bazel query "$@" 'attr(tags, starter-consumer, //...)' --output=label)"
  test -n "$targets"
  # shellcheck disable=SC2086
  bazel build --nobuild "$@" $targets
  bazel test "$@" //tools/testing/starters:starter_goldens_test
}

hermetic() {
  local targets
  targets="$(bazel query --lockfile_mode=error "$@" 'attr(tags, hermetic-consumer, //...) except filter("_upstream$", //...) except filter("_msvc$", //...)' --output=label)"
  test -n "$targets"
  # shellcheck disable=SC2086
  bazel build --nobuild --lockfile_mode=error "$@" //tools/testing/hermetic:isolation_probe_test //tools/testing/hermetic:consumer_repos_test $targets
}

docs() {
  tools/dx lint //docs/... //tools/docs/... -- --nobuild "$@"
  bazel build "$@" //tools/docs/site:user_api
}

families() {
  local dir
  for dir in families/*/; do
    [ -f "$dir/MODULE.bazel" ] || continue
    echo "::group::$dir"
    (cd "$dir" && ../../tools/dx build //... -- --nobuild "$@")
    echo "::endgroup::"
  done
}

cli() {
  bazel test "$@" //cli/cli:dx_cli_test
}

case "$set_name" in
  workflow | preflight | preflight-windows-arm64 | starter | hermetic | docs | families | cli) "$set_name" "$@" ;;
  all)
    for each in workflow preflight preflight-windows-arm64 starter hermetic docs families cli; do
      echo "::group::$each"
      "$each" "$@"
      echo "::endgroup::"
    done
    ;;
  *)
    echo "unknown set: $set_name" >&2
    exit 2
    ;;
esac
