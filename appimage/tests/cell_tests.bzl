"""Analysis test that an arm64 target cell is rejected."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _arm64_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "target cell linux_arm64 is unsupported")
    return analysistest.end(env)

arm64_cell_test = analysistest.make(
    _arm64_impl,
    expect_failure = True,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))},
)
