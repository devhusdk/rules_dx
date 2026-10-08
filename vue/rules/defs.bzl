"""Experimental minimal Vue wrappers."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library")

_vue_library_forward = dx_framework_forward_rule("vue", ".vue")

def vue_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over js_library for Vue SFCs."""
    dx_framework_library(name, srcs, _js_library, _vue_library_forward, visibility = visibility, **kwargs)
