"""MDX document libraries, app builds and compile checks."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", "js_test", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app")

_mdx_library_forward = dx_framework_forward_rule("mdx", ".mdx")

_MDX_TOOL = Label("//mdx/rules:mdx_tool")

_MDX_TOOL_ENTRY = Label("//mdx/rules:mdx_tool_entry")

def mdx_library(name, srcs, visibility = None, **kwargs):
    """Source-only wrapper over js_library for MDX documents."""
    dx_framework_library(name, srcs, _js_library, _mdx_library_forward, skip = [], visibility = visibility, **kwargs)

def _mdx_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def _mdx_vite_build(name, out, srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = None):
    """Runs one Vite build of an MDX app into one output directory."""
    js_run_binary(
        name = name,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               _mdx_node_modules(node_modules, ["vite", "@mdx-js/rollup"]),
        args = ["build", out, config] + ([ssr_entry] if ssr_entry else []),
        chdir = native.package_name(),
        env = {"MDX_TOOL_PACKAGE": native.package_name()},
        mnemonic = "MdxSsrBuild" if ssr_entry else "MdxBuild",
        out_dirs = [out],
        tool = _MDX_TOOL,
        visibility = visibility,
        tags = tags,
    )

def mdx_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite MDX app into a bundled web app tree and an optional SSR tree."""
    tags = kwargs.get("tags", [])
    dist = name + "_dist"
    _mdx_vite_build(dist, dist + ".out", srcs, config, node_modules, package_json, public, deps, ["//visibility:private"], tags)
    dx_framework_web_app(name = name, dist = ":" + dist, what = "mdx_build", visibility = visibility, **kwargs)
    if ssr_entry:
        _mdx_vite_build(name + "_ssr", name + "_ssr", srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = ssr_entry)

def mdx_check(name, srcs, node_modules, package_json = None, deps = None, visibility = None, **kwargs):
    """Compiles each MDX file into a cached report and fails the test on errors."""
    report = name + ".mdx_check.txt"
    js_run_binary(
        name = name + "_report",
        srcs = list(srcs) + ([package_json] if package_json else []) + (deps or []) +
               _mdx_node_modules(node_modules, ["@mdx-js/mdx"]),
        args = ["check", report, native.package_name()] + [src for src in srcs if src.endswith(".mdx")],
        chdir = native.package_name(),
        mnemonic = "MdxCheck",
        outs = [report],
        tool = _MDX_TOOL,
        visibility = visibility,
        tags = [tag for tag in kwargs.get("tags", []) if tag != "manual"],
    )
    js_test(
        name = name,
        args = ["verify", "$(rootpath :" + report + ")"],
        data = [":" + report],
        entry_point = _MDX_TOOL_ENTRY,
        visibility = visibility,
        **kwargs
    )
