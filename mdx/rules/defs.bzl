"""Experimental minimal MDX wrappers."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library")

_mdx_library_forward = dx_framework_forward_rule("mdx", ".mdx")

def mdx_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over js_library for MDX documents."""
    dx_framework_library(name, srcs, _js_library, _mdx_library_forward, visibility = visibility, **kwargs)
