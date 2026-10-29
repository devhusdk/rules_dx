"""Runs the dx_gotype driver over one rules_go package and records its report."""

load("@rules_go//go:def.bzl", "GoArchive", "GoInfo")
load("@rules_go//go/private:providers.bzl", "GoStdLib")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

GO_TOOLCHAIN_TYPE = "@rules_go//go:toolchain"

_DRIVERS = {
    "errcheck": "_errcheck",
    "gotype": "_gotype",
    "govet": "_govet",
    "staticcheck": "_staticcheck",
}

_MNEMONICS = {
    "errcheck": "GoErrcheck",
    "gotype": "GoTypeCheck",
    "govet": "GoVet",
    "staticcheck": "GoStaticcheck",
}

GoAnalysisInfo = provider(
    doc = "Reports of the dx Go analysis drivers for one rules_go package.",
    fields = {
        "reports": "dict: registry tool id to the File holding that driver's JSON report.",
    },
)

def go_lang_version(sdk_version):
    """Returns the go<major>.<minor> language version for one Go SDK version."""
    parts = sdk_version.removeprefix("go").split(".")
    minor = ""
    if len(parts) >= 2:
        for c in parts[1].elems():
            if not c.isdigit():
                break
            minor += c
    if len(parts) < 2 or not parts[0].isdigit() or minor == "":
        fail("gofumpt: cannot derive -lang from Go SDK version '" + sdk_version + "'")
    return "go" + parts[0] + "." + minor

def _first(value):
    return value[0] if type(value) == "list" else value

def _install_suffix(mode):
    suffix = mode.goos + "_" + mode.goarch
    if mode.race:
        suffix += "_race"
    elif mode.msan:
        suffix += "_msan"
    return suffix

def _go_files(files):
    return [f for f in files if f.extension == "go"]

def _package_sources(target, ctx):
    attrs = ctx.rule.attr
    if ctx.rule.kind.endswith("go_test"):
        files = list(ctx.rule.files.srcs)
        for embed in getattr(attrs, "embed", []):
            if GoInfo in embed:
                files += list(embed[GoInfo].srcs)
        return (_go_files(files), _importpath(attrs, ctx))
    source = target[GoArchive].source
    return (_go_files(source.srcs), source.importpath)

def _importpath(attrs, ctx):
    for embed in getattr(attrs, "embed", []):
        if GoInfo in embed and embed[GoInfo].importpath:
            return embed[GoInfo].importpath
    declared = getattr(attrs, "importpath", "")
    if declared:
        return declared
    return "test/" + ctx.label.package + "/" + ctx.label.name

def _go_analysis_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = _first(attrs.upstream)
        if GoAnalysisInfo not in upstream:
            return []
        info = upstream[GoAnalysisInfo]
        return [info, OutputGroupInfo(**{"dx_" + tool: depset([report]) for tool, report in info.reports.items()})]
    if GoArchive not in target:
        return []
    archive = target[GoArchive]
    (sources, importpath) = _package_sources(target, ctx)
    if not sources or not importpath:
        return []
    mode = archive.source.mode
    own_label = archive.data.label
    is_test = ctx.rule.kind.endswith("go_test")
    package_file = {}
    import_map = {}
    exports = []
    for dep in archive.direct:
        if is_test and dep.data.label == own_label:
            continue
        for data in dep.transitive.to_list():
            if not data.importmap or data.export_file == None:
                continue
            package_file[data.importmap] = data.export_file.path
            exports.append(data.export_file)
            if data.importpath and data.importpath != data.importmap:
                import_map[data.importpath] = data.importmap
    stdlib = ctx.attr._go_stdlib[GoStdLib]
    toolchain = ctx.toolchains[GO_TOOLCHAIN_TYPE]
    config = {
        "Cgo": not mode.pure,
        "GOARCH": mode.goarch,
        "GOOS": mode.goos,
        "GoFiles": [f.path for f in sources],
        "GoVersion": go_lang_version(toolchain.sdk.version) if toolchain != None else "",
        "ImportMap": import_map,
        "ImportPath": importpath,
        "OwnedFiles": [f.path for f in _go_files(ctx.rule.files.srcs) if f.is_source],
        "PackageFile": package_file,
        "StdlibDir": stdlib.root_file.path + "/" + _install_suffix(mode),
        "Tags": list(mode.tags),
    }
    config_file = ctx.actions.declare_file("dx_go_analysis/%s.config.json" % target.label.name)
    ctx.actions.write(config_file, json.encode(config))
    reports = {}
    for tool in sorted(_DRIVERS):
        report = ctx.actions.declare_file("dx_%s/%s.json" % (tool, target.label.name))
        ctx.actions.run(
            mnemonic = _MNEMONICS[tool],
            progress_message = tool + " %{label}",
            executable = getattr(ctx.attr, _DRIVERS[tool]).files_to_run,
            arguments = ["-out", report.path, config_file.path],
            inputs = depset(sources + [config_file], transitive = [depset(exports), stdlib.libs]),
            outputs = [report],
            resource_set = dx_quality_heavy_resources,
        )
        reports[tool] = report
    return [GoAnalysisInfo(reports = reports), OutputGroupInfo(**{"dx_" + tool: depset([report]) for tool, report in reports.items()})]

go_analysis_aspect = aspect(
    implementation = _go_analysis_impl,
    doc = "Runs the dx Go analysis drivers once per rules_go package against the compiled export data of its dependencies.",
    attr_aspects = ["upstream"],
    attrs = {
        "_go_stdlib": attr.label(default = "@rules_go//:stdlib"),
        "_errcheck": attr.label(
            default = "//config:tool_errcheck",
            cfg = "exec",
            executable = True,
        ),
        "_gotype": attr.label(
            default = "//config:tool_gotype",
            cfg = "exec",
            executable = True,
        ),
        "_govet": attr.label(
            default = "//config:tool_govet",
            cfg = "exec",
            executable = True,
        ),
        "_staticcheck": attr.label(
            default = "//config:tool_staticcheck",
            cfg = "exec",
            executable = True,
        ),
    },
    toolchains = [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
)
