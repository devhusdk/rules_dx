"""Analysis tests for the Scala docs shards on a Scala toolchain other than 2.13."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:failure_test.bzl", "failure_test_rule")
load("//tools/docs/jvm:aspect.bzl", "real_jvm_docs_aspect")

_SCALA3 = {str(Label("@rules_scala_config//:scala_version")): "3.3.9"}

def _aspect_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    groups = target[OutputGroupInfo] if OutputGroupInfo in target else None
    shards = getattr(groups, "dx_docs_shards", depset()).to_list() if groups else []
    asserts.equals(env, ctx.attr.shards, len(shards), "dx_docs_shards of " + str(target.label))
    return analysistest.end(env)

_ATTRS = {"shards": attr.int(mandatory = True)}

_default_aspect_test = analysistest.make(
    _aspect_impl,
    attrs = _ATTRS,
    extra_target_under_test_aspects = [real_jvm_docs_aspect],
)

_scala3_aspect_test = analysistest.make(
    _aspect_impl,
    attrs = _ATTRS,
    config_settings = _SCALA3,
    extra_target_under_test_aspects = [real_jvm_docs_aspect],
)

_scala3_shard_failure_test = failure_test_rule(config_settings = _SCALA3)

def scala_version_tests(name, lib, shard):
    """Checks the docs aspect skips and scala_doc_shard rejects a Scala library on Scala 3."""
    _default_aspect_test(
        name = name + "_default_aspect",
        target_under_test = lib,
        shards = 1,
        size = "small",
    )
    _scala3_aspect_test(
        name = name + "_scala3_aspect",
        target_under_test = lib,
        shards = 0,
        size = "small",
    )
    _scala3_shard_failure_test(
        name = name + "_scala3_shard",
        target_under_test = shard,
        expected_failure_substring = "only Scala 2.13 is supported; the toolchain is 3.3.9",
        size = "small",
    )
    native.test_suite(
        name = name,
        tests = [
            name + "_default_aspect",
            name + "_scala3_aspect",
            name + "_scala3_shard",
        ],
    )
