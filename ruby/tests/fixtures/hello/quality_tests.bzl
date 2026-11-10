"""Analysis tests that Ruby targets own their main for quality."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_dx//quality:sources.bzl", "QualitySourcesInfo")

def _owns_impl(expected):
    def impl(ctx):
        env = analysistest.begin(ctx)
        direct = analysistest.target_under_test(env)[QualitySourcesInfo].direct_sources
        asserts.equals(env, ["ruby"], sorted(direct.keys()))
        asserts.equals(env, expected, sorted([f.basename for f in direct["ruby"].to_list()]))
        return analysistest.end(env)

    return impl

main_outside_srcs_test = analysistest.make(_owns_impl(["hello.rb", "main_outside_srcs.rb"]))
rspec_owns_spec_test = analysistest.make(_owns_impl(["greeter_spec.rb"]))
