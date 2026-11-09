"""Minimal TypeScript wrappers."""

load("@aspect_rules_jest//jest:defs.bzl", _jest_test = "jest_test")
load("@aspect_rules_js//js:defs.bzl", _js_binary = "js_binary", _js_test = "js_test")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("@aspect_rules_ts//ts:defs.bzl", _TsConfigInfo = "TsConfigInfo", _ts_project = "ts_project")
load("//javascript/rules:defs.bzl", "javascript_binary")
load("//libs/starlark:fuzz.bzl", "dx_fuzz_test")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//libs/starlark:node_test.bzl", "node_jest_upstream_attrs", "node_test_forward_rule", "node_test_runner_rejection", "node_test_wrap_args")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap_test")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":kwargs.bzl", "typescript_forward_kwargs", "typescript_inner_kwargs")
load(":proto.bzl", "ts_proto_srcs")

_DX_TS_LIBRARY_PROVIDES = [
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

_typescript_library_forward = dx_library_forward_rule(
    provides = _DX_TS_LIBRARY_PROVIDES,
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

def typescript_srcs_rejection(srcs, rule = "typescript_library"):
    """Returns the rejection for forbidden srcs of the named rule, or None."""
    bad = [src for src in srcs or [] if _is_declaration(src)]
    if bad:
        return (rule + " takes real sources only; declaration " +
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
        return ("typescript_library declaration_srcs takes declaration files " +
                "only; list real sources in srcs instead: " +
                ", ".join(sorted(bad)))
    return None

def typescript_upstream_srcs(srcs, declaration_srcs):
    """Returns the combined upstream sources for real plus declaration inputs."""
    return list(srcs or []) + list(declaration_srcs or [])

def typescript_empty_library_rejection(srcs, declaration_srcs):
    """Returns the rejection for a project with neither srcs nor declaration_srcs, or None."""
    if not srcs and not declaration_srcs:
        return "typescript_library needs srcs or declaration_srcs"
    return None

def typescript_transpiler(transpiler):
    """Returns tsc when no transpiler is set, else the value unchanged."""
    if transpiler == None:
        return "tsc"
    return transpiler

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

def dx_tsconfig_content(extends_path, files, declaration):
    """Returns the scoped tsconfig object; declaration mirrors the ts_project attribute."""
    content = {"extends": extends_path, "files": files, "include": []}
    if declaration:
        content["compilerOptions"] = {"declaration": True}
    return content

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
        content = json.encode(dx_tsconfig_content(extends_path, src_files, ctx.attr.declaration)),
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

_dx_tsconfig = rule(
    implementation = _dx_write_tsconfig_impl,
    attrs = {
        "declaration": attr.bool(
            doc = "Writes compilerOptions.declaration so it matches the ts_project attribute.",
        ),
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

def _dx_write_tsconfig(name, files, out, extends, declaration = False, **kwargs):
    """Instantiates the scoped tsconfig writer in the package directory."""
    if out.find("/") >= 0:
        fail("tsconfig should be generated in the package directory, to make relative pathing simple")
    _dx_tsconfig(
        name = name,
        files = files,
        extends = extends,
        declaration = declaration,
        out = out,
        **kwargs
    )

def typescript_scoped_tsconfig_rejection(kwargs):
    """Returns the rejection for unsupported scoped-tsconfig kwargs, or None."""
    tsconfig = kwargs.get("tsconfig", None)
    if tsconfig == None:
        return None
    if type(tsconfig) == "dict":
        return ("typescript_library scopes its own sources through a generated " +
                "tsconfig; a dictionary tsconfig is not supported: pass a " +
                "tsconfig file instead.")
    isolated = kwargs.get("isolated_typecheck", None)
    if isolated != None and type(isolated) != "bool" and type(isolated) != "select":
        return ("typescript_library isolated_typecheck must be True or False; " +
                "got a value that is neither.")
    extends = kwargs.get("extends", None)
    if extends != None and extends != tsconfig:
        return ("typescript_library derives extends from tsconfig; omit extends " +
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
        declaration = kwargs.get("declaration", None) == True,
        visibility = ["//visibility:private"],
        **typescript_inner_kwargs(None, kwargs)
    )

    return dx_scoped_tsconfig_out(name, kwargs)

def _typescript_wrap_library(name, srcs = None, declaration_srcs = None, visibility = None, **kwargs):
    empty_rejection = typescript_empty_library_rejection(srcs, declaration_srcs)
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
    forward_kwargs = typescript_forward_kwargs(scoped)
    _typescript_library_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = list(srcs or []),
        visibility = visibility,
        **forward_kwargs
    )

def typescript_library(name, srcs = None, declaration_srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over ts_project, isolated typecheck on by default, transpiler defaults to tsc; srcs may be omitted for a declaration-only project, declaration_srcs holds declaration inputs outside quality ownership the upstream is manual with the rewritten tags."""
    _typescript_wrap_library(name, srcs, declaration_srcs = declaration_srcs, visibility = visibility, **kwargs)

def typescript_proto_library(name, proto, deps = None, visibility = None, **kwargs):
    """Compiles a proto_library to TypeScript with protoc-gen-es; deps should hold @bufbuild/protobuf; proto must sit in the proto_library's package."""
    ts_proto_srcs(
        name = name + "_srcs",
        proto = proto,
        visibility = ["//visibility:private"],
        tags = kwargs.get("tags", None),
        testonly = kwargs.get("testonly", None),
    )
    if kwargs.get("tsconfig", None) != None and kwargs.get("isolated_typecheck", None) == None:
        kwargs["isolated_typecheck"] = False
    typescript_library(
        name = name,
        srcs = [":" + name + "_srcs"],
        deps = deps,
        visibility = visibility,
        **kwargs
    )

_typescript_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_TS_SOURCE_SPECS,
    what = "typescript_*",
    allow_files = _DX_TS_SOURCE_EXTS,
    upstream_providers = [[DefaultInfo]],
    runtime = "besteffort",
)

_TS_ENTRY_EXTS = {".ts": ".js", ".mts": ".mjs", ".cts": ".cjs"}

def typescript_binary_main(srcs, main):
    """Returns the entry source of a typescript_binary: main, or the only src."""
    if main != None:
        return main
    if len(srcs or []) == 1:
        return srcs[0]
    return None

def typescript_binary_rejection(srcs, main):
    """Returns the rejection for bad typescript_binary srcs or main, or None."""
    if not srcs:
        return "typescript_binary needs srcs"
    entry = typescript_binary_main(srcs, main)
    if entry == None:
        return "typescript_binary needs main when srcs has more than one file"
    if entry not in srcs:
        return "typescript_binary main must be one of srcs: " + str(entry)
    if typescript_compiled_entry(entry) == None:
        return "typescript_binary main must end in .ts, .mts or .cts: " + str(entry)
    return None

def typescript_binary_kwargs_rejection(kwargs):
    """Returns the rejection for a user entry_point on typescript_binary, or None."""
    if kwargs.get("entry_point", None) != None:
        return "typescript_binary derives entry_point from main; set main instead of entry_point"
    return None

def typescript_compiled_entry(main):
    """Maps a .ts, .mts or .cts entry to its compiled .js, .mjs or .cjs path, or None."""
    for ext, out in _TS_ENTRY_EXTS.items():
        if main.endswith(ext) and not _is_declaration(main):
            return main[:-len(ext)] + out
    return None

def typescript_binary_upstream_kwargs(name, entry, deps, data, kwargs):
    """Returns the private js_binary kwargs: args and data forwarded, env held back for the forwarder."""
    out = dx_binary_upstream_kwargs(kwargs, None)
    out.pop("aspect_hints", None)
    out["entry_point"] = ":" + typescript_compiled_entry(entry).lstrip(":")
    out["data"] = [":" + name + "_ts"] + list(deps or []) + list(data or [])
    return out

def typescript_binary_forward_kwargs(data, kwargs):
    """Returns the forwarder kwargs: args, env and env_inherit, with data so runfiles expansion resolves."""
    run_kwargs = dict(kwargs)
    if data != None:
        run_kwargs["data"] = list(data)
    return dx_binary_forward_kwargs(run_kwargs)

def typescript_binary(name, *, srcs = None, main = None, deps = None, data = None, tsconfig = None, transpiler = None, declaration = None, visibility = None, **kwargs):
    """Minimal wrapper over js_binary for TypeScript sources: compiles srcs with a private ts_project and runs the compiled main. main defaults to the only src."""
    srcs = list(srcs or [])
    rejection = typescript_binary_kwargs_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    rejection = typescript_srcs_rejection(srcs, "typescript_binary")
    if rejection != None:
        fail(rejection)
    rejection = typescript_binary_rejection(srcs, main)
    if rejection != None:
        fail(rejection)
    entry = typescript_binary_main(srcs, main)
    _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, None, kwargs, testonly = kwargs.get("testonly", None))
    upstream_kwargs = typescript_binary_upstream_kwargs(name, entry, deps, data, kwargs)
    _js_binary(
        name = name + "_upstream",
        **upstream_kwargs
    )
    _typescript_binary_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **typescript_binary_forward_kwargs(data, kwargs)
    )

_typescript_test = node_test_forward_rule("typescript_*", _DX_TS_SOURCE_EXTS, _DX_TS_SOURCE_SPECS)

def typescript_test_rejection(runner, node_modules, main, kwargs):
    """Returns the rejection for bad typescript_test arguments, or None."""
    return node_test_runner_rejection("typescript_test", runner, node_modules, main, kwargs)

def _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, declaration_srcs, kwargs, testonly = True):
    """Declares the private ts_project for one TypeScript test or binary."""
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
        testonly = testonly,
        visibility = ["//visibility:private"],
        **_dx_scoped_tsconfig(name + "_ts", upstream_srcs, ts_kwargs)
    )

