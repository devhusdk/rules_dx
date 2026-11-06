"""Coverage source manifests from the instrumented files of top-level targets."""

DX_COVERAGE_SOURCES_OUTPUT_GROUP = "dx_coverage_sources"

DxCoverageSourcesInfo = provider(
    doc = "The inputs of one coverage source manifest.",
    fields = {
        "cell": "The target platform as `<os>_<cpu>`, or unknown.",
        "files": "Depset of instrumented files.",
        "prod": "Depset of files that production dependencies own.",
    },
)

_OS = ["linux", "macos", "windows"]
_CPU = ["x86_64", "arm64"]

def _cell(ctx):
    """Returns the target platform as `<os>_<cpu>`, or unknown."""
    os = [name for name in _OS if ctx.target_platform_has_constraint(getattr(ctx.attr, "_os_" + name)[platform_common.ConstraintValueInfo])]
    cpu = [name for name in _CPU if ctx.target_platform_has_constraint(getattr(ctx.attr, "_cpu_" + name)[platform_common.ConstraintValueInfo])]
    if len(os) == 1 and len(cpu) == 1:
        return os[0] + "_" + cpu[0]
    return "unknown"

def _row(kind, file):
    """Returns the manifest row for one main-repo file, or None for other repos."""
    if file.short_path.startswith("../"):
        return None
    return "%s %s %s" % (kind, "source" if file.is_source else "generated", file.short_path)

def _rows(files, prod):
    """Returns the manifest rows for files, prod when a production dependency owns them."""
    owned = {file: True for file in prod.to_list()}
    rows = [_row("prod" if file in owned else "test", file) for file in files.to_list()]
    return [row for row in rows if row]

def dx_coverage_source_lines(info):
    """Returns the manifest lines for a DxCoverageSourcesInfo."""
    return ["v1 cell " + info.cell] + _rows(info.files, info.prod)

def _dx_coverage_sources_aspect_impl(target, ctx):
    """Writes the target's instrumented main-repo sources as one manifest."""
    if AnalysisTestResultInfo in target:
        return []
    test = ctx.rule.kind.endswith("_test") or getattr(ctx.rule.attr, "testonly", False)
    files = target[InstrumentedFilesInfo].instrumented_files if InstrumentedFilesInfo in target else depset()
    deps = list(getattr(ctx.rule.attr, "deps", []))
    for name in ["main", "upstream"]:
        dep = getattr(ctx.rule.attr, name, None)
        if type(dep) == "Target":
            deps.append(dep)
        elif type(dep) == "list":
            deps.extend(dep)
    dep_prod = [
        dep[DxCoverageSourcesInfo].prod
        for dep in deps
        if type(dep) == "Target" and DxCoverageSourcesInfo in dep
    ]
    prod = depset(transitive = dep_prod + ([] if test else [files]))
    if not files:
        return [DxCoverageSourcesInfo(cell = "unknown", files = files, prod = prod)] if dep_prod else []
    cell = _cell(ctx)
    args = ctx.actions.args()
    args.set_param_file_format("multiline")
    args.add("v1 cell " + cell)
    args.add_all(_rows(files, prod))
    manifest = ctx.actions.declare_file(ctx.label.name + ".dx_coverage_sources.txt")
    ctx.actions.write(manifest, args)
    return [
        DxCoverageSourcesInfo(cell = cell, files = files, prod = prod),
        OutputGroupInfo(**{DX_COVERAGE_SOURCES_OUTPUT_GROUP: depset([manifest])}),
    ]

dx_coverage_sources_aspect = aspect(
    implementation = _dx_coverage_sources_aspect_impl,
    doc = "Writes the instrumented main-repo sources of a top-level target.",
    attr_aspects = ["deps", "main", "upstream"],
    attrs = {
        "_cpu_arm64": attr.label(default = "@platforms//cpu:arm64"),
        "_cpu_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
        "_os_linux": attr.label(default = "@platforms//os:linux"),
        "_os_macos": attr.label(default = "@platforms//os:macos"),
        "_os_windows": attr.label(default = "@platforms//os:windows"),
    },
)
