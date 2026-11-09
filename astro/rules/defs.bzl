"""Astro component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app", "dx_vite_build")

_astro_library_forward = dx_framework_forward_rule("astro", ".astro")

_ASTRO_TOOL = Label("//astro/rules:astro_tool")

def astro_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Astro components."""
    dx_framework_library(name, srcs, _js_library, _astro_library_forward, skip = [], visibility = visibility, **kwargs)

def astro_build(name, srcs, config, node_modules, package_json = None, public = None, deps = None, visibility = None, **kwargs):
    """Builds one static Astro site into a bundled web app tree. The config is a package-relative file path."""
    dist = name + "_dist"
    dx_vite_build(
        dist,
        dist + ".out",
        srcs,
        config,
        node_modules,
        ["astro"],
        "Astro",
        package_json,
        public,
        deps,
        ["//visibility:private"],
        kwargs.get("tags", []),
        tool = _ASTRO_TOOL,
    )
    dx_framework_web_app(name = name, dist = ":" + dist, what = "astro_build", visibility = visibility, **kwargs)
