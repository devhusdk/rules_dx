"""Relocatable browser applications over unbundled modules."""

load("//python/rules:defs.bzl", "python_binary")

_WEB_SERVER_SRC = "//javascript/rules:server.py"

def _web_app_impl(ctx):
    """Stages the bindgen outputs, index page, and assets into one directory."""
    wasm_files = [file for file in ctx.files.bindgen if file.extension == "wasm"]
    if len(wasm_files) != 1:
        fail("web_app: '" + str(ctx.attr.bindgen.label) + "' provides " +
             str(len(wasm_files)) + " .wasm files, want exactly one web-target module")
    js_files = [file for file in ctx.files.bindgen if file.extension == "js"]
    if len(js_files) == 0:
        fail("web_app: '" + str(ctx.attr.bindgen.label) +
             "' provides no .js loader, want one web-target module")
    index = ctx.file.index
    if index.extension != "html":
        fail("web_app: 'index' must be one .html file, got '" + index.basename + "'")
    top_names = {}
    for file in ctx.files.bindgen:
        if file.basename in top_names:
            fail("web_app: duplicate staged name '" + file.basename + "'")
        top_names[file.basename] = file
    asset_names = {}
    for file in ctx.files.assets:
        if file.basename in asset_names:
            fail("web_app: duplicate asset name '" + file.basename + "'")
        asset_names[file.basename] = file
    staged = ctx.actions.declare_directory(ctx.label.name)
    commands = ["mkdir -p \"" + staged.path + "/assets\""]
    for name in sorted(top_names.keys()):
        commands.append("cp -r \"" + top_names[name].path + "\" \"" + staged.path + "/" + name + "\"")
    commands.append("cp \"" + index.path + "\" \"" + staged.path + "/index.html\"")
    for name in sorted(asset_names.keys()):
        commands.append("cp \"" + asset_names[name].path + "\" \"" + staged.path + "/assets/" + name + "\"")
    ctx.actions.run_shell(
        outputs = [staged],
        inputs = ctx.files.bindgen + ctx.files.assets + [index],
        command = " && ".join(commands),
        mnemonic = "StageWebApp",
        progress_message = "Staging web application %{label}",
    )
    return [DefaultInfo(files = depset([staged]))]

_web_app = rule(
    implementation = _web_app_impl,
    attrs = {
        "assets": attr.label_list(allow_files = True),
        "bindgen": attr.label(mandatory = True),
        "index": attr.label(mandatory = True, allow_single_file = True),
    },
)

def web_app(name, bindgen, index, assets = [], visibility = None, tags = None):
    """Stages one relocatable browser directory from web-target bindgen outputs."""
    _web_app(
        name = name,
        bindgen = bindgen,
        index = index,
        assets = assets,
        visibility = visibility,
        tags = tags,
    )

def web_server(
        name,
        app,
        port = 8080,
        headers = {},
        mime_overrides = {},
        threads = False,
        visibility = None,
        **kwargs):
    """Serves one staged web_app over loopback HTTP with declared headers."""
    python_binary(
        name = name,
        srcs = [_WEB_SERVER_SRC],
        main = "server.py",
        data = [app],
        env = {
            "DX_WEB_APP": "$(rootpath %s)" % app,
            "DX_WEB_HEADERS": json.encode(headers),
            "DX_WEB_MIME_OVERRIDES": json.encode(mime_overrides),
            "DX_WEB_PORT": str(port),
            "DX_WEB_THREADS": "1" if threads else "0",
        },
        visibility = visibility,
        **kwargs
    )
