"""MDX document libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_build")

_mdx_library_forward = dx_framework_forward_rule("mdx", ".mdx")

def mdx_library(name, srcs, visibility = None, **kwargs):
    """Source-only wrapper over js_library for MDX documents."""
    dx_framework_library(name, srcs, _js_library, _mdx_library_forward, skip = [], visibility = visibility, **kwargs)

def mdx_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite MDX app into a bundled web app tree and an optional SSR tree."""
    dx_framework_build(name, "mdx_build", "Mdx", ["vite", "@mdx-js/rollup"], srcs, config, node_modules, package_json = package_json, public = public, ssr_entry = ssr_entry, deps = deps, visibility = visibility, **kwargs)
