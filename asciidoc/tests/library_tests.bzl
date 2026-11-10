"""asciidoc_library ownership proofs."""

load("//core/quality:library_tests.bzl", "owns_class_test")

owns_asciidoc_class_test = owns_class_test("asciidoc", ["index.adoc", "guide.asciidoc"])
