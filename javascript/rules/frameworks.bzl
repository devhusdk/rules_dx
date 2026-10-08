"""JS-framework library wrappers over js_library."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_wrap")
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

def dx_framework_library(name, srcs, upstream_rule, forward_rule, visibility = None, **kwargs):
    """Instantiates one JS-framework library with standard skip tags."""
    tags = list(kwargs.pop("tags", []))
    for tag in ["no-format", "no-lint", "no-typecheck"]:
        if tag not in tags:
            tags.append(tag)
    kwargs["tags"] = tags
    dx_wrap(name, upstream_rule, forward_rule, srcs, visibility = visibility, **kwargs)
