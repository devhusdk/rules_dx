"""Helm chart ownership for dx lint."""

load("//helm/private:providers.bzl", "HelmChartInfo")
load("//core/quality:sources.bzl", "quality_sources_info")

_HELM_RELEASE_NAME_MAX = 53

def _label_ok(label):
    if len(label) == 0 or label[0] == "-" or label[-1] == "-":
        return False
    for c in label.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789-":
            return False
    return True

def _dns_label_error(what, value, max_len, limit_note = "", dotted = False):
    if len(value) > max_len:
        return what + " '" + value + "' must be at most " + str(max_len) + " characters" + limit_note
    labels = value.split(".") if dotted else [value]
    for label in labels:
        if not _label_ok(label):
            if dotted:
                return what + " '" + value + "' must be dot-separated DNS-1123 labels"
            return what + " '" + value + "' must be a DNS-1123 label"
    return ""

def _join(package, rel):
    return rel if package == "" else package + "/" + rel

def _helm_chart_impl(ctx):
    who = "helm_library " + str(ctx.label) + ": "
    package = ctx.label.package
    for error in [
        _dns_label_error("release_name", ctx.attr.release_name, _HELM_RELEASE_NAME_MAX, " (Helm limit)", True),
        _dns_label_error("namespace", ctx.attr.namespace, 63),
    ]:
        if error != "":
            fail(who + error)
    seen = {}
    for f in ctx.files.srcs + ctx.files.values:
        if f.short_path in seen:
            fail(who + "duplicate file '" + f.short_path + "'")
        seen[f.short_path] = True
    for f in ctx.files.srcs:
        if f.extension == "tgz":
            fail(who + "archive '" + f.short_path + "' belongs in subcharts, not srcs")
        if f.short_path.startswith("../") or not f.short_path.startswith(_join(package, "")):
            fail(who + "src '" + f.short_path + "' must be under the chart package '" + package + "'")
    chart_yaml = _join(package, "Chart.yaml")
    if chart_yaml not in [f.short_path for f in ctx.files.srcs]:
        fail(who + "srcs must contain " + chart_yaml)
    for f in ctx.files.subcharts:
        if f.extension != "tgz" or not f.short_path.startswith(_join(package, "charts/")):
            fail(who + "subchart '" + f.short_path + "' must be a .tgz under " + _join(package, "charts/"))
    return [
        DefaultInfo(files = depset(ctx.files.srcs + ctx.files.values + ctx.files.subcharts)),
        quality_sources_info({"helm": ctx.files.srcs + ctx.files.values}, str(ctx.label)),
        HelmChartInfo(
            chart_root = package,
            subcharts = ctx.files.subcharts,
            namespace = ctx.attr.namespace,
            release_name = ctx.attr.release_name,
            values = ctx.files.values,
        ),
    ]

helm_library = rule(
    implementation = _helm_chart_impl,
    doc = "Owns one Helm chart, the package holding Chart.yaml, for dx lint.",
    attrs = {
        "subcharts": attr.label_list(
            allow_files = [".tgz"],
            doc = "Vendored subchart archives under charts/; never fetched.",
        ),
        "namespace": attr.string(default = "default", doc = "Namespace passed to helm template."),
        "release_name": attr.string(default = "release", doc = "Release name passed to helm template; at most 53 characters."),
        "srcs": attr.label_list(allow_files = True, doc = "Chart files of this package, including Chart.yaml and templates."),
        "values": attr.label_list(
            allow_files = [".yaml", ".yml"],
            doc = "Values files in order; later files override earlier ones.",
        ),
    },
)
