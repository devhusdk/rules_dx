"""helm_chart proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//helm/rules:providers.bzl", "HelmChartInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_helm_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["helm"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["Chart.yaml", "cm.yaml", "values-prod.yaml", "values.yaml"],
        sorted([f.basename for f in direct["helm"].to_list()]),
    )
    return analysistest.end(env)

owns_helm_class_test = analysistest.make(_owns_helm_class_impl)

def _chart_provider_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[HelmChartInfo]
    asserts.equals(env, "helm/tests", info.chart_root)
    asserts.equals(env, "demo", info.release_name)
    asserts.equals(env, "prod", info.namespace)
    asserts.equals(env, ["values-prod.yaml"], [f.basename for f in info.values])
    asserts.equals(env, ["dep-1.0.0.tgz"], [f.basename for f in info.dependencies])
    return analysistest.end(env)

chart_provider_test = analysistest.make(_chart_provider_impl)
