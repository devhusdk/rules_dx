"""kubernetes_manifests and kubernetes_schemas proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:native_config.bzl", "DxNativeConfigInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_kubernetes_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["kubernetes"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["app.yaml"],
        [f.basename for f in direct["kubernetes"].to_list()],
    )
    return analysistest.end(env)

owns_kubernetes_class_test = analysistest.make(_owns_kubernetes_class_impl)

def _schemas_config_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    info = target[DxNativeConfigInfo]
    asserts.equals(env, "kubeconform", info.tool_id)
    asserts.equals(env, "schemas.kubeconform.json", info.config.basename)
    asserts.equals(
        env,
        [
            "schemas.kubeconform/core/deployment-apps-v1.json",
            "schemas.kubeconform/crd/example.com/widget_v1.json",
            "schemas.kubeconform.json",
        ],
        sorted([f.short_path.removeprefix("kubernetes/tests/") for f in info.closure.to_list()]),
    )
    return analysistest.end(env)

schemas_config_test = analysistest.make(_schemas_config_impl)
