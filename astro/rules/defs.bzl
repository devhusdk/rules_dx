"""Astro component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_web_app")

_astro_library_forward = dx_framework_forward_rule("astro", ".astro")

_ASTRO_TOOL = Label("//astro/rules:astro_tool")

def astro_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Astro components."""
    dx_framework_library(name, srcs, _js_library, _astro_library_forward, skip = [], visibility = visibility, **kwargs)

def _astro_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def astro_build(name, srcs, config, node_modules, package_json = None, public = None, deps = None, visibility = None, **kwargs):
    """Builds one static Astro site into a bundled web app tree. The config is a package-relative file path."""
    dist = name + "_dist"
    js_run_binary(
        resource_set = "mem_2g",
        name = dist,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               _astro_node_modules(node_modules, ["astro"]),
        args = ["build", dist + ".out", config],
        chdir = native.package_name(),
        mnemonic = "AstroBuild",
        out_dirs = [dist + ".out"],
        tool = _ASTRO_TOOL,
        visibility = ["//visibility:private"],
        tags = kwargs.get("tags", []),
    )
    dx_framework_web_app(name = name, dist = ":" + dist, what = "astro_build", visibility = visibility, **kwargs)
