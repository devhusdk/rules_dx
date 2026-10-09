"""Relocatable browser applications with a managed development server."""

load("@rules_rust_wasm_bindgen//:providers.bzl", _RustWasmBindgenInfo = "RustWasmBindgenInfo")
load("//deploy/rules:resources.bzl", "resource_mapping_error", "resource_path_error")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load("//python/rules:defs.bzl", "python_binary")
load("//quality:sources.bzl", "QualitySourcesInfo")

JavascriptWebAppInfo = provider(
    doc = "Staged browser application identity for dx web servers and bundlers.",
    fields = {
        "entry": "String: staged entry JavaScript module basename.",
        "entry_file": "File or None: staged entry module, None for bundled trees.",
        "files": "Depset of Files: every staged application file.",
        "index_file": "File or None: staged index.html, None for bundled trees.",
        "snippets": "File or None: staged snippets directory, None for bundled trees.",
        "threads": "Bool: True when the Wasm build needs cross-origin isolation.",
        "wasm": "String: staged Wasm module basename.",
        "wasm_file": "File or None: staged Wasm module, None for bundled trees.",
    },
)

_WEB_APP_SOURCE_SPECS = [("text", "txt")]

_WEB_JS_EXTENSIONS = ["js", "mjs"]

def _web_bindgen_entry(ctx, info):
    """Returns the single servable entry module, or fails."""
    modules = [f for f in info.js.to_list() if f.extension in _WEB_JS_EXTENSIONS]
    entry = [f for f in modules if not f.basename.endswith("_bg.js")]
    if len(info.js.to_list()) == 0:
        fail("javascript_web_app " + str(ctx.label) + ": " +
             display_label(ctx.attr.bindgen.label) + " produces no JavaScript output")
    if len(entry) == 0:
        fail("javascript_web_app " + str(ctx.label) + ": " +
             display_label(ctx.attr.bindgen.label) + " produces no servable entry module")
    if len(entry) > 1:
        fail("javascript_web_app " + str(ctx.label) + ": " +
             display_label(ctx.attr.bindgen.label) + " produces more than one entry module")
    return entry[0]

def _web_stage_file(ctx, prefix, staged, src):
    """Symlinks one source file under the staged prefix."""
    out = ctx.actions.declare_file(prefix + src.basename)
    ctx.actions.symlink(output = out, target_file = src)
    staged.append(out)
    return out

def _javascript_web_app_impl(ctx):
    """Stages one relocatable browser directory from HTML, bindgen, and assets."""
    prefix = ctx.label.name + "/"
    staged = []
    html = ctx.file.html
    index = ctx.actions.declare_file(prefix + "index.html")
    ctx.actions.symlink(output = index, target_file = html)
    staged.append(index)
    bindgen = ctx.attr.bindgen
    if _RustWasmBindgenInfo not in bindgen:
        fail("javascript_web_app " + str(ctx.label) +
             ": javascript_web_app requires bindgen to provide RustWasmBindgenInfo")
    info = bindgen[_RustWasmBindgenInfo]
    entry = _web_bindgen_entry(ctx, info)
    if info.wasm == None:
        fail("javascript_web_app " + str(ctx.label) + ": " +
             display_label(bindgen.label) + " produces no Wasm output")
    if info.snippets == None:
        fail("javascript_web_app " + str(ctx.label) + ": " +
             display_label(bindgen.label) + " produces no snippets directory")
    entry_out = _web_stage_file(ctx, prefix, staged, entry)
    wasm_out = _web_stage_file(ctx, prefix, staged, info.wasm)
    snippets_out = ctx.actions.declare_directory(prefix + info.snippets.basename)
    ctx.actions.symlink(output = snippets_out, target_file = info.snippets)
    staged.append(snippets_out)
    entries = []
    for target in ctx.attr.assets:
        logical = ctx.attr.assets[target]
        path_error = resource_path_error(logical)
        if path_error != "":
            fail("javascript_web_app " + str(ctx.label) + ": invalid path '" + logical +
                 "' from " + display_label(target.label) + ": " + path_error)
        entries.append((target, logical))
    mapping_error = resource_mapping_error([item[1] for item in entries])
    if mapping_error != "":
        fail("javascript_web_app " + str(ctx.label) + ": " + mapping_error)
    by_path = {}
    for item in entries:
        by_path[item[1]] = item[0]
    asset_sources = []
    for logical in sorted(by_path.keys()):
        target = by_path[logical]
        files = target[DefaultInfo].files.to_list()
        if len(files) == 0:
            fail("javascript_web_app " + str(ctx.label) + ": " + display_label(target.label) +
                 " provides no files")
        if len(files) != 1:
            fail("javascript_web_app " + str(ctx.label) + ": " + display_label(target.label) +
                 " must provide exactly one file")
        out = ctx.actions.declare_file(prefix + logical)
        ctx.actions.symlink(output = out, target_file = files[0])
        staged.append(out)
        asset_sources.append(files[0])
    manifest = ctx.actions.declare_file(prefix + "manifest.json")
    ctx.actions.write(
        output = manifest,
        content = json.encode({
            "entry": entry.basename,
            "version": 1,
            "wasm": info.wasm.basename,
            "threads": ctx.attr.threads,
        }) + "\n",
    )
    staged.append(manifest)
    return [
        DefaultInfo(
            files = depset(staged),
            runfiles = ctx.runfiles(files = staged),
        ),
        JavascriptWebAppInfo(
            entry = entry.basename,
            entry_file = entry_out,
            files = depset(staged),
            index_file = index,
            snippets = snippets_out,
            threads = ctx.attr.threads,
            wasm = info.wasm.basename,
            wasm_file = wasm_out,
        ),
        dx_quality_sources([html] + asset_sources, _WEB_APP_SOURCE_SPECS, str(ctx.label)),
    ]

