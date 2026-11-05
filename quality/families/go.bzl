"""Go real quality aspects."""

load("@rules_go//go:def.bzl", "GoArchive")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:go_analysis.bzl", "GO_TOOLCHAIN_TYPE", "GoAnalysisInfo", "go_analysis_aspect", "go_lang_version")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//quality:unused_roots.bzl", "DxUnusedRootsInfo", "unused_roots_aspect")

TOOLS = {
    "deadcode": {
        "attr": attr.label(default = "//go/tools/analysis/cmd/dx_deadcode", cfg = "exec", executable = True),
        "capabilities": ["unused"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "go",
    },
    "errcheck": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "gofumpt": {
        "attr": attr.label(default = "//config:tool_gofumpt", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a gofumpt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "gotype": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "govet": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "modfmt": {
        "attr": attr.label(default = "//config:tool_modfmt", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a modfmt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "staticcheck": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
}

_DEADCODE_ATTRS = {
    "_deadcode_bin": attr.label(default = "//config:tool_deadcode", allow_single_file = True, cfg = "exec", doc = "The pinned golang.org/x/tools deadcode binary."),
}

SHARD_SPECS = {
    "real_go_format": {
        "capability": "format",
        "doc": "Additive Go format family aspect (gofumpt).",
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_lint": {
        "capability": "lint",
        "doc": "Additive Go lint family aspect (delegated dx_govet, dx_errcheck and dx_staticcheck over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_unused": {
        "capability": "unused",
        "doc": "Additive Go unused-code family aspect (deadcode over the target's whole closure in a staged GOPATH).",
        "extra_attrs": _DEADCODE_ATTRS,
        "requires": [unused_roots_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Go typecheck family aspect (delegated dx_gotype over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
}

def _gofumpt_lang(ctx, what):
    """Derives gofumpt -lang from the resolved rules_go SDK."""
    toolchain = ctx.toolchains[GO_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail(what + ": gofumpt needs a registered rules_go Go toolchain to derive -lang")
    return go_lang_version(toolchain.sdk.version)

def _go_analysis_hook(tool):
    def _hook(target, _ctx, resolved, _capability, what, _shared):
        if GoAnalysisInfo not in target:
            if GoArchive not in target:
                return without(resolved, tool)
            fail(what + ": " + tool + " needs the Go analysis aspect report for a rules_go target")
        report = target[GoAnalysisInfo].reports[tool]
        return real_context(upstream = [report], late_inputs = [report])

    return _hook

def _gofumpt_hook(_target, ctx, _resolved, _capability, what, _shared):
    return real_context(options = {"edition": _gofumpt_lang(ctx, what)})

def _workspace_path(f):
    return f.short_path if f.is_source and ".." not in f.short_path.split("/") else None

def _deadcode_hook(target, ctx, resolved, _capability, what, _shared):
    is_test = ctx.rule.kind.endswith("go_test")
    if GoArchive not in target or DxUnusedRootsInfo not in target:
        return without(resolved, "deadcode")
    archive = target[GoArchive]
    if not is_test and not archive.source.is_main:
        return without(resolved, "deadcode")
    toolchain = ctx.toolchains[GO_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail(what + ": deadcode needs a registered rules_go Go toolchain")
    sdk = toolchain.sdk
    tool_files = []
    inputs = []
    files = {}
    workspace = {}
    pairs = []
    for data in archive.transitive.to_list():
        if not data.importpath:
            continue
        for f in list(data.srcs) + list(getattr(data, "_embedsrcs", [])):
            rel = data.importpath + "/" + f.basename
            stage(tool_files, inputs, "dx-deadcode/gopath/src/" + rel, f)
            ws_path = _workspace_path(f)
            if ws_path != None and f.extension == "go":
                files[rel] = ws_path
                workspace[data.importpath] = True
                pairs.append((ws_path, f, False))
    importpath = archive.data.importpath or "dx/" + ctx.label.package + "/" + ctx.label.name
    config = ctx.actions.declare_file(target.label.name + "-dx-deadcode-config.json")
    ctx.actions.write(config, json.encode({
        "files": files,
        "goarch": archive.source.mode.goarch,
        "goos": archive.source.mode.goos,
        "packages": [importpath],
        "test": is_test,
        "workspace": sorted(workspace.keys()),
    }))
    stage(tool_files, inputs, "dx-deadcode/config.json", config)
    roots = target[DxUnusedRootsInfo]
    roots_file = ctx.actions.declare_file(target.label.name + "-dx-unused-roots.json")
    ctx.actions.write(roots_file, json.encode({
        "keep": sorted({f.short_path: True for f in roots.keep.to_list() if f.extension == "go"}.keys()),
        "mains": sorted({f.short_path: True for f in roots.mains.to_list() if f.extension == "go"}.keys()),
        "public": sorted({f.short_path: True for f in roots.public.to_list() if f.extension == "go"}.keys()),
    }))
    stage(tool_files, inputs, "dx-unused/roots.json", roots_file)
    inputs += sdk.srcs.to_list() + [sdk.root_file, sdk.go, ctx.file._deadcode_bin]
    return real_context(
        resolve_pairs = pairs,
        tool_files = tool_files,
        inputs = inputs,
        options = {"path_tools": [("go", sdk.go), ("deadcode", ctx.file._deadcode_bin)]},
    )

def _harness_sources(target, capability, direct_sources):
    if capability != "unused" or DxUnusedRootsInfo not in target:
        return direct_sources
    closure = [f for f in target[DxUnusedRootsInfo].sources.to_list() if f.extension == "go"]
    return {"go": depset(closure)} if closure else direct_sources

HOOK_ORDER = ["errcheck", "gotype", "govet", "staticcheck", "gofumpt", "deadcode"]

HOOKS = {
    "deadcode": _deadcode_hook,
    "gofumpt": _gofumpt_hook,
} | {tool: _go_analysis_hook(tool) for tool in ["errcheck", "gotype", "govet", "staticcheck"]}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = _harness_sources,
    toolchain = None,
    check_policy = False,
)

real_go_format_aspect = real_make_aspect(_CFG, "real_go_format")

real_go_lint_aspect = real_make_aspect(_CFG, "real_go_lint")

real_go_typecheck_aspect = real_make_aspect(_CFG, "real_go_typecheck")

real_go_unused_aspect = real_make_aspect(_CFG, "real_go_unused")
