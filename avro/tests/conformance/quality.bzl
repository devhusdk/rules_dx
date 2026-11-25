"""Format subject of the Avro conformance suite."""

load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//javascript/private/quality:js.bzl", "real_js_format_aspect")

avro_format_subject = config_subject_rule([real_js_format_aspect])
