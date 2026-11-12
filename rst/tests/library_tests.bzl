"""rst_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_rst_class_test = owns_class_test("rst", ["index.rst", "guide.rest"])
