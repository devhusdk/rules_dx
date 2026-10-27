"""Minimal TypeScript wrappers."""

load("@aspect_rules_jest//jest:defs.bzl", _jest_test = "jest_test")
load("@aspect_rules_js//js:defs.bzl", _js_test = "js_test")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("@aspect_rules_ts//ts:defs.bzl", _TsConfigInfo = "TsConfigInfo", _ts_project = "ts_project")
load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("//javascript/rules:node_test.bzl", "node_test_forward_rule", "node_test_plain_rejection", "node_test_reporter_rejection", "node_wrap_test")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_upstream_tags")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_TS_PROJECT_PROVIDES = [
    _JsInfo,
    _TsConfigInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_TS_SOURCE_SPECS = [
    ("typescript", ["ts", "mts", "cts"], [".d.ts", ".d.mts", ".d.cts"]),
    ("tsx", "tsx"),
]
_DX_TS_SOURCE_EXTS = [".ts", ".tsx", ".mts", ".cts"]

_typescript_project_forward = dx_library_forward_rule(
    provides = _DX_TS_PROJECT_PROVIDES,
    required_providers = [(_JsInfo, "JsInfo"), (_TsConfigInfo, "TsConfigInfo")],
    quality_specs = _DX_TS_SOURCE_SPECS,
    what = "typescript_*",
    allow_files = _DX_TS_SOURCE_EXTS,
    upstream_providers = [[_JsInfo]],
)

_DX_TS_DECLARATION_SUFFIXES = [".d.ts", ".d.mts", ".d.cts"]

def _is_declaration(src):
    """Returns whether a source path is an inert declaration file."""
    for suffix in _DX_TS_DECLARATION_SUFFIXES:
        if src.endswith(suffix):
            return True
    return False

def typescript_srcs_rejection(srcs):
    """Returns the rejection for forbidden typescript_project srcs, or None."""
    bad = [src for src in srcs or [] if _is_declaration(src)]
    if bad:
        return ("typescript_project takes real sources only; declaration " +
                "files are inert and must not be listed in srcs: " +
                ", ".join(sorted(bad)))
    return None

def _is_declaration_label(src):
    """Returns whether a declaration input is statically a declaration file."""
    text = str(src)
    if ":" in text:
        return True
    return _is_declaration(text)

def typescript_declaration_srcs_rejection(declaration_srcs):
    """Returns the rejection for non-declaration declaration inputs, or None."""
    bad = [str(src) for src in declaration_srcs or [] if not _is_declaration_label(src)]
    if bad:
        return ("typescript_project declaration_srcs takes declaration files " +
                "only; list real sources in srcs instead: " +
                ", ".join(sorted(bad)))
    return None

def typescript_upstream_srcs(srcs, declaration_srcs):
    """Returns the combined upstream sources for real plus declaration inputs."""
    return list(srcs or []) + list(declaration_srcs or [])

def typescript_empty_project_rejection(srcs, declaration_srcs):
    """Returns the rejection for a project with neither srcs nor declaration_srcs, or None."""
    if not srcs and not declaration_srcs:
        return "typescript_project needs srcs or declaration_srcs"
    return None

def typescript_transpiler(transpiler):
    """Returns tsc when no transpiler is set, else the value unchanged."""
    if transpiler == None:
        return "tsc"
    return transpiler

def typescript_inner_kwargs(tags, kwargs):
    """Returns the tags and target_compatible_with kwargs for inner targets."""
    out = {}
    inner_tags = tags if tags != None else kwargs.get("tags", None)
    if inner_tags != None:
        out["tags"] = list(inner_tags)
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

_DX_TSCONFIG_EXTS = (".ts", ".tsx", ".mts", ".cts")

def dx_tsconfig_is_source(basename):
    """Returns whether a basename is listed in a scoped tsconfig files array."""
    return basename.endswith(_DX_TSCONFIG_EXTS)

def dx_tsconfig_relpath(from_path, to_path):
    """Returns the relative path from one file to another file."""
    from_segs = from_path.split("/")[:-1]
    to_segs = to_path.split("/")
    common = 0
    for i in range(min(len(from_segs), len(to_segs))):
        if from_segs[i] != to_segs[i]:
            break
        common += 1
    rel = [".."] * (len(from_segs) - common) + to_segs[common:]
    path = "/".join(rel)
    if not path.startswith("../"):
        path = "./" + path
    return path

