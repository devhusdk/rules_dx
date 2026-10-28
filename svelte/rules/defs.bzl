"""Svelte component libraries, app builds and type checks."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", "js_test", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app")

_svelte_library_forward = dx_framework_forward_rule("svelte", ".svelte")

_SVELTE_TOOL = Label("//svelte/rules:svelte_tool")

_SVELTE_TOOL_ENTRY = Label("//svelte/rules:svelte_tool_entry")

def svelte_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Svelte components."""
    dx_framework_library(name, srcs, _js_library, _svelte_library_forward, skip = [], visibility = visibility, **kwargs)

def _svelte_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def _svelte_vite_build(name, out, srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = None):
    """Runs one Vite build of a Svelte app into one output directory."""
    js_run_binary(
        name = name,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               _svelte_node_modules(node_modules, ["vite", "@sveltejs/vite-plugin-svelte", "svelte"]),
        args = ["build", out, config] + ([ssr_entry] if ssr_entry else []),
        chdir = native.package_name(),
        mnemonic = "SvelteSsrBuild" if ssr_entry else "SvelteBuild",
        out_dirs = [out],
        tool = _SVELTE_TOOL,
        visibility = visibility,
        tags = tags,
    )

def svelte_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite Svelte app into a bundled web app tree and an optional SSR tree."""
    tags = kwargs.get("tags", [])
    dist = name + "_dist"
    _svelte_vite_build(dist, dist + ".out", srcs, config, node_modules, package_json, public, deps, ["//visibility:private"], tags)
    dx_framework_web_app(name = name, dist = ":" + dist, what = "svelte_build", visibility = visibility, **kwargs)
    if ssr_entry:
        _svelte_vite_build(name + "_ssr", name + "_ssr", srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = ssr_entry)

def svelte_check(name, srcs, tsconfig, node_modules, package_json = None, deps = None, visibility = None, **kwargs):
    """Runs svelte-check into a cached report and fails the test on errors."""
    report = name + ".svelte_check.txt"
    js_run_binary(
        name = name + "_report",
        srcs = list(srcs) + [tsconfig] + ([package_json] if package_json else []) + (deps or []) +
               _svelte_node_modules(node_modules, ["svelte", "svelte-check", "typescript"]),
        args = ["check", report, tsconfig],
        chdir = native.package_name(),
        mnemonic = "SvelteCheck",
        outs = [report],
        tool = _SVELTE_TOOL,
        visibility = visibility,
        tags = [tag for tag in kwargs.get("tags", []) if tag != "manual"],
    )
    js_test(
        name = name,
        args = ["verify", "$(rootpath :" + report + ")"],
        data = [":" + report],
        entry_point = _SVELTE_TOOL_ENTRY,
        visibility = visibility,
        **kwargs
    )
