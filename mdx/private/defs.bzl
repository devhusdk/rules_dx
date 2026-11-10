"""MDX document libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/private:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_build", "dx_framework_test")

visibility(["//..."])

_mdx_library_forward = dx_framework_forward_rule("mdx", ".mdx")

def mdx_library(name, srcs, visibility = None, **kwargs):
    """Source-only wrapper over js_library for MDX documents."""
    dx_framework_library(name, srcs, _js_library, _mdx_library_forward, skip = [], visibility = visibility, **kwargs)

def mdx_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite MDX app into a bundled web app tree and an optional SSR tree."""
    dx_framework_build(name, "mdx_build", "Mdx", ["vite", "@mdx-js/rollup"], srcs, config, node_modules, package_json = package_json, public = public, ssr_entry = ssr_entry, deps = deps, visibility = visibility, **kwargs)

def mdx_test(name, *, srcs, node_modules, data = None, **kwargs):
    """Runs jest over MDX components, compiled by the linked @mdx-js/mdx. Coverage maps back to the .mdx sources."""
    dx_framework_test(name, srcs, Label("//mdx/private:jest_config"), Label("//mdx/private:jest_transform"), ["@mdx-js/mdx", "source-map"], node_modules, data = data, **kwargs)
