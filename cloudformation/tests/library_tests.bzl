"""cloudformation_library proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_cloudformation_class_test = owns_class_test("cloudformation", ["template.yaml"])
