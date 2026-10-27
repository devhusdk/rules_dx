"""Coverlet coverage for .NET test forwarders."""

load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")

DxDotnetCoverageInfo = provider(
    doc = "Instrumented first-party .NET assemblies under one target.",
    fields = {
        "assemblies": "Depset of instrumented assembly files.",
        "pdbs": "Depset of their PDB files.",
    },
)

def _deps(ctx):
    out = list(getattr(ctx.rule.attr, "deps", None) or [])
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if upstream != None:
        out.extend(upstream if type(upstream) == "list" else [upstream])
    return out

def _dx_dotnet_coverage_aspect_impl(target, ctx):
    if not ctx.configuration.coverage_enabled:
        return []
    deps = [dep for dep in _deps(ctx) if type(dep) == "Target" and DxDotnetCoverageInfo in dep]
    assemblies = []
    pdbs = []
    if DotnetAssemblyRuntimeInfo in target and ctx.coverage_instrumented():
        info = target[DotnetAssemblyRuntimeInfo]
        if info.pdbs:
            assemblies = info.libs
            pdbs = info.pdbs
    return [DxDotnetCoverageInfo(
        assemblies = depset(assemblies, transitive = [dep[DxDotnetCoverageInfo].assemblies for dep in deps]),
        pdbs = depset(pdbs, transitive = [dep[DxDotnetCoverageInfo].pdbs for dep in deps]),
    )]

dx_dotnet_coverage_aspect = aspect(
    implementation = _dx_dotnet_coverage_aspect_impl,
    doc = "Collects instrumented first-party .NET assemblies.",
    attr_aspects = ["deps", "upstream"],
)

def dx_dotnet_coverage_attrs():
    """Returns the attributes coverlet coverage needs."""
    return {
        "_dx_coverlet": attr.label(
            default = Label("//tools/coverage/dotnet:coverlet"),
            executable = True,
            cfg = "target",
        ),
        "_dx_coverlet_launcher": attr.label(
            default = Label("//tools/coverage/dotnet:coverlet_launcher"),
            executable = True,
            cfg = "target",
        ),
    }

def _rlocation(ctx, file):
    path = file.short_path
    if path.startswith("../"):
        return path[3:]
    return ctx.workspace_name + "/" + path

def dx_dotnet_test_coverage(ctx, upstream):
    """Returns coverage providers that run the upstream .NET test under coverlet."""
    test_dlls = upstream[DotnetAssemblyRuntimeInfo].libs
    if not test_dlls:
        fail(str(ctx.label) + ": upstream test has no assembly")
    test_dll = test_dlls[0]
    assemblies = []
    pdbs = []
    if DxDotnetCoverageInfo in upstream:
        assemblies = upstream[DxDotnetCoverageInfo].assemblies.to_list()
        pdbs = upstream[DxDotnetCoverageInfo].pdbs.to_list()
    if not ctx.coverage_instrumented():
        assemblies = [a for a in assemblies if a != test_dll]
    upstream_exe = upstream[DefaultInfo].files_to_run.executable
    coverlet = ctx.attr._dx_coverlet[DefaultInfo]
    launcher = ctx.attr._dx_coverlet_launcher[DefaultInfo]
    lines = [
        "workspace\t" + ctx.workspace_name,
        "upstream\t" + _rlocation(ctx, upstream_exe),
        "coverlet\t" + _rlocation(ctx, coverlet.files_to_run.executable),
        "test_assembly\t" + _rlocation(ctx, test_dll),
    ] + ["instrument\t" + _rlocation(ctx, a) for a in assemblies]
    config = ctx.actions.declare_file(ctx.label.name + ".coverlet.cfg")
    ctx.actions.write(config, "\n".join(lines) + "\n")

    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream"],
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    env["DX_COVERLET_CONFIG"] = _rlocation(ctx, config)
    runfiles = ctx.runfiles(
        files = [config, upstream_exe, coverlet.files_to_run.executable] + assemblies + pdbs,
        transitive_files = instrumented.instrumented_files,
    ).merge_all([coverlet.default_runfiles, launcher.default_runfiles])
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = runfiles,
        executable = ctx.executable._dx_coverlet_launcher,
    )
