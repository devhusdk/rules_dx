"""asciidoc_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_asciidoc_class_test = owns_class_test("asciidoc", ["index.adoc", "guide.asciidoc"])
