"""make_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_make_class_test = owns_class_test("make", ["Makefile", "rules.mk"])
