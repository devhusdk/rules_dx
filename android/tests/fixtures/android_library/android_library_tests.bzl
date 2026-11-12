"""Analysis tests of the providers android_java_library and android_kotlin_library return."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_android//providers:providers.bzl", "AndroidIdeInfo", "AndroidLibraryAarInfo")
load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KtJvmInfo")

def _providers_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, JavaInfo in target, "%s has no JavaInfo" % target.label)
    asserts.true(env, AndroidLibraryAarInfo in target, "%s has no AndroidLibraryAarInfo" % target.label)
    asserts.true(env, AndroidIdeInfo in target, "%s has no AndroidIdeInfo" % target.label)
    if AndroidIdeInfo in target:
        asserts.false(env, target[AndroidIdeInfo].defines_android_resources, "%s defines Android resources" % target.label)
    asserts.equals(env, ctx.attr.kotlin, KtJvmInfo in target)
    if JavaInfo in target:
        jars = [jar.basename for jar in target[JavaInfo].runtime_output_jars]
        asserts.true(env, jars, "%s has no runtime output jar" % target.label)
    return analysistest.end(env)

_providers_test = analysistest.make(
    _providers_impl,
    attrs = {
        "kotlin": attr.bool(mandatory = True),
    },
)

def android_library_providers_test(name, library, kotlin):
    """Asserts the Android library `library` of this package returns its JVM and Android providers."""
    _providers_test(
        name = name,
        size = "small",
        target_under_test = ":" + library,
        kotlin = kotlin,
    )
