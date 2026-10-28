"""Helm chart ownership for dx lint."""

load("//helm/rules:providers.bzl", "HelmChartInfo")
load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

def _dns_label_error(what, value):
    ok = len(value) > 0 and len(value) <= 63 and value[0] != "-" and value[-1] != "-"
    for c in value.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789-":
            ok = False
    if not ok:
        return what + " '" + value + "' must be a DNS-1123 label"
    return ""

def _join(package, rel):
    return rel if package == "" else package + "/" + rel

def _helm_chart_impl(ctx):
    who = "helm_chart " + str(ctx.label) + ": "
    package = ctx.label.package
    for what, value in [("release_name", ctx.attr.release_name), ("namespace", ctx.attr.namespace)]:
        error = _dns_label_error(what, value)
        if error != "":
            fail(who + error)
    seen = {}
    for f in ctx.files.srcs + ctx.files.values:
        if f.short_path in seen:
            fail(who + "duplicate file '" + f.short_path + "'")
        seen[f.short_path] = True
    for f in ctx.files.srcs:
        if f.extension == "tgz":
            fail(who + "archive '" + f.short_path + "' belongs in dependencies, not srcs")
    chart_yaml = _join(package, "Chart.yaml")
    if chart_yaml not in [f.short_path for f in ctx.files.srcs]:
        fail(who + "srcs must contain " + chart_yaml)
    for f in ctx.files.dependencies:
        if f.extension != "tgz" or not f.short_path.startswith(_join(package, "charts/")):
            fail(who + "dependency '" + f.short_path + "' must be a .tgz under " + _join(package, "charts/"))
    direct_sources = {"helm": depset(ctx.files.srcs + ctx.files.values)}
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(ctx.files.srcs + ctx.files.values + ctx.files.dependencies)),
        QualitySourcesInfo(direct_sources = direct_sources),
        HelmChartInfo(
            chart_root = package,
            dependencies = ctx.files.dependencies,
            namespace = ctx.attr.namespace,
            release_name = ctx.attr.release_name,
            values = ctx.files.values,
        ),
    ]

helm_chart = rule(
    implementation = _helm_chart_impl,
    doc = "Owns one Helm chart, the package holding Chart.yaml, for dx lint.",
    attrs = {
        "dependencies": attr.label_list(
            allow_files = [".tgz"],
            doc = "Vendored subchart archives under charts/; never fetched.",
        ),
        "namespace": attr.string(default = "default", doc = "Namespace passed to helm template."),
        "release_name": attr.string(default = "release", doc = "Release name passed to helm template."),
        "srcs": attr.label_list(allow_files = True, doc = "Chart files, including Chart.yaml and templates."),
        "values": attr.label_list(
            allow_files = [".yaml", ".yml"],
            doc = "Values files in order; later files override earlier ones.",
        ),
    },
)
