"""Reports whether the execution platform is Windows."""

load("//core/quality/artifacts:platforms.bzl", "execution_is_windows")

ExecOsInfo = provider(
    doc = "Operating system of the platform a target is built for.",
    fields = {"windows": "True when that platform is Windows."},
)

def _exec_os_impl(ctx):
    windows = ctx.attr._windows_os[platform_common.ConstraintValueInfo]
    return [ExecOsInfo(windows = execution_is_windows(ctx, windows))]

exec_os = rule(
    implementation = _exec_os_impl,
    attrs = {"_windows_os": attr.label(default = "@platforms//os:windows")},
    doc = "Provides ExecOsInfo; depend on it with cfg = \"exec\" to learn the executor's OS.",
)
