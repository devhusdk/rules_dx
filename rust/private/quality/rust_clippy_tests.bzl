"""Unit tests of the test crate lint exemption in the dx clippy aspect."""

load("@bazel_skylib//lib:partial.bzl", "partial")
load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load(":rust_clippy.bzl", "test_crate_flags")

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

_drops_exempt_lints_test = unittest.make(_drops_exempt_lints_test_impl)
_keeps_other_lints_test = unittest.make(_keeps_other_lints_test_impl)

def rust_clippy_test_suite(name, size = "small"):
    unittest.suite(
        name,
        partial.make(_drops_exempt_lints_test, size = size),
        partial.make(_keeps_other_lints_test, size = size),
    )