_javascript_web_app = rule(
    implementation = _javascript_web_app_impl,
    provides = [DefaultInfo, JavascriptWebAppInfo, QualitySourcesInfo],
    attrs = {
        "assets": attr.label_keyed_string_dict(allow_files = True),
        "bindgen": attr.label(mandatory = True),
        "html": attr.label(mandatory = True, allow_single_file = True),
        "threads": attr.bool(default = False),
    },
)

def javascript_web_app(name, html, bindgen, assets = None, threads = False, visibility = None, **kwargs):
    """Stages one relocatable browser directory from HTML, bindgen, and assets."""
    _javascript_web_app(
        name = name,
        html = html,
        bindgen = bindgen,
        assets = assets or {},
        threads = threads,
        visibility = visibility,
        **kwargs
    )

_WEB_LAUNCHER_RUNFILES = [
    "rules_dx/javascript/rules/web_server.py",
    "_main/javascript/rules/web_server.py",
    "javascript/rules/web_server.py",
]

def _web_launcher_candidates(repo, package, app):
    """Returns the runfile candidates naming one staged index file."""
    middle = (package + "/") if package != "" else ""
    tail = middle + app + "/index.html"
    if repo == "":
        return ["rules_dx/" + tail, "_main/" + tail, tail]
    return [repo + "/" + tail]

def _web_py_list(values):
    """Renders one format-stable Python string list."""
    if len(values) == 0:
        return "[]"
    return "[\n" + "\n".join(["    " + repr(value) + "," for value in values]) + "\n]"

def _web_header_flag(name, value):
    """Renders one baked server header argument."""
    return "--header=" + name + ": " + value

def _javascript_web_server_launcher_impl(ctx):
    """Writes one baked development-server launcher for a staged application."""
    info = ctx.attr.app[JavascriptWebAppInfo]
    if info.threads and not ctx.attr.cross_origin_isolated:
        fail("javascript_web_server " + str(ctx.label) + ": app " +
             display_label(ctx.attr.app.label) +
             " builds threaded Wasm and requires cross_origin_isolated = True")
    baked = []
    for name in sorted(ctx.attr.headers.keys()):
        if name == "" or ":" in name:
            fail("javascript_web_server " + str(ctx.label) + ": invalid header name '" +
                 name + "'")
        baked.append(_web_header_flag(name, ctx.attr.headers[name]))
    if ctx.attr.cross_origin_isolated:
        baked.append("--cross-origin-isolated")
    baked = baked + ["--host=" + ctx.attr.host, "--port=" + str(ctx.attr.port)]
    launcher = ctx.actions.declare_file(ctx.label.name + ".launch.py")
    ctx.actions.write(
        output = launcher,
        content = "\n".join([
            "import os",
            "import sys",
            "",
            "from python.runfiles.runfiles import Create",
            "",
            "_SCRIPT_CANDIDATES = " + _web_py_list(_WEB_LAUNCHER_RUNFILES),
            "_INDEX_CANDIDATES = " + _web_py_list(_web_launcher_candidates(
                ctx.attr.app.label.workspace_name,
                ctx.attr.app.label.package,
                ctx.attr.app.label.name,
            )),
            "_BAKED = " + _web_py_list(baked),
            "",
            "",
            "def _resolve(runfiles, candidates):",
            "    for candidate in candidates:",
            "        path = runfiles.Rlocation(candidate)",
            "        if path is not None and os.path.isfile(path):",
            "            return path",
            "    return None",
            "",
            "",
            "def main():",
            "    runfiles = Create()",
            "    if runfiles is None:",
            "        print(\"javascript_web_server: no runfiles\", file=sys.stderr)",
            "        return 2",
            "    script = _resolve(runfiles, _SCRIPT_CANDIDATES)",
            "    if script is None:",
            "        print(\"javascript_web_server: cannot locate web_server.py\", file=sys.stderr)",
            "        return 2",
            "    index = _resolve(runfiles, _INDEX_CANDIDATES)",
            "    if index is None:",
            "        print(\"javascript_web_server: cannot locate index.html\", file=sys.stderr)",
            "        return 2",
            "    root = os.path.dirname(index)",
            "    argv = [sys.executable, script, \"--root=\" + root] + _BAKED + sys.argv[1:]",
            "    os.execv(sys.executable, argv)",
            "    return 2",
            "",
            "",
            "if __name__ == \"__main__\":",
            "    sys.exit(main())",
            "",
        ]),
    )
    return [DefaultInfo(files = depset([launcher]))]

