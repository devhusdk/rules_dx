"""JS-framework library wrappers over js_library."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_wrap")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":web.bzl", "JavascriptWebAppInfo")

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

def dx_vite_build(name, out, srcs, config, node_modules, packages, mnemonic, package_json, public, deps, visibility, tags, ssr_entry = None, keep_entry_path = False, tool = _VITE_TOOL):
    """Runs one framework build into one output directory."""
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
    )
