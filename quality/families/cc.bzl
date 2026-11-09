"""C, C++, Objective-C, CUDA and QML real quality aspects."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//quality:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load("//quality:family_hooks.bzl", "stage")
load("//quality:nvcc_syntax.bzl", "NvccSyntaxInfo", "nvcc_syntax_aspect")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

_QML_CONTEXT_EXTENSIONS = ["qml", "qmltypes", "js", "mjs"]

QmlContextInfo = provider(
    doc = "QML import context files reachable through deps.",
    fields = {"files": "depset[File]: qmldir, qml, qmltypes and js files."},
)

def _qml_context_impl(target, ctx):
    if target.label.repo_name != "":
        return [QmlContextInfo(files = depset())]
    direct = [
        f
        for f in target[DefaultInfo].files.to_list()
        if f.basename == "qmldir" or f.extension in _QML_CONTEXT_EXTENSIONS
    ]
    deps = getattr(ctx.rule.attr, "deps", [])
    if type(deps) != "list":
        deps = [deps]
    transitive = [
        dep[QmlContextInfo].files
        for dep in deps
        if type(dep) == "Target" and QmlContextInfo in dep
    ]
    return [QmlContextInfo(files = depset(direct, transitive = transitive))]

_qml_context_aspect = aspect(
    implementation = _qml_context_impl,
    doc = "Collects QML import context files through deps.",
    attr_aspects = ["deps"],
    provides = [QmlContextInfo],
)

TOOLS = {
    "clang_format": {
        "attr": attr.label(default = "@llvm//tools:clang-format", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "cc",
    },
    "clang_syntax": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "clang_tidy": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "nvcc_syntax": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "qmlformat": {
        "attr": attr.label(default = "//config:tool_qmlformat", allow_files = True, cfg = "exec"),
        "fallback": "=@dx_qt//:qmlformat",
        "kind": "setting",
        "resources": "light",
        "shard": "cc",
    },
    "qmllint": {
        "attr": attr.label(default = "//config:tool_qmllint", allow_files = True, cfg = "exec"),
        "fallback": "=@dx_qt//:qmllint",
        "kind": "setting",
        "resources": "medium",
        "shard": "cc",
    },
}

SHARD_SPECS = {
    "real_cc_format": {
        "capability": "format",
        "doc": "Additive C/C++ format family aspect (clang-format).",
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_cc_lint": {
        "capability": "lint",
        "doc": "Additive C/C++ lint family aspect (clang-tidy).",
        "extra_attrs": {
            "_config_profile": attr.label(default = "//config:config_profile"),
        },
        "requires": [clang_syntax_aspect, _qml_context_aspect],
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_cc_typecheck": {
        "capability": "typecheck",
        "doc": "Additive C/C++/Objective-C/CUDA typecheck family aspect (compiler -fsyntax-only replay, nvcc -cuda replay).",
        "requires": [clang_syntax_aspect, nvcc_syntax_aspect, _qml_context_aspect],
        "shard": "cc",
        "suffix": "-cc",
    },
}

def _clang_info(target, what):
    if ClangSyntaxInfo not in target:
        fail(what + ": clang_syntax and clang_tidy need a cc_*, objc_* or cuda_* target with compile actions (a header-only library needs --features=parse_headers)")
    return target[ClangSyntaxInfo]

def _clang_tidy_hook(target, _ctx, _resolved, _capability, what, _shared):
    info = _clang_info(target, what)
    if info.tidy_error:
        fail(info.tidy_error)
    return real_context(upstream = info.tidy_diagnostics, late_inputs = info.tidy_diagnostics)

def _clang_syntax_hook(target, _ctx, _resolved, _capability, what, _shared):
    info = _clang_info(target, what)
    tool_files = []
    inputs = []
    for i in range(len(info.diagnostics)):
        inputs.append(info.diagnostics[i])
        stem = info.diagnostics[i].basename.rpartition(".")[0]
        stage(tool_files, inputs, "dx-clang-syntax/" + stem + "/exit", info.statuses[i])
        stage(tool_files, inputs, "dx-clang-syntax/" + stem + "/driver", info.drivers[i])
    return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

def _nvcc_syntax_hook(target, _ctx, _resolved, _capability, what, _shared):
    if NvccSyntaxInfo not in target:
        fail(what + ": nvcc_syntax needs a cuda_library or cuda_objects target with a CudaCompile action (a header-only library has none)")
    info = target[NvccSyntaxInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.diagnostics)):
        inputs.append(info.diagnostics[i])
        stem = info.diagnostics[i].basename.rpartition(".")[0]
        stage(tool_files, inputs, "dx-nvcc-syntax/" + stem + "/exit", info.statuses[i])
    return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

def _qmllint_hook(target, _ctx, _resolved, _capability, _what, _shared):
    context_files = list(target[DefaultInfo].files.to_list())
    if QmlContextInfo in target:
        context_files.extend(target[QmlContextInfo].files.to_list())
    pairs = []
    for f in context_files:
        ws_path = f.short_path
        if ws_path.startswith("../"):
            ws_path = "_external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        if f.basename != "qmldir" and f.extension not in _QML_CONTEXT_EXTENSIONS:
            continue
        pairs.append((ws_path, f, False))
    return real_context(resolve_pairs = pairs)

def _cc_sources(target, ctx):
    if CcInfo not in target:
        return None
    srcs = getattr(ctx.rule.files, "srcs", None)
    hdrs = getattr(ctx.rule.files, "hdrs", None)
    non_arc_srcs = getattr(ctx.rule.files, "non_arc_srcs", None)
    if srcs == None and hdrs == None and non_arc_srcs == None:
        return struct(files = [], error = "upstream cc target has no readable srcs, hdrs or non_arc_srcs; unsupported ownership is unchecked, not clean")
    files = []
    for group in [srcs, hdrs, non_arc_srcs]:
        if group != None:
            files.extend(group)
    return struct(files = files, error = "")

RECOGNIZER = _cc_sources

HOOK_ORDER = ["clang_syntax", "clang_tidy", "nvcc_syntax", "qmllint"]

HOOKS = {
    "clang_syntax": _clang_syntax_hook,
    "clang_tidy": _clang_tidy_hook,
    "nvcc_syntax": _nvcc_syntax_hook,
    "qmllint": _qmllint_hook,
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [RECOGNIZER],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_cc_format_aspect = real_make_aspect(_CFG, "real_cc_format")

real_cc_lint_aspect = real_make_aspect(_CFG, "real_cc_lint")

real_cc_typecheck_aspect = real_make_aspect(_CFG, "real_cc_typecheck")

CcShardProbeInfo = provider(
    doc = "Shard names the cc shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_cc_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "cc", CcShardProbeInfo)

real_cc_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "cc", CcShardProbeInfo)

real_cc_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "cc", CcShardProbeInfo)
