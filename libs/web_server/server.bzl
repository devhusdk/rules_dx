"""Development server for staged browser applications."""

load("//libs/starlark:defs.bzl", "display_label")
load("//python:defs.bzl", "python_binary")

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

_WEB_LAUNCHER_RUNFILES = [
    "rules_dx/libs/web_server/web_server.py",
    "_main/libs/web_server/web_server.py",
    "libs/web_server/web_server.py",
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
            "import runpy",
            "import sys",
            "",
            "from python.runfiles.runfiles import Create  # ty: ignore[unresolved-import]",
            "",
            "_SCRIPT_CANDIDATES = " + _web_py_list(_WEB_LAUNCHER_RUNFILES),
            "_INDEX_CANDIDATES = " + _web_py_list(_web_launcher_candidates(
                ctx.attr.app.label.repo_name,
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
            "    server = runpy.run_path(script, run_name=\"dx_web_server\")",
            "    return server[\"run\"]([\"--root=\" + root] + _BAKED + sys.argv[1:])",
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
    doc = "Writes the development-server launcher for one staged app.",
    attrs = {
        "app": attr.label(
            doc = "Staged browser app directory from javascript_web_app to serve.",
            mandatory = True,
            providers = [JavascriptWebAppInfo],
        ),
        "cross_origin_isolated": attr.bool(
            doc = "Serve with cross-origin isolation headers enabled.",
            default = False,
        ),
        "headers": attr.string_dict(doc = "Extra HTTP response headers sent with every response."),
        "host": attr.string(
            doc = "Host address the development server listens on.",
            default = "127.0.0.1",
        ),
        "port": attr.int(doc = "TCP port the development server listens on.", default = 8000),
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
    binary_kwargs["data"] = list(kwargs.get("data", [])) + [app, Label("//libs/web_server:web_server")]
    binary_kwargs["deps"] = list(kwargs.get("deps", [])) + [Label("@rules_python//python/runfiles:runfiles")]
    launcher = ":" + name + "_launcher"
    binary_kwargs.setdefault("main", launcher)
    python_binary(
        name = name,
        srcs = [launcher],
        visibility = visibility,
        **binary_kwargs
    )