def typescript_test(name, *, srcs = None, runner = "jest", node_modules = None, main = None, data = None, deps = None, tsconfig = None, transpiler = None, declaration = None, declaration_srcs = None, package_json = None, visibility = None, env_inherit = None, **kwargs):
    """Minimal wrapper over jest_test, or js_test with runner = "node", for TypeScript sources. runner defaults to "jest"; declaration_srcs are outside quality ownership."""
    srcs = srcs or []
    rejection = typescript_srcs_rejection(srcs, "typescript_test")
    if rejection != None:
        fail(rejection)
    decl_rejection = typescript_declaration_srcs_rejection(declaration_srcs)
    if decl_rejection != None:
        fail(decl_rejection)
    runner_rejection = typescript_test_rejection(runner, node_modules, main, kwargs)
    if runner_rejection != None:
        fail(runner_rejection)
    _typescript_test_project(name, srcs, deps, tsconfig, transpiler, declaration, declaration_srcs, kwargs)
    if runner == "node":
        upstream = _js_test
        upstream_kwargs = {"entry_point": main}
    else:
        upstream = _jest_test
        upstream_kwargs = node_jest_upstream_attrs(node_modules, kwargs)
    a = node_test_wrap_args(name, srcs, [":" + name + "_ts"] + list(deps or []) + list(data or []), package_json, upstream_kwargs, env_inherit, kwargs)
    dx_wrap_test(name, upstream, _typescript_test, None, visibility = visibility, quality_srcs = srcs, upstream_kwargs = a.upstream_kwargs, extra_forward_kwargs = a.extra_forward_kwargs, **a.kwargs)

