"""Quality subjects of the Android conformance suite."""

load("//android/private/quality:android_sdk.bzl", "android_sdk_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//java/private/quality:jvm.bzl", "real_jvm_lint_aspect", "real_pkl_lint_aspect")

android_lint_subject = config_subject_rule([android_sdk_aspect, real_jvm_lint_aspect, real_pkl_lint_aspect], [OutputGroupInfo])
