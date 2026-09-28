"""Experimental minimal Svelte wrappers."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//libs/starlark:wrapper.bzl", "dx_framework_forward_rule", "dx_framework_library")

_svelte_library_forward = dx_framework_forward_rule("svelte", ".svelte")

def svelte_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over js_library for Svelte components."""
    dx_framework_library(name, srcs, _js_library, _svelte_library_forward, visibility = visibility, **kwargs)
