"""Analysis test that the resolved Kotlin toolchain does not report unused deps."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin:kt_toolchain_type"

_ReportModeInfo = provider(
    doc = "The resolved Kotlin toolchain's unused-deps reporting mode.",
    fields = {"mode": "The `experimental_report_unused_deps` value."},
)

def _report_mode_impl(ctx):
    return [_ReportModeInfo(mode = ctx.toolchains[_KT_TOOLCHAIN_TYPE].experimental_report_unused_deps)]

_report_mode = rule(
    implementation = _report_mode_impl,
    doc = "Returns the resolved Kotlin toolchain's unused-deps reporting mode.",
    toolchains = [_KT_TOOLCHAIN_TYPE],
)

def _report_off_impl(ctx):
    env = analysistest.begin(ctx)
    mode = analysistest.target_under_test(env)[_ReportModeInfo].mode
    asserts.equals(
        env,
        "off",
        mode,
        "Kotlin toolchain reports unused deps as %r; any reporting mode makes JdepsMerge open the kapt stub jar or the -kt.jar without declaring them" % mode,
    )
    return analysistest.end(env)

_report_off_test = analysistest.make(_report_off_impl)

def kotlin_report_unused_deps_off_test(name):
    """Asserts the resolved Kotlin toolchain has unused-deps reporting off."""
    _report_mode(name = name + "_subject")
    _report_off_test(
        name = name,
        size = "small",
        target_under_test = ":" + name + "_subject",
    )
