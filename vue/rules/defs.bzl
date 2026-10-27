"""Vue component libraries, app builds and type checks."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", "js_test", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app")

_vue_library_forward = dx_framework_forward_rule("vue", ".vue")

_VUE_TOOL = Label("//vue/rules:vue_tool")

_VUE_TOOL_ENTRY = Label("//vue/rules:vue_tool_entry")

def vue_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Vue SFCs."""
    dx_framework_library(name, srcs, _js_library, _vue_library_forward, skip = [], visibility = visibility, **kwargs)

def _vue_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def _vue_vite_build(name, out, srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = None):
    """Runs one Vite build of a Vue app into one output directory."""
    js_run_binary(
        name = name,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               _vue_node_modules(node_modules, ["vite", "@vitejs/plugin-vue", "vue"]),
        args = ["build", out, config] + ([ssr_entry] if ssr_entry else []),
        chdir = native.package_name(),
        mnemonic = "VueSsrBuild" if ssr_entry else "VueBuild",
        out_dirs = [out],
        tool = _VUE_TOOL,
        visibility = visibility,
        tags = tags,
    )

def vue_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite Vue app into a bundled web app tree and an optional SSR tree."""
    tags = kwargs.get("tags", [])
    dist = name + "_dist"
    _vue_vite_build(dist, dist + ".out", srcs, config, node_modules, package_json, public, deps, ["//visibility:private"], tags)
    dx_framework_web_app(name = name, dist = ":" + dist, what = "vue_build", visibility = visibility, **kwargs)
    if ssr_entry:
        _vue_vite_build(name + "_ssr", name + "_ssr", srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = ssr_entry)

def vue_check(name, srcs, tsconfig, node_modules, package_json = None, deps = None, visibility = None, **kwargs):
    """Runs vue-tsc into a cached report and fails the test on errors."""
    report = name + ".vue_check.txt"
    js_run_binary(
        name = name + "_report",
        srcs = list(srcs) + [tsconfig] + ([package_json] if package_json else []) + (deps or []) +
               _vue_node_modules(node_modules, ["vue", "vue-tsc", "typescript"]),
        args = ["check", report, tsconfig],
        chdir = native.package_name(),
        mnemonic = "VueCheck",
        outs = [report],
        tool = _VUE_TOOL,
        visibility = visibility,
        tags = [tag for tag in kwargs.get("tags", []) if tag != "manual"],
    )
    js_test(
        name = name,
        args = ["verify", "$(rootpath :" + report + ")"],
        data = [":" + report],
        entry_point = _VUE_TOOL_ENTRY,
        visibility = visibility,
        **kwargs
    )
