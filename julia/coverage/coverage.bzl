"""Coverage.jl-style LCOV for Julia test forwarders."""

load("//libs/starlark:rlocation.bzl", "rlocation_path")

DxJuliaCoverageInfo = provider(
    doc = "Instrumented first-party Julia sources under one target.",
    fields = {"files": "Depset of instrumented source files."},
)

def _dx_julia_coverage_aspect_impl(target, ctx):
    if not ctx.configuration.coverage_enabled:
        return []
    deps = list(getattr(ctx.rule.attr, "deps", None) or [])
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if upstream != None:
        deps.append(upstream)
    own = depset()
    if InstrumentedFilesInfo in target:
        own = target[InstrumentedFilesInfo].instrumented_files
    return [DxJuliaCoverageInfo(files = depset(
        transitive = [own] + [dep[DxJuliaCoverageInfo].files for dep in deps if DxJuliaCoverageInfo in dep],
    ))]

dx_julia_coverage_aspect = aspect(
    implementation = _dx_julia_coverage_aspect_impl,
    doc = "Collects the instrumented Julia sources a test depends on.",
    attr_aspects = ["deps", "upstream"],
)

def dx_julia_coverage_attrs():
    """Returns the attributes the Julia coverage runner needs."""
    return {
        "_dx_julia_coverage_runner": attr.label(
            default = Label("//julia/coverage:runner"),
            executable = True,
            cfg = "target",
        ),
    }


def dx_julia_test_coverage(ctx, upstream):
    """Returns coverage providers that run the upstream Julia test and write LCOV."""
    upstream_exe = upstream[DefaultInfo].files_to_run.executable
    runner = ctx.attr._dx_julia_coverage_runner[DefaultInfo]
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream"],
    )
    sources = depset(transitive = [
        instrumented.instrumented_files,
        upstream[DxJuliaCoverageInfo].files if DxJuliaCoverageInfo in upstream else depset(),
    ])
    paths = sorted({f.short_path: True for f in sources.to_list() if not f.short_path.startswith("../")}.keys())
    config = ctx.actions.declare_file(ctx.label.name + ".julia_coverage.cfg")
    ctx.actions.write(config, "\n".join(
        ["workspace\t" + ctx.workspace_name, "upstream\t" + rlocation_path(ctx, upstream_exe)] +
        ["source\t" + p for p in paths],
    ) + "\n")
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    env["DX_JULIA_COVERAGE_CONFIG"] = rlocation_path(ctx, config)
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = [config, upstream_exe]).merge(runner.default_runfiles),
        executable = ctx.executable._dx_julia_coverage_runner,
    )
