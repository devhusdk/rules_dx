"""Analysis tests of the providers android_java_library and android_kotlin_library return."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_android//providers:providers.bzl", "AndroidIdeInfo", "AndroidLibraryAarInfo")
load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KtJvmInfo")
load("//android/private/quality:android_sdk.bzl", "android_sdk_aspect")
load("//java/private/quality:jvm_check.bzl", "jvm_check_aspect")

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

def android_library_providers_test(name, library, kotlin, size = "small"):
    """Asserts the Android library `library` of this package returns its JVM and Android providers."""
    _providers_test(
        name = name,
        size = size,
        target_under_test = ":" + library,
        kotlin = kotlin,
    )

_CheckActionsInfo = provider(
    doc = "Actions of the subject's library with the JVM check aspect applied.",
    fields = {"actions": "The library's actions, the check aspect's included."},
)

def _check_subject_impl(ctx):
    library = ctx.attr.library[0] if type(ctx.attr.library) == "list" else ctx.attr.library
    return [_CheckActionsInfo(actions = library.actions)]

# Both subjects check the same library, so one builds it in another output directory to keep their
# check outputs apart.
def _opt_impl(_settings, _attr):
    return {"//command_line_option:compilation_mode": "opt"}

_opt_transition = transition(
    implementation = _opt_impl,
    inputs = [],
    outputs = ["//command_line_option:compilation_mode"],
)

_with_sdk_subject = rule(
    implementation = _check_subject_impl,
    doc = "Applies android_sdk_aspect and then jvm_check_aspect to the library.",
    attrs = {"library": attr.label(mandatory = True, aspects = [android_sdk_aspect, jvm_check_aspect])},
)

_without_sdk_subject = rule(
    implementation = _check_subject_impl,
    doc = "Applies jvm_check_aspect alone to the library.",
    attrs = {"library": attr.label(mandatory = True, cfg = _opt_transition, aspects = [jvm_check_aspect])},
)

def _android_jar_inputs(env, mnemonic):
    target = analysistest.target_under_test(env)
    runs = [a for a in target[_CheckActionsInfo].actions if a.mnemonic == mnemonic]
    asserts.equals(env, 1, len(runs), "want one %s action" % mnemonic)
    return [f for run in runs for f in run.inputs.to_list() if f.basename == "android.jar"]

def _with_sdk_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, _android_jar_inputs(env, ctx.attr.mnemonic), "%s has no android.jar input" % ctx.attr.mnemonic)
    return analysistest.end(env)

def _without_sdk_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, [], _android_jar_inputs(env, ctx.attr.mnemonic))
    return analysistest.end(env)

_with_sdk_test = analysistest.make(_with_sdk_impl, attrs = {"mnemonic": attr.string(mandatory = True)})

_without_sdk_test = analysistest.make(_without_sdk_impl, attrs = {"mnemonic": attr.string(mandatory = True)})

def android_sdk_classpath_tests(name, library, mnemonic, size = "small"):
    """Asserts the `mnemonic` check of `library` gets android.jar only after android_sdk_aspect."""
    _with_sdk_subject(name = name + "_with_sdk_subject", library = ":" + library, testonly = True)
    _without_sdk_subject(name = name + "_without_sdk_subject", library = ":" + library, testonly = True)
    _with_sdk_test(name = name + "_with_sdk", size = size, target_under_test = name + "_with_sdk_subject", mnemonic = mnemonic)
    _without_sdk_test(name = name + "_without_sdk", size = size, target_under_test = name + "_without_sdk_subject", mnemonic = mnemonic)
