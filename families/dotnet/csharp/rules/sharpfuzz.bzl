"""SharpFuzz instrumentation and the `<name>` harness plus `<name>_run` runner shared by the .NET fuzz macros."""

load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("@rules_dx//libs/starlark:fuzz.bzl", "dx_fuzz_runner")
load("@rules_dx//libs/starlark:fuzz_bench.bzl", "merge_tags")

SHARPFUZZ = [
    Label("@paket.main//sharpfuzz"),
]

_LINUX = ["@platforms//os:linux"]

def _instrument_impl(ctx):
    lib = ctx.attr.lib
    runtime = lib[DotnetAssemblyRuntimeInfo]
    if runtime.nuget_info != None:
        return [lib[DotnetAssemblyCompileInfo], runtime, lib[DefaultInfo]]
    libs = []
    for dll in runtime.libs:
        copy = ctx.actions.declare_file("{}/{}".format(ctx.label.name, dll.basename))
        ctx.actions.run_shell(
            inputs = [dll],
            outputs = [copy],
            tools = [ctx.executable._sharpfuzz],
            command = 'cp "$1" "$2" && chmod u+w "$2" && "$3" "$2"',
            arguments = [dll.path, copy.path, ctx.executable._sharpfuzz.path],
            mnemonic = "SharpFuzzInstrument",
            progress_message = "Instrumenting %{output} with SharpFuzz",
        )
        libs.append(copy)
    return [
        DefaultInfo(
            files = depset(libs),
            runfiles = ctx.runfiles(files = libs).merge(lib[DefaultInfo].default_runfiles),
        ),
        lib[DotnetAssemblyCompileInfo],
        DotnetAssemblyRuntimeInfo(
            name = runtime.name,
            version = runtime.version,
            libs = libs,
            pdbs = [],
            xml_docs = runtime.xml_docs,
            native = runtime.native,
            data = runtime.data,
            resource_assemblies = runtime.resource_assemblies,
            appsetting_files = runtime.appsetting_files,
            nuget_info = runtime.nuget_info,
            deps = runtime.deps,
            direct_deps_depsjson_fragment = runtime.direct_deps_depsjson_fragment,
        ),
    ]

dx_sharpfuzz_instrument = rule(
    implementation = _instrument_impl,
    attrs = {
        "lib": attr.label(
            doc = "Library whose assemblies SharpFuzz instruments. NuGet packages pass through unchanged.",
            mandatory = True,
            providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
        ),
        "_sharpfuzz": attr.label(
            default = Label("@paket.main//sharpfuzz.commandline/tools:sharpfuzz"),
            executable = True,
            cfg = "exec",
        ),
    },
    provides = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    doc = "Re-exports `lib` with its assemblies instrumented by SharpFuzz.",
)

def sharpfuzz_fuzz(name, binary, srcs, deps, visibility, tags, kwargs):
    """Declares the instrumented deps, the harness `<name>` and the `<name>_run` runner. binary is the language's binary macro."""
    instrumented = []
    for i, dep in enumerate(deps or []):
        wrapped = "{}_sharpfuzz_{}".format(name, i)
        dx_sharpfuzz_instrument(
            name = wrapped,
            lib = dep,
            testonly = True,
            visibility = ["//visibility:private"],
        )
        instrumented.append(":" + wrapped)
    kwargs = dict(kwargs)
    kwargs["target_compatible_with"] = kwargs.get("target_compatible_with", _LINUX)
    binary(
        name = name,
        srcs = srcs,
        deps = instrumented + SHARPFUZZ,
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        visibility = visibility,
        **kwargs
    )
    dx_fuzz_runner(name, Label("@rules_dx//dotnet/fuzz:libfuzzer_dotnet"), engine = "dotnet", harness = ":" + name, visibility = visibility)
