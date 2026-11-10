"""Analysis tests for which platform and compiler combinations accept CUDA targets."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cuda:defs.bzl", "CUDA_TARGET")

_COMPILER = str(Label("//cc:compiler"))
_INCOMPATIBLE = [str(Label("@platforms//:incompatible"))]

_CompatInfo = provider(doc = "The constraints CUDA_TARGET resolved to.", fields = ["labels"])

def _probe_impl(ctx):
    return [_CompatInfo(labels = [str(target.label) for target in ctx.attr.compat])]

_compat_probe = rule(
    implementation = _probe_impl,
    attrs = {"compat": attr.label_list()},
)

def _compat_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, ctx.attr.expected, analysistest.target_under_test(env)[_CompatInfo].labels)
    return analysistest.end(env)

def _make(platform, compiler):
    return analysistest.make(
        _compat_impl,
        attrs = {"expected": attr.string_list()},
        config_settings = {
            "//command_line_option:platforms": str(Label("//cuda/tests:" + platform + "_platform")),
            _COMPILER: compiler,
        },
    )

_windows_llvm_test = _make("windows_x86_64", "llvm")
_windows_msvc_test = _make("windows_x86_64", "msvc")
_windows_arm64_llvm_test = _make("windows_arm64", "llvm")
_windows_arm64_msvc_test = _make("windows_arm64", "msvc")
_linux_llvm_test = _make("linux_x86_64", "llvm")

def cuda_cell_tests(name):
    """Declares a probe carrying CUDA_TARGET and checks it per platform and compiler.

    Args:
      name: suite name and prefix of the declared targets.
    """
    probe = name + "_probe"
    _compat_probe(name = probe, compat = CUDA_TARGET, tags = ["manual"])
    _windows_llvm_test(name = name + "_windows_llvm", target_under_test = ":" + probe, expected = _INCOMPATIBLE, size = "small")
    _windows_msvc_test(name = name + "_windows_msvc", target_under_test = ":" + probe, expected = [], size = "small")
    _windows_arm64_llvm_test(name = name + "_windows_arm64_llvm", target_under_test = ":" + probe, expected = _INCOMPATIBLE, size = "small")
    _windows_arm64_msvc_test(name = name + "_windows_arm64_msvc", target_under_test = ":" + probe, expected = [], size = "small")
    _linux_llvm_test(name = name + "_linux_llvm", target_under_test = ":" + probe, expected = [], size = "small")
    native.test_suite(
        name = name,
        tests = [
            ":" + name + "_windows_llvm",
            ":" + name + "_windows_msvc",
            ":" + name + "_windows_arm64_llvm",
            ":" + name + "_windows_arm64_msvc",
            ":" + name + "_linux_llvm",
        ],
    )
