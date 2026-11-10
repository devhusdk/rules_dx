"""bicep_library ownership and provider proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//bicep/private:registry.bzl", "registry_errors")
load("//bicep/private:providers.bzl", "BicepInfo")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_bicep_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["bicep"], sorted(direct.keys()))
    asserts.equals(env, ["app.bicep"], [f.basename for f in direct["bicep"].to_list()])
    return analysistest.end(env)

owns_bicep_class_test = analysistest.make(_owns_bicep_class_impl)

def _transitive_context_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[BicepInfo]
    asserts.equals(env, ["app.bicep", "lib.bicep"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, ["data.json"], [f.basename for f in info.transitive_data.to_list()])
    return analysistest.end(env)

transitive_context_test = analysistest.make(_transitive_context_impl)

def _registry_deps_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[BicepInfo]
    asserts.equals(env, ["main.json", "manifest"], sorted([f.basename for f in info.transitive_registry.to_list()]))
    asserts.equals(env, ["registry_app.bicep"], [f.basename for f in info.transitive_srcs.to_list()])
    return analysistest.end(env)

registry_deps_test = analysistest.make(_registry_deps_impl)

_REF = "br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.3"
_DIGEST = "sha256:" + "a" * 64

def registry_unit_tests(name):
    """Declares unit tests of the registry_module ref and digest checks."""
    starlark_test(name = name, mode = "unit", checks = [
        expect_equal("valid", registry_errors(_REF, _DIGEST), []),
        expect_equal("port in host", registry_errors("br:localhost:5000/m:1", _DIGEST), []),
        expect_equal("not br", len(registry_errors("ts:x/y:1", _DIGEST)), 1),
        expect_equal("no tag", len(registry_errors("br:mcr.microsoft.com/m", _DIGEST)), 1),
        expect_equal("empty tag", len(registry_errors("br:mcr.microsoft.com/m:", _DIGEST)), 1),
        expect_equal("no prefix", len(registry_errors(_REF, "a" * 64)), 1),
        expect_equal("short digest", len(registry_errors(_REF, "sha256:abc")), 1),
        expect_equal("upper digest", len(registry_errors(_REF, "sha256:" + "A" * 64)), 1),
        expect_equal("both bad", len(registry_errors("x", "y")), 2),
    ])
