"""JS-framework library wrappers over js_library."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//javascript/rules:defs.bzl", "javascript_test")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_wrap")
load("//libs/web_server:server.bzl", "JavascriptWebAppInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def dx_framework_forward_rule(language, ext):
    """Creates the forwarding rule for one JS-framework wrapper."""
    return dx_library_forward_rule(
        provides = [_JsInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
        required_providers = [(_JsInfo, "JsInfo")],
        quality_specs = [(language, language)],
        what = language + "_*",
        allow_files = [ext],
        upstream_providers = [[_JsInfo]],
    )

def dx_framework_library(name, srcs, upstream_rule, forward_rule, skip, visibility = None, **kwargs):
    """Instantiates one JS-framework library that skips the named quality capabilities."""
    tags = list(kwargs.pop("tags", []))
    for capability in skip:
        tag = "no-" + capability
        if tag not in tags:
            tags.append(tag)
    kwargs["tags"] = tags
    dx_wrap(name, upstream_rule, forward_rule, srcs, visibility = visibility, **kwargs)

def dx_framework_test(name, srcs, config, transform, packages, node_modules, data = None, **kwargs):
    """Runs jest over framework components. `transform` compiles them, `config` builds the jest config from the shared helper, and `packages` are the node_modules packages (the compiler) the transform needs."""
    if "config" in kwargs:
        fail(name + ": config is set by the framework test rule")
    javascript_test(
        name = name,
        srcs = srcs,
        config = config,
        data = list(data or []) + [transform, Label("//javascript/rules:framework_jest_config")] +
               [node_modules + "/" + package for package in packages],
        node_modules = node_modules,
        node_options = ["--experimental-vm-modules"],
        **kwargs
    )

def _dx_framework_web_app_impl(ctx):
    """Stages one built framework dist tree as a bundled web app."""
    dist = ctx.files.dist
    if len(dist) != 1 or not dist[0].is_directory:
        fail(ctx.attr.what + " " + str(ctx.label) + ": dist must be exactly one directory")
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

_dx_framework_web_app = rule(
    implementation = _dx_framework_web_app_impl,
    doc = "Exposes one framework dist tree as a bundled web app.",
    provides = [DefaultInfo, JavascriptWebAppInfo],
    attrs = {
        "dist": attr.label(
            doc = "Framework dist directory exposed as the web app.",
            mandatory = True,
            allow_files = True,
        ),
        "what": attr.string(
            doc = "Descriptive framework name used in error messages.",
            mandatory = True,
        ),
    },
)

def dx_framework_web_app(name, dist, what, **kwargs):
    """Exposes one built framework dist directory as a JavascriptWebAppInfo web app."""
    _dx_framework_web_app(name = name, dist = dist, what = what, **kwargs)

_VITE_TOOL = Label("//javascript/rules:vite_tool")

def _dx_framework_vite_run(name, out, srcs, config, node_modules, packages, mnemonic, package_json, public, deps, visibility, tags, testonly, target_compatible_with, tool, ssr_entry = None, keep_entry_path = False):
    env = {"VITE_TOOL_PACKAGE": native.package_name()}
    if keep_entry_path:
        env["VITE_TOOL_KEEP_ENTRY_PATH"] = "1"
    js_run_binary(
        resource_set = "mem_2g",
        name = name,
        srcs = list(srcs) + [config] + ([package_json] if package_json else []) + (public or []) + (deps or []) +
               [node_modules + "/" + package for package in packages],
        args = ["build", out, config] + ([ssr_entry] if ssr_entry else []),
        chdir = native.package_name(),
        env = env,
        mnemonic = mnemonic + "SsrBuild" if ssr_entry else mnemonic + "Build",
        out_dirs = [out],
        tool = tool,
        visibility = visibility,
        tags = tags,
        testonly = testonly,
        target_compatible_with = target_compatible_with,
    )

def dx_framework_build(name, what, mnemonic, packages, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, tool = _VITE_TOOL, keep_entry_path = False, **kwargs):
    """Builds one framework app into a bundled web app tree and an optional SSR tree. Creates the private `<name>_dist` build target behind the web app `<name>`. With `ssr_entry` it also creates `<name>_ssr`, which takes the given visibility. `tags`, `testonly` and `target_compatible_with` reach every target."""
    tags = kwargs.get("tags", [])
    testonly = kwargs.get("testonly", False)
    compatible = kwargs.get("target_compatible_with", None)
    dist = name + "_dist"
    _dx_framework_vite_run(dist, dist + ".out", srcs, config, node_modules, packages, mnemonic, package_json, public, deps, ["//visibility:private"], tags, testonly, compatible, tool, keep_entry_path = keep_entry_path)
    dx_framework_web_app(name = name, dist = ":" + dist, what = what, visibility = visibility, **kwargs)
    if ssr_entry:
        _dx_framework_vite_run(name + "_ssr", name + "_ssr", srcs, config, node_modules, packages, mnemonic, package_json, public, deps, visibility, tags, testonly, compatible, tool, ssr_entry = ssr_entry, keep_entry_path = keep_entry_path)