def _dx_write_tsconfig_impl(ctx):
    extends_path = dx_tsconfig_relpath(ctx.outputs.out.short_path, ctx.file.extends.short_path)
    local_package_prefix = "%s/" % ctx.label.package if ctx.label.package else ""
    if len(ctx.label.repo_name) > 0:
        local_package_prefix = "../{}/{}".format(ctx.label.repo_name, local_package_prefix)
    path_to_root = "/".join([".."] * (ctx.label.package.count("/") + 1))
    local_package_prefix_len = len(local_package_prefix)
    root_prefix = "./%s/" % path_to_root
    src_files = []
    for f in ctx.files.files:
        if not dx_tsconfig_is_source(f.basename):
            continue
        short_path = f.short_path
        if short_path.startswith(local_package_prefix):
            src_files.append("./" + short_path[local_package_prefix_len:])
        else:
            src_files.append(root_prefix + short_path)
    ctx.actions.write(
        output = ctx.outputs.out,
        content = json.encode({"extends": extends_path, "files": src_files, "include": []}),
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

_dx_tsconfig = rule(
    implementation = _dx_write_tsconfig_impl,
    attrs = {
        "extends": attr.label(
            doc = "Inherited tsconfig file named in extends.",
            allow_single_file = True,
            mandatory = True,
        ),
        "files": attr.label_list(
            doc = "TypeScript sources listed in the files array.",
            allow_files = True,
        ),
        "out": attr.output(
            doc = "Generated scoped tsconfig file.",
            mandatory = True,
        ),
    },
    doc = "Writes a scoped tsconfig naming an inherited file and these sources with an empty include.",
)

def _dx_write_tsconfig(name, files, out, extends, **kwargs):
    """Instantiates the scoped tsconfig writer in the package directory."""
    if out.find("/") >= 0:
        fail("tsconfig should be generated in the package directory, to make relative pathing simple")
    _dx_tsconfig(
        name = name,
        files = files,
        extends = extends,
        out = out,
        **kwargs
    )

def typescript_scoped_tsconfig_rejection(kwargs):
    """Returns the rejection for unsupported scoped-tsconfig kwargs, or None."""
    tsconfig = kwargs.get("tsconfig", None)
    if tsconfig == None:
        return None
    if type(tsconfig) == "dict":
        return ("typescript_project scopes its own sources through a generated " +
                "tsconfig; a dictionary tsconfig is not supported: pass a " +
                "tsconfig file instead.")
    isolated = kwargs.get("isolated_typecheck", None)
    if isolated != None and type(isolated) != "bool" and type(isolated) != "select":
        return ("typescript_project isolated_typecheck must be True or False; " +
                "got a value that is neither.")
    extends = kwargs.get("extends", None)
    if extends != None and extends != tsconfig:
        return ("typescript_project derives extends from tsconfig; omit extends " +
                "or name the same file as tsconfig.")
    return None

def dx_scoped_tsconfig_out(name, kwargs):
    """Returns scoped-tsconfig kwargs with the documented typecheck default."""
    tsconfig = kwargs.get("tsconfig", None)
    if tsconfig == None:
        return dict(kwargs)
    generated = "tsconfig_" + name + ".json"
    out = dict(kwargs)
    out["tsconfig"] = generated
    out["extends"] = tsconfig
    if out.get("isolated_typecheck", None) == None:
        out["isolated_typecheck"] = True
    return out

def _dx_scoped_tsconfig(name, srcs, kwargs):
    """Returns kwargs with a tsconfig that lists only this target's own sources."""
    rejection = typescript_scoped_tsconfig_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    tsconfig = kwargs.get("tsconfig", None)
    if tsconfig == None:
        return dict(kwargs)

    generated = "tsconfig_" + name
    _dx_write_tsconfig(
        name = generated,
        files = srcs,
        extends = tsconfig,
        out = generated + ".json",
        visibility = ["//visibility:private"],
        **typescript_inner_kwargs(None, kwargs)
    )

    return dx_scoped_tsconfig_out(name, kwargs)

def _typescript_wrap_project(name, srcs = None, declaration_srcs = None, visibility = None, **kwargs):
    empty_rejection = typescript_empty_project_rejection(srcs, declaration_srcs)
    if empty_rejection != None:
        fail(empty_rejection)
    rejection = typescript_srcs_rejection(srcs)
    if rejection != None:
        fail(rejection)
    decl_rejection = typescript_declaration_srcs_rejection(declaration_srcs)
    if decl_rejection != None:
        fail(decl_rejection)
    upstream_srcs = typescript_upstream_srcs(srcs, declaration_srcs)
    scoped = _dx_scoped_tsconfig(name, upstream_srcs, kwargs)
    upstream_kwargs = dict(scoped)
    upstream_kwargs["srcs"] = upstream_srcs
    upstream_kwargs["transpiler"] = typescript_transpiler(scoped.get("transpiler", None))
    upstream_kwargs["visibility"] = ["//visibility:private"]
    upstream_kwargs["tags"] = dx_upstream_tags(scoped.get("tags", None))
    _ts_project(
        name = name + "_upstream",
        **upstream_kwargs
    )
    inner_kwargs = typescript_inner_kwargs(None, scoped)
    native.filegroup(
        name = name + "_typecheck",
        srcs = [":" + name + "_upstream"],
        output_group = "typecheck",
        visibility = ["//visibility:private"],
        **dict(inner_kwargs, tags = upstream_kwargs["tags"], **({"testonly": scoped["testonly"]} if scoped.get("testonly", None) != None else {}))
    )
    build_test(
        name = name + "_typecheck_test",
        targets = [":" + name + "_typecheck"],
        size = "small",
        visibility = visibility,
        **inner_kwargs
    )
    forward_kwargs = {}
    if scoped.get("aspect_hints", None) != None:
        forward_kwargs["aspect_hints"] = scoped["aspect_hints"]
    if scoped.get("hdrs", None) != None:
        forward_kwargs["hdrs"] = scoped["hdrs"]
    if scoped.get("tags", None) != None:
        forward_kwargs["tags"] = scoped["tags"]
    if scoped.get("testonly", None) != None:
        forward_kwargs["testonly"] = scoped["testonly"]
    _typescript_project_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = list(srcs or []),
        visibility = visibility,
        **forward_kwargs
    )