def _compiled_path(main):
    if not main.endswith(".ts"):
        fail("%s must be a .ts file" % main)
    package = native.package_name()
    return (package + "/" if package else "") + main[:-len(".ts")] + ".js"

def typescript_bench(name, *, srcs, node_modules, main = None, data = None, args = None, tags = None, **kwargs):
    """A typescript_test run by dx bench. `main`, the first of srcs by default, exports a tinybench Bench as its default export; its results land in dx-bench.json. `node_modules` must link tinybench."""
    typescript_test(
        name = name,
        srcs = srcs,
        runner = "node",
        main = Label("//javascript/rules:bench_driver"),
        data = list(data or []) + [node_modules],
        args = [_compiled_path(main or srcs[0])] + list(args or []),
        tags = merge_tags(tags, "dx-bench", "manual"),
        **with_default_size(kwargs)
    )

def typescript_fuzz_test(name, *, srcs, node_modules, main = None, data = None, deps = None, visibility = None, tags = None, **kwargs):
    """A Jazzer.js fuzz target plus a `<name>_run` runner for `dx fuzz`. `main`, the first of srcs by default, exports `fuzz(data)`. `node_modules` must link @jazzer.js/core."""
    kwargs = dict(kwargs)
    fixed_args = [_compiled_path(main or srcs[0])] + list(kwargs.pop("fixed_args", []))
    dx_fuzz_test(
        name,
        typescript_library,
        {"srcs": srcs, "deps": deps},
        tags = tags,
        visibility = visibility,
        fuzz_rule = javascript_binary,
        fuzz_kwargs = {
            "main": Label("//javascript/rules:fuzz_driver"),
            "data": [":" + name + "_bin", node_modules] + list(data or []),
            "fixed_args": fixed_args,
        },
        **kwargs
    )

_playwright_test = node_test_forward_rule("typescript_playwright_test", _DX_TS_SOURCE_EXTS, _DX_TS_SOURCE_SPECS)

def typescript_playwright_test(name, *, srcs, config, node_modules, app = None, data = None, package_json = None, visibility = None, env_inherit = None, **kwargs):
    """Runs Playwright tests in a pinned headless Chromium; app is started by the config's webServer from DX_PLAYWRIGHT_APP on DX_PLAYWRIGHT_PORT."""
    rejection = typescript_srcs_rejection(srcs, "typescript_playwright_test")
    if rejection != None:
        fail(rejection)
    kwargs["env"] = dict(kwargs.pop("env", None) or {})
    env = kwargs["env"]
    env["DX_PLAYWRIGHT_CONFIG"] = "$(rlocationpath %s)" % config
    env["DX_PLAYWRIGHT_MARKER"] = "$(rlocationpath %s)" % Label("@dx_playwright_chromium//:marker")
    upstream_data = [config, Label("@dx_playwright_chromium//:browsers"), Label("@dx_playwright_chromium//:marker"), node_modules] + list(srcs) + list(data or [])
    if app != None:
        env["DX_PLAYWRIGHT_APP"] = "$(rlocationpath %s)" % app
        upstream_data.append(app)
    upstream_kwargs = {
        "entry_point": Label("//typescript/playwright:runner"),
        "no_copy_to_bin": [Label("@dx_playwright_chromium//:browsers"), Label("@dx_playwright_chromium//:marker")],
    }
    a = node_test_wrap_args(name, srcs, upstream_data, package_json, upstream_kwargs, env_inherit, kwargs)
    dx_wrap_test(name, _js_test, _playwright_test, None, visibility = visibility, quality_srcs = srcs, upstream_kwargs = a.upstream_kwargs, extra_forward_kwargs = a.extra_forward_kwargs, **a.kwargs)
