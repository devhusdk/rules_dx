"""Relocatable browser applications with a managed development server."""

load("@rules_rust_wasm_bindgen//:providers.bzl", _RustWasmBindgenInfo = "RustWasmBindgenInfo")
load("//deploy/rules:resources.bzl", "resource_mapping_error", "resource_path_error")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:resources.bzl", "dx_bundler_resources")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load("//libs/web_server:server.bzl", "JavascriptWebAppInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

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
    for module in info.js.to_list():
        if module != entry:
            _web_stage_file(ctx, prefix, staged, module)
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
    doc = "Stages one browser app directory from HTML, bindgen output and assets.",
    provides = [DefaultInfo, JavascriptWebAppInfo, QualitySourcesInfo],
    attrs = {
        "assets": attr.label_keyed_string_dict(
            doc = "Asset targets mapped to the logical paths they are served at.",
            allow_files = True,
        ),
        "bindgen": attr.label(
            doc = "rust_binary(wasm = \"browser\") target providing the Wasm and JS glue.",
            mandatory = True,
        ),
        "html": attr.label(
            doc = "HTML entry page copied into the staged app.",
            mandatory = True,
            allow_single_file = True,
        ),
        "threads": attr.bool(
            doc = "Set when the Wasm build uses threads and needs cross origin isolation.",
            default = False,
        ),
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
    tool = ctx.attr.bundler[DefaultInfo].files_to_run
    if tool.executable == None:
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
        executable = ctx.attr.bundler[DefaultInfo].files_to_run,
        inputs = depset(inputs),
        outputs = [out],
        arguments = arguments,
        mnemonic = "DxWebBundle",
        resource_set = dx_bundler_resources,
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
    doc = "Bundles one staged app with a declared bundler.",
    attrs = {
        "app": attr.label(
            doc = "Staged browser app directory from javascript_web_app to bundle.",
            mandatory = True,
            providers = [JavascriptWebAppInfo],
        ),
        "bundler": attr.label(
            doc = "Executable bundler tool run to produce the bundle.",
            mandatory = True,
            cfg = "exec",
            executable = True,
        ),
        "bundler_args": attr.string_list(
            doc = "Bundler arguments with path placeholders, one of which must stage the output.",
        ),
        "config": attr.label(
            doc = "Optional config file substituted for the CONFIG placeholder.",
            allow_single_file = True,
        ),
        "entry": attr.label(
            doc = "Optional entry module substituted for the ENTRY placeholder.",
            allow_single_file = True,
        ),
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
