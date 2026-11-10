"""Test-only rule that exposes the Roslyn SARIF logs of a C# library."""

load("@rules_dx//core/quality:roslyn.bzl", "RoslynInfo", "roslyn_aspect")

def _roslyn_sarif_impl(ctx):
    return [DefaultInfo(files = depset(ctx.attr.dep[RoslynInfo].sarifs))]

roslyn_sarif = rule(
    implementation = _roslyn_sarif_impl,
    attrs = {"dep": attr.label(aspects = [roslyn_aspect], mandatory = True)},
)

_TOOLCHAIN = "@rules_dotnet//dotnet:toolchain_type"

def _sdk_recommended_impl(ctx):
    toolchain = ctx.toolchains[_TOOLCHAIN]
    csc = toolchain.csharp_compiler.files_to_run.executable
    major = csc.path.partition("/sdk/")[2].partition("/")[0].partition(".")[0]
    suffix = "/analyzers/build/config/analysislevel_%s_recommended.globalconfig" % major
    found = [f for f in toolchain.runtime.default_runfiles.files.to_list() + toolchain.csharp_compiler.default_runfiles.files.to_list() if f.path.endswith(suffix)]
    found = {f.path: f for f in found}.values()
    if len(found) != 1:
        fail("want one analysislevel_%s_recommended.globalconfig in the SDK, got %d" % (major, len(found)))
    return [DefaultInfo(files = depset(list(found)))]

sdk_recommended = rule(
    implementation = _sdk_recommended_impl,
    toolchains = [config_common.toolchain_type(_TOOLCHAIN)],
    doc = "The Recommended analysis level globalconfig of the pinned .NET SDK.",
)
