"""JS-framework library wrappers over js_library."""

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
        "dist": attr.label(mandatory = True, allow_files = True),
        "what": attr.string(mandatory = True),
    },
)

def dx_framework_web_app(name, dist, what, **kwargs):
    """Exposes one built framework dist directory as a JavascriptWebAppInfo web app."""
    _dx_framework_web_app(name = name, dist = dist, what = what, **kwargs)
