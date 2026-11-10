"""Astro component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_build", "dx_framework_test")

_astro_library_forward = dx_framework_forward_rule("astro", ".astro")

_ASTRO_TOOL = Label("//astro/rules:astro_tool")

def astro_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Astro components."""
    dx_framework_library(name, srcs, _js_library, _astro_library_forward, skip = [], visibility = visibility, **kwargs)

def astro_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Astro site into a bundled web app tree and an optional SSR tree. The config is a package-relative file path."""
    dx_framework_build(name, "astro_build", "Astro", ["astro"], srcs, config, node_modules, package_json = package_json, public = public, ssr_entry = ssr_entry, deps = deps, visibility = visibility, tool = _ASTRO_TOOL, **kwargs)

def astro_test(name, *, srcs, node_modules, data = None, **kwargs):
    """Runs jest over Astro components, compiled by the linked @astrojs/compiler-rs. Coverage maps back to the .astro sources."""
    dx_framework_test(name, srcs, Label("//astro/rules:jest_config"), Label("//astro/rules:jest_transform"), ["@astrojs/compiler-rs", "astro"], node_modules, data = data, **kwargs)
