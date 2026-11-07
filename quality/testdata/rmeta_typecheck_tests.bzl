"""Analysis test that a pipelined rust_library exposes its rmeta rustc output."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect")
load("@rules_rust//rust:rust_common.bzl", "rust_common")

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

def _clippy_rmeta_impl(ctx):
    """Checks the Clippy action reads dependency .rmeta files and no library .rlib file."""
    env = analysistest.begin(ctx)
    clippy = [a for a in analysistest.target_under_test(env).actions if a.mnemonic == "Clippy"]
    asserts.equals(env, 1, len(clippy), "want one Clippy action")
    if len(clippy) == 1:
        names = [f.basename for f in clippy[0].inputs.to_list()]
        asserts.true(env, any([n.startswith("libfixture_upstream_rust-") and n.endswith(".rmeta") for n in names]), "no library .rmeta input")
        asserts.false(env, any([n.startswith("libfixture_upstream_rust-") and n.endswith(".rlib") for n in names]), "unexpected library .rlib input")
    return analysistest.end(env)

clippy_rmeta_inputs_test = analysistest.make(
    _clippy_rmeta_impl,
    config_settings = {
        str(Label("@rules_rust//rust/settings:pipelined_compilation")): True,
        str(Label("@rules_rust//rust/settings:rustc_output_diagnostics")): True,
    },
    extra_target_under_test_aspects = [rust_clippy_aspect],
)

def _rmeta_paths_impl(ctx):
    """Checks the Rustc action writes its own .rmeta, apart from the RustcMetadata output."""
    env = analysistest.begin(ctx)
    actions = analysistest.target_under_test(env).actions
    full = [a for a in actions if a.mnemonic == "Rustc"]
    meta = [a for a in actions if a.mnemonic == "RustcMetadata"]
    asserts.equals(env, 1, len(full), "want one Rustc action")
    asserts.equals(env, 1, len(meta), "want one RustcMetadata action")
    if len(full) == 1 and len(meta) == 1:
        argv = full[0].argv
        asserts.true(env, any([a.startswith("--emit=metadata=") and "/_rmeta_full/" in a for a in argv]), "no --emit=metadata= under _rmeta_full: %s" % argv)
        asserts.false(env, any([a.startswith("--emit=") and "metadata" in a and "metadata=" not in a for a in argv]), "bare metadata emit: %s" % argv)
        shared = [f.path for f in full[0].outputs.to_list() if f in meta[0].outputs.to_list()]
        asserts.equals(env, [], shared, "outputs shared with RustcMetadata")
    return analysistest.end(env)

pipelined_rmeta_paths_test = analysistest.make(
    _rmeta_paths_impl,
    config_settings = {
        str(Label("@rules_rust//rust/settings:pipelined_compilation")): True,
    },
)
