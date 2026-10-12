"""Layer-4 loss restore-or-wont-fix fixture."""

LOSS_DAEMON_EXIT3 = "wont-fix"
LOSS_BUILDIFIER_REWRITE = "wont-fix"
LOSS_FULL_WIRING = "wont-fix"

DAEMON_HERMETIC_EXEC = "cli/cli/src/exec/bazel.rs code 3"
DAEMON_HERMETIC_TEST = "bazel_forwards_argv_verbatim_and_exit_code"
DAEMON_HERMETIC_PROBE = "ArgvProbe code Some(3)"

BUILDIFIER_HERMETIC_MATRIX = "//quality/testdata:runner_matrix"
BUILDIFIER_HERMETIC_CASE = "matrix_starlark_format_fail"
BUILDIFIER_HERMETIC_DIRTY = "matrix/starlark_dirty.bzl"
BUILDIFIER_HERMETIC_REWRITE = "x=1 to x = 1"
BUILDIFIER_HERMETIC_TOOL = "@dx_tools//:buildifier"

WIRING_HERMETIC_SMOKE = "bazel build //examples/adopt-rust/... --config=dx_dev"
WIRING_HERMETIC_ASPECT = "//quality/testdata:real_aspect_presence"
WIRING_HERMETIC_PRESET = "//quality:preset_parity_test"
WIRING_SMOKE_ONLY = "smoke-only"

HERMETIC_CUSTOMER_PATH = "hermetic pins stay the customer path"
REJECTED_NESTED_HARNESS = "nested-Bazel CI harness rejected"
REJECTED_NON_CUSTOMER_HARNESS = "non-customer harness in CI rejected"
REJECTED_SECOND_BAZEL = "no second-Bazel download"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
