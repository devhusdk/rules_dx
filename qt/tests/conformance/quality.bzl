"""Lint and format subjects of the Qt conformance suite, built for the Qt platform of the host."""

load("//cc/private/quality:cc.bzl", "real_cc_format_aspect", "real_cc_lint_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//qt/private:platform.bzl", "QT_PLATFORM_TRANSITION")

qt_format_subject = config_subject_rule([real_cc_format_aspect], [OutputGroupInfo], cfg = QT_PLATFORM_TRANSITION)

qt_lint_subject = config_subject_rule([real_cc_lint_aspect], [OutputGroupInfo], cfg = QT_PLATFORM_TRANSITION)
