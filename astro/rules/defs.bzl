"""Astro component libraries, app builds and type checks."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary", "js_test", _js_library = "js_library")
load("//javascript/rules:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library")
load("//javascript/rules:web.bzl", "JavascriptWebAppInfo")

_astro_library_forward = dx_framework_forward_rule("astro", ".astro")

_ASTRO_TOOL = Label("//astro/rules:astro_tool")

_ASTRO_TOOL_ENTRY = Label("//astro/rules:astro_tool_entry")

def astro_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Astro components."""
    dx_framework_library(name, srcs, _js_library, _astro_library_forward, skip = [], visibility = visibility, **kwargs)

def _astro_web_app_impl(ctx):
    """Stages one built Astro dist tree as a bundled web app."""
    dist = ctx.files.dist
    if len(dist) != 1 or not dist[0].is_directory:
        fail("astro_build " + str(ctx.label) + ": dist must be exactly one directory")
    out = ctx.actions.declare_directory(ctx.label.name)
    ctx.actions.symlink(output = out, target_file = dist[0])
    files = depset([out])
    return [
        DefaultInfo(files = files, runfiles = ctx.runfiles(files = [out])),
        JavascriptWebAppInfo(
            entry = "",
            entry_file = None,
            files = files,
            index_file = None,
            snippets = None,
            threads = False,
            wasm = "",
            wasm_file = None,
        ),
    ]

_astro_web_app = rule(
    implementation = _astro_web_app_impl,
    doc = "Exposes one Astro dist tree as a bundled web app.",
    provides = [DefaultInfo, JavascriptWebAppInfo],
    attrs = {
        "dist": attr.label(mandatory = True, allow_files = True),
    },
)

def _astro_node_modules(node_modules, packages):
    """Returns the consumer npm labels for the given package names."""
    return [node_modules + "/" + package for package in packages]

def astro_build(name, srcs, config, node_modules, package_json = None, public = None, deps = None, visibility = None, **kwargs):
    """Builds one static Astro site into a bundled web app tree."""
    dist = name + "_dist"
    js_run_binary(
        name = dist,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               _astro_node_modules(node_modules, ["astro"]),
        args = ["build", dist + ".out"],
        chdir = native.package_name(),
        mnemonic = "AstroBuild",
        out_dirs = [dist + ".out"],
        tool = _ASTRO_TOOL,
        visibility = ["//visibility:private"],
        tags = kwargs.get("tags", []),
    )
    _astro_web_app(name = name, dist = ":" + dist, visibility = visibility, **kwargs)

def astro_check(name, srcs, tsconfig, config, node_modules, package_json = None, deps = None, visibility = None, **kwargs):
    """Runs astro check into a cached report and fails the test on errors."""
    report = name + ".astro_check.txt"
    js_run_binary(
        name = name + "_report",
        srcs = list(srcs) + [tsconfig, config] + ([package_json] if package_json else []) + (deps or []) +
               _astro_node_modules(node_modules, ["astro", "@astrojs/check", "typescript"]),
        args = ["check", report, tsconfig],
        chdir = native.package_name(),
        mnemonic = "AstroCheck",
        outs = [report],
        tool = _ASTRO_TOOL,
        visibility = visibility,
        tags = [tag for tag in kwargs.get("tags", []) if tag != "manual"],
    )
    js_test(
        name = name,
        args = ["verify", "$(rootpath :" + report + ")"],
        data = [":" + report],
        entry_point = _ASTRO_TOOL_ENTRY,
        visibility = visibility,
        **kwargs
    )