def typescript_project(name, srcs = None, declaration_srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over ts_project, isolated typecheck on by default, transpiler defaults to tsc; srcs may be omitted for a declaration-only project, declaration_srcs holds declaration inputs outside quality ownership the upstream is manual with the rewritten tags, and <name>_typecheck_test gets the user's tags."""
    _typescript_wrap_project(name, srcs, declaration_srcs = declaration_srcs, visibility = visibility, **kwargs)

_typescript_test = node_test_forward_rule("typescript_*", _DX_TS_SOURCE_EXTS, _DX_TS_SOURCE_SPECS)

def typescript_test_rejection(kwargs):
    """Returns the rejection for forbidden typescript_test kwargs, or None."""
    return node_test_reporter_rejection("typescript_test", "typescript_js_test", kwargs)

def typescript_js_test_rejection(kwargs):
    """Returns the rejection for forbidden typescript_js_test kwargs, or None."""
    return node_test_plain_rejection("typescript_test", "typescript_js_test", kwargs)

def _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, declaration_srcs, kwargs):
    """Declares the private test-only ts_project for one TypeScript test."""
    ts_kwargs = {}
    if deps != None:
        ts_kwargs["deps"] = list(deps)
    if tsconfig != None:
        ts_kwargs["tsconfig"] = tsconfig
    ts_kwargs["transpiler"] = typescript_transpiler(transpiler)
    ts_kwargs.update(typescript_inner_kwargs(None, kwargs))
    if ts_kwargs.get("tags") != None:
        ts_kwargs["tags"] = upstream_tags(ts_kwargs["tags"])
    if declaration != None:
        ts_kwargs["declaration"] = declaration
    upstream_srcs = typescript_upstream_srcs(srcs, declaration_srcs)
    _ts_project(
        name = name + "_ts",
        srcs = upstream_srcs,
        testonly = True,
        visibility = ["//visibility:private"],
        **_dx_scoped_tsconfig(name + "_ts", upstream_srcs, ts_kwargs)
    )

def typescript_test(name, srcs, node_modules, data = None, deps = None, tsconfig = None, transpiler = None, declaration = None, declaration_srcs = None, package_json = None, visibility = None, env_inherit = None, **kwargs):
    """Minimal wrapper over jest_test for TypeScript sources; declaration_srcs holds declaration inputs outside quality ownership."""
    rejection = typescript_srcs_rejection(srcs)
    if rejection != None:
        fail(rejection)
    decl_rejection = typescript_declaration_srcs_rejection(declaration_srcs)
    if decl_rejection != None:
        fail(decl_rejection)
    reporter_rejection = typescript_test_rejection(kwargs)
    if reporter_rejection != None:
        fail(reporter_rejection)
    _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, declaration_srcs, kwargs)
    node_wrap_test(name, _jest_test, _typescript_test, srcs, [":" + name + "_ts"] + list(deps or []) + list(data or []), package_json, {"node_modules": node_modules}, env_inherit, visibility, kwargs)

def typescript_js_test(name, srcs, entry_point, data = None, deps = None, tsconfig = None, transpiler = None, declaration = None, declaration_srcs = None, package_json = None, visibility = None, env_inherit = None, **kwargs):
    """Minimal wrapper over js_test for TypeScript sources; declaration_srcs holds declaration inputs outside quality ownership."""
    rejection = typescript_srcs_rejection(srcs)
    if rejection != None:
        fail(rejection)
    decl_rejection = typescript_declaration_srcs_rejection(declaration_srcs)
    if decl_rejection != None:
        fail(decl_rejection)
    reporter_rejection = typescript_js_test_rejection(kwargs)
    if reporter_rejection != None:
        fail(reporter_rejection)
    _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, declaration_srcs, kwargs)
    node_wrap_test(name, _js_test, _typescript_test, srcs, [":" + name + "_ts"] + list(deps or []) + list(data or []), package_json, {"entry_point": entry_point}, env_inherit, visibility, kwargs)
