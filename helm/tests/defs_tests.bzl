"""helm_library proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:owns_class_test.bzl", "owns_class_test")
load("//helm/private:providers.bzl", "HelmChartInfo")

owns_helm_class_test = owns_class_test(
    "helm",
    ["Chart.yaml", "cm.yaml", "values-prod.yaml", "values.yaml"],
    sort = True,
)

def _chart_provider_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[HelmChartInfo]
    asserts.equals(env, "helm/tests", info.chart_root)
    asserts.equals(env, ctx.attr.expected_release_name, info.release_name)
    asserts.equals(env, "prod", info.namespace)
    asserts.equals(env, ["values-prod.yaml"], [f.basename for f in info.values])
    asserts.equals(env, ["dep-1.0.0.tgz"], [f.basename for f in info.subcharts])
    return analysistest.end(env)

chart_provider_test = analysistest.make(
    _chart_provider_impl,
    attrs = {"expected_release_name": attr.string(mandatory = True)},
)
