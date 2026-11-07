"""MDX document libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app")

_mdx_library_forward = dx_framework_forward_rule("mdx", ".mdx")

_MDX_TOOL = Label("//mdx/rules:mdx_tool")

def mdx_library(name, srcs, visibility = None, **kwargs):
    """Source-only wrapper over js_library for MDX documents."""
    dx_framework_library(name, srcs, _js_library, _mdx_library_forward, skip = [], visibility = visibility, **kwargs)

def _mdx_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def _mdx_vite_build(name, out, srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = None):
    """Runs one Vite build of an MDX app into one output directory."""
    js_run_binary(
        resource_set = "mem_2g",
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