javascript_web_server_launcher = rule(
    implementation = _javascript_web_server_launcher_impl,
    attrs = {
        "app": attr.label(mandatory = True, providers = [JavascriptWebAppInfo]),
        "cross_origin_isolated": attr.bool(default = False),
        "headers": attr.string_dict(),
        "host": attr.string(default = "127.0.0.1"),
        "port": attr.int(default = 8000),
    },
)

def javascript_web_server(
        name,
        app,
        port = 8000,
        host = "127.0.0.1",
        headers = None,
        cross_origin_isolated = False,
        visibility = None,
        **kwargs):
    """Declares one runnable loopback server for a staged browser application."""
    launcher_kwargs = {}
    for key in ["tags", "testonly"]:
        if key in kwargs:
            launcher_kwargs[key] = kwargs[key]
    javascript_web_server_launcher(
        name = name + "_launcher",
        app = app,
        cross_origin_isolated = cross_origin_isolated,
        headers = headers or {},
        host = host,
        port = port,
        visibility = ["//visibility:private"],
        **launcher_kwargs
    )
    binary_kwargs = dict(kwargs)
    binary_kwargs["data"] = list(kwargs.get("data", [])) + [app, "@rules_dx//javascript/rules:web_server"]
    binary_kwargs["deps"] = list(kwargs.get("deps", [])) + ["@rules_python//python/runfiles:runfiles"]
    launcher = ":" + name + "_launcher"
    binary_kwargs.setdefault("main", launcher)
    python_binary(
        name = name,
        srcs = [launcher],
        visibility = visibility,
        **binary_kwargs
    )

def _web_bundle_arg(value, paths):
    """Substitutes one bundler argument placeholder with a staged path."""
    for token in sorted(paths.keys()):
        value = value.replace(token, paths[token])
    return value

def _javascript_web_bundle_impl(ctx):
    """Bundles one staged application through a declared bundler executable."""
    info = ctx.attr.app[JavascriptWebAppInfo]
    if info.entry_file == None or info.wasm_file == None:
        fail("javascript_web_bundle " + str(ctx.label) + ": app " +
             display_label(ctx.attr.app.label) + " is already bundled")
    exe = ctx.attr.bundler[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("javascript_web_bundle " + str(ctx.label) + ": bundler " +
             display_label(ctx.attr.bundler.label) + " has no executable")
    if ctx.attr.entry:
        entry = ctx.file.entry
    else:
        entry = info.entry_file
    stages_out = False
    for value in ctx.attr.bundler_args:
        if "{OUT}" in value:
            stages_out = True
    if not stages_out:
        fail("javascript_web_bundle " + str(ctx.label) + ": bundler_args must stage {OUT}")
    out = ctx.actions.declare_directory(ctx.label.name)
    paths = {
        "{ENTRY}": entry.path,
        "{INDEX}": info.index_file.path,
        "{OUT}": out.path,
        "{WASM}": info.wasm_file.path,
    }
    if ctx.attr.config:
        paths["{CONFIG}"] = ctx.file.config.path
    arguments = [_web_bundle_arg(value, paths) for value in ctx.attr.bundler_args]
    inputs = [entry, info.index_file, info.wasm_file] + info.files.to_list()
    if ctx.attr.config:
        inputs.append(ctx.file.config)
    ctx.actions.run(
        executable = exe,
        inputs = depset(inputs, transitive = [ctx.attr.bundler[DefaultInfo].default_runfiles.files]),
        outputs = [out],
        arguments = arguments,
        mnemonic = "DxWebBundle",
        progress_message = "Dx bundle web application %{label}",
    )
    return [
        DefaultInfo(files = depset([out])),
        JavascriptWebAppInfo(
            entry = info.entry,
            entry_file = None,
            files = depset([out]),
            index_file = None,
            snippets = None,
            threads = info.threads,
            wasm = info.wasm,
            wasm_file = None,
        ),
    ]

_javascript_web_bundle = rule(
    implementation = _javascript_web_bundle_impl,
    attrs = {
        "app": attr.label(mandatory = True, providers = [JavascriptWebAppInfo]),
        "bundler": attr.label(mandatory = True, cfg = "exec", executable = True),
        "bundler_args": attr.string_list(),
        "config": attr.label(allow_single_file = True),
        "entry": attr.label(allow_single_file = True),
    },
)

def javascript_web_bundle(
        name,
        app,
        bundler,
        config = None,
        entry = None,
        bundler_args = None,
        visibility = None,
        **kwargs):
    """Bundles one staged application through a declared bundler executable."""
    _javascript_web_bundle(
        name = name,
        app = app,
        bundler = bundler,
        bundler_args = bundler_args or [],
        config = config,
        entry = entry,
        visibility = visibility,
        **kwargs
    )
