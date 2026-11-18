"""Analysis test that the `crate` extension's `cargo-bazel` lives in the `dx_cargo_bazel` repository."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _impl(ctx):
    env = analysistest.begin(ctx)
    files = analysistest.target_under_test(env)[DefaultInfo].files.to_list()
    asserts.equals(env, 1, len(files), "dx_cargo_bazel//:generator files")
    if files:
        asserts.true(env, files[0].owner.repo_name.endswith("dx_cargo_bazel"), "cargo-bazel repository: {}".format(files[0].owner))
        asserts.true(env, files[0].basename in ["cargo-bazel", "cargo-bazel.exe"], "cargo-bazel file: {}".format(files[0].basename))
    return analysistest.end(env)

cargo_bazel_generator_test = analysistest.make(_impl)
