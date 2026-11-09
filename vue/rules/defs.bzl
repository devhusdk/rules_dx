"""Vue component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app", "dx_vite_build")

_vue_library_forward = dx_framework_forward_rule("vue", ".vue")

def vue_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Vue SFCs."""
    dx_framework_library(name, srcs, _js_library, _vue_library_forward, skip = [], visibility = visibility, **kwargs)

def _vue_vite_build(name, out, srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = None):
    """Runs one Vite build of a Vue app into one output directory."""
    dx_vite_build(name, out, srcs, config, node_modules, ["vite", "@vitejs/plugin-vue", "vue"], "Vue", package_json, public, deps, visibility, tags, ssr_entry = ssr_entry)

def vue_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite Vue app into a bundled web app tree and an optional SSR tree."""
    tags = kwargs.get("tags", [])
    dist = name + "_dist"
    _vue_vite_build(dist, dist + ".out", srcs, config, node_modules, package_json, public, deps, ["//visibility:private"], tags)
    dx_framework_web_app(name = name, dist = ":" + dist, what = "vue_build", visibility = visibility, **kwargs)
    if ssr_entry:
        _vue_vite_build(name + "_ssr", name + "_ssr", srcs, config, node_modules, package_json, public, deps, visibility, tags, ssr_entry = ssr_entry)
