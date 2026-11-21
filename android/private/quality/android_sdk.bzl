"""Exposes the android.jar of the resolved Android SDK to the JVM quality aspects."""

load("@rules_android//providers:providers.bzl", "AndroidIdeInfo")
load("//java/private/quality:jvm_info.bzl", "JvmAndroidSdkInfo")

_ANDROID_SDK_TOOLCHAIN_TYPE = "@rules_android//toolchains/android_sdk:toolchain_type"
_ANDROID_KINDS = ["android_library", "kt_android_library"]

def _android_sdk_impl(_target, ctx):
    if ctx.rule.kind not in _ANDROID_KINDS:
        return [JvmAndroidSdkInfo(jars = depset())]
    toolchain = ctx.toolchains[_ANDROID_SDK_TOOLCHAIN_TYPE]
    if toolchain == None or getattr(toolchain, "android_sdk_info", None) == None:
        fail("%s: %s needs a resolved Android SDK toolchain for quality checks" % (ctx.label, ctx.rule.kind))
    return [JvmAndroidSdkInfo(jars = depset([toolchain.android_sdk_info.android_jar]))]

android_sdk_aspect = aspect(
    implementation = _android_sdk_impl,
    doc = "Exposes the android.jar of the resolved Android SDK for android_library and kt_android_library targets. Applies only to targets that advertise AndroidIdeInfo.",
    attr_aspects = ["upstream"],
    provides = [JvmAndroidSdkInfo],
    required_providers = [AndroidIdeInfo],
    toolchains = [config_common.toolchain_type(_ANDROID_SDK_TOOLCHAIN_TYPE, mandatory = False)],
)
