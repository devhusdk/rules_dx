"""Analysis tests for the Rust target triple the Linux platforms resolve to."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_TripleInfo = provider(doc = "The target triple of the resolved Rust toolchain.", fields = ["triple"])

def _probe_impl(ctx):
    return [_TripleInfo(triple = ctx.toolchains["@rules_rust//rust:toolchain_type"].target_triple.str)]

_triple_probe = rule(
    implementation = _probe_impl,
    toolchains = ["@rules_rust//rust:toolchain_type"],
)

def _make(platform, triple):
    def _impl(ctx):
        env = analysistest.begin(ctx)
        asserts.equals(env, triple, analysistest.target_under_test(env)[_TripleInfo].triple)
        return analysistest.end(env)

    return analysistest.make(
        _impl,
        config_settings = {
            "//command_line_option:platforms": str(Label("//tools/platforms:" + platform)),
        },
    )

_linux_x86_64_test = _make("linux_x86_64", "x86_64-unknown-linux-gnu")
_linux_x86_64_musl_test = _make("linux_x86_64_musl", "x86_64-unknown-linux-musl")

def musl_toolchain_tests(name, size = "small"):
    """Checks that the musl Linux platform takes the musl Rust toolchain and plain Linux keeps gnu."""
    probe = name + "_probe"
    _triple_probe(name = probe, tags = ["manual"])
    compatible = ["@platforms//cpu:x86_64", "@platforms//os:linux"]
    _linux_x86_64_test(
        name = name + "_linux_x86_64",
        target_under_test = ":" + probe,
        target_compatible_with = compatible,
        size = size,
    )
    _linux_x86_64_musl_test(
        name = name + "_linux_x86_64_musl",
        target_under_test = ":" + probe,
        target_compatible_with = compatible,
        size = size,
    )
    native.test_suite(
        name = name,
        tests = [
            ":" + name + "_linux_x86_64",
            ":" + name + "_linux_x86_64_musl",
        ],
    )
