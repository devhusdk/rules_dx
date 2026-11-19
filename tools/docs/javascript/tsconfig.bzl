"""Finds the package tsconfig.json of a JavaScript library behind a rules_dx wrapper."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//core/quality:sources.bzl", "forwarded_upstream")

JsDocsTsconfigInfo = provider(
    doc = "The tsconfig.json files of the package that the direct deps of a JavaScript library provide.",
    fields = {"tsconfigs": "List of tsconfig.json files in the package of the library."},
)

def _js_docs_tsconfig_impl(target, ctx):
    if hasattr(ctx.rule.attr, "upstream"):
        upstream = forwarded_upstream(ctx.rule.attr, JsDocsTsconfigInfo)
        return [upstream[JsDocsTsconfigInfo]] if upstream else []
    if _JsInfo not in target:
        return []
    package = ctx.label.package
    suffix = "/" + package if package else ""
    return [JsDocsTsconfigInfo(tsconfigs = [
        f
        for f in getattr(ctx.rule.files, "deps", [])
        if f.basename == "tsconfig.json" and f.dirname.endswith(suffix)
    ])]

js_docs_tsconfig_aspect = aspect(
    implementation = _js_docs_tsconfig_impl,
    attr_aspects = ["upstream"],
    doc = "Reads the package tsconfig.json from the direct deps of a JavaScript library behind a rules_dx wrapper.",
)

def js_docs_tsconfigs(target):
    """Returns the package tsconfig.json files that `js_docs_tsconfig_aspect` found for `target`."""
    return target[JsDocsTsconfigInfo].tsconfigs if JsDocsTsconfigInfo in target else []
