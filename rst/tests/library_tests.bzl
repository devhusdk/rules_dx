"""rst_library ownership proofs."""

load("//core/quality:library_tests.bzl", "owns_class_test")

owns_rst_class_test = owns_class_test("rst", ["index.rst", "guide.rest"])
