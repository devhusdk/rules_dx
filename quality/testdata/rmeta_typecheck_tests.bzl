"""Analysis test that a pipelined rust_library exposes its rmeta rustc output."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _rmeta_impl(ctx):
    """Checks the rustc_rmeta_output group holds one .rmeta.rustc-output file."""
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    rmeta = target[OutputGroupInfo]["rustc_rmeta_output"].to_list()
    asserts.equals(env, 1, len(rmeta), "want one rustc_rmeta_output file")
    if len(rmeta) == 1:
        asserts.true(env, rmeta[0].basename.endswith(".rmeta.rustc-output"), "unexpected file: " + rmeta[0].basename)
    return analysistest.end(env)

rmeta_typecheck_test = analysistest.make(
    _rmeta_impl,
    config_settings = {
        str(Label("@rules_rust//rust/settings:pipelined_compilation")): True,
        str(Label("@rules_rust//rust/settings:rustc_output_diagnostics")): True,
    },
)
