"""Helm chart ownership for dx lint."""

load("//helm/private:providers.bzl", "HelmChartInfo")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

# Helm's maxReleaseNameLen, pkg/chart/v2/util/validate_name.go.
_HELM_RELEASE_NAME_MAX = 53

def _dns_label_error(what, value, max_len, limit_note = ""):
    if len(value) > max_len:
        return what + " '" + value + "' must be at most " + str(max_len) + " characters" + limit_note
    ok = len(value) > 0 and value[0] != "-" and value[-1] != "-"
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
    for error in [
        _dns_label_error("release_name", ctx.attr.release_name, _HELM_RELEASE_NAME_MAX, " (Helm limit)"),
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
    chart_yaml = _join(package, "Chart.yaml")
    if chart_yaml not in [f.short_path for f in ctx.files.srcs]:
        fail(who + "srcs must contain " + chart_yaml)
    for f in ctx.files.subcharts:
        if f.extension != "tgz" or not f.short_path.startswith(_join(package, "charts/")):
            fail(who + "subchart '" + f.short_path + "' must be a .tgz under " + _join(package, "charts/"))
    direct_sources = {"helm": depset(ctx.files.srcs + ctx.files.values)}
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(ctx.files.srcs + ctx.files.values + ctx.files.subcharts)),
        QualitySourcesInfo(direct_sources = direct_sources),
        HelmChartInfo(
            chart_root = package,
            subcharts = ctx.files.subcharts,
            namespace = ctx.attr.namespace,
            release_name = ctx.attr.release_name,
            values = ctx.files.values,
        ),
    ]

helm_chart = rule(
    implementation = _helm_chart_impl,
    doc = "Owns one Helm chart, the package holding Chart.yaml, for dx lint.",
    attrs = {
        "subcharts": attr.label_list(
            allow_files = [".tgz"],
            doc = "Vendored subchart archives under charts/; never fetched.",
        ),
        "namespace": attr.string(default = "default", doc = "Namespace passed to helm template."),
        "release_name": attr.string(default = "release", doc = "Release name passed to helm template; at most 53 characters."),
        "srcs": attr.label_list(allow_files = True, doc = "Chart files, including Chart.yaml and templates."),
        "values": attr.label_list(
            allow_files = [".yaml", ".yml"],
            doc = "Values files in order; later files override earlier ones.",
        ),
    },
)
