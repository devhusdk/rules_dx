"""SharpFuzz instrumentation and the `<name>` harness plus `<name>_run` runner shared by the .NET fuzz macros."""

load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("@rules_dx//libs/starlark:fuzz.bzl", "dx_fuzz_runner")
load("@rules_dx//libs/starlark:fuzz_bench.bzl", "merge_tags")

SHARPFUZZ = [
    Label("@paket.main//sharpfuzz"),
]

_LINUX = ["@platforms//os:linux"]

DxSharpFuzzInfo = provider(
    doc = "A target's runtime info with first-party assemblies instrumented by SharpFuzz.",
    fields = {
        "runtime": "DotnetAssemblyRuntimeInfo whose deps are the instrumented transitive deps.",
        "files": "Depset of every instrumented assembly under the target.",
    },
)

def _aspect_impl(target, ctx):
    if DotnetAssemblyRuntimeInfo not in target:
        return []
    runtime = target[DotnetAssemblyRuntimeInfo]
    if runtime.nuget_info != None:
        return [DxSharpFuzzInfo(runtime = runtime, files = depset())]
    dep_infos = [dep[DxSharpFuzzInfo] for dep in getattr(ctx.rule.attr, "deps", []) if DxSharpFuzzInfo in dep]
    dep_runtimes = [info.runtime for info in dep_infos]
    libs = []
    for dll in runtime.libs:
        copy = ctx.actions.declare_file("{}_sharpfuzz/{}".format(ctx.label.name, dll.basename))
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
    return [DxSharpFuzzInfo(
        runtime = DotnetAssemblyRuntimeInfo(
            name = runtime.name,
            version = runtime.version,
            libs = libs,
            pdbs = [],
            xml_docs = runtime.xml_docs,
            native = runtime.native,
            data = runtime.data,
            resource_assemblies = runtime.resource_assemblies,
            appsetting_files = runtime.appsetting_files,
            nuget_info = None,
            deps = depset(dep_runtimes, transitive = [r.deps for r in dep_runtimes]),
            direct_deps_depsjson_fragment = runtime.direct_deps_depsjson_fragment,
        ),
        files = depset(libs, transitive = [info.files for info in dep_infos]),
    )]

dx_sharpfuzz_aspect = aspect(
    implementation = _aspect_impl,
    attr_aspects = ["deps"],
    attrs = {
        "_sharpfuzz": attr.label(
            default = Label("@paket.main//sharpfuzz.commandline/tools:sharpfuzz"),
            executable = True,
            cfg = "exec",
        ),
    },
    doc = "Instruments every first-party assembly under a target. NuGet packages pass through unchanged.",
)

def _instrument_impl(ctx):
    lib = ctx.attr.lib
    info = lib[DxSharpFuzzInfo]
    files = info.files
    return [
        DefaultInfo(
            files = files,
            runfiles = ctx.runfiles(transitive_files = files).merge(lib[DefaultInfo].default_runfiles),
        ),
        lib[DotnetAssemblyCompileInfo],
        info.runtime,
    ]

dx_sharpfuzz_instrument = rule(
    implementation = _instrument_impl,
    attrs = {
        "lib": attr.label(
            doc = "Library whose first-party assemblies, direct and transitive, SharpFuzz instruments. NuGet packages pass through unchanged.",
            mandatory = True,
            providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
            aspects = [dx_sharpfuzz_aspect],
        ),
    },
    provides = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    doc = "Re-exports `lib` with its first-party assemblies instrumented by SharpFuzz.",
)

def sharpfuzz_fuzz(name, binary, srcs, deps, visibility, tags, kwargs):
    """Declares the instrumented deps, the harness `<name>` and the `<name>_run` runner. binary is the language's binary macro."""
    instrumented = []
    for i, dep in enumerate(deps or []):
        wrapped = "{}_sharpfuzz_{}".format(name, i)
        dx_sharpfuzz_instrument(
            name = wrapped,
            lib = dep,
            tags = ["manual"],
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
