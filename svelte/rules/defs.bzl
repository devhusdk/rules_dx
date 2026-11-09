"""Svelte component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_build")

_svelte_library_forward = dx_framework_forward_rule("svelte", ".svelte")

def svelte_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Svelte components."""
    dx_framework_library(name, srcs, _js_library, _svelte_library_forward, skip = [], visibility = visibility, **kwargs)

def svelte_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite Svelte app into a bundled web app tree and an optional SSR tree."""
    dx_framework_build(name, "svelte_build", "Svelte", ["vite", "@sveltejs/vite-plugin-svelte", "svelte"], srcs, config, node_modules, package_json = package_json, public = public, ssr_entry = ssr_entry, deps = deps, visibility = visibility, keep_entry_path = True, **kwargs)
