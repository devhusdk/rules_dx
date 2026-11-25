"""Lint and format subjects of the Flutter conformance suite."""

load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//dart/private/quality:dart.bzl", "real_dart_format_aspect", "real_dart_lint_aspect")

flutter_format_subject = config_subject_rule([real_dart_format_aspect])

flutter_lint_subject = config_subject_rule([real_dart_lint_aspect])
