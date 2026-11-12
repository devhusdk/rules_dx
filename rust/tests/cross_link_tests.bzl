"""Analysis tests for the Rust link flags the Windows cross targets resolve to."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//rust/private:defs.bzl", "CLANG_CL_LINK_FLAGS")

_COMPILER = str(Label("//cc:compiler"))

_FlagsInfo = provider(doc = "The link flags a select resolved to.", fields = ["flags"])

def _probe_impl(ctx):
    return [_FlagsInfo(flags = ctx.attr.flags)]

_flags_probe = rule(
    implementation = _probe_impl,
    attrs = {"flags": attr.string_list()},
)

def _flags_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, CLANG_CL_LINK_FLAGS, analysistest.target_under_test(env)[_FlagsInfo].flags)
    return analysistest.end(env)

def _make(platform):
    return analysistest.make(
        _flags_impl,
        config_settings = {
            "//command_line_option:platforms": str(Label("//tools/platforms:" + platform)),
            _COMPILER: "llvm",
        },
    )

_windows_arm64_test = _make("windows_arm64")
_windows_x86_64_test = _make("windows_x86_64")

def cross_link_tests(name):
    """Checks that the Windows cross platforms take the clang-cl Rust link flags.

    Args:
      name: suite name and prefix of the declared targets.
    """
    probe = name + "_probe"
    _flags_probe(
        name = probe,
        flags = select({
            Label("//rust/private:clang_cl"): CLANG_CL_LINK_FLAGS,
            "//conditions:default": [],
        }),
        tags = ["manual"],
    )
    _windows_arm64_test(name = name + "_windows_arm64", target_under_test = ":" + probe, size = "small")
    _windows_x86_64_test(name = name + "_windows_x86_64", target_under_test = ":" + probe, size = "small")
    native.test_suite(
        name = name,
        tests = [
            ":" + name + "_windows_arm64",
            ":" + name + "_windows_x86_64",
        ],
    )
