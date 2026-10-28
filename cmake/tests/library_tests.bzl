"""cmake_library ownership proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _owns_cmake_class_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    direct = target[QualitySourcesInfo].direct_sources
    asserts.equals(env, ["cmake"], sorted(direct.keys()))
    asserts.equals(
        env,
        ["CMakeLists.txt", "helpers.cmake"],
        [f.basename for f in direct["cmake"].to_list()],
    )
    return analysistest.end(env)

owns_cmake_class_test = analysistest.make(_owns_cmake_class_impl)
