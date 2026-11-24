"""Tests of the dx clippy aspect."""

load("@bazel_skylib//lib:partial.bzl", "partial")
load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts", "unittest")
load("//rust/private/quality:rust_clippy.bzl", "dx_clippy_aspect", "test_crate_flags")

def _drops_exempt_lints_test_impl(ctx):
    env = unittest.begin(ctx)
    asserts.equals(env, [], test_crate_flags([
        "-Dclippy::unwrap_used",
        "--deny=clippy::expect_used",
        "-Wclippy::panic",
        "--force-warn=clippy::unwrap-in-result",
        "-Fclippy::indexing_slicing",
    ]))
    return unittest.end(env)

def _keeps_other_lints_test_impl(ctx):
    env = unittest.begin(ctx)
    flags = ["-Dclippy::todo", "-Dunsafe_op_in_unsafe_fn", "-Dunused_must_use", "-Dwarnings", "--cap-lints=warn"]
    asserts.equals(env, flags, test_crate_flags(flags))
    return unittest.end(env)

def _clippy_out_dir_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_under_test(env).actions
    clippy = [a for a in actions if a.mnemonic == "Clippy"]
    asserts.equals(env, 1, len(clippy), "want one Clippy action")
    crate_dirs = [f.dirname for a in actions if a.mnemonic in ["Rustc", "RustcMetadata"] for f in a.outputs.to_list()]
    asserts.true(env, len(crate_dirs) > 0, "no Rustc or RustcMetadata outputs")
    if len(clippy) == 1:
        out_dirs = [arg[len("--out-dir="):] for arg in clippy[0].argv if arg.startswith("--out-dir=")]
        asserts.equals(env, 1, len(out_dirs), "want one --out-dir: %s" % clippy[0].argv)
        if len(out_dirs) == 1:
            asserts.false(env, out_dirs[0] in crate_dirs, "Clippy writes its .rmeta beside the crate outputs in " + out_dirs[0])
    return analysistest.end(env)

_drops_exempt_lints_test = unittest.make(_drops_exempt_lints_test_impl)
_keeps_other_lints_test = unittest.make(_keeps_other_lints_test_impl)
clippy_out_dir_test = analysistest.make(
    _clippy_out_dir_test_impl,
    config_settings = {
        str(Label("@rules_rust//rust/settings:pipelined_compilation")): True,
    },
    extra_target_under_test_aspects = [dx_clippy_aspect],
)

def rust_clippy_test_suite(name, size = "small"):
    unittest.suite(
        name,
        partial.make(_drops_exempt_lints_test, size = size),
        partial.make(_keeps_other_lints_test, size = size),
    )
